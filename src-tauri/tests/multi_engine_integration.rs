//! Disposable Compose fixtures only. Run with --include-ignored --test-threads=1.
use table_por::{models::*, service::Workbench};
use uuid::Uuid;
fn request(engine: Engine) -> ConnectRequest {
    ConnectRequest {
        engine,
        name: "Fixture".into(),
        host: "127.0.0.1".into(),
        port: if engine == Engine::Mysql {
            13306
        } else {
            16379
        },
        database: if engine == Engine::Mysql {
            "tablepor_test"
        } else {
            "0"
        }
        .into(),
        username: "tablepor_reader".into(),
        password: if engine == Engine::Mysql {
            "local-mysql-reader"
        } else {
            "local-redis-reader"
        }
        .into(),
        schema: "".into(),
        tls: TlsMode::LocalOnly,
        root_ca_pem: String::new(),
        manual_edits: false,
    }
}
fn query(id: &str, sql: &str) -> QueryRequest {
    QueryRequest {
        schema: None,
        connection_id: id.into(),
        request_id: Uuid::new_v4().to_string(),
        sql: sql.into(),
        explain: false,
    }
}
fn browse(id: &str) -> BrowseRequest {
    BrowseRequest {
        schema: None,
        connection_id: id.into(),
        request_id: Uuid::new_v4().to_string(),
        table: "items".into(),
        filters: vec![],
        filter_match: FilterMatch::All,
        sort_column: Some("id".into()),
        descending: true,
        page: 0,
        page_size: 50,
    }
}
fn redis_request(id: &str, args: &[&str]) -> RedisRequest {
    RedisRequest {
        connection_id: id.into(),
        request_id: Uuid::new_v4().to_string(),
        args: args.iter().map(|v| (*v).into()).collect(),
    }
}

#[tokio::test]
#[ignore = "requires local Compose fixtures"]
async fn mysql_reads_metadata_filters_sort_and_exact_decimals() {
    let app = Workbench::default();
    let info = app.connect(request(Engine::Mysql)).await.unwrap();
    assert_eq!(info.engine, Engine::Mysql);
    assert!(!info.manual_edits);
    assert_eq!(info.tables.len(), 1);
    assert_eq!(info.tables[0].name, "items");
    assert!(info.tables[0].columns.iter().all(|c| !c.editable));
    let result = app.browse(browse(&info.id)).await.unwrap();
    assert_eq!(result.rows.len(), 2);
    assert_eq!(app.count(browse(&info.id)).await.unwrap().rows, "2");
    assert_eq!(result.rows[0][0].as_deref(), Some("2"));
    assert_eq!(
        result.rows[1][2].as_deref(),
        Some("12345678901234567890.1234567890")
    );
    let mut req = browse(&info.id);
    req.filters = vec![
        Filter {
            column: "id".into(),
            operator: FilterOperator::GreaterThan,
            value: "0".into(),
        },
        Filter {
            column: "note".into(),
            operator: FilterOperator::IsNull,
            value: "".into(),
        },
    ];
    assert_eq!(app.browse(req).await.unwrap().rows.len(), 1);
    let mut counted = browse(&info.id);
    counted.filters = vec![Filter {
        column: "note".into(),
        operator: FilterOperator::IsNull,
        value: String::new(),
    }];
    assert_eq!(app.count(counted).await.unwrap().rows, "1");
    let mut req = browse(&info.id);
    req.filters = vec![Filter {
        column: "label".into(),
        operator: FilterOperator::Contains,
        value: "' OR 1=1 --".into(),
    }];
    assert!(app.browse(req).await.unwrap().rows.is_empty());
    let data = app
        .query(query(
            &info.id,
            "SELECT id, amount FROM `tablepor_test`.`items` ORDER BY id LIMIT 2",
        ))
        .await
        .unwrap();
    assert_eq!(
        data.rows[0][1].as_deref(),
        Some("12345678901234567890.1234567890")
    );
    let empty = app
        .query(query(
            &info.id,
            "SELECT * FROM `tablepor_test`.`items` WHERE id = 999",
        ))
        .await
        .unwrap();
    assert!(empty.rows.is_empty());
    assert_eq!(empty.columns.len(), 5);
    assert_eq!(app.indexes(&info.id, "items").await.unwrap().len(), 2);
    assert!(
        app.overview(&info.id)
            .await
            .unwrap()
            .server_version
            .starts_with("8.")
    );
    let mut explain = query(&info.id, "SELECT * FROM `tablepor_test`.`items` LIMIT 1");
    explain.explain = true;
    assert!(!app.query(explain).await.unwrap().rows.is_empty());
    app.disconnect(&info.id).await.unwrap();
    assert_eq!(
        app.browse(browse(&info.id)).await.unwrap_err().code,
        "NOT_CONNECTED"
    );
}

