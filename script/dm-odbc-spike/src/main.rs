//! 达梦 ODBC spike（M0 清单）：建表 + CRUD + 并发压测 + 故障注入。
//! 通过 unixODBC + 厂商 libdodbc.so 直连 DM8。

use anyhow::{Context, bail};
use odbc_api::{Connection, ConnectionOptions, Cursor, Environment};

const CONN: &str = "Driver={DM8 ODBC DRIVER};SERVER={server};TCP_PORT={port};UID=SYSDBA;PWD={pwd}";

fn main() -> anyhow::Result<()> {
    let server = std::env::var("DM_SERVER").unwrap_or_else(|_| "127.0.0.1".into());
    let port = std::env::var("DM_PORT").unwrap_or_else(|_| "5236".into());
    let pwd = std::env::var("DM_PWD").unwrap_or_else(|_| "Dameng2026".into());

    // 故障注入 1：错误口令必须干净失败（不 panic、不挂起）。
    let env = Environment::new()?;
    let bad = CONN
        .replace("{server}", &server)
        .replace("{port}", &port)
        .replace("{pwd}", "WRONG_PWD");
    match env.connect_with_connection_string(&bad, ConnectionOptions::default()) {
        Ok(_) => bail!("错误口令竟然连接成功——故障注入未生效"),
        Err(error) => println!("[fault-injection] bad password rejected cleanly: {}", short(&format!("{error:#}"))),
    }

    let conn = env
        .connect_with_connection_string(
            &CONN.replace("{server}", &server).replace("{port}", &port).replace("{pwd}", &pwd),
            ConnectionOptions::default(),
        )
        .context("connect DM8 via ODBC")?;
    println!("[connect] OK");

    // 建表（幂等）。
    exec(&conn, "DROP TABLE IF EXISTS dm_odbc_spike").ok();
    exec(&conn, "CREATE TABLE dm_odbc_spike(id INT PRIMARY KEY, payload VARCHAR(128), ts TIMESTAMP)")?;
    println!("[ddl] table created");

    // CRUD。
    for id in 0..20 {
        exec(&conn, &format!("INSERT INTO dm_odbc_spike VALUES ({id}, 'row-{id}', SYSDATE)"))?;
    }
    let count: u32 = query_scalar(&conn, "SELECT COUNT(*) FROM dm_odbc_spike")?;
    assert_eq!(count, 20, "20 inserts must land");
    exec(&conn, "UPDATE dm_odbc_spike SET payload = 'updated' WHERE id = 1")?;
    exec(&conn, "DELETE FROM dm_odbc_spike WHERE id = 2")?;
    let after: u32 = query_scalar(&conn, "SELECT COUNT(*) AS CNT FROM dm_odbc_spike")?;
    assert_eq!(after, 19, "update keeps count, delete removes one");
    println!("[crud] insert/update/delete/select OK (20 -> 19 rows)");

    // 并发压测：4 线程 × 25 次写入（各自独立连接）。
    let mut handles = Vec::new();
    for worker in 0..4 {
        let server = server.clone();
        let port = port.clone();
        let pwd = pwd.clone();
        handles.push(std::thread::spawn(move || -> anyhow::Result<usize> {
            let env = Environment::new()?;
            let conn = env.connect_with_connection_string(
                &CONN.replace("{server}", &server).replace("{port}", &port).replace("{pwd}", &pwd),
                ConnectionOptions::default(),
            )?;
            for i in 0..25 {
                let id = 1000 + worker * 100 + i;
                exec(&conn, &format!("INSERT INTO dm_odbc_spike VALUES ({id}, 'w{worker}-{i}', SYSDATE)"))?;
            }
            Ok(25)
        }));
    }
    let mut inserted = 0usize;
    for handle in handles {
        inserted += handle.join().map_err(|error| anyhow::anyhow!("{error:?}"))??;
    }
    let total: u32 = query_scalar(&conn, "SELECT COUNT(*) AS CNT FROM dm_odbc_spike")?;
    assert_eq!(total, 19 + inserted as u32, "concurrent inserts must all land");
    println!("[concurrency] 4 workers x 25 inserts OK (total {total} rows)");

    // 故障注入 2：访问不存在的表必须干净报错。
    match exec(&conn, "SELECT * FROM no_such_table_xyz") {
        Ok(()) => bail!("访问不存在表竟然成功"),
        Err(error) => println!("[fault-injection] missing table rejected cleanly: {}", short(&format!("{error:#}"))),
    }

    exec(&conn, "DROP TABLE dm_odbc_spike")?;
    println!("SPIKE PASS: ODBC 连接/建表/CRUD/并发/故障注入 全部通过 —— 达梦 Rust ODBC 路线成立");
    Ok(())
}

fn exec(conn: &Connection, sql: &str) -> anyhow::Result<()> {
    conn.execute(sql, ())
        .with_context(|| format!("execute: {sql}"))?;
    Ok(())
}

fn query_scalar(conn: &Connection, sql: &str) -> anyhow::Result<u32> {
    // odbc-api 8：CursorRow 走 get_data 手动缓冲（列下标从 1 起）。
    let mut cursor = conn.execute(sql, ())?.expect("query returns cursor");
    let mut row = cursor.next_row()?.expect("at least one row");
    let mut buffer = Vec::new();
    let has_data = row.get_text(1, &mut buffer)?;
    anyhow::ensure!(has_data, "scalar query returned no row data");
    let text = String::from_utf8(buffer).context("scalar not utf8")?;
    text.trim().parse::<u32>().context("scalar not numeric")
}

fn short(text: &str) -> String {
    text.lines().next().unwrap_or("").chars().take(110).collect()
}
