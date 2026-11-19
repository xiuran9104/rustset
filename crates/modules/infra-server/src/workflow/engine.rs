//! 引擎核心：条件求值（纯逻辑）+ 实例推进。
//!
//! advance() 以实例级 advisory lock 串行化（同工单 provisioning 的模式）：
//! 依次处理未决步骤——condition 为假跳过、外部等待型挂起、就地可执行型
//! （auto/notification）立即执行后继续，直到全部终态或等待外部输入。

use rustset_framework_database::DbPool;
use rustset_framework_tenant::TenantContext;
use serde_json::{Value, json};
use sqlx::Row;


/// 条件求值：{"field","op","value"}，field 先查 payload 再查 context。
/// 纯逻辑，无 IO，单测覆盖。空条件（null）恒为 true。
pub(crate) fn evaluate_condition(condition: Option<&Value>, payload: &Value, context: &Value) -> bool {
    // 空条件（None 或显式 null）恒为 true。
    let Some(condition) = condition.filter(|c| !c.is_null()) else { return true };
    let field = condition.get("field").and_then(Value::as_str).unwrap_or_default();
    let op = condition.get("op").and_then(Value::as_str).unwrap_or("eq");
    let expected = condition.get("value");
    let actual = payload.get(field).or_else(|| context.get(field));
    match op {
        "exists" => actual.is_some(),
        "not_exists" => actual.is_none(),
        _ => {
            let Some(actual) = actual else { return false };
            match op {
                "eq" => loose_eq(actual, expected),
                "ne" => !loose_eq(actual, expected),
                "gt" => compare(actual, expected).map(|o| o > 0).unwrap_or(false),
                "gte" => compare(actual, expected).map(|o| o >= 0).unwrap_or(false),
                "lt" => compare(actual, expected).map(|o| o < 0).unwrap_or(false),
                "lte" => compare(actual, expected).map(|o| o <= 0).unwrap_or(false),
                "in" => expected
                    .and_then(Value::as_array)
                    .map(|list| list.iter().any(|item| loose_eq(actual, Some(item))))
                    .unwrap_or(false),
                _ => false,
            }
        }
    }
}

fn loose_eq(a: &Value, b: Option<&Value>) -> bool {
    let Some(b) = b else { return false };
    if a == b {
        return true;
    }
    // 数字/字符串宽容比较（payload 里 "8080" vs 8080）。
    let numeric = |v: &Value| {
        v.as_i64()
            .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
    };
    if let (Some(x), Some(y)) = (numeric(a), numeric(b)) {
        return x == y;
    }
    matches!((a.as_str(), b.as_str()), (Some(x), Some(y)) if x == y)
}

fn compare(a: &Value, b: Option<&Value>) -> Option<i32> {
    let b = b?;
    if let (Some(x), Some(y)) = (a.as_f64(), b.as_f64()) {
        return Some(x.partial_cmp(&y)? as i32);
    }
    if let (Some(x), Some(y)) = (a.as_str(), b.as_str()) {
        return Some(x.cmp(y) as i32);
    }
    None
}