#[tokio::test]
#[ignore = "requires local Compose fixtures"]
async fn mysql_connection_failures_identify_the_failing_check_without_secrets() {
    let app = Workbench::default();
    let mut settings = request(Engine::Mysql);
    settings.password = "CANARY_BAD_PASSWORD".into();
    let err = app.connect(settings).await.unwrap_err();
    assert_eq!(err.code, "MYSQL_AUTH");
    assert!(!err.message.contains("CANARY"));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let mut settings = request(Engine::Mysql);
    settings.port = port;
    assert_eq!(
        app.connect(settings).await.unwrap_err().code,
        "MYSQL_CONNECTION"
    );
    let mut settings = request(Engine::Mysql);
    settings.tls = TlsMode::VerifyFull;
    assert_eq!(app.connect(settings).await.unwrap_err().code, "MYSQL_TLS");
}

#[tokio::test]
#[ignore = "requires local Compose fixtures"]
async fn mysql_private_ca_authenticates_and_reads_catalog_over_verified_tls() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};
    // A loopback TLS front end exercises the actual MySQL SSL request, certificate
    // verification and authentication. The disposable backing server stays local.
    let warm = Workbench::default();
    let info = warm.connect(request(Engine::Mysql)).await.unwrap();
    warm.disconnect(&info.id).await.unwrap();
    let key = rcgen::KeyPair::from_pem(include_str!("fixtures/tls/test-only-key.pem")).unwrap();
    let mut params = rcgen::CertificateParams::new(vec!["localhost".into()]).unwrap();
    let now = std::time::SystemTime::now();
    params.not_before = (now - std::time::Duration::from_secs(60)).into();
    params.not_after = (now + std::time::Duration::from_secs(86400)).into();
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
    let cert = params.self_signed(&key).unwrap().pem();
    let identity =
        native_tls::Identity::from_pkcs8(cert.as_bytes(), key.serialize_pem().as_bytes()).unwrap();
    let acceptor =
        tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::new(identity).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    async fn packet<S: tokio::io::AsyncRead + Unpin>(socket: &mut S) -> Vec<u8> {
        let mut head = [0; 4];
        socket.read_exact(&mut head).await.unwrap();
        let size = usize::from(head[0]) | usize::from(head[1]) << 8 | usize::from(head[2]) << 16;
        assert!(size < 4 * 1024 * 1024);
        let mut data = vec![0; size + 4];
        data[..4].copy_from_slice(&head);
        socket.read_exact(&mut data[4..]).await.unwrap();
        data
    }
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut backing = TcpStream::connect("127.0.0.1:13306").await.unwrap();
        socket.write_all(&packet(&mut backing).await).await.unwrap();
        let ssl = packet(&mut socket).await;
        assert_eq!(ssl.len(), 36);
        assert_ne!(ssl[5] & 0x08, 0);
        let mut tls = acceptor.accept(socket).await.unwrap();
        let mut login = packet(&mut tls).await;
        login[5] &= !0x08; // Backing socket is loopback plaintext; front end is TLS.
        login[3] -= 1;
        backing.write_all(&login).await.unwrap();
        loop {
            let mut reply = packet(&mut backing).await;
            reply[3] += 1;
            tls.write_all(&reply).await.unwrap();
            if reply[4] == 0 {
                break;
            }
            // Warmed caching_sha2_password must use fast authentication here.
            assert_eq!(&reply[4..], &[1, 3]);
        }
        let _ = tokio::io::copy_bidirectional(&mut tls, &mut backing).await;
    });
    let app = Workbench::default();
    let mut settings = request(Engine::Mysql);
    settings.host = "localhost".into();
    settings.port = port;
    settings.tls = TlsMode::VerifyFull;
    settings.root_ca_pem = cert;
    let info = app.connect(settings).await.unwrap();
    assert_eq!(info.tables[0].name, "items");
    app.disconnect(&info.id).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
