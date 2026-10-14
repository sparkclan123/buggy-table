//! Explicit read-command allowlist. Never pass a raw command line to Redis.
use crate::{models::*, policy::MAX_RESULT_BYTES, remote_profile::RemoteProfile};
use redis::{AsyncConnectionConfig, RedisConnectionInfo, Value, aio::MultiplexedConnection};
use std::time::{Duration, Instant};

fn failure(_: redis::RedisError) -> AppError {
    AppError::new(
        "REDIS_ERROR",
        "Redis rejected the operation. Check the endpoint, verified TLS, authentication, ACL permissions and key type. Raw server details are withheld.",
    )
}

struct Database {
    client: MultiplexedConnection,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Database {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn open(p: &RemoteProfile) -> AppResult<Database> {
    let mut settings = RedisConnectionInfo::default()
        .set_db(
            p.database
                .parse()
                .map_err(|_| AppError::policy("Invalid Redis database."))?,
        )
        .set_skip_set_lib_name();
    if !p.username.is_empty() {
        settings = settings.set_username(&p.username);
    }
    if !p.password.is_empty() {
        settings = settings.set_password(p.password.as_str());
    }
    let connect = async {
        let tcp = tokio::net::TcpStream::connect((p.host.as_str(), p.port))
            .await
            .map_err(|_| AppError::new("REDIS_CONNECTION", "Cannot reach the Redis endpoint."))?;
        let stream: Box<dyn crate::bounded_stream::Transport> = match p.tls {
            TlsMode::Require => return Err(AppError::policy("Redis requires verified TLS.")),
            TlsMode::LocalOnly => Box::new(tcp),
            TlsMode::VerifyFull => {
                let verifier = native_tls::TlsConnector::builder()
                    .min_protocol_version(Some(native_tls::Protocol::Tlsv12))
                    .build()
                    .map_err(|_| AppError::new("TLS_ERROR", "Cannot initialize verified TLS."))?;
                let tls = tokio_native_tls::TlsConnector::from(verifier).connect(&p.host, tcp).await
                    .map_err(|_| AppError::new("TLS_ERROR", "Redis TLS certificate or hostname verification failed; plaintext fallback is disabled."))?;
                Box::new(tls)
            }
        };
        // SCAN COUNT is only a hint. Bound bytes before the driver buffers the response.
        let stream = crate::bounded_stream::BoundedStream::new(stream, 8 * 1024 * 1024);
        let (client, driver) = MultiplexedConnection::new_with_config(
            &settings,
            stream,
            AsyncConnectionConfig::new().set_response_timeout(Some(Duration::from_secs(10))),
        )
        .await
        .map_err(failure)?;
        Ok(Database {
            client,
            task: tokio::spawn(driver),
        })
    };
    tokio::time::timeout(Duration::from_secs(6), connect)
        .await
        .map_err(|_| AppError::new("TIMEOUT", "Redis connection or authentication timed out."))?
}

pub async fn connect(p: &RemoteProfile, id: String) -> AppResult<ConnectionInfo> {
    let mut conn = open(p).await?;
    let _: String = redis::cmd("PING")
        .query_async(&mut conn.client)
        .await
        .map_err(failure)?;
    // INFO may be denied by a least-privilege ACL. Connecting does not require it.
    Ok(p.info(
        id,
        "Redis · version available through INFO server".into(),
        vec![],
    ))
}

fn number(s: &str, max: u64) -> bool {
    s.parse::<u64>().is_ok_and(|n| n <= max)
}

pub fn validate(args: &[String]) -> AppResult<Vec<String>> {
    if args.is_empty() || args.len() > 7 || args.iter().any(|v| v.len() > 4096 || v.contains('\0'))
    {
        return Err(AppError::policy(
            "Use one supported command with at most 7 arguments, each at most 4096 bytes.",
        ));
    }
    let mut a = args.to_vec();
    a[0] = a[0].to_ascii_uppercase();
    let n = a.len();
    let valid = match a[0].as_str() {
        "PING" | "DBSIZE" => n == 1,
        "INFO" => {
            n == 2
                && matches!(
                    a[1].as_str(),
                    "server" | "memory" | "stats" | "keyspace" | "clients"
                )
        }
        "TYPE" | "TTL" | "PTTL" | "STRLEN" | "HLEN" | "LLEN" | "SCARD" | "ZCARD" | "GET" => n == 2,
        "EXISTS" => n == 2,
        "GETRANGE" | "LRANGE" | "ZRANGE" => {
            n == 4
                && number(&a[2], 100_000)
                && number(&a[3], 100_200)
                && a[2]
                    .parse::<u64>()
                    .ok()
                    .zip(a[3].parse::<u64>().ok())
                    .is_some_and(|(start, end)| {
                        end >= start && end - start < if a[0] == "GETRANGE" { 65536 } else { 200 }
                    })
        }
        "SCAN" => {
            (n == 2 || n == 4 && a[2].eq_ignore_ascii_case("MATCH")) && number(&a[1], u64::MAX)
        }
        "HSCAN" | "SSCAN" | "ZSCAN" => n == 3 && number(&a[2], u64::MAX),
        _ => false,
    };
    if !valid {
        return Err(AppError::policy(
            "Only bounded reads are allowed: PING, DBSIZE, INFO section, SCAN cursor [MATCH pattern], GET, TYPE, TTL/PTTL, EXISTS, lengths, bounded ranges and collection scans. Writes, scripts, modules, administration and transactions are blocked.",
        ));
    }
    if matches!(a[0].as_str(), "SCAN" | "HSCAN" | "SSCAN" | "ZSCAN") {
        a.extend(["COUNT".into(), "100".into()]);
    }
    Ok(a)
}

pub async fn command(p: &RemoteProfile, args: &[String]) -> AppResult<RedisResult> {
    let args = validate(args)?;
    let started = Instant::now();
    let mut conn = open(p).await?;
    let value = if args[0] == "GET" {
        // Redis GET can return 512 MiB; preview only the first 64 KiB.
        let exists: bool = redis::cmd("EXISTS")
            .arg(&args[1])
            .query_async(&mut conn.client)
            .await
            .map_err(failure)?;
        if !exists {
            serde_json::Value::Null
        } else {
            let bytes: Vec<u8> = redis::cmd("GETRANGE")
                .arg(&args[1])
                .arg(0)
                .arg(65535)
                .query_async(&mut conn.client)
                .await
                .map_err(failure)?;
            serde_json::json!({"preview": bytes_json(bytes), "limitBytes": 65536, "note": "First 64 KiB only; use STRLEN for full length. Reads are not a snapshot."})
        }
    } else {
        let mut cmd = redis::cmd(&args[0]);
        cmd.arg(&args[1..]);
        let value: Value = cmd.query_async(&mut conn.client).await.map_err(failure)?;
        json_value(value, &mut 0)?
    };
    Ok(RedisResult {
        value,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

fn bytes_json(bytes: Vec<u8>) -> serde_json::Value {
    match String::from_utf8(bytes) {
        Ok(text) => serde_json::Value::String(text),
        Err(error) => {
            serde_json::json!({"encoding": "hex", "data": error.as_bytes().iter().map(|b| format!("{b:02x}")).collect::<String>()})
        }
    }
}
fn json_value(value: Value, budget: &mut usize) -> AppResult<serde_json::Value> {
    *budget += 8;
    let result = match value {
        Value::Nil => serde_json::Value::Null,
        Value::Int(n) => serde_json::Value::String(n.to_string()),
        Value::BulkString(v) => {
            *budget += v.len().saturating_mul(2);
            bytes_json(v)
        }
        Value::SimpleString(v) => {
            *budget += v.len();
            serde_json::Value::String(v)
        }
        Value::Okay => serde_json::Value::String("OK".into()),
        Value::Array(values) => {
            if values.len() > 10_000 {
                return Err(AppError::policy(
                    "Redis returned too many elements for a preview.",
                ));
            }
            serde_json::Value::Array(
                values
                    .into_iter()
                    .map(|v| json_value(v, budget))
                    .collect::<AppResult<_>>()?,
            )
        }
        _ => return Err(AppError::policy("Unsupported Redis response type.")),
    };
    if *budget > MAX_RESULT_BYTES {
        return Err(AppError::policy(
            "Redis response exceeds the 4 MiB display budget. Narrow the scan or range.",
        ));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|v| (*v).into()).collect()
    }
    #[test]
    fn allows_only_bounded_reads() {
        for a in [
            vec!["GET", "key"],
            vec!["SCAN", "0", "MATCH", "session:*"],
            vec!["LRANGE", "list", "0", "199"],
            vec!["INFO", "memory"],
        ] {
            assert!(validate(&args(&a)).is_ok());
        }
        for a in [
            vec!["FLUSHALL"],
            vec!["DEL", "x"],
            vec!["GETDEL", "x"],
            vec!["SET", "x", "y"],
            vec!["EVAL", "return 1", "0"],
            vec!["FCALL_RO", "x", "0"],
            vec!["CONFIG", "GET", "*"],
            vec!["KEYS", "*"],
            vec!["LRANGE", "x", "0", "-1"],
            vec!["SCAN", "0", "COUNT", "999999"],
            vec!["AUTH", "x"],
            vec!["SELECT", "2"],
            vec!["GET", "x", "DEL", "x"],
            vec!["MONITOR"],
        ] {
            assert!(validate(&args(&a)).is_err(), "{a:?}");
        }
    }
}