/// DAG 可达性：从起点（index 0）和活跃步骤出发，沿已完成步骤的 next 出口
/// BFS 传播。done 步骤按 next 顺序评估第一个命中 when 的出口（无 when 的
/// 出口为无条件默认）；空 next 或无命中 → index+1（V1 线性语义）。
/// 纯函数，无 IO，单测覆盖。
pub(crate) fn compute_reachable(steps: &[sqlx::postgres::PgRow], payload: &Value, context: &Value) -> std::collections::HashSet<String> {
    use std::collections::{HashSet, VecDeque};

    let by_key: std::collections::HashMap<String, &sqlx::postgres::PgRow> = steps
        .iter()
        .map(|row| (row.get::<String, _>("step_key"), row))
        .collect();
    let ordered_keys: Vec<String> = steps.iter().map(|row| row.get::<String, _>("step_key")).collect();

    let mut reachable: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();

    // 起点：index 0 或当前 active 的步骤。
    if let Some(first) = ordered_keys.first() {
        reachable.insert(first.clone());
        queue.push_back(first.clone());
    }
    for row in steps {
        if row.get::<&str, _>("status") == "active" {
            let key: String = row.get("step_key");
            if reachable.insert(key.clone()) {
                queue.push_back(key);
            }
        }
    }

    while let Some(key) = queue.pop_front() {
        let Some(row) = by_key.get(&key) else { continue };
        let status: &str = row.get("status");
        let next: Value = row.get("next");
        let next_array = next.as_array().cloned().unwrap_or_default();
        let index: i32 = row.get("step_index");

        match status {
            // done：沿实际选路的出口传播（评估条件，取第一个命中）。
            "done" => {
                let mut propagated = false;
                for edge in &next_array {
                    let target = edge.get("to").and_then(Value::as_str).unwrap_or_default();
                    if target.is_empty() { continue; }
                    let when = edge.get("when");
                    let hit = when
                        .filter(|w| !w.is_null())
                        .map(|w| evaluate_condition(Some(w), payload, context))
                        .unwrap_or(true);
                    if hit {
                        if reachable.insert(target.to_string()) {
                            queue.push_back(target.to_string());
                        }
                        propagated = true;
                        break; // 第一个命中即选路
                    }
                }
                if !propagated {
                    let next_index = index + 1;
                    if let Some(next_key) = ordered_keys.get(next_index as usize) {
                        if reachable.insert(next_key.clone()) {
                            queue.push_back(next_key.clone());
                        }
                    }
                }
            }
            // active / pending（可达集内的）：结局未知/即将激活，向所有出口传播（不剪枝）。
            "active" | "pending" => {
                for edge in &next_array {
                    if let Some(target) = edge.get("to").and_then(Value::as_str) {
                        if !target.is_empty() && reachable.insert(target.to_string()) {
                            queue.push_back(target.to_string());
                        }
                    }
                }
                if next_array.is_empty() {
                    let next_index = index + 1;
                    if let Some(next_key) = ordered_keys.get(next_index as usize) {
                        if reachable.insert(next_key.clone()) {
                            queue.push_back(next_key.clone());
                        }
                    }
                }
            }
            // skipped：不传播（被跳过的分支不向后延伸）。
            // failed/cancelled：不传播（已终止）。
            _ => {}
        }
    }
    reachable
}

