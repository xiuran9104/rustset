//! 方言翻译规则（docs/xinchuang-adaptation-plan.md §4.1.2）。
//!
//! 规则分两类：
//! * 确定性改写——类型映射、cast 剥离、`ilike` 重写等，机械应用；
//! * 评审项——`ON CONFLICT`（达梦 MERGE 重写）、部分索引、数组列等
//!   语义敏感构造，只标记不改写，逐条人工评审后登记。
//!
//! 金仓 / openGauss 走 PG 兼容模式，文件原样复制（`sql/<flavor>/README.md`）；
//! 达梦应用下述规则。`0001_initial.sql` 是 pg_dump 基线（34.6 万行），按方案
//! R2 走「参考库导出 → 翻译 → 回灌 → 行数/checksum 比对」流水线，不走本工具
//! 的逐文件翻译入口。

/// 翻译目标方言。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Kingbase,
    OpenGauss,
    Dameng,
}

impl Target {
    /// 解析 `--target` 参数。
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_lowercase().as_str() {
            "kingbase" | "kb" => Some(Self::Kingbase),
            "opengauss" | "og" => Some(Self::OpenGauss),
            "dm" | "dameng" | "dm8" => Some(Self::Dameng),
            _ => None,
        }
    }
}

/// 单条评审发现：需要人工确认或后续规则覆盖的位置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub line: usize,
    /// 稳定类别，供报告聚合与 CI 门禁（`--strict`）使用。
    pub category: Category,
    pub detail: String,
}

/// 需人工评审的构造类别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    /// `ON CONFLICT` → 达梦需手工改写 MERGE INTO。
    OnConflict,
    /// `EXCLUDE` 约束 → 唯一索引或应用层保证（0035）。
    Exclude,
    /// 数组类型/数组字面量（`text[]`、`ARRAY[`、`'{}'`）。
    Array,
    /// 迁移内 `interval` 字面量。
    Interval,
    /// gin 索引已降级为普通索引（功能降级登记）。
    GinIndex,
    /// 部分索引 WHERE 已剥离（功能降级登记）。
    PartialIndex,
    /// `IF NOT EXISTS` 已剥离，需人工确认幂等性。
    IfNotExists,
    /// `FOR UPDATE` 事务流——合同测试覆盖项，达梦侧行为需专项验证。
    ForUpdate,
    /// `NULLS NOT DISTINCT` 已改写为 COALESCE 表达式唯一索引。
    NullsNotDistinct,
    /// PG10+ identity 标记语句已省略（nextval 默认值保留）。
    IdentityMarker,
    /// 未识别的 pg_dump 指令或 PG 专有函数（`to_char` 等）。
    Other,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Self::OnConflict => "on-conflict",
            Self::Exclude => "exclude-constraint",
            Self::Array => "array",
            Self::Interval => "interval",
            Self::GinIndex => "gin-index",
            Self::PartialIndex => "partial-index",
            Self::IfNotExists => "if-not-exists",
            Self::ForUpdate => "for-update",
            Self::NullsNotDistinct => "nulls-not-distinct",
            Self::IdentityMarker => "identity-marker",
            Self::Other => "other",
        }
    }
}

/// 翻译结果：改写后的 SQL 与评审发现列表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translation {
    pub sql: String,
    pub findings: Vec<Finding>,
}

/// 翻译一段迁移 SQL。
pub fn translate(target: Target, source: &str) -> Translation {
    match target {
        // 金仓：PG 兼容度高（upsert/DO NOTHING/FILTER/jsonb 实测原生支持），
        // 仅 NULLS NOT DISTINCT（PG15 语法，金仓内核 PG12 系）需改写。
        Target::Kingbase => translate_kingbase(source),
        // openGauss：PG 兼容为主，但对内核（源自 PG 9.2）缺失的构造做
        // 实测驱动的改写（2026-11-06 于 openGauss 7.0.0-RC2 探测）。
        Target::OpenGauss => translate_opengauss(source),
        Target::Dameng => translate_dameng(source),
    }
}

