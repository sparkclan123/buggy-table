//! Measures client overhead against disposable Compose fixtures only.
//! Start `docker compose up -d --wait`, then run with --no-default-features.
//! BUGGY_BENCH_DELAY_MS adds latency to each relay read in each direction;
//! it is a controlled approximation, not a WAN throughput benchmark.
use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use table_por::{models::*, service::Workbench};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinSet,
};
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
    let delay_ms = setting("BUGGY_BENCH_DELAY_MS", 0);
    let samples = setting("BUGGY_BENCH_SAMPLES", 30);
    assert!((1..=100).contains(&samples) && delay_ms <= 50);
    for engine in [Engine::Mysql, Engine::Postgres] {
        let mysql = engine == Engine::Mysql;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let connections = Arc::new(AtomicUsize::new(0));
        let accepted = connections.clone();
        let proxy = tokio::spawn(async move {
            let mut tasks = JoinSet::new();
            while let Ok((incoming, _)) = listener.accept().await {
                accepted.fetch_add(1, Ordering::Relaxed);
                tasks.spawn(async move {
                    let outgoing = TcpStream::connect(if mysql {
                        "127.0.0.1:13306"
                    } else {
                        "127.0.0.1:15432"
                    })
                    .await?;
                    incoming.set_nodelay(true)?;
                    outgoing.set_nodelay(true)?;
                    let (ir, iw) = incoming.into_split();
                    let (or, ow) = outgoing.into_split();
                    let delay = Duration::from_millis(delay_ms as u64);
                    tokio::try_join!(forward(ir, ow, delay), forward(or, iw, delay))?;
                    Ok::<_, std::io::Error>(())
                });
                while tasks.try_join_next().is_some() {}
            }
        });
        let result = measure(engine, port, samples, delay_ms, &connections).await;
        proxy.abort();
        result?;
    }
    Ok(())
}
async fn measure(
    engine: Engine,
    port: u16,
    samples: usize,
    delay: usize,
    connections: &AtomicUsize,
) -> Result<()> {
    let mysql = engine == Engine::Mysql;
    let schema = if mysql { "tablepor_test" } else { "public" };
    let table = if mysql { "items" } else { "edit_items" };
    let app = Workbench::default();
    let started = Instant::now();
    let info = app
        .connect(ConnectRequest {
            engine,
            name: "Disposable benchmark".into(),
            host: "127.0.0.1".into(),
            port,
            database: "tablepor_test".into(),
            username: "tablepor_reader".into(),
            password: if mysql {
                "local-mysql-reader"
            } else {
                "local-reader-only"
            }
            .into(),
            schema: schema.into(),
            tls: TlsMode::LocalOnly,
            root_ca_pem: String::new(),
            manual_edits: false,
        })
        .await?;
    let connect_ms = started.elapsed().as_secs_f64() * 1000.0;
    for operation in ["query", "browse"] {
        let before = connections.load(Ordering::Relaxed);
        let mut totals = vec![];
        let mut reused = 0;
        let mut validation = vec![];
        let mut execution = vec![];
        for sample in 0..=samples {
            let started = Instant::now();
            let result = if operation == "query" {
                app.query(QueryRequest {
                    database: None,
                    connection_id: info.id.clone(),
                    request_id: Uuid::new_v4().to_string(),
                    schema: Some(schema.into()),
                    sql: format!(
                        "SELECT id, label, amount FROM {schema}.{table} ORDER BY id LIMIT 2"
                    ),
                    explain: false,
                })
                .await?
            } else {
                app.browse(BrowseRequest {
                    database: None,
                    connection_id: info.id.clone(),
                    request_id: Uuid::new_v4().to_string(),
                    schema: Some(schema.into()),
                    table: table.into(),
                    filters: vec![],
                    filter_match: FilterMatch::All,
                    sort_column: None,
                    descending: false,
                    page: 0,
                    page_size: 100,
                })
                .await?
            };
            assert_eq!(result.rows.len(), 2);
            assert_eq!(result.rows[0][0].as_deref(), Some("1"));
            if sample > 0 {
                totals.push(started.elapsed().as_secs_f64() * 1000.0);
                if let Some(t) = result.timings {
                    reused += usize::from(t.connection_reused);
                    validation.push(t.validation_ms);
                    execution.push(t.execution_ms);
                }
            }
        }
        totals.sort_by(f64::total_cmp);
        validation.sort_unstable();
        execution.sort_unstable();
        println!(
            "{}",
            serde_json::json!({
                "engine": if mysql {"mysql"} else {"postgres"}, "operation": operation,
                "samples": samples, "rows": 2, "delayPerDirectionMs": delay,
                "medianTotalMs": totals[samples/2], "p95TotalMs": totals[(samples*95).div_ceil(100)-1],
                "connectMs": connect_ms, "newConnectionsIncludingWarmup": connections.load(Ordering::Relaxed)-before,
                "reusedSamples": reused, "medianValidationMs": validation.get(samples/2), "medianExecutionMs": execution.get(samples/2),
            })
        );
    }
    app.disconnect(&info.id).await?;
    Ok(())
}