/// 步骤配置校验（definition.rs 保存时与实例展开时共用）。
pub(crate) fn validate_definition(definition: &Value) -> Result<Vec<Value>, String> {
    let steps = definition
        .get("steps")
        .and_then(Value::as_array)
        .ok_or("definition.steps 必须是非空数组")?;
    if steps.is_empty() {
        return Err("至少需要一个步骤".to_string());
    }
    let valid_types = [
        "approval",
        "manual",
        "auto",
        "notification",
        "ticket_wait",
        "llm",
        "knowledge",
        "agent",
    ];
    let mut keys: Vec<String> = Vec::new();
    for (index, step) in steps.iter().enumerate() {
        let key = step
            .get("key")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if key.is_empty()
            || key.len() > 64
            || !key
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(format!("步骤 {} 的 key 只能是小写字母/数字/下划线", index + 1));
        }
        if keys.contains(&key) {
            return Err(format!("步骤 key 重复：{key}"));
        }
        keys.push(key.clone());
        let name = step.get("name").and_then(Value::as_str).unwrap_or_default();
        if name.trim().is_empty() || name.chars().count() > 128 {
            return Err(format!("步骤 {key} 的名称必填且不超过 128 字符"));
        }
        let step_type = step.get("type").and_then(Value::as_str).unwrap_or_default();
        if !valid_types.contains(&step_type) {
            return Err(format!(
                "步骤 {key} 的类型无效：{step_type}（允许 {}）",
                valid_types.join("/")
            ));
        }
        let config = step.get("config").cloned().unwrap_or_else(|| json!({}));
        match step_type {
            "approval" | "manual" => {
                if config.get("assignee").map(Value::is_object).unwrap_or(false)
                    && (config["assignee"].get("role").is_some()
                        || config["assignee"].get("userIds").is_some()
                        || config["assignee"].get("usernames").is_some())
                {
                    // ok
                } else {
                    return Err(format!("步骤 {key} 需要配置审批/执行人（assignee）"));
                }
                if let Some(on_timeout) = config.get("onTimeout").and_then(Value::as_str) {
                    if !["escalate_notify", "auto_reject"].contains(&on_timeout) {
                        return Err(format!(
                            "步骤 {key} 的 onTimeout 只允许 escalate_notify/auto_reject"
                        ));
                    }
                }
            }
            "auto" => {
                let action = config.get("action").and_then(Value::as_str).unwrap_or_default();
                if super::actions::is_known_action(action).is_none() {
                    return Err(format!("步骤 {key} 的自动动作未知：{action}"));
                }
            }
            "ticket_wait" => {
                if config.get("ticketId").is_none() && config.get("ticketIdFrom").is_none() {
                    return Err(format!("步骤 {key} 需要配置 ticketId 或 ticketIdFrom"));
                }
            }
            _ => {}
        }
        if let Some(condition) = step.get("condition") {
            if condition.is_null() {
                continue;
            }
            if !condition.is_object()
                || condition.get("field").and_then(Value::as_str).unwrap_or_default().is_empty()
            {
                return Err(format!("步骤 {key} 的 condition 需要 field 字段"));
            }
        }
    }
    // next 出口校验：to 必须引用存在的 key，且从任一步骤沿 next 走不能成环。
    for step in steps.iter() {
        let key = step.get("key").and_then(Value::as_str).unwrap_or_default();
        if let Some(next_array) = step.get("next").and_then(Value::as_array) {
            if next_array.is_empty() {
                continue;
            }
            for edge in next_array {
                let to = edge.get("to").and_then(Value::as_str).unwrap_or_default();
                if to.is_empty() {
                    return Err(format!("步骤 {key} 的 next 出口缺少 to 字段"));
                }
                if !keys.contains(&to.to_string()) {
                    return Err(format!("步骤 {key} 的 next 出口指向不存在的步骤：{to}"));
                }
            }
        }
    }
    if let Some(cycle) = find_cycle(steps) {
        return Err(format!("流程存在环：{cycle}"));
    }
    Ok(steps.clone())
}

/// 检测 next 出口构成的环（不含线性 index+1 回退——那天然无环）。
/// 返回环上的某个 key 或 None。
fn find_cycle(steps: &[Value]) -> Option<String> {
    use std::collections::{HashMap, HashSet};
    let edges: HashMap<&str, Vec<&str>> = steps
        .iter()
        .filter_map(|step| {
            let key = step.get("key").and_then(Value::as_str)?;
            let next = step
                .get("next")
                .and_then(Value::as_array)?
                .iter()
                .filter_map(|e| e.get("to").and_then(Value::as_str))
                .collect::<Vec<_>>();
            if next.is_empty() { None } else { Some((key, next)) }
        })
        .collect();

    let mut visited: HashSet<&str> = HashSet::new();
    let mut in_stack: HashSet<&str> = HashSet::new();

    fn dfs<'a>(
        node: &'a str,
        edges: &HashMap<&'a str, Vec<&'a str>>,
        visited: &mut HashSet<&'a str>,
        in_stack: &mut HashSet<&'a str>,
    ) -> Option<&'a str> {
        if in_stack.contains(node) {
            return Some(node);
        }
        if visited.contains(node) {
            return None;
        }
        visited.insert(node);
        in_stack.insert(node);
        for next in edges.get(node).into_iter().flatten() {
            if let Some(cycle) = dfs(next, edges, visited, in_stack) {
                return Some(cycle);
            }
        }
        in_stack.remove(node);
        None
    }

    for step in steps {
        if let Some(key) = step.get("key").and_then(Value::as_str) {
            if let Some(cycle) = dfs(key, &edges, &mut visited, &mut in_stack) {
                return Some(cycle.to_string());
            }
        }
    }
    None
}