#[ignore = "requires local Compose fixtures"]
async fn mysql_rejects_mutation_indirection_and_write_accounts() {
    let app = Workbench::default();
    let info = app.connect(request(Engine::Mysql)).await.unwrap();
    for sql in [
        "DELETE FROM tablepor_test.items",
        "TRUNCATE tablepor_test.items",
        "SELECT SLEEP(1)",
        "SELECT LOAD_FILE('/etc/passwd')",
        "SELECT 1 INTO OUTFILE '/tmp/out'",
        "SELECT @x:=1",
        "SELECT * FROM tablepor_test.items FOR UPDATE",
        "SELECT * FROM tablepor_test.items_view",
        "SELECT 1; DELETE FROM tablepor_test.items",
        "SELECT /*!50000 SLEEP(1) */ 1",
        "CALL dangerous()",
        "SELECT * FROM mysql.user",
        "SELECT 'a\\'; DELETE FROM tablepor_test.items --'",
    ] {
        assert_eq!(
            app.query(query(&info.id, sql)).await.unwrap_err().code,
            "POLICY_DENIED",
            "{sql}"
        );
    }
    let mut r = request(Engine::Mysql);
    r.username = "tablepor_writer".into();
    r.password = "local-mysql-writer".into();
    assert_eq!(app.connect(r).await.unwrap_err().code, "POLICY_DENIED");
    assert_eq!(
        app.save(SaveRequest {
            schema: None,
            connection_id: info.id.clone(),
            request_id: Uuid::new_v4().to_string(),
            table: "items".into(),
            rows: vec![]
        })
        .await
        .unwrap_err()
        .code,
        "POLICY_DENIED"
    );
    let mut r = browse(&info.id);
    r.sort_column = Some("id; DELETE FROM items".into());
    assert_eq!(app.browse(r).await.unwrap_err().code, "POLICY_DENIED");
    assert_eq!(app.browse(browse(&info.id)).await.unwrap().rows.len(), 2);
}

