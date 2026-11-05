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
        // 金仓 / openGauss：PG 兼容模式，逐字复制；仅扫描评审项供报告参考。
        Target::Kingbase | Target::OpenGauss => {
            let mut findings = Vec::new();
            scan_review_items(source, &mut findings);
            Translation {
                sql: source.to_owned(),
                findings,
            }
        }
        Target::Dameng => translate_dameng(source),
    }
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
        if let Some(position) = index_where_clause(&line) {
            line = line[..position].trim_end().to_owned();
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

        // cast 与函数改写。
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
    Translation { sql, findings }
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
    fn trailing_newline_preserved() {
        let translation = translate(Target::Dameng, "SELECT 1;\n");
        assert!(translation.sql.ends_with('\n'));
    }
}
