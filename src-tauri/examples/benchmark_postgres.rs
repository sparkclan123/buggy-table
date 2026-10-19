//! Disposable Compose fixture ONLY. Creates and removes its own synthetic schema.
//! BUGGY_BENCH_TABLES=100 BUGGY_BENCH_DELAY_MS=10 cargo run --release
//!   --manifest-path src-tauri/Cargo.toml --no-default-features --example benchmark_postgres
use std::{
    error::Error,
    time::{Duration, Instant},
};
use table_por::{models::*, service::Workbench};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinSet,
};
use tokio_postgres::NoTls;
use uuid::Uuid;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn setting(name: &str, default: usize) -> usize {
    std::env::var(name)
        .map(|v| v.parse().expect("integer benchmark setting"))
        .unwrap_or(default)
}

async fn forward(
    mut from: tokio::net::tcp::OwnedReadHalf,
    mut to: tokio::net::tcp::OwnedWriteHalf,
    delay: Duration,
) -> std::io::Result<()> {
    let mut buffer = vec![0; 65536];
    loop {
        let n = from.read(&mut buffer).await?;
        if n == 0 {
            return to.shutdown().await;
        }
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
        }
        to.write_all(&buffer[..n]).await?;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let count = setting("BUGGY_BENCH_TABLES", 100);
    let delay_ms = setting("BUGGY_BENCH_DELAY_MS", 0);
    let samples = setting("BUGGY_BENCH_SAMPLES", 5);
    assert!((1..=500).contains(&count) && (1..=100).contains(&samples) && delay_ms <= 50);
    let (admin, connection) = tokio_postgres::connect(
        "host=127.0.0.1 port=15432 dbname=tablepor_test user=fixture_admin password=local-fixture-admin", NoTls
    ).await?;
    let admin_task = tokio::spawn(async move {
        let _ = connection.await;
    });
    // Unique namespace; never drop or modify an existing schema/table.
    let schema = format!("bench_{}", Uuid::new_v4().simple());
    let mut ddl =
        format!("CREATE SCHEMA {schema}; GRANT USAGE ON SCHEMA {schema} TO tablepor_reader;");
    for i in 0..count {
        ddl.push_str(&format!("CREATE TABLE {schema}.item_{i:03} (id bigint PRIMARY KEY, label text, amount numeric(20,2));"));
    }
    ddl.push_str(&format!("INSERT INTO {schema}.item_000 SELECT i, 'synthetic row', i * 1.25 FROM generate_series(1,450) i; GRANT SELECT ON ALL TABLES IN SCHEMA {schema} TO tablepor_reader;"));
    admin.batch_execute(&ddl).await?;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let proxy = tokio::spawn(async move {
        let mut connections = JoinSet::new();
        while let Ok((incoming, _)) = listener.accept().await {
            connections.spawn(async move {
                let outgoing = TcpStream::connect("127.0.0.1:15432").await?;
                incoming.set_nodelay(true)?;
                outgoing.set_nodelay(true)?;
                let (ir, iw) = incoming.into_split();
                let (or, ow) = outgoing.into_split();
                let delay = Duration::from_millis(delay_ms as u64);
                tokio::try_join!(forward(ir, ow, delay), forward(or, iw, delay))?;
                Ok::<_, std::io::Error>(())
            });
            while connections.try_join_next().is_some() {}
        }
    });
    let result = measure(&schema, port, count, delay_ms, samples).await;
    proxy.abort();
    // Cleanup happens even if a measured app operation fails.
    admin
        .batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
        .await?;
    admin_task.abort();
    result
}

async fn measure(
    schema: &str,
    port: u16,
    tables: usize,
    delay_ms: usize,
    samples: usize,
) -> Result<()> {
    for manual_edits in [true, false] {
        let app = Workbench::default();
        let info = app
            .connect(ConnectRequest {
                engine: Engine::Postgres,
                name: "Synthetic benchmark".into(),
                host: "127.0.0.1".into(),
                port,
                database: "tablepor_test".into(),
                username: "tablepor_reader".into(),
                password: "local-reader-only".into(),
                schema: schema.into(),
                tls: TlsMode::LocalOnly,
                root_ca_pem: String::new(),
                manual_edits,
            })
            .await?;
        for operation in ["query", "browse"] {
            let mut totals = Vec::new();
            let mut reported = Vec::new();
            for sample in 0..=samples {
                let started = Instant::now();
                let result = if operation == "query" {
                    app.query(QueryRequest {
                        connection_id: info.id.clone(),
                        request_id: Uuid::new_v4().to_string(),
                        schema: Some(schema.into()),
                        sql: format!(
                            "SELECT id, label, amount FROM {schema}.item_000 ORDER BY id LIMIT 200"
                        ),
                        explain: false,
                    })
                    .await?
                } else {
                    app.browse(BrowseRequest {
                        connection_id: info.id.clone(),
                        request_id: Uuid::new_v4().to_string(),
                        schema: Some(schema.into()),
                        table: "item_000".into(),
                        filters: vec![],
                        filter_match: FilterMatch::All,
                        sort_column: None,
                        descending: false,
                        page: 0,
                        page_size: 50,
                    })
                    .await?
                };
                let total = started.elapsed().as_secs_f64() * 1000.0;
                assert_eq!(
                    result.rows.len(),
                    if operation == "query" { 200 } else { 50 }
                );
                assert_eq!(result.rows[0][0].as_deref(), Some("1"));
                if sample > 0 {
                    totals.push(total);
                    reported.push(result.elapsed_ms);
                }
            }
            totals.sort_by(f64::total_cmp);
            reported.sort_unstable();
            println!(
                "{}",
                serde_json::json!({
                    "tables": tables, "delayPerDirectionMs": delay_ms, "mode": if manual_edits {"manual"} else {"strict"},
                    "operation": operation, "rows": if operation == "query" {200} else {50}, "samples": samples, "medianTotalMs": totals[samples/2],
                    "p95TotalMs": totals[(samples * 95).div_ceil(100) - 1], "medianReportedMs": reported[samples/2],
                })
            );
        }
        app.disconnect(&info.id).await?;
    }
    Ok(())
}