/// 实例推进：处理所有可推进的步骤，直到等待外部输入或全部终态。
/// 返回 Ok(())；内部错误以 tracing 记录并保留现场（可重试）。
pub(crate) async fn advance(state: &crate::InfraState, tenant: &TenantContext, instance_id: i64) {
    if let Err(error) = advance_inner(state, tenant, instance_id).await {
        tracing::warn!(instance_id, %error, "flow advance failed");
    }
}

async fn advance_inner(state: &crate::InfraState, tenant: &TenantContext, instance_id: i64) -> Result<(), String> {
    // 循环驱动：notification 落库后回到下一轮（避免 async 直接递归）。
    for _round in 0..64 {
        let progressed = advance_once(state, tenant, instance_id).await?;
        if !progressed {
            break;
        }
    }
    Ok(())
}

/// 单轮推进。返回 true = 有进展（notification 完成等），false = 等待外部或已收尾。
async fn advance_once(state: &crate::InfraState, tenant: &TenantContext, instance_id: i64) -> Result<bool, String> {
    let pool = &state.pool;
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    // 实例级串行化：与工单 provisioning 相同的 advisory lock 模式。
    let locked = sqlx::query_scalar::<_, bool>(
        "SELECT pg_try_advisory_xact_lock(hashtextextended($1, 0))",
    )
    .bind(format!("rustset:flow:{instance_id}"))
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    if !locked {
        return Ok(false); // 其他推进者持有锁；由它完成推进
    }

    let instance = sqlx::query(
        "SELECT id, title, payload, context, status, definition_snapshot FROM infra_flow_instance
         WHERE id=$1 AND tenant_id=$2 AND deleted=0",
    )
    .bind(instance_id)
    .bind(tenant.id())
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    let Some(instance) = instance else { return Ok(false) };
    if instance.get::<&str, _>("status") != "running" {
        return Ok(false);
    }
    let title: String = instance.get("title");
    let payload: Value = instance.get("payload");
    let context: Value = instance.get("context");
    let snapshot: Value = instance.get("definition_snapshot");
    let flow_name = snapshot
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let steps = sqlx::query(
        "SELECT id, step_key, step_index, name, type, status, config, attempt_count, next
         FROM infra_flow_step
         WHERE instance_id=$1 AND tenant_id=$2
         ORDER BY step_index",
    )
    .bind(instance_id)
    .bind(tenant.id())
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    // ---- DAG 可达性：从已完成/活跃步骤沿 next 出口 BFS，不可达 pending 置 skipped ----
    let reachable = compute_reachable(&steps, &payload, &context);
    for step in &steps {
        let status: &str = step.get("status");
        let step_key: String = step.get("step_key");
        let step_id: i64 = step.get("id");
        if status == "pending" && !reachable.contains(&step_key) {
            sqlx::query(
                "UPDATE infra_flow_step SET status='skipped', finished_at=now()
                 WHERE id=$1 AND tenant_id=$2 AND status='pending'",
            )
            .bind(step_id)
            .bind(tenant.id())
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        }
    }

    let mut current_index: i64 = 0;
    let mut any_failed = false;
    for step in &steps {
        let status: &str = step.get("status");
        let index: i32 = step.get("step_index");
        let step_type: &str = step.get("type");
        let config: Value = step.get("config");
        let step_id: i64 = step.get("id");
        let step_name: String = step.get("name");
        let attempt_count: i32 = step.get("attempt_count");

        match status {
            "done" | "skipped" | "cancelled" => {}
            "failed" => {
                any_failed = true;
                current_index = index as i64 + 1;
            }
            "pending" => {
                // DAG：不可达的 pending 已在上面批量置 skipped；这里跳过本轮已处理的
                let step_key: String = step.get("step_key");
                if !reachable.contains(&step_key) {
                    continue;
                }
                let condition = config.get("condition");
                if !evaluate_condition(condition, &payload, &context) {
                    sqlx::query(
                        "UPDATE infra_flow_step SET status='skipped', finished_at=now()
                         WHERE id=$1 AND tenant_id=$2 AND status='pending'",
                    )
                    .bind(step_id)
                    .bind(tenant.id())
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                    continue;
                }
                // 激活步骤；超时时刻用 chrono 计算（信创规则：不用 SQL interval）。
                let timeout_at = config
                    .get("timeoutHours")
                    .and_then(Value::as_f64)
                    .filter(|hours| *hours > 0.0)
                    .map(|hours| {
                        chrono::Utc::now()
                            .checked_add_signed(
                                chrono::Duration::seconds((hours * 3600.0) as i64),
                            )
                            .map(|t| t.naive_utc())
                    })
                    .flatten();
                sqlx::query(
                    "UPDATE infra_flow_step
                     SET status='active', started_at=now(), timeout_at=$3
                     WHERE id=$1 AND tenant_id=$2 AND status='pending'",
                )
                .bind(step_id)
                .bind(tenant.id())
                .bind(timeout_at)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
                current_index = index as i64 + 1;

                match step_type {
                    "approval" | "manual" => {
                        // 动态审批人：fromContext 指向 context 中的用户名（点路径），
                        // 命中则并入步骤 assignee 列——待办列表、审批校验
                        // （assignee_matches）与会签期望集共用该列，无需另行适配。
                        let mut assignee = config.get("assignee").cloned().unwrap_or_else(|| json!({}));
                        if let Some(username) = config
                            .get("fromContext")
                            .and_then(Value::as_str)
                            .and_then(|path| super::variables::context_string_at(&context, path))
                            .filter(|name| !name.is_empty())
                        {
                            let mut names = assignee
                                .get("usernames")
                                .and_then(Value::as_array)
                                .cloned()
                                .unwrap_or_default();
                            let entry = json!(username);
                            if !names.contains(&entry) {
                                names.push(entry);
                            }
                            if let Some(obj) = assignee.as_object_mut() {
                                obj.insert("usernames".to_string(), json!(names));
                            }
                            sqlx::query(
                                "UPDATE infra_flow_step SET assignee=$3 WHERE id=$1 AND tenant_id=$2",
                            )
                            .bind(step_id)
                            .bind(tenant.id())
                            .bind(&assignee)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| e.to_string())?;
                        }
                        // 等待外部输入。先提交事务再发通知：事务持有的连接
                        // 不释放时用池发通知会自我死锁（小池下 PoolTimedOut）。
                        tx.commit().await.map_err(|e| e.to_string())?;
                        let recipients =
                            super::notify::resolve_recipients(pool, tenant.id(), &assignee).await;
                        super::notify::send_step_notice(
                            pool,
                            tenant.id(),
                            "flow_step_todo",
                            &flow_name,
                            &step_name,
                            &title,
                            &recipients,
                            &json!({}),
                        )
                        .await;
                        return Ok(false);
                    }
                    "ticket_wait" => {
                        sqlx::query(
                            "UPDATE infra_flow_step SET retry_at=now()
                             WHERE id=$1 AND tenant_id=$2",
                        )
                        .bind(step_id)
                        .bind(tenant.id())
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                        // 等待型：由 runner 轮询工单状态，不能继续激活下一步。
                        tx.commit().await.map_err(|e| e.to_string())?;
                        return Ok(false);
                    }
                    "notification" => {
                        // 先在事务内落 done，提交后再用池发通知（避免事务连接
                        // 与通知连接互相等待）。
                        sqlx::query(
                            "UPDATE infra_flow_step
                             SET status='done', finished_at=now(), operator='system'
                             WHERE id=$1 AND tenant_id=$2",
                        )
                        .bind(step_id)
                        .bind(tenant.id())
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                        tx.commit().await.map_err(|e| e.to_string())?;
                        let assignee = config.get("assignee").cloned().unwrap_or_else(|| json!({}));
                        let template = config
                            .get("templateCode")
                            .and_then(Value::as_str)
                            .unwrap_or("flow_step_todo");
                        // 自定义模板参数：先做 Value 级插值（{{payload.x}}/
                        // {{context.x}}），再交给 send_step_notice 做 {key} 替换。
                        let notice_params = config
                            .get("params")
                            .map(|p| super::variables::interpolate_value(p, &payload, &context))
                            .unwrap_or_else(|| json!({}));
                        let recipients =
                            super::notify::resolve_recipients(pool, tenant.id(), &assignee).await;
                        super::notify::send_step_notice(
                            pool, tenant.id(), template, &flow_name, &step_name, &title,
                            &recipients, &notice_params,
                        )
                        .await;
                        let notified = json!({ "notified": recipients.len() });
                        let _ = sqlx::query(
                            "UPDATE infra_flow_step SET result=result || $2 WHERE id=$1",
                        )
                        .bind(step_id)
                        .bind(notified)
                        .execute(pool)
                        .await;
                        // 有进展：回到驱动循环处理下一步。
                        return Ok(true);
                    }
                    "auto" | "llm" | "knowledge" | "agent" => {
                        // 提交当前事务再执行动作（动作内含网络 IO 与独立写入），
                        // 执行结果由 execute_step 落库并触发下一轮推进；
                        // Box::pin 断开 execute_step ⇄ advance 的异步递归。
                        // AI 节点（llm/knowledge/agent）走同一路径——execute_step
                        // 内部按类型分发到 ai_nodes 执行器。
                        tx.commit().await.map_err(|e| e.to_string())?;
                        Box::pin(super::actions::execute_step(
                            state,
                            tenant,
                            instance_id,
                            step_id,
                        ))
                        .await;
                        return Ok(false);
                    }
                    _ => {}
                }
            }
            "active" => {
                current_index = index as i64 + 1;
                if step_type == "approval" || step_type == "manual" || step_type == "ticket_wait" {
                    // 等待外部输入/轮询，返回。
                    tx.commit().await.map_err(|e| e.to_string())?;
                    return Ok(false);
                }
                if step_type == "auto" || step_type == "llm" || step_type == "knowledge" || step_type == "agent" {
                    // auto 步骤 active：要么重试到期（runner 调来），要么退避中。
                    let retry_due = sqlx::query_scalar::<_, bool>(
                        "SELECT retry_at IS NULL OR retry_at <= now() FROM infra_flow_step
                         WHERE id=$1",
                    )
                    .bind(step_id)
                    .fetch_one(&mut *tx)
                    .await
                    .unwrap_or(true);
                    if retry_due && attempt_count < 3 {
                        tx.commit().await.map_err(|e| e.to_string())?;
                        Box::pin(super::actions::execute_step(
                            state,
                            tenant,
                            instance_id,
                            step_id,
                        ))
                        .await;
                        return Ok(false);
                    }
                    if attempt_count >= 3 {
                        sqlx::query(
                            "UPDATE infra_flow_step SET status='failed', finished_at=now(),
                                 result=jsonb_set(result,'{lastError}','\"重试次数用尽\"')
                             WHERE id=$1 AND tenant_id=$2",
                        )
                        .bind(step_id)
                        .bind(tenant.id())
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                        any_failed = true;
                        continue;
                    }
                    tx.commit().await.map_err(|e| e.to_string())?;
                    return Ok(false); // 退避等待
                }
            }
            _ => {}
        }
    }

    // 全部步骤终态：收尾实例。
    let final_status = if any_failed { "failed" } else { "completed" };
    sqlx::query(
        "UPDATE infra_flow_instance
         SET status=$3, current_index=$4, finish_time=now(), update_time=now()
         WHERE id=$1 AND tenant_id=$2 AND status='running'",
    )
    .bind(instance_id)
    .bind(tenant.id())
    .bind(final_status)
    .bind(current_index)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn condition_eq_loose_types() {
        let payload = json!({"riskLevel": "high", "port": 8080, "region": "cn-north"});
        assert!(evaluate_condition(
            Some(&json!({"field":"riskLevel","op":"eq","value":"high"})),
            &payload,
            &json!({})
        ));
        assert!(evaluate_condition(
            Some(&json!({"field":"port","op":"eq","value":"8080"})),
            &payload,
            &json!({})
        ));
        assert!(!evaluate_condition(
            Some(&json!({"field":"riskLevel","op":"eq","value":"normal"})),
            &payload,
            &json!({})
        ));
    }

    #[test]
    fn condition_ops_and_context_fallback() {
        let payload = json!({"cpu": 8});
        let context = json!({"ticketId": 42, "env": "prod"});
        assert!(evaluate_condition(Some(&json!({"field":"cpu","op":"gt","value":4})), &payload, &context));
        assert!(!evaluate_condition(Some(&json!({"field":"cpu","op":"lt","value":4})), &payload, &context));
        assert!(evaluate_condition(Some(&json!({"field":"env","op":"eq","value":"prod"})), &payload, &context));
        assert!(evaluate_condition(Some(&json!({"field":"ticketId","op":"exists"})), &payload, &context));
        assert!(evaluate_condition(
            Some(&json!({"field":"env","op":"in","value":["prod","stage"]})),
            &payload,
            &context
        ));
        // 空条件恒真
        assert!(evaluate_condition(None, &payload, &context));
        assert!(evaluate_condition(Some(&Value::Null), &payload, &context));
        // 缺字段比较为 false
        assert!(!evaluate_condition(Some(&json!({"field":"missing","op":"eq","value":"x"})), &payload, &context));
    }

    #[test]
    fn definition_validation_rules() {
        let good = json!({"steps":[
            {"key":"a","name":"审批","type":"approval","config":{"assignee":{"role":"super_admin"}}},
            {"key":"b","name":"通知","type":"notification","config":{}}
        ]});
        assert!(validate_definition(&good).is_ok());

        let empty = json!({"steps":[]});
        assert!(validate_definition(&empty).is_err());

        let dup_key = json!({"steps":[
            {"key":"a","name":"1","type":"notification","config":{}},
            {"key":"a","name":"2","type":"notification","config":{}}
        ]});
        assert!(validate_definition(&dup_key).is_err());

        let bad_action = json!({"steps":[
            {"key":"a","name":"x","type":"auto","config":{"action":"nope"}}
        ]});
        assert!(validate_definition(&bad_action).is_err());

        let approval_no_assignee = json!({"steps":[
            {"key":"a","name":"审批","type":"approval","config":{}}
        ]});
        assert!(validate_definition(&approval_no_assignee).is_err());
    }

    #[test]
    fn next_edge_validation_rejects_unknown_target() {
        let def = json!({"steps":[
            {"key":"a","name":"A","type":"notification","config":{},
             "next":[{"to":"nonexistent"}]}
        ]});
        let err = validate_definition(&def).unwrap_err();
        assert!(err.contains("不存在的步骤"), "got: {err}");
    }

    #[test]
    fn next_edge_validation_rejects_cycle() {
        let def = json!({"steps":[
            {"key":"a","name":"A","type":"notification","config":{},
             "next":[{"to":"b"}]},
            {"key":"b","name":"B","type":"notification","config":{},
             "next":[{"to":"a"}]}
        ]});
        let err = validate_definition(&def).unwrap_err();
        assert!(err.contains("环"), "got: {err}");
    }

    #[test]
    fn next_edge_cycle_self_reference() {
        let def = json!({"steps":[
            {"key":"a","name":"A","type":"notification","config":{},
             "next":[{"to":"a"}]}
        ]});
        assert!(validate_definition(&def).is_err());
    }

    #[test]
    fn valid_dag_definition_passes() {
        let def = json!({"steps":[
            {"key":"route","name":"分支","type":"notification","config":{},
             "next":[
                {"when":{"field":"type","op":"eq","value":"a"},"to":"branch_a"},
                {"to":"branch_b"}
             ]},
            {"key":"branch_a","name":"A分支","type":"notification","config":{}},
            {"key":"branch_b","name":"B分支","type":"notification","config":{},
             "next":[{"to":"final"}]},
            {"key":"final","name":"结束","type":"notification","config":{}}
        ]});
        assert!(validate_definition(&def).is_ok());
    }

    #[test]
    fn linear_definition_without_next_still_valid() {
        let def = json!({"steps":[
            {"key":"a","name":"A","type":"notification","config":{}},
            {"key":"b","name":"B","type":"notification","config":{}}
        ]});
        assert!(validate_definition(&def).is_ok());
    }
}
