//! sql-translate：将 `sql/postgresql/` 迁移链翻译为金仓 / openGauss / 达梦方言
//! （docs/xinchuang-adaptation-plan.md §4.1.6.3、§4.1.8）。
//!
//! 用法：
//!
//! ```text
//! cargo run -p sql-translate -- --target dm --output target/sql-translate/dm --report
//! cargo run -p sql-translate -- --target kingbase --output target/sql-translate/kingbase
//! cargo run -p sql-translate -- --target dm --output sql/dm --strict   # CI 门禁
//! ```
//!
//! 达梦输出默认写到暂存目录；`sql/dm` 仅在 0001 流水线（导出→翻译→回灌→
//! checksum 比对）验证通过并完成人工评审后才允许入库。`--strict` 使评审项
//! 成为错误，供翻译链固化后接入 CI。

use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod rules;

use rules::{Category, Target, translate};

struct Args {
    target: Target,
    source: PathBuf,
    output: PathBuf,
    report: bool,
    strict: bool,
    /// 只翻译手写迁移（跳过 pg_dump 基线 0001，按方案 R2 走导出流水线）。
    skip_baseline: bool,
}

fn parse_args() -> Option<Args> {
    let mut target = None;
    let mut source = PathBuf::from("sql/postgresql");
    let mut output = None;
    let mut report = false;
    let mut strict = false;
    let mut skip_baseline = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--target" => target = args.next().and_then(|value| Target::parse(&value)),
            "--source" => source = args.next().map(PathBuf::from)?,
            "--output" => output = args.next().map(PathBuf::from),
            "--report" => report = true,
            "--strict" => strict = true,
            "--skip-baseline" => skip_baseline = true,
            "--help" | "-h" => return None,
            _ => return None,
        }
    }
    Some(Args {
        target: target?,
        source,
        output: output?,
        report,
        strict,
        skip_baseline,
    })
}

fn usage() -> &'static str {
    "usage: sql-translate --target <kingbase|opengauss|dm> --output <dir> \
     [--source sql/postgresql] [--report] [--strict] [--skip-baseline]"
}

fn main() -> ExitCode {
    let Some(args) = parse_args() else {
        eprintln!("{}", usage());
        return ExitCode::from(2);
    };

    let files = match collect_source_files(&args.source, args.skip_baseline) {
        Ok(files) => files,
        Err(error) => {
            eprintln!("error: {error}");
            return ExitCode::from(2);
        }
    };
    if files.is_empty() {
        eprintln!("error: no .sql migrations found in {}", args.source.display());
        return ExitCode::from(2);
    }
    if let Err(error) = std::fs::create_dir_all(&args.output) {
        eprintln!("error: create {}: {error}", args.output.display());
        return ExitCode::from(2);
    }

    let mut review_total = 0usize;
    let mut per_file_findings: Vec<(std::ffi::OsString, Vec<rules::Finding>)> = Vec::new();
    for path in &files {
        let source_sql = match std::fs::read_to_string(path) {
            Ok(sql) => sql,
            Err(error) => {
                eprintln!("error: read {}: {error}", path.display());
                return ExitCode::from(2);
            }
        };
        let file_name = path.file_name().expect("migration file name").to_owned();
        let translation = translate(args.target, &source_sql);
        let destination = args.output.join(&file_name);
        if let Err(error) = std::fs::write(&destination, &translation.sql) {
            eprintln!("error: write {}: {error}", destination.display());
            return ExitCode::from(2);
        }
        review_total += translation.findings.len();
        if !translation.findings.is_empty() {
            println!(
                "{}: {} review item(s)",
                file_name.to_string_lossy(),
                translation.findings.len()
            );
            if args.report {
                for finding in &translation.findings {
                    println!(
                        "  line {:>5}  {:<18} {}",
                        finding.line,
                        finding.category.label(),
                        finding.detail
                    );
                }
            }
            per_file_findings.push((file_name, translation.findings));
        }
    }

    println!(
        "translated {} file(s) -> {} ({} review item(s){})",
        files.len(),
        args.output.display(),
        review_total,
        if args.strict { "; strict mode" } else { "" }
    );
    // 类别摘要：评审报告按构造类型聚合，便于分派专项处理。
    let all_findings: Vec<rules::Finding> = per_file_findings
        .iter()
        .flat_map(|(_, findings)| findings.iter().cloned())
        .collect();
    if !all_findings.is_empty() {
        println!("review summary by category:");
        for (category, count) in summarize(&all_findings) {
            println!("  {:<18} {}", category.label(), count);
        }
    }
    if args.strict && !per_file_findings.is_empty() {
        eprintln!(
            "error: {} file(s) still carry manual-review items; \
             resolve them before committing the generated chain",
            per_file_findings.len()
        );
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

/// 列出待翻译文件：编号排序；`--skip-baseline` 跳过 0001（pg_dump 基线
/// 按方案 R2 走导出流水线，不进逐文件翻译）。
fn collect_source_files(source: &Path, skip_baseline: bool) -> std::io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(source)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|extension| extension == "sql")
                && !(skip_baseline
                    && path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("0001")))
        })
        .collect();
    files.sort();
    Ok(files)
}

/// 评审类别聚合（报告尾部摘要用）；保留 pub 以便后续 lib 化供 CI 复用。
#[allow(dead_code)]
fn summarize(findings: &[rules::Finding]) -> Vec<(Category, usize)> {
    let mut counts: Vec<(Category, usize)> = Vec::new();
    for finding in findings {
        if let Some(entry) = counts.iter_mut().find(|(category, _)| *category == finding.category)
        {
            entry.1 += 1;
        } else {
            counts.push((finding.category, 1));
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_collection_skips_baseline_and_sorts() {
        let temporary = std::env::temp_dir().join(format!("sql-translate-{}", std::process::id()));
        std::fs::create_dir_all(&temporary).unwrap();
        for name in ["0002_b.sql", "0001_initial.sql", "0003_c.sql", "notes.md"] {
            std::fs::write(temporary.join(name), "SELECT 1;").unwrap();
        }
        let files = collect_source_files(&temporary, true).unwrap();
        let names: Vec<String> = files
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["0002_b.sql", "0003_c.sql"]);
        let all = collect_source_files(&temporary, false).unwrap();
        assert_eq!(all.len(), 3);
        std::fs::remove_dir_all(&temporary).unwrap();
    }

    #[test]
    fn strict_mode_aggregates_findings() {
        let findings = vec![rules::Finding {
            line: 1,
            category: Category::OnConflict,
            detail: "x".into(),
        }];
        let summary = summarize(&findings);
        assert_eq!(summary, vec![(Category::OnConflict, 1)]);
    }
}