/// openGauss 改写（每条规则都在兼容矩阵上实测过，见 §4.1.2 注）：
/// * `ON CONFLICT … DO UPDATE SET` → `ON DUPLICATE KEY UPDATE`（EXCLUDED 支持）
/// * `INSERT INTO t (cols…) … ON CONFLICT DO NOTHING` → 首列自赋值的
///   `ON DUPLICATE KEY UPDATE`（openGauss 无 DO NOTHING）
/// * `NULLS NOT DISTINCT` 唯一索引/约束 → `COALESCE(col::text, 标记)` 表达式
///   唯一索引（openGauss 把 NULL 视为互异）
fn translate_opengauss(source: &str) -> Translation {
    let mut findings = Vec::new();
    scan_review_items(source, &mut findings);
    let mut sql = source.to_owned();

    // (1) ON CONFLICT [target] DO UPDATE SET —— 目标列由主键/唯一键冲突
    // 语义接管，EXCLUDED 在 openGauss 可用。
    let (rewritten, count) = rewrite_on_conflict_update(&sql);
    if count > 0 {
        sql = rewritten;
        findings.push(Finding {
            line: 0,
            category: Category::OnConflict,
            detail: format!("{count} ON CONFLICT…DO UPDATE rewritten to ON DUPLICATE KEY UPDATE"),
        });
    }

    // (2) ON CONFLICT DO NOTHING —— 需要自赋值列，取 INSERT 列表首列。
    let (rewritten, count) = rewrite_on_conflict_do_nothing(&sql);
    if count > 0 {
        sql = rewritten;
        findings.push(Finding {
            line: 0,
            category: Category::OnConflict,
            detail: format!("{count} ON CONFLICT DO NOTHING rewritten (first-column self-assignment)"),
        });
    }

    // (3) NULLS NOT DISTINCT 唯一索引（可带部分索引 WHERE）。
    let (rewritten, count) = rewrite_nulls_not_distinct_index(&sql);
    if count > 0 {
        sql = rewritten;
        findings.push(Finding {
            line: 0,
            category: Category::NullsNotDistinct,
            detail: format!("{count} NULLS NOT DISTINCT index rewritten to COALESCE expression index"),
        });
    }

    // (4) 表约束形态的 UNIQUE NULLS NOT DISTINCT。
    let (rewritten, count) = rewrite_nulls_not_distinct_constraint(&sql);
    if count > 0 {
        sql = rewritten;
        findings.push(Finding {
            line: 0,
            category: Category::NullsNotDistinct,
            detail: format!("{count} UNIQUE NULLS NOT DISTINCT constraint rewritten to expression index"),
        });
    }

    // (5) PG10+ identity 标记语句：openGauss 无 `ADD GENERATED` 语法，
    // 且 pg_dump 不会为 identity 列单独发 CREATE SEQUENCE——序列是隐式的。
    // 改写为显式 `CREATE SEQUENCE … OWNED BY` + `SET DEFAULT nextval`，
    // 后续 setval 语句与插入行为保持一致。
    let identity = regex::Regex::new(
        r#"(?is)ALTER\s+TABLE\s+(?P<table>[\w.]+)\s+ALTER\s+COLUMN\s+(?P<column>\w+)\s+ADD\s+GENERATED\s+(?:BY\s+DEFAULT|ALWAYS)\s+AS\s+IDENTITY\s*\(\s*SEQUENCE\s+NAME\s+(?P<sequence>[\w.]+)[^;]*;"#,
    )
    .unwrap();
    let identity_count = identity.captures_iter(&sql).count();
    if identity_count > 0 {
        sql = identity
            .replace_all(&sql, |caps: &regex::Captures| {
                let sequence = caps.name("sequence").map_or("", |m| m.as_str());
                // search_path 在迁移执行时为空（pg_dump 前导 set_config），序列名必须全限定。
                format!(
                    "CREATE SEQUENCE {sequence} OWNED BY {table}.{column};\nALTER TABLE {table} ALTER COLUMN {column} SET DEFAULT nextval('{sequence}');",
                    table = &caps["table"],
                    column = &caps["column"],
                    sequence = sequence,
                )
            })
            .into_owned();
        findings.push(Finding {
            line: 0,
            category: Category::IdentityMarker,
            detail: format!("{identity_count} identity columns rewritten to explicit CREATE SEQUENCE + SET DEFAULT nextval"),
        });
    }

    // (5b) `ADD COLUMN IF NOT EXISTS`：openGauss（9.2 系内核）不支持
    // （CREATE/DROP … IF NOT EXISTS 均支持，实测仅 ADD COLUMN 缺失）。
    // 改写为逐列 DO 块 + duplicate_column 异常捕获，保持幂等语义。
    let (rewritten, add_column_count) = rewrite_add_column_if_not_exists(&sql);
    if add_column_count > 0 {
        sql = rewritten;
        findings.push(Finding {
            line: 0,
            category: Category::IfNotExists,
            detail: format!(
                "{add_column_count} ADD COLUMN IF NOT EXISTS clauses wrapped in duplicate_column-guard DO blocks"
            ),
        });
    }

    // (5b2) plpgsql 动态 SQL 里的 `EXECUTE format('ALTER TABLE … ADD COLUMN
    // IF NOT EXISTS …')`：同样用嵌套异常块包裹，保持幂等。
    let dynamic_add = regex::Regex::new(
        r#"(?is)EXECUTE\s+format\(\s*'ALTER\s+TABLE\s+(?P<table>[^']*)\s+ADD\s+COLUMN\s+IF\s+NOT\s+EXISTS\s+(?P<definition>[^']*)'\s*(?P<args>[^;]*?)\)\s*;"#,
    )
    .unwrap();
    let dynamic_count = dynamic_add.captures_iter(&sql).count();
    if dynamic_count > 0 {
        sql = dynamic_add
            .replace_all(&sql, |caps: &regex::Captures| {
                format!(
                    "BEGIN EXECUTE format('ALTER TABLE {table} ADD COLUMN {definition}'{args}); EXCEPTION WHEN duplicate_column THEN NULL; END;",
                    table = &caps["table"],
                    definition = &caps["definition"],
                    args = &caps["args"],
                )
            })
            .into_owned();
        findings.push(Finding {
            line: 0,
            category: Category::IfNotExists,
            detail: format!("{dynamic_count} dynamic ADD COLUMN IF NOT EXISTS wrapped in duplicate_column guard"),
        });
    }

    // (5c) gin `jsonb_path_ops` 操作符类：openGauss 未提供（默认
    // jsonb_ops 生效）；仅降级索引内部实现，语义不变。
    let jpo = regex::Regex::new(r"(?i)\s+jsonb_path_ops").unwrap();
    let jpo_count = jpo.find_iter(&sql).count();
    if jpo_count > 0 {
        sql = jpo.replace_all(&sql, "").into_owned();
        findings.push(Finding {
            line: 0,
            category: Category::GinIndex,
            detail: format!("{jpo_count} jsonb_path_ops opclass stripped (og default jsonb_ops)"),
        });
    }

    // (5d) 同一 ALTER TABLE 语句内对同列连续两次 ALTER COLUMN：
    // openGauss 误报 "cannot alter type twice"，拆成两条语句。
    let split_alter = regex::Regex::new(
        r#"(?is)ALTER\s+TABLE\s+(?P<table>[\w.]+)\s+ALTER\s+COLUMN\s+(?P<col>\w+)\s+SET\s+DEFAULT\s+(?P<default>[^,;]+),\s*ALTER\s+COLUMN\s+(?P<col2>\w+)\s+SET\s+NOT\s+NULL"#,
    )
    .unwrap();
    let split_count = split_alter.captures_iter(&sql).count();
    if split_count > 0 {
        sql = split_alter
            .replace_all(&sql, |caps: &regex::Captures| {
                // 两次子句必须作用于同一列；不同列时保持原样（交由评审）。
                if caps["col"] != caps["col2"] {
                    return caps[0].to_owned();
                }
                format!(
                    "ALTER TABLE {t} ALTER COLUMN {c} SET DEFAULT {d};\nALTER TABLE {t} ALTER COLUMN {c} SET NOT NULL",
                    t = &caps["table"],
                    c = &caps["col"],
                    d = caps["default"].trim(),
                )
            })
            .into_owned();
        let split_done = split_alter
            .captures_iter(&sql)
            .filter(|caps| caps["col"] == caps["col2"])
            .count();
        if split_done > 0 {
            findings.push(Finding {
                line: 0,
                category: Category::Other,
                detail: format!("{split_done} multi-clause ALTER COLUMN split into separate statements"),
            });
        }
    }

    // (5e) `to_regclass('s.t') IS NOT NULL`：openGauss 无此函数（PG 9.2 系
    // 内核），改写为 pg_class/pg_namespace 等价判定。
    let to_regclass = regex::Regex::new(
        r#"(?is)to_regclass\(\s*'(?P<qualified>[\w.]+)'\s*\)\s+IS\s+NOT\s+NULL"#,
    )
    .unwrap();
    let regclass_count = to_regclass.captures_iter(&sql).count();
    if regclass_count > 0 {
        sql = to_regclass
            .replace_all(&sql, |caps: &regex::Captures| {
                let qualified = &caps["qualified"];
                let (schema, table) = match qualified.split_once('.') {
                    Some((schema, table)) => (schema.to_owned(), table.to_owned()),
                    None => ("public".to_owned(), qualified.to_owned()),
                };
                format!(
                    "EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace \
                     WHERE n.nspname = '{schema}' AND c.relname = '{table}')"
                )
            })
            .into_owned();
        findings.push(Finding {
            line: 0,
            category: Category::Other,
            detail: format!("{regclass_count} to_regclass checks rewritten to pg_class EXISTS"),
        });
    }

    // (5f) `U&'\XXXX…'` Unicode 转义字符串（PG 13+）：openGauss 不支持，
    // 改写为 chr(N) 连接（仅处理纯 \XXXX 转义的字面量）。
    let unicode_str = regex::Regex::new(r#"U&'(?P<body>(?:\\[0-9A-Fa-f]{4})+)'"#).unwrap();
    let unicode_count = unicode_str.captures_iter(&sql).count();
    if unicode_count > 0 {
        sql = unicode_str
            .replace_all(&sql, |caps: &regex::Captures| {
                let body = caps["body"].to_uppercase();
                let parts: Vec<String> = body
                    .split('\\')
                    .filter(|part| !part.is_empty())
                    .filter_map(|part| u32::from_str_radix(part, 16).ok())
                    .map(|code| format!("chr({code})"))
                    .collect();
                parts.join("||")
            })
            .into_owned();
        findings.push(Finding {
            line: 0,
            category: Category::Other,
            detail: format!("{unicode_count} U& unicode strings rewritten to chr() concatenation"),
        });
    }

    // (5g) `jsonb -> text键`：openGauss 把 `->` 解析为 jsonb->int（数组
    // 取下标），text 重载缺失（实测）。改写为 jsonb_extract_path(expr, key)。
    // 捕获第二个 `>` 以区分双箭头 `->>`（保持不变）。
    let arrow_text = regex::Regex::new(
        r"(?is)(?P<left>[\w.]+(?:\([^)]*\))?)\s*->(?P<second>>?)(?P<key>[a-zA-Z_][\w]*)",
    )
    .unwrap();
    let arrow_count = {
        let mut count = 0;
        for caps in arrow_text.captures_iter(&sql) {
            let left = caps.name("left").map_or("", |m| m.as_str());
            let double_arrow = caps.name("second").map_or("", |m| m.as_str()) == ">";
            // 只改写明确的 jsonb 列（attributes）的单箭头列名键形态，
            // 避免误伤算术减法与双箭头。
            if left.contains("attributes") && !double_arrow {
                count += 1;
            }
        }
        count
    };
    if arrow_count > 0 {
        sql = arrow_text
            .replace_all(&sql, |caps: &regex::Captures| {
                let left = caps.name("left").map_or("", |m| m.as_str());
                let double_arrow = caps.name("second").map_or("", |m| m.as_str()) == ">";
                if !left.contains("attributes") || double_arrow {
                    return caps[0].to_owned();
                }
                format!("jsonb_extract_path({}, {})", left, &caps["key"])
            })
            .into_owned();
        findings.push(Finding {
            line: 0,
            category: Category::Other,
            detail: format!("{arrow_count} jsonb -> text-key rewritten to jsonb_extract_path"),
        });
    }

    // (5h) CREATE TABLE 内联 `GENERATED … AS IDENTITY`：openGauss 不支持
    // （仅支持 PG10 前置语法），改写为 serial 系类型——序列自动命名为
    // <表>_<列>_seq，与 pg_dump 的 setval 引用一致。
    let inline_identity = regex::Regex::new(
        r#"(?is)(?P<col>[a-zA-Z_][\w]*)\s+(?P<type>bigserial|bigint|int|integer|smallint)\s+GENERATED\s+(?:BY\s+DEFAULT|ALWAYS)\s+AS\s+IDENTITY"#,
    )
    .unwrap();
    let identity_inline_count = inline_identity.captures_iter(&sql).count();
    if identity_inline_count > 0 {
        sql = inline_identity
            .replace_all(&sql, |caps: &regex::Captures| {
                let serial = match &caps["type"] {
                    "bigint" => "bigserial",
                    "int" | "integer" => "serial",
                    "bigserial" => "bigserial",
                    _ => "smallserial",
                };
                format!("{} {}", &caps["col"], serial)
            })
            .into_owned();
        findings.push(Finding {
            line: 0,
            category: Category::IdentityMarker,
            detail: format!("{identity_inline_count} inline identity columns rewritten to serial types"),
        });
    }

    // (6) openGauss 不支持的 GUC SET（pg_dump 前导；会话级安全参数，
    // 对应用侧迁移无意义）。statement_timeout/standard_conforming_strings/
    // row_security 为 og 认识的参数，保留。
    let unknown_set = regex::Regex::new(
        r"(?ims)^SET\s+(?:default_table_access_method|lock_timeout|idle_in_transaction_session_timeout|idle_session_timeout|transaction_timeout|row_security)\s*=\s*[^;]*;\s*$",
    )
    .unwrap();
    let set_count = unknown_set.find_iter(&sql).count();
    if set_count > 0 {
        sql = unknown_set
            .replace_all(&sql, "-- og: PG-specific SET omitted")
            .into_owned();
        findings.push(Finding {
            line: 0,
            category: Category::Other,
            detail: format!("{set_count} PG-specific SET statements omitted"),
        });
    }

    Translation { sql, findings }
}

/// `ALTER TABLE t ADD COLUMN IF NOT EXISTS …, …;` → 每列一个 DO 块
/// （`EXCEPTION WHEN duplicate_column`），混合子句（SET DEFAULT 等）
/// 保持为普通 ALTER TABLE。子句按顶层逗号拆分（尊重括号）。
fn rewrite_add_column_if_not_exists(sql: &str) -> (String, usize) {
    let statement = regex::Regex::new(
        r#"(?is)ALTER\s+TABLE\s+(?P<table>[\w.]+)\s+(?P<clauses>(?:ADD\s+COLUMN\s+IF\s+NOT\s+EXISTS)[^;]*?);"#,
    )
    .unwrap();
    let mut count = 0;
    let result = statement
        .replace_all(sql, |caps: &regex::Captures| {
            let table = &caps["table"];
            let clauses = split_top_level_commas(&caps["clauses"]);
            let mut rewritten_columns = 0;
            let mut parts: Vec<String> = Vec::new();
            for clause in clauses {
                let clause = clause.trim();
                let stripped = clause
                    .strip_prefix("ADD COLUMN IF NOT EXISTS ")
                    .or_else(|| clause.strip_prefix("add column if not exists "));
                match stripped {
                    Some(definition) => {
                        rewritten_columns += 1;
                        parts.push(format!(
                            "DO $$ BEGIN ALTER TABLE {table} ADD COLUMN {definition}; EXCEPTION WHEN duplicate_column THEN NULL; END $$;"
                        ));
                    }
                    None => parts.push(format!("ALTER TABLE {table} {clause}")),
                }
            }
            count += rewritten_columns;
            parts.join("\n")
        })
        .into_owned();
    (result, count)
}

/// 按顶层逗号拆分（括号深度归零才切分）。
fn split_top_level_commas(input: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0usize;
    for character in input.chars() {
        match character {
            '(' => {
                depth += 1;
                current.push(character);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(character);
            }
            ',' if depth == 0 => {
                parts.push(current.clone());
                current.clear();
            }
            _ => current.push(character),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts
}

/// `COALESCE(col::text, marker)` 表达式列：把 NULL 归并为可比较的文本键。
fn coalesce_columns(columns: &str) -> String {
    columns
        .split(',')
        .map(|column| {
            let column = column.trim();
            format!("(COALESCE({column}::text, '\\x01OG_NULL'))")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn rewrite_on_conflict_update(sql: &str) -> (String, usize) {
    // 目标列（可带 WHERE 谓词的排他目标）整体丢弃，SET 部分保留。
    let pattern = regex_on_conflict_update();
    let mut count = 0;
    let result = pattern
        .replace_all(sql, |caps: &regex::Captures| {
            count += 1;
            format!("ON DUPLICATE KEY UPDATE {}", caps.name("set").map_or("", |m| m.as_str()))
        })
        .into_owned();
    (result, count)
}

fn rewrite_on_conflict_do_nothing(sql: &str) -> (String, usize) {
    // INSERT INTO <table> (<cols>) … ON CONFLICT DO NOTHING（跨行、非贪婪）。
    let pattern = regex_on_conflict_nothing();
    let mut count = 0;
    let result = pattern
        .replace_all(sql, |caps: &regex::Captures| {
            count += 1;
            let columns = &caps["cols"];
            let first = columns.split(',').next().map(str::trim).unwrap_or("id");
            format!(
                "INSERT INTO {} ({}){} ON DUPLICATE KEY UPDATE {} = EXCLUDED.{}",
                &caps["table"],
                columns,
                caps.name("middle").map_or("", |m| m.as_str()),
                first,
                first
            )
        })
        .into_owned();
    (result, count)
}

fn rewrite_nulls_not_distinct_index(sql: &str) -> (String, usize) {
    let pattern = regex_nnd_index();
    let mut count = 0;
    let result = pattern
        .replace_all(sql, |caps: &regex::Captures| {
            count += 1;
            let partial = caps.name("where").map_or("", |m| m.as_str());
            format!(
                "CREATE UNIQUE INDEX {if_not_exists}{name} ON {table} ({coalesced}) {partial}",
                if_not_exists = caps.name("ine").map_or("", |m| m.as_str()),
                name = &caps["name"],
                table = &caps["table"],
                coalesced = coalesce_columns(&caps["cols"]),
                partial = partial,
            )
        })
        .into_owned();
    (result, count)
}

fn rewrite_nulls_not_distinct_constraint(sql: &str) -> (String, usize) {
    let pattern = regex_nnd_constraint();
    let mut count = 0;
    let result = pattern
        .replace_all(sql, |caps: &regex::Captures| {
            count += 1;
            format!(
                // 约束改写为索引；IF NOT EXISTS 保证重跑幂等（og 支持）。
                "CREATE UNIQUE INDEX IF NOT EXISTS {name} ON {table} ({coalesced})",
                name = &caps["name"],
                table = &caps["table"],
                coalesced = coalesce_columns(&caps["cols"]),
            )
        })
        .into_owned();
    (result, count)
}

// 正则在模块底部集中定义（regex crate 由 Cargo.toml 引入）。
fn regex_on_conflict_update() -> regex::Regex {
    regex::Regex::new(r"(?is)ON\s+CONFLICT(?:\s*\([^)]*\))*?\s+DO\s+UPDATE\s+SET\s+(?P<set>[^;]*)").unwrap()
}

fn regex_on_conflict_nothing() -> regex::Regex {
    regex::Regex::new(r#"(?is)INSERT\s+INTO\s+(?P<table>[\w.]+)\s*\((?P<cols>[^)]*)\)(?P<middle>.*?)ON\s+CONFLICT\s+DO\s+NOTHING"#).unwrap()
}

fn regex_nnd_index() -> regex::Regex {
    regex::Regex::new(r#"(?is)CREATE\s+UNIQUE\s+INDEX\s+(?P<ine>IF\s+NOT\s+EXISTS\s+)?(?P<name>[\w.]+)\s+ON\s+(?P<table>[\w.]+)\s*\((?P<cols>[^)]*)\)\s*NULLS\s+NOT\s+DISTINCT\s*(?P<where>WHERE[^;]*)?"#).unwrap()
}

fn regex_nnd_constraint() -> regex::Regex {
    regex::Regex::new(r#"(?is)ALTER\s+TABLE\s+(?P<table>[\w.]+)[^;]*?ADD\s+CONSTRAINT\s+(?P<name>[\w.]+)\s+UNIQUE\s+NULLS\s+NOT\s+DISTINCT\s*\((?P<cols>[^)]*)\)"#).unwrap()
}

/// 金仓 V8R6（PG 模式，实测 2026-11-08）：唯一不兼容构造是
/// `NULLS NOT DISTINCT`——改写与 openGauss 相同的逐表达式 COALESCE
/// 唯一索引（金仓同样接受该形式）；其余方言扫描项仅登记不改写。
fn translate_kingbase(source: &str) -> Translation {
    let mut findings = Vec::new();
    scan_review_items(source, &mut findings);
    let mut sql = source.to_owned();

    let (rewritten, count) = rewrite_nulls_not_distinct_index(&sql);
    if count > 0 {
        sql = rewritten;
        findings.push(Finding {
            line: 0,
            category: Category::NullsNotDistinct,
            detail: format!("{count} NULLS NOT DISTINCT index rewritten to COALESCE expression index"),
        });
    }
    let (rewritten, count) = rewrite_nulls_not_distinct_constraint(&sql);
    if count > 0 {
        sql = rewritten;
        findings.push(Finding {
            line: 0,
            category: Category::NullsNotDistinct,
            detail: format!("{count} UNIQUE NULLS NOT DISTINCT constraint rewritten to expression index"),
        });
    }
    Translation { sql, findings }
}

fn translate_dameng(source: &str) -> Translation {
    let mut findings = Vec::new();
    let mut out = Vec::with_capacity(source.len());
    for (index, raw_line) in source.lines().enumerate() {
        let line = raw_line;
        let line_number = index + 1;

        // 评审项先行记录（对原行判断，不受改写影响）。
        scan_line(line, &mut findings, line_number);

        let mut line = line.to_owned();

        if line.contains("IF NOT EXISTS") {
            // 达梦不支持 IF NOT EXISTS；幂等化改写需结合达梦异常块语法
            // 逐条验证，此处剥离并登记。
            line = line.replace("IF NOT EXISTS ", "");
            findings.push(Finding {
                line: line_number,
                category: Category::IfNotExists,
                detail: "IF NOT EXISTS stripped; verify idempotency against DM8".into(),
            });
        }

        if line.contains("USING gin") {
            line = line.replace(" USING gin", "");
            findings.push(Finding {
                line: line_number,
                category: Category::GinIndex,
                detail: "gin index degraded to btree; register the functional gap".into(),
            });
        } else {
            line = line.replace(" USING btree", "");
        }

        // 部分索引：剥离 WHERE 谓词并登记（达梦无部分索引）。
        // 保留语句末尾分号——谓词后就是 `;`，一起剥掉会让后续语句粘连。
        if let Some(position) = index_where_clause(&line) {
            let had_semicolon = line[position..].trim().ends_with(';');
            line = line[..position].trim_end().to_owned();
            if had_semicolon {
                line.push(';');
            }
            findings.push(Finding {
                line: line_number,
                category: Category::PartialIndex,
                detail: "partial-index WHERE clause stripped; review uniqueness impact".into(),
            });
        }

        // 类型映射（词边界替换，避免误伤标识符）。
        for (from, to) in TYPE_MAP {
            line = replace_word(&line, from, to);
        }
        // character varying(n) → VARCHAR(n)（两词类型名）。
        line = regex_varying().replace_all(&line, "VARCHAR").into_owned();
        // `'<literal>'::<TYPE>(n)?` cast：达梦无 :: 语法，剥 cast 保留字面量。
        line = regex_string_cast().replace_all(&line, "'").into_owned();
        // timestamp 变体：达梦无 "without time zone" 词组。
        line = regex_ts_without_tz().replace_all(&line, "TIMESTAMP").into_owned();
        line = regex_ts_with_tz().replace_all(&line, "TIMESTAMP WITH TIME ZONE").into_owned();
        // `public` 是达梦保留字且无此 schema：剥限定前缀（对象落在默认
        // schema；其余自定义 schema 如 ai/infra/toonflow 保留限定）。
        line = line.replace("public.", "");
        // `WITH x(cols…) AS (VALUES …), y AS (VALUES …) SELECT …`：
        // 达梦不支持 VALUES 作为语句级行集构造（实测 -2007）。
        // 改写为 `WITH x AS (SELECT … UNION ALL SELECT …) SELECT …`。
        // 单行级 CTE 仅在本行闭合时才可逐行处理；多行 VALUES 统一在
        // 语句级预处理（statement_level_rewrites）完成。
        if line.to_uppercase().contains(" AS (") && line.to_uppercase().contains("VALUES (") {
            findings.push(Finding {
                line: line_number,
                category: Category::Other,
                detail: "VALUES-as-rowset CTE detected; rewritten at statement level".into(),
            });
        }

        // 达梦保留字作为标识符（列名等）：加引号。仅登记实测命中项——
        // 像 type/comment 这类会误伤 TYPE 关键字与 COMMENT ON 语法，
        // 命中时用更窄的列位置规则处理。
        line = regex_dm_reserved_domain().replace_all(&line, "\"domain\"").into_owned();

        // cast 与函数改写。
        // 序列语法（实测）：PG 的 NO MINVALUE/NO MAXVALUE 被拒；
        // START WITH/INCREMENT BY/CACHE n 原样可用；AS <type> 剥离。
        if line.to_uppercase().contains("CREATE SEQUENCE") {
            line = regex_no_minvalue().replace_all(&line, "").into_owned();
            line = regex_no_maxvalue().replace_all(&line, "").into_owned();
            line = regex_seq_as_type().replace_all(&line, "").into_owned();
            // CACHE n 实测为非法序列属性（-3602）；缺省缓存即可。
            line = regex_seq_cache().replace_all(&line, "").into_owned();
        }
        line = line.replace("::regclass", "");
        line = rewrite_nextval(&line);
        line = rewrite_ilike(&line);

        // 达梦支持 `TIMESTAMP WITH TIME ZONE`；`timestamptz` 缩写显式展开。
        line = replace_word(&line, "timestamptz", "TIMESTAMP WITH TIME ZONE");

        out.push(line);
    }

    let mut sql = out.join("\n");
    if source.ends_with('\n') {
        sql.push('\n');
    }
    // 语句级改写：WITH … AS (VALUES …) 行集 → UNION ALL SELECT 列表。
    let (rewritten, count) = rewrite_values_cte(&sql);
    if count > 0 {
        sql = rewritten;
        findings.push(Finding {
            line: 0,
            category: Category::Other,
            detail: format!("{count} VALUES-rowset CTEs rewritten to UNION ALL SELECT"),
        });
    }
    Translation { sql, findings }
}

/// `name(col1, col2) AS (VALUES (v1, v2), (v3, v4))` →
/// `name AS (SELECT 'v1', 'v2' FROM DUAL UNION ALL SELECT 'v3', 'v4' FROM DUAL)`。
/// 列名列表丢弃（达梦 WITH 列重命名支持未验证，SELECT 列即语义）；
/// 行内字面量保持原样（`''::type` cast 已在行级剥离）。
fn rewrite_values_cte(sql: &str) -> (String, usize) {
    let mut result = sql.to_owned();
    let mut count = 0usize;
    // 改写本身会移动后续字节，find 必须每轮重扫（clippy while-let 建议不适用：
    // 迭代变量随替换变化）。
    #[allow(clippy::while_let_loop)]
    while let Some(open_byte) = find_values_cte_start(&result) {
        // 从 open_byte 起做配对深度扫描，找闭括号字节位置。
        let Some(close_byte) = matching_paren(&result, open_byte) else {
            break;
        };
        let inner = result[open_byte + 1..close_byte].trim().to_owned();
        let Some(values_part) = inner.strip_prefix("VALUES") else {
            break;
        };
        let rows = split_top_level_commas(values_part);
        let mut selects = Vec::new();
        for row in &rows {
            let row_inner = row.trim();
            let Some(stripped) = row_inner.strip_prefix('(') else {
                continue;
            };
            let row_body = stripped.trim_end_matches(')').trim().to_owned();
            if row_body.is_empty() {
                continue;
            }
            selects.push(format!("SELECT {} FROM DUAL", row_body));
        }
        if selects.is_empty() {
            break;
        }
        // open 指向 "AS (" 的 "("；只替换括号段，"AS" 关键字保留在原文中。
        let replacement = format!("({});", selects.join(" UNION ALL "));
        // 替换 [open_byte, close_byte]（含括号本身），分号跟在语句尾。
        result.replace_range(open_byte..=close_byte, &replacement);
        count += 1;
    }
    (result, count)
}

/// `position` 处 `(` 的配对闭括号字节索引（忽略引号内内容）。
fn matching_paren(sql: &str, open: usize) -> Option<usize> {
    let bytes = sql.as_bytes();
    let mut depth = 0i32;
    // SQL 字符串：单引号包裹，'' 为转义引号（状态不变）。
    let mut in_string = false;
    let mut index = open;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if byte == b'\'' {
                if index + 1 < bytes.len() && bytes[index + 1] == b'\'' {
                    index += 2; // 转义引号
                    continue;
                }
                in_string = false;
            }
        } else {
            match byte {
                b'\'' => in_string = true,
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(index);
                    }
                }
                _ => {}
            }
        }
        index += 1;
    }
    None
}

/// 找下一个 `AS (` 后紧跟 `VALUES` 的 `(` 字节索引。
fn find_values_cte_start(sql: &str) -> Option<usize> {
    let upper = sql.to_uppercase();
    let mut search_from = 0usize;
    loop {
        let offset = upper[search_from..].find(" AS (")? + search_from;
        let open = offset + " AS (".len() - 1;
        let rest = upper[open + 1..].trim_start();
        if rest.starts_with("VALUES") {
            return Some(open);
        }
        search_from = open + 1;
    }
}

/// DDL 类型映射表（§4.1.6.2）。顺序无关：均为互不前缀重叠的完整词。
const TYPE_MAP: &[(&str, &str)] = &[
    ("jsonb", "TEXT"),
    ("uuid", "CHAR(36)"),
    ("boolean", "BIT"),
    ("bool", "BIT"),
    ("bytea", "BLOB"),
];

/// 词边界替换（不处理引号内文本——迁移 DDL 中关键字不出现在字符串里）。
fn replace_word(line: &str, from: &str, to: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut position = 0;
    while position < line.len() {
        let start_boundary =
            position == 0 || !is_word_byte(bytes[position - 1]);
        let end = position + from.len();
        let end_boundary = end >= bytes.len() || !is_word_byte(bytes[end]);
        if start_boundary && end_boundary && line[position..].starts_with(from) {
            result.push_str(to);
            position = end;
        } else {
            let ch = line[position..].chars().next().unwrap();
            result.push(ch);
            position += ch.len_utf8();
        }
    }
    result
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// `nextval('public.x_seq'::regclass)` → `x_seq.NEXTVAL`（::regclass 已剥离）。
fn regex_string_cast() -> regex::Regex {
    regex::Regex::new(r"(?i)'::[a-z_\[\]]+(\(\d+\))?").unwrap()
}

fn regex_ts_without_tz() -> regex::Regex {
    regex::Regex::new(r"(?i)timestamp\s+without\s+time\s+zone").unwrap()
}

fn regex_ts_with_tz() -> regex::Regex {
    regex::Regex::new(r"(?i)timestamp\s+with\s+time\s+zone").unwrap()
}

fn regex_dm_reserved_domain() -> regex::Regex {
    regex::Regex::new(r#"(?i)\bdomain\b"#).unwrap()
}

fn regex_no_minvalue() -> regex::Regex {
    regex::Regex::new(r"(?i)\s+NO\s+MINVALUE").unwrap()
}

fn regex_no_maxvalue() -> regex::Regex {
    regex::Regex::new(r"(?i)\s+NO\s+MAXVALUE").unwrap()
}

fn regex_seq_as_type() -> regex::Regex {
    regex::Regex::new(r"(?i)\s+AS\s+(bigint|integer|int|smallint)").unwrap()
}

fn regex_seq_cache() -> regex::Regex {
    regex::Regex::new(r"(?i)\s+CACHE\s+\d+").unwrap()
}

fn regex_varying() -> regex::Regex {
    regex::Regex::new(r"(?i)character\s+varying").unwrap()
}

fn rewrite_nextval(line: &str) -> String {
    if !line.contains("nextval(") {
        return line.to_owned();
    }
    let mut result = line.to_owned();
    while let Some(start) = result.find("nextval('") {
        let rest = &result[start + "nextval('".len()..];
        let Some(end_quote) = rest.find('\'') else {
            break;
        };
        let qualified = &rest[..end_quote];
        let sequence = qualified.rsplit('.').next().unwrap_or(qualified);
        // 连同 nextval( 的闭括号一并替换，避免残留 `)`。
        let mut end = start + "nextval('".len() + end_quote + 1;
        if result[end..].starts_with(')') {
            end += 1;
        }
        result.replace_range(start..end, &format!("{sequence}.NEXTVAL"));
    }
    result
}

/// `ILIKE` → `UPPER(x) LIKE UPPER(y)`。迁移链中的 `ilike` 仅出现在
/// `col ILIKE 'pat'` 简单形式：左操作数取前一分隔符（`(`/`,`/`=`）之后，
/// 右操作数取到下一分隔符（`)`/`,`）或行尾，其余形式由评审报告兜底。
fn rewrite_ilike(line: &str) -> String {
    let upper = line.to_uppercase();
    let Some(position) = upper.find(" ILIKE ") else {
        return line.to_owned();
    };
    let bytes = line.as_bytes();
    let left_start = bytes[..position]
        .iter()
        .rposition(|byte| matches!(byte, b'(' | b',' | b'=' | b' ' | b'\t'))
        .map_or(0, |index| index + 1);
    let after = position + " ILIKE ".len();
    let right_end = bytes[after..]
        .iter()
        .position(|byte| matches!(byte, b')' | b',' | b';' | b' ' | b'\t'))
        .map_or(line.len(), |index| after + index);
    let left = line[left_start..position].trim();
    let right = line[after..right_end].trim();
    format!(
        "{}UPPER({}) LIKE UPPER({}){}",
        &line[..left_start],
        left,
        right,
        &line[right_end..]
    )
}

/// 找到部分索引的 ` WHERE ` 谓词起点（仅限 CREATE INDEX 行）。
fn index_where_clause(line: &str) -> Option<usize> {
    let upper = line.to_uppercase();
    if !upper.contains("CREATE") || !upper.contains("INDEX") {
        return None;
    }
    upper.find(" WHERE ")
}

/// 全文评审项扫描（金仓 / openGauss 报告也用它）。
fn scan_review_items(source: &str, findings: &mut Vec<Finding>) {
    for (index, line) in source.lines().enumerate() {
        scan_line(line, findings, index + 1);
    }
}

/// 单行评审项扫描。
fn scan_line(line: &str, findings: &mut Vec<Finding>, number: usize) {
    let upper = line.to_uppercase();
    let mut push = |category: Category, detail: String| {
        findings.push(Finding {
            line: number,
            category,
            detail,
        });
    };
    if upper.contains("ON CONFLICT") {
        push(Category::OnConflict, "rewrite to MERGE INTO for DM8".into());
    }
    if upper.contains("EXCLUDE USING") {
        push(
            Category::Exclude,
            "replace with a unique index or application guarantee".into(),
        );
    }
    if line.contains("[]") || upper.contains("ARRAY[") || upper.contains("'{}'") {
        push(
            Category::Array,
            "array column/literal needs TEXT storage and explicit default".into(),
        );
    }
    if upper.contains("INTERVAL ") {
        push(
            Category::Interval,
            "interval literal must become a computed timestamp or numeric column".into(),
        );
    }
    if upper.contains("FOR UPDATE") {
        push(
            Category::ForUpdate,
            "lock semantics verified by contract tests on each flavor".into(),
        );
    }
    if upper.contains("TO_CHAR(") || upper.contains("EXTRACT(") || upper.contains("GEN_RANDOM_UUID(")
    {
        push(
            Category::Other,
            "PG-specific function requires a dialect branch or app-side replacement".into(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kingbase_copies_verbatim_and_reports() {
        let source = "CREATE TABLE t(a jsonb);\nINSERT INTO t ON CONFLICT (id) DO NOTHING;\n";
        let translation = translate(Target::Kingbase, source);
        assert_eq!(translation.sql, source);
        assert!(translation
            .findings
            .iter()
            .any(|finding| finding.category == Category::OnConflict && finding.line == 2));
    }

    #[test]
    fn dameng_maps_types() {
        let source = "CREATE TABLE t(id uuid NOT NULL, data jsonb DEFAULT '{}', flag boolean, raw bytea);\n";
        let translation = translate(Target::Dameng, source);
        assert!(translation.sql.contains("id CHAR(36) NOT NULL"));
        assert!(translation.sql.contains("data TEXT DEFAULT '{}'"));
        assert!(translation.sql.contains("flag BIT"));
        assert!(translation.sql.contains("raw BLOB"));
        // 数组字面量默认值仍在，但已登记评审
        assert!(translation
            .findings
            .iter()
            .any(|finding| finding.category == Category::Array));
    }

    #[test]
    fn dameng_does_not_break_identifiers() {
        let source = "CREATE TABLE device_uuid_map(uuid_col uuid);\n";
        let translation = translate(Target::Dameng, source);
        // device_uuid_map 与 uuid_col 不应被词边界替换误伤成 CHAR(36)
        assert!(translation.sql.contains("device_uuid_map"));
        assert!(translation.sql.contains("uuid_col CHAR(36)"));
    }

    #[test]
    fn dameng_rewrites_sequence_defaults() {
        let source =
            "id bigint DEFAULT nextval('public.infra_file_seq'::regclass) NOT NULL,\n";
        let translation = translate(Target::Dameng, source);
        assert!(translation.sql.contains("DEFAULT infra_file_seq.NEXTVAL NOT NULL"));
        assert!(!translation.sql.contains("::regclass"));
        assert!(!translation.sql.contains("nextval("));
    }

    #[test]
    fn dameng_rewrites_ilike() {
        let source = "WHERE name ILIKE '%abc%';\n";
        let translation = translate(Target::Dameng, source);
        assert!(translation.sql.contains("UPPER(name) LIKE UPPER('%abc%')"));
    }

    #[test]
    fn dameng_strips_if_not_exists_and_reports() {
        let source = "CREATE TABLE IF NOT EXISTS t(id bigint);\nCREATE INDEX IF NOT EXISTS idx ON t(id);\n";
        let translation = translate(Target::Dameng, source);
        assert!(!translation.sql.contains("IF NOT EXISTS"));
        assert_eq!(
            translation
                .findings
                .iter()
                .filter(|finding| finding.category == Category::IfNotExists)
                .count(),
            2
        );
    }

    #[test]
    fn dameng_degrades_special_indexes() {
        let source =
            "CREATE INDEX idx ON t USING gin (payload);\nCREATE INDEX partial_idx ON t(ip) WHERE (deleted = 0);\n";
        let translation = translate(Target::Dameng, source);
        assert!(!translation.sql.contains("USING gin"));
        assert!(!translation.sql.contains("WHERE (deleted = 0)"));
        assert!(translation
            .findings
            .iter()
            .any(|finding| finding.category == Category::GinIndex));
        assert!(translation
            .findings
            .iter()
            .any(|finding| finding.category == Category::PartialIndex));
    }

    #[test]
    fn dameng_rewrites_values_cte() {
        // 0003 的真实形态：WITH defs(cols) AS (VALUES rows) SELECT
        let source = "WITH defs(name, path, sort) AS (\n    VALUES ('A', 'a', 10),\n    ('B', 'b', 20)\n) SELECT * FROM defs;\n";
        let translation = translate(Target::Dameng, source);
        assert!(translation.sql.contains("AS (SELECT 'A', 'a', 10 FROM DUAL UNION ALL SELECT 'B', 'b', 20 FROM DUAL)"));
        assert!(!translation.sql.contains("VALUES"));
        assert!(translation
            .findings
            .iter()
            .any(|finding| finding.detail.contains("VALUES-rowset")));
    }

    #[test]
    fn dameng_preserves_index_where_semicolon() {
        let source = "CREATE INDEX idx_t ON t(col) WHERE deleted = 0;\nCREATE INDEX idx_u ON u(col);\n";
        let translation = translate(Target::Dameng, source);
        assert_eq!(
            translation.sql.matches(';').count(),
            2,
            "partial-index strip must keep the trailing semicolon: {}",
            translation.sql
        );
    }

    #[test]
    fn dameng_strips_sequence_cache_and_casts() {
        let source = "CREATE SEQUENCE s START WITH 1 INCREMENT BY 1 CACHE 1;\nCREATE TABLE t(v VARCHAR(64) DEFAULT ''::VARCHAR NOT NULL);\nCREATE TABLE u(ts timestamp without time zone DEFAULT CURRENT_TIMESTAMP);\n";
        let translation = translate(Target::Dameng, source);
        assert!(!translation.sql.contains("CACHE 1"));
        assert!(translation.sql.contains("DEFAULT '' NOT NULL"), "{}", translation.sql);
        assert!(translation.sql.contains("ts TIMESTAMP DEFAULT"), "{}", translation.sql);
    }

    #[test]
    fn dameng_flags_merge_and_lock_items() {
        let source =
            "INSERT INTO t SELECT * FROM u ON CONFLICT (id) DO UPDATE SET a=1;\nSELECT 1 FOR UPDATE;\n";
        let translation = translate(Target::Dameng, source);
        assert_eq!(translation.sql, source); // 不盲改，留给人工 MERGE 重写
        assert!(translation
            .findings
            .iter()
            .any(|finding| finding.category == Category::OnConflict));
        assert!(translation
            .findings
            .iter()
            .any(|finding| finding.category == Category::ForUpdate));
    }

    #[test]
    fn opengauss_rewrites_on_conflict_forms() {
        // DO UPDATE：目标列丢弃，SET 保留（EXCLUDED 在 openGauss 可用）
        let source = "INSERT INTO t(a,b) SELECT x,y FROM u\n ON CONFLICT (id) DO UPDATE SET a=EXCLUDED.a;\n";
        let translation = translate(Target::OpenGauss, source);
        assert!(translation.sql.contains("ON DUPLICATE KEY UPDATE a=EXCLUDED.a"));
        assert!(!translation.sql.contains("ON CONFLICT"));

        // DO NOTHING：首列自赋值（openGauss 无 DO NOTHING）
        let source = "INSERT INTO public.infra_port_rule (name,proto)\n VALUES ('a','tcp')\nON CONFLICT DO NOTHING;\n";
        let translation = translate(Target::OpenGauss, source);
        assert!(translation
            .sql
            .contains("ON DUPLICATE KEY UPDATE name = EXCLUDED.name"));
        assert!(!translation.sql.contains("ON CONFLICT"));
    }

    #[test]
    fn opengauss_rewrites_nulls_not_distinct_index() {
        let source = "CREATE UNIQUE INDEX IF NOT EXISTS idx_t ON public.t(tenant_id,ip) NULLS NOT DISTINCT;\n";
        let translation = translate(Target::OpenGauss, source);
        assert!(translation.sql.contains(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_t ON public.t ((COALESCE(tenant_id::text, '\\x01OG_NULL')), (COALESCE(ip::text, '\\x01OG_NULL')))"
        ));
        assert!(!translation.sql.contains("NULLS NOT DISTINCT"));

        // 带部分索引 WHERE 的形态（0037）
        let source = "CREATE UNIQUE INDEX IF NOT EXISTS idx_c\n    ON public.c(tenant_id, instance_id, cloud_provider_config_id)\n    NULLS NOT DISTINCT\n    WHERE tenant_id IS NOT NULL AND deleted=0;\n";
        let translation = translate(Target::OpenGauss, source);
        assert!(translation.sql.contains("COALESCE(cloud_provider_config_id::text"));
        assert!(translation.sql.contains("WHERE tenant_id IS NOT NULL AND deleted=0"));
    }

    #[test]
    fn opengauss_rewrites_nulls_not_distinct_constraint() {
        let source = "ALTER TABLE public.cmdb_instance_unique_value ADD CONSTRAINT cmdb_instance_unique_value_key UNIQUE NULLS NOT DISTINCT (tenant_id, model_id, value);\n";
        let translation = translate(Target::OpenGauss, source);
        assert!(translation.sql.contains(
            "CREATE UNIQUE INDEX IF NOT EXISTS cmdb_instance_unique_value_key ON public.cmdb_instance_unique_value ((COALESCE(tenant_id::text"
        ));
        assert!(translation.sql.contains("COALESCE(value::text"));
        assert!(!translation.sql.contains("NULLS NOT DISTINCT"));
        assert!(!translation.sql.contains("ADD CONSTRAINT"));
    }

    #[test]
    fn trailing_newline_preserved() {
        let translation = translate(Target::Dameng, "SELECT 1;\n");
        assert!(translation.sql.ends_with('\n'));
    }
}