async fn seed_redis() {
    let client =
        redis::Client::open("redis://fixture_admin:local-redis-admin@127.0.0.1:16379/0").unwrap();
    let mut c = client.get_multiplexed_async_connection().await.unwrap();
    for (key, value) in [
        ("fixture:hello", "hello redis"),
        ("fixture:literal", "<script>alert(1)</script>"),
    ] {
        let _: () = redis::cmd("SET")
            .arg(key)
            .arg(value)
            .query_async(&mut c)
            .await
            .unwrap();
    }
    let _: () = redis::cmd("SET")
        .arg("fixture:large")
        .arg("x".repeat(100_000))
        .query_async(&mut c)
        .await
        .unwrap();
    let _: () = redis::cmd("SET")
        .arg("fixture:binary")
        .arg([0xff_u8, 0xfe].as_slice())
        .query_async(&mut c)
        .await
        .unwrap();
    let _: usize = redis::cmd("HSET")
        .arg("fixture:hash")
        .arg("name")
        .arg("Test")
        .query_async(&mut c)
        .await
        .unwrap();
}
#[tokio::test]
#[ignore = "requires local Compose fixtures"]
async fn redis_scans_inspects_previews_and_preserves_binary() {
    seed_redis().await;
    let app = Workbench::default();
    let info = app.connect(request(Engine::Redis)).await.unwrap();
    assert_eq!(info.engine, Engine::Redis);
    assert!(info.tables.is_empty());
    let scan = app
        .redis_command(redis_request(
            &info.id,
            &["SCAN", "0", "MATCH", "fixture:*"],
        ))
        .await
        .unwrap();
    assert_eq!(scan.value[0], "0");
    assert!(scan.value[1].as_array().unwrap().len() >= 5);
    let data = app
        .redis_command(redis_request(&info.id, &["GET", "fixture:hello"]))
        .await
        .unwrap();
    assert_eq!(data.value["preview"], "hello redis");
    let data = app
        .redis_command(redis_request(&info.id, &["GET", "fixture:large"]))
        .await
        .unwrap();
    assert_eq!(data.value["preview"].as_str().unwrap().len(), 65536);
    let data = app
        .redis_command(redis_request(&info.id, &["GET", "fixture:binary"]))
        .await
        .unwrap();
    assert_eq!(data.value["preview"]["data"], "fffe");
    let ttl = app
        .redis_command(redis_request(&info.id, &["TTL", "fixture:hello"]))
        .await
        .unwrap();
    assert_eq!(ttl.value, "-1");
    assert!(
        app.redis_command(redis_request(&info.id, &["HSCAN", "fixture:hash", "0"]))
            .await
            .unwrap()
            .value[1]
            .is_array()
    );
    assert!(
        app.redis_command(redis_request(&info.id, &["GET", "fixture:missing"]))
            .await
            .unwrap()
            .value
            .is_null()
    );
    app.disconnect(&info.id).await.unwrap();
    assert_eq!(
        app.redis_command(redis_request(&info.id, &["PING"]))
            .await
            .unwrap_err()
            .code,
        "NOT_CONNECTED"
    );
}
#[tokio::test]
#[ignore = "requires local Compose fixtures"]
async fn redis_blocks_writes_even_with_admin_credentials_and_redacts_errors() {
    seed_redis().await;
    let app = Workbench::default();
    let mut r = request(Engine::Redis);
    r.username = "fixture_admin".into();
    r.password = "local-redis-admin".into();
    let info = app.connect(r).await.unwrap();
    for args in [
        vec!["FLUSHALL"],
        vec!["DEL", "fixture:hello"],
        vec!["SET", "fixture:hello", "changed"],
        vec!["EVAL", "return redis.call('FLUSHALL')", "0"],
        vec!["ACL", "LIST"],
        vec!["CONFIG", "GET", "*"],
        vec!["GETDEL", "fixture:hello"],
        vec!["MULTI"],
        vec!["RESTORE", "x", "0", "x"],
        vec!["MODULE", "LIST"],
    ] {
        assert_eq!(
            app.redis_command(redis_request(&info.id, &args))
                .await
                .unwrap_err()
                .code,
            "POLICY_DENIED"
        );
    }
    assert_eq!(
        app.redis_command(redis_request(&info.id, &["GET", "fixture:hello"]))
            .await
            .unwrap()
            .value["preview"],
        "hello redis"
    );
    let error = app
        .redis_command(redis_request(
            &info.id,
            &["LRANGE", "fixture:hello", "0", "2"],
        ))
        .await
        .unwrap_err();
    assert_eq!(error.code, "REDIS_ERROR");
    assert!(!error.message.contains("fixture:hello"));
    assert_eq!(
        app.query(query(&info.id, "SELECT 1"))
            .await
            .unwrap_err()
            .code,
        "POLICY_DENIED"
    );
}
#[tokio::test]
async fn adapters_reject_unsafe_transport_and_manual_write_flags_before_connecting() {
    let app = Workbench::default();
    for engine in [Engine::Mysql, Engine::Redis] {
        let mut r = request(engine);
        r.tls = TlsMode::Require;
        assert_eq!(app.connect(r).await.unwrap_err().code, "INVALID_CONNECTION");
        let mut r = request(engine);
        r.tls = TlsMode::VerifyFull;
        r.root_ca_pem = "unsupported CA".into();
        assert_eq!(
            app.connect(r).await.unwrap_err().code,
            if engine == Engine::Mysql {
                "INVALID_CA"
            } else {
                "INVALID_CONNECTION"
            }
        );
        let mut r = request(engine);
        r.host = "example.com".into();
        assert_eq!(app.connect(r).await.unwrap_err().code, "TLS_REQUIRED");
        let mut r = request(engine);
        r.manual_edits = true;
        assert_eq!(app.connect(r).await.unwrap_err().code, "POLICY_DENIED");
    }
    let mut r = request(Engine::Redis);
    r.database = "0?password=oops".into();
    assert_eq!(app.connect(r).await.unwrap_err().code, "INVALID_CONNECTION");
}
#[tokio::test]
#[ignore = "requires local Compose fixtures"]
async fn verified_tls_does_not_fall_back_to_plaintext() {
    let app = Workbench::default();
    for engine in [Engine::Mysql, Engine::Redis] {
        let mut r = request(engine);
        r.tls = TlsMode::VerifyFull;
        assert!(app.connect(r).await.is_err());
    }
}

#[tokio::test]
#[ignore = "requires local Compose fixtures"]
async fn redis_rejects_oversized_collection_before_full_response_is_buffered() {
    let client =
        redis::Client::open("redis://fixture_admin:local-redis-admin@127.0.0.1:16379/0").unwrap();
    let mut admin = client.get_multiplexed_async_connection().await.unwrap();
    let _: usize = redis::cmd("HSET")
        .arg("fixture:oversized")
        .arg("field")
        .arg("x".repeat(9 * 1024 * 1024))
        .query_async(&mut admin)
        .await
        .unwrap();
    let app = Workbench::default();
    let info = app.connect(request(Engine::Redis)).await.unwrap();
    let result = app
        .redis_command(redis_request(
            &info.id,
            &["HSCAN", "fixture:oversized", "0"],
        ))
        .await;
    assert!(result.is_err());
    assert_eq!(
        app.redis_command(redis_request(&info.id, &["PING"]))
            .await
            .unwrap()
            .value,
        "PONG"
    );
}

#[tokio::test]
#[ignore = "requires local Compose fixtures"]
async fn mysql_server_denies_writes_and_app_detects_privilege_drift() {
    use mysql_async::{Conn, OptsBuilder, prelude::Queryable};
    let opts = OptsBuilder::default()
        .ip_or_hostname("127.0.0.1")
        .tcp_port(13306)
        .prefer_socket(false)
        .user(Some("tablepor_reader"))
        .pass(Some("local-mysql-reader"));
    let mut reader = Conn::new(opts).await.unwrap();
    assert!(
        reader
            .query_drop("UPDATE tablepor_test.items SET label='forbidden' WHERE id=1")
            .await
            .is_err()
    );
    let app = Workbench::default();
    let info = app.connect(request(Engine::Mysql)).await.unwrap();
    let opts = OptsBuilder::default()
        .ip_or_hostname("127.0.0.1")
        .tcp_port(13306)
        .prefer_socket(false)
        .user(Some("root"))
        .pass(Some("local-mysql-admin"));
    let mut admin = Conn::new(opts).await.unwrap();
    admin
        .query_drop("GRANT UPDATE ON tablepor_test.items TO 'tablepor_reader'@'%'")
        .await
        .unwrap();
    let result = app.browse(browse(&info.id)).await;
    // Restore the synthetic account before asserting, even if the policy fails.
    admin
        .query_drop("REVOKE UPDATE ON tablepor_test.items FROM 'tablepor_reader'@'%'")
        .await
        .unwrap();
    assert_eq!(result.unwrap_err().code, "POLICY_DENIED");
    assert_eq!(
        app.browse(browse(&info.id)).await.unwrap().rows[1][1].as_deref(),
        Some("First")
    );
}
