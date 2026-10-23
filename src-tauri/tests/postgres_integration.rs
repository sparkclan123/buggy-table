//! Run only against `docker compose up -d --wait` using --ignored --test-threads=1.
use std::{sync::Arc, time::Duration};
use table_por::{models::*, service::Workbench};
use tokio_postgres::NoTls;
use uuid::Uuid;

fn fixture_port() -> u16 {
    std::env::var("BUGGY_TEST_POSTGRES_PORT")
        .unwrap_or_else(|_| "15432".into())
        .parse()
        .expect("valid fixture port")
}

fn request() -> ConnectRequest {
    ConnectRequest {
        engine: Engine::Postgres,
        name: "Test reader".into(),
        host: "127.0.0.1".into(),
        port: fixture_port(),
        database: "tablepor_test".into(),
        username: "tablepor_reader".into(),
        password: "local-reader-only".into(),
        schema: "public".into(),
        tls: TlsMode::LocalOnly,
        root_ca_pem: String::new(),
        manual_edits: false,
    }
}

fn query(connection_id: &str, sql: &str) -> QueryRequest {
    QueryRequest {
        database: None,
        schema: None,
        connection_id: connection_id.into(),
        request_id: Uuid::new_v4().to_string(),
        sql: sql.into(),
        explain: false,
    }
}

fn browse(connection_id: &str) -> BrowseRequest {
    BrowseRequest {
        database: None,
        schema: None,
        connection_id: connection_id.into(),
        request_id: Uuid::new_v4().to_string(),
        table: "orders".into(),
        filters: vec![],
        filter_match: FilterMatch::All,
        sort_column: None,
        descending: false,
        page: 0,
        page_size: 50,
    }
}

async fn admin() -> (tokio_postgres::Client, tokio::task::JoinHandle<()>) {
    let (client, connection) = tokio_postgres::connect(&format!("host=127.0.0.1 port={} dbname=tablepor_test user=fixture_admin password=local-fixture-admin", fixture_port()), NoTls).await.unwrap();
    (
        client,
        tokio::spawn(async move {
            let _ = connection.await;
        }),
    )
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn reader_can_browse_query_inspect_and_observe() {
    let app = Workbench::default();
    let info = app.connect(request()).await.unwrap();
    assert!(info.tables.iter().any(|t| t.name == "orders"));
    if let Ok(expected) = std::env::var("BUGGY_TEST_POSTGRES_VERSION") {
        assert_eq!(
            info.server_version.parse::<u32>().unwrap() / 10_000,
            expected.parse::<u32>().unwrap(),
            "matrix must use the requested server major"
        );
    }
    for name in ["orders_view", "rls_table"] {
        assert!(
            info.tables
                .iter()
                .find(|t| t.name == name)
                .unwrap()
                .read_blocked_reason
                .is_some()
        );
    }
    assert!(
        info.tables
            .iter()
            .find(|t| t.name == "custom_payload")
            .unwrap()
            .read_blocked_reason
            .is_none()
    );
    let page = app.browse(browse(&info.id)).await.unwrap();
    assert_total_timings(&page);
    assert_eq!(page.rows.len(), 50);
    assert!(page.truncated);
    assert_eq!(page.rows[0][0].as_deref(), Some("1"));
    assert_eq!(page.rows[1][0].as_deref(), Some("2")); // Numeric, not text ordering.
    let mut next = browse(&info.id);
    next.page = 2;
    assert_eq!(app.browse(next).await.unwrap().rows.len(), 50);
    let data = app
        .query(query(
            &info.id,
            "SELECT id, total FROM public.orders WHERE total > 100 ORDER BY id LIMIT 5",
        ))
        .await
        .unwrap();
    assert_eq!(data.rows.len(), 5);
    assert_total_timings(&data);
    let mut explain = query(&info.id, "SELECT * FROM public.orders WHERE id = 1");
    explain.explain = true;
    let plan = app.query(explain).await.unwrap();
    assert!(plan.rows[0][0].as_ref().unwrap().contains("Plan"));
    assert!(
        app.indexes(&info.id, "orders")
            .await
            .unwrap()
            .iter()
            .any(|i| i.primary)
    );
    assert_eq!(
        app.overview(&info.id).await.unwrap().database,
        "tablepor_test"
    );
    app.disconnect(&info.id).await.unwrap();
    assert_eq!(
        app.query(query(&info.id, "SELECT 1"))
            .await
            .unwrap_err()
            .code,
        "NOT_CONNECTED"
    );
}

fn assert_total_timings(result: &QueryResult) {
    let timing = result.timings.as_ref().expect("PostgreSQL stage timings");
    assert!(
        result.elapsed_ms
            >= timing.connection_ms
                + timing.validation_ms
                + timing.execution_ms
                + timing.cleanup_ms
    );
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn warm_reads_reuse_one_idle_backend_and_recover_from_a_closed_socket() {
    let app = Workbench::default();
    let info = app.connect(request()).await.unwrap();
    let (admin, task) = admin().await;
    let backend = admin.query_one("SELECT pid, state, xact_start IS NULL FROM pg_stat_activity WHERE application_name = 'table-por' AND usename = 'tablepor_reader'", &[]).await.unwrap();
    let pid: i32 = backend.get(0);
    assert_eq!(backend.get::<_, String>(1), "idle");
    assert!(
        backend.get::<_, bool>(2),
        "idle connections must not retain a transaction"
    );
    for _ in 0..3 {
        let result = app.browse(browse(&info.id)).await.unwrap();
        assert!(result.timings.unwrap().connection_reused);
    }
    app.count(browse(&info.id)).await.unwrap();
    app.schemas(&info.id).await.unwrap();
    let backend = admin
        .query_one(
            "SELECT state, xact_start IS NULL FROM pg_stat_activity WHERE pid = $1",
            &[&pid],
        )
        .await
        .unwrap();
    assert_eq!(backend.get::<_, String>(0), "idle");
    assert!(backend.get::<_, bool>(1));
    assert!(
        admin
            .query_one("SELECT pg_terminate_backend($1)", &[&pid])
            .await
            .unwrap()
            .get::<_, bool>(0)
    );
    let recovered = app.query(query(&info.id, "SELECT 1")).await.unwrap();
    assert_eq!(recovered.rows[0][0].as_deref(), Some("1"));
    assert!(!recovered.timings.unwrap().connection_reused);
    assert!(
        app.query(query(&info.id, "SELECT 1"))
            .await
            .unwrap()
            .timings
            .unwrap()
            .connection_reused
    );
    app.disconnect(&info.id).await.unwrap();
    // Socket shutdown is asynchronous; verify that disconnect releases the idle backend.
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let count: i64 = admin.query_one("SELECT count(*) FROM pg_stat_activity WHERE application_name = 'table-por' AND usename = 'tablepor_reader'", &[]).await.unwrap().get(0);
            if count == 0 { break; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.unwrap();
    task.abort();
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn failed_reads_discard_the_backend_and_never_replay_the_query() {
    let app = Workbench::default();
    let info = app.connect(request()).await.unwrap();
    assert!(app.query(query(&info.id, "SELECT 1 / 0")).await.is_err());
    let next = app.query(query(&info.id, "SELECT 42")).await.unwrap();
    assert_eq!(next.rows[0][0].as_deref(), Some("42"));
    assert!(!next.timings.unwrap().connection_reused);
    assert!(
        app.query(query(&info.id, "SELECT 42"))
            .await
            .unwrap()
            .timings
            .unwrap()
            .connection_reused
    );
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn save_trigger_session_changes_cannot_contaminate_reused_reads() {
    let (admin, task) = admin().await;
    let schema = format!("pool_{}", Uuid::new_v4().simple());
    admin
        .batch_execute(&format!(
            "CREATE SCHEMA {schema};
        GRANT USAGE ON SCHEMA {schema} TO tablepor_editor;
        CREATE TABLE {schema}.items(id integer PRIMARY KEY, label text);
        INSERT INTO {schema}.items VALUES(1, 'before');
        GRANT SELECT, UPDATE ON {schema}.items TO tablepor_editor;
        CREATE FUNCTION {schema}.change_session() RETURNS trigger LANGUAGE plpgsql AS $body$
        BEGIN
            PERFORM pg_catalog.set_config('search_path', 'public', false);
            PERFORM pg_catalog.set_config('default_transaction_read_only', 'off', false);
            PERFORM pg_catalog.set_config('application_name', 'fixture-write-session', false);
            RETURN NEW;
        END $body$;
        CREATE TRIGGER change_session BEFORE UPDATE ON {schema}.items
            FOR EACH ROW EXECUTE FUNCTION {schema}.change_session();"
        ))
        .await
        .unwrap();
    let app = Workbench::default();
    let mut settings = request();
    settings.username = "tablepor_editor".into();
    settings.password = "local-editor-only".into();
    settings.manual_edits = true;
    settings.schema = schema.clone();
    let info = app.connect(settings).await.unwrap();
    let pid: i32 = admin.query_one("SELECT pid FROM pg_stat_activity WHERE application_name = 'table-por' AND usename = 'tablepor_editor'", &[]).await.unwrap().get(0);
    let save = SaveRequest {
        database: None,
        connection_id: info.id.clone(),
        request_id: Uuid::new_v4().to_string(),
        schema: Some(schema.clone()),
        table: "items".into(),
        rows: vec![RowChange {
            insert: false,
            original: [
                ("id".into(), Some("1".into())),
                ("label".into(), Some("before".into())),
            ]
            .into(),
            changes: [("label".into(), Some("after".into()))].into(),
            delete: false,
        }],
    };
    let saved = app.save(save).await;
    let mut read = query(&info.id, &format!("SELECT label FROM {schema}.items"));
    read.schema = Some(schema.clone());
    let read = app.query(read).await;
    let unchanged = admin.query_opt("SELECT state, xact_start IS NULL FROM pg_stat_activity WHERE pid = $1 AND application_name = 'table-por'", &[&pid]).await.unwrap();
    app.disconnect(&info.id).await.unwrap();
    admin
        .batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
    task.abort();
    assert_eq!(saved.unwrap().updated_rows, 1);
    let read = read.unwrap();
    assert_eq!(read.rows[0][0].as_deref(), Some("after"));
    assert!(read.timings.unwrap().connection_reused);
    let backend = unchanged.expect("writes must not replace or modify the reusable read session");
    assert_eq!(backend.get::<_, String>(0), "idle");
    assert!(backend.get::<_, bool>(1));
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn schema_catalog_above_the_old_500_table_limit_is_complete() {
    let (admin, task) = admin().await;
    let schema = format!("large_{}", Uuid::new_v4().simple());
    let mut ddl =
        format!("CREATE SCHEMA {schema}; GRANT USAGE ON SCHEMA {schema} TO tablepor_reader;");
    for index in 0..520 {
        ddl.push_str(&format!(
            "CREATE TABLE {schema}.table_{index:03} (id integer);"
        ));
    }
    ddl.push_str(&format!(
        "GRANT SELECT ON ALL TABLES IN SCHEMA {schema} TO tablepor_reader;"
    ));
    admin.batch_execute(&ddl).await.unwrap();
    let app = Workbench::default();
    let mut settings = request();
    settings.schema = schema.clone();
    let info = app.connect(settings).await.unwrap();
    assert_eq!(info.tables.len(), 520);
    assert_eq!(info.tables.last().unwrap().name, "table_519");
    assert!(
        info.tables
            .iter()
            .all(|t| t.columns.len() == 1 && t.read_blocked_reason.is_none())
    );
    let data = app
        .query(query(
            &info.id,
            &format!("SELECT id FROM {schema}.table_519"),
        ))
        .await
        .unwrap();
    assert!(data.rows.is_empty());
    assert_eq!(data.columns, ["id"]);
    app.disconnect(&info.id).await.unwrap();
    admin
        .batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
    task.abort();
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn json_arrays_and_enums_are_discovered_filtered_and_saved_without_losing_values() {
    let (admin, task) = admin().await;
    let schema = format!("rich_{}", Uuid::new_v4().simple());
    admin
        .batch_execute(&format!(
            r#"
        CREATE SCHEMA {schema};
        GRANT USAGE ON SCHEMA {schema} TO tablepor_reader, tablepor_editor;
        CREATE TYPE {schema}."state kind" AS ENUM ('new', 'done', 'O''Reilly');
        CREATE TABLE {schema}.rich (
          id bigint PRIMARY KEY, doc json, indexed_doc jsonb, tags text[], nums bigint[],
          state {schema}."state kind", states {schema}."state kind"[], docs json[], label text);
        INSERT INTO {schema}.rich VALUES (1, '{{"a":1, "a":2}}', '{{"a":2}}',
          ARRAY['one','two'], ARRAY[9007199254740993], 'new', ARRAY['new']::{schema}."state kind"[],
          ARRAY['{{"b": 1}}']::json[], 'original');
        CREATE VIEW {schema}.rich_view AS SELECT * FROM {schema}.rich;
        CREATE MATERIALIZED VIEW {schema}.rich_snapshot AS SELECT * FROM {schema}.rich;
        CREATE DOMAIN {schema}.custom_domain AS text CHECK (VALUE <> '');
        CREATE TABLE {schema}.domain_table (value {schema}.custom_domain);
        GRANT SELECT ON ALL TABLES IN SCHEMA {schema} TO tablepor_reader, tablepor_editor;
        GRANT UPDATE ON {schema}.rich TO tablepor_editor;
    "#
        ))
        .await
        .unwrap();
    let app = Workbench::default();
    let mut settings = editor_request();
    settings.schema = schema.clone();
    let info = app.connect(settings).await.unwrap();
    assert_eq!(info.tables.len(), 4);
    assert!(
        info.tables
            .iter()
            .find(|t| t.name == "rich")
            .unwrap()
            .read_blocked_reason
            .is_none()
    );
    assert!(
        info.tables
            .iter()
            .find(|t| t.name == "rich_view")
            .unwrap()
            .read_blocked_reason
            .is_some()
    );
    assert!(
        info.tables
            .iter()
            .find(|t| t.name == "domain_table")
            .unwrap()
            .read_blocked_reason
            .is_some()
    );
    let original = editable_rows(&app, &info.id, "rich").await;
    assert_eq!(original.rows[0][1].as_deref(), Some("{\"a\":1, \"a\":2}"));
    assert_eq!(original.rows[0][4].as_deref(), Some("{9007199254740993}"));
    let sql = app
        .query(query(&info.id, &format!("SELECT * FROM {schema}.rich")))
        .await
        .unwrap();
    assert_eq!(sql.rows, original.rows);
    assert_eq!(
        editable_rows(&app, &info.id, "rich_snapshot").await.rows,
        original.rows
    );
    for (column, value) in [
        ("doc", "{\"a\":1, \"a\":2}"),
        ("tags", "{one,two}"),
        ("state", "new"),
        ("states", "{new}"),
        ("indexed_doc", "{\"a\": 2}"),
    ] {
        let mut filtered = browse(&info.id);
        filtered.table = "rich".into();
        filtered.filters = vec![Filter {
            column: column.into(),
            value: value.into(),
            operator: FilterOperator::Equals,
        }];
        assert_eq!(
            app.browse(filtered).await.unwrap().rows.len(),
            1,
            "filter {column}"
        );
    }
    let mut change = edit(&original, 0, "label", Some("saved"));
    change.changes.extend([
        ("doc".into(), Some("{\"changed\": true}".into())),
        ("indexed_doc".into(), Some("{\"changed\": true}".into())),
        ("tags".into(), Some("{three,four}".into())),
        ("state".into(), Some("O'Reilly".into())),
        ("states".into(), Some("{done,new}".into())),
    ]);
    app.save(save(&info.id, "rich", vec![change]))
        .await
        .unwrap();
    let saved = editable_rows(&app, &info.id, "rich").await;
    assert_eq!(saved.rows[0][1].as_deref(), Some("{\"changed\": true}"));
    assert_eq!(saved.rows[0][5].as_deref(), Some("O'Reilly"));
    assert_eq!(saved.rows[0][6].as_deref(), Some("{done,new}"));
    // JSON conflict detection must notice textual changes as well as values.
    admin
        .batch_execute(&format!(
            "UPDATE {schema}.rich SET doc = '{{\"changed\":true}}'"
        ))
        .await
        .unwrap();
    assert_eq!(
        app.save(save(
            &info.id,
            "rich",
            vec![edit(&saved, 0, "label", Some("stale"))]
        ))
        .await
        .unwrap_err()
        .code,
        "EDIT_CONFLICT"
    );
    let fresh = editable_rows(&app, &info.id, "rich").await;
    assert!(
        app.save(save(
            &info.id,
            "rich",
            vec![edit(&fresh, 0, "doc", Some("invalid JSON"))]
        ))
        .await
        .is_err()
    );
    assert_eq!(editable_rows(&app, &info.id, "rich").await.rows, fresh.rows);
    app.disconnect(&info.id).await.unwrap();
    // A normal strict reader can read the same newly supported types.
    let mut settings = request();
    settings.schema = schema.clone();
    let reader = app.connect(settings).await.unwrap();
    assert_eq!(
        editable_rows(&app, &reader.id, "rich").await.rows,
        fresh.rows
    );
    app.disconnect(&reader.id).await.unwrap();
    admin
        .batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
    task.abort();
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn result_limits_keep_payload_bounded_and_allow_next_operation() {
    let (admin, task) = admin().await;
    let schema = format!("limits_{}", Uuid::new_v4().simple());
    admin
        .batch_execute(&format!(
            "CREATE SCHEMA {schema}; GRANT USAGE ON SCHEMA {schema} TO tablepor_reader;
         CREATE TABLE {schema}.wide_rows (id integer PRIMARY KEY, payload text);
         INSERT INTO {schema}.wide_rows SELECT i, repeat('x', 2097152) FROM generate_series(1,3) i;
         CREATE TABLE {schema}.many_rows (id integer PRIMARY KEY);
         INSERT INTO {schema}.many_rows SELECT i FROM generate_series(1,1200) i;
         GRANT SELECT ON ALL TABLES IN SCHEMA {schema} TO tablepor_reader;"
        ))
        .await
        .unwrap();
    let app = Workbench::default();
    let mut settings = request();
    settings.schema = schema.clone();
    let info = app.connect(settings).await.unwrap();
    let mut page_request = browse(&info.id);
    page_request.table = "wide_rows".into();
    let page = app.browse(page_request).await.unwrap();
    assert!(page.truncated);
    assert_eq!(page.rows.len(), 1);
    let data = app
        .query(query(
            &info.id,
            &format!("SELECT * FROM {schema}.wide_rows ORDER BY id"),
        ))
        .await
        .unwrap();
    assert!(data.truncated);
    assert_eq!(data.rows.len(), 1);
    let data = app
        .query(query(
            &info.id,
            &format!("SELECT id FROM {schema}.many_rows ORDER BY id"),
        ))
        .await
        .unwrap();
    assert!(data.truncated);
    assert_eq!(data.rows.len(), 1000);
    let again = app.query(query(&info.id, "SELECT 1")).await.unwrap();
    assert_eq!(again.rows[0][0].as_deref(), Some("1"));
    assert!(
        !again.timings.unwrap().connection_reused,
        "cancelled transports must not return to the idle slot"
    );
    app.disconnect(&info.id).await.unwrap();
    admin
        .batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
    task.abort();
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn batched_catalog_keeps_table_columns_permissions_and_scope_separate() {
    let (admin, task) = admin().await;
    let schema = format!("catalog_{}", Uuid::new_v4().simple());
    admin.batch_execute(&format!(
        "CREATE SCHEMA {schema};
         GRANT USAGE ON SCHEMA {schema} TO tablepor_editor;
         CREATE TABLE {schema}.zeta (tenant integer, id bigint, dropped text, amount numeric(30,10), PRIMARY KEY(tenant,id));
         ALTER TABLE {schema}.zeta DROP COLUMN dropped;
         CREATE TABLE {schema}.alpha (id bigint PRIMARY KEY, label text NOT NULL, copied text GENERATED ALWAYS AS (label) STORED);
         CREATE TABLE {schema}.empty_table ();
         CREATE TYPE {schema}.custom_record AS (value text);
         CREATE TABLE {schema}.blocked (id bigint, payload {schema}.custom_record);
         GRANT SELECT ON ALL TABLES IN SCHEMA {schema} TO tablepor_editor;
         GRANT UPDATE(label) ON {schema}.alpha TO tablepor_editor;
         GRANT UPDATE(amount) ON {schema}.zeta TO tablepor_editor;"
    )).await.unwrap();
    let app = Workbench::default();
    let mut settings = request();
    settings.schema = schema.clone();
    settings.username = "tablepor_editor".into();
    settings.password = "local-editor-only".into();
    settings.manual_edits = true;
    let info = app.connect(settings).await.unwrap();
    assert_eq!(
        info.tables
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        ["alpha", "blocked", "empty_table", "zeta"]
    );
    let alpha = &info.tables[0];
    assert_eq!(
        alpha
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["id", "label", "copied"]
    );
    assert!(alpha.columns[0].primary_key && !alpha.columns[0].editable);
    assert!(alpha.columns[1].editable && !alpha.columns[1].nullable);
    assert!(!alpha.columns[2].editable);
    assert!(info.tables[1].read_blocked_reason.is_some());
    assert!(info.tables[2].columns.is_empty());
    let zeta = &info.tables[3];
    assert_eq!(
        zeta.columns
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        ["tenant", "id", "amount"]
    );
    assert!(zeta.columns[0].primary_key && zeta.columns[1].primary_key);
    assert!(zeta.columns[2].editable && zeta.columns[2].data_type == "numeric");

    // Fresh catalog data must observe grant and DDL changes without reconnecting.
    admin
        .batch_execute(&format!(
            "REVOKE UPDATE(label) ON {schema}.alpha FROM tablepor_editor;
         ALTER TABLE {schema}.zeta ADD COLUMN note text;
         ALTER TABLE {schema}.alpha ENABLE ROW LEVEL SECURITY;"
        ))
        .await
        .unwrap();
    let refreshed = app.schema_tables(&info.id, &schema).await.unwrap();
    assert!(
        refreshed
            .iter()
            .find(|t| t.name == "alpha")
            .unwrap()
            .read_blocked_reason
            .is_some()
    );
    assert_eq!(
        refreshed
            .iter()
            .find(|t| t.name == "zeta")
            .unwrap()
            .columns
            .last()
            .unwrap()
            .name,
        "note"
    );
    assert_eq!(
        app.query(query(&info.id, &format!("SELECT * FROM {schema}.alpha")))
            .await
            .unwrap_err()
            .code,
        "POLICY_DENIED"
    );
    admin
        .batch_execute(&format!(
            "REVOKE USAGE ON SCHEMA {schema} FROM tablepor_editor"
        ))
        .await
        .unwrap();
    assert_eq!(
        app.schema_tables(&info.id, &schema).await.unwrap_err().code,
        "SCHEMA_UNAVAILABLE"
    );
    app.disconnect(&info.id).await.unwrap();
    admin
        .batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
    task.abort();
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn preserves_precise_values_nulls_and_empty_result_columns() {
    let app = Workbench::default();
    let info = app.connect(request()).await.unwrap();
    let data = app
        .query(query(&info.id, "SELECT * FROM public.precision_values"))
        .await
        .unwrap();
    assert_eq!(
        data.rows[0],
        vec![
            Some("9007199254740993".into()),
            Some("12345678901234567890.1234567890".into()),
            None
        ]
    );
    let empty = app
        .query(query(
            &info.id,
            "SELECT id FROM public.orders WHERE id = -1",
        ))
        .await
        .unwrap();
    assert_eq!(empty.columns, vec!["id"]);
    assert!(empty.rows.is_empty());
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn refuses_admin_accounts_and_destructive_sql() {
    let app = Workbench::default();
    let mut admin_request = request();
    admin_request.username = "fixture_admin".into();
    admin_request.password = "local-fixture-admin".into();
    assert_eq!(
        app.connect(admin_request).await.unwrap_err().code,
        "UNSAFE_ROLE"
    );
    let info = app.connect(request()).await.unwrap();
    for sql in [
        "DELETE FROM public.orders",
        "TRUNCATE public.orders",
        "SELECT public.dangerous()",
        "SELECT 1; DROP TABLE public.orders",
        "SELECT * FROM public.orders_view",
    ] {
        assert_eq!(
            app.query(query(&info.id, sql)).await.unwrap_err().code,
            "POLICY_DENIED",
            "{sql}"
        );
    }
    let rows = app
        .query(query(&info.id, "SELECT id FROM public.orders"))
        .await
        .unwrap();
    assert_eq!(rows.rows.len(), 450);
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn server_role_denies_writes_even_without_app_policy() {
    let (client, connection) = tokio_postgres::connect(&format!("host=127.0.0.1 port={} dbname=tablepor_test user=tablepor_reader password=local-reader-only", fixture_port()), NoTls).await.unwrap();
    let task = tokio::spawn(async move {
        let _ = connection.await;
    });
    for sql in [
        "DELETE FROM public.orders",
        "TRUNCATE public.orders",
        "DROP TABLE public.orders",
        "SELECT public.dangerous()",
        "CREATE TEMP TABLE bypass (id int)",
    ] {
        assert!(
            client.batch_execute(sql).await.is_err(),
            "server must deny {sql}"
        );
    }
    task.abort();
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn filter_values_are_bound_and_errors_do_not_echo_them() {
    let app = Workbench::default();
    let info = app.connect(request()).await.unwrap();
    let mut r = browse(&info.id);
    r.filters = vec![Filter {
        column: "customer".into(),
        operator: FilterOperator::Equals,
        value: "' OR true; DELETE FROM public.orders; --".into(),
    }];
    assert!(app.browse(r).await.unwrap().rows.is_empty());
    let mut r = browse(&info.id);
    r.filters = vec![Filter {
        column: "total".into(),
        operator: FilterOperator::GreaterThan,
        value: "CANARY_PRIVATE_VALUE".into(),
    }];
    let error = app.browse(r).await.unwrap_err();
    assert!(!error.message.contains("CANARY_PRIVATE_VALUE"));
    let mut r = browse(&info.id);
    r.filters = vec![Filter {
        column: "customer".into(),
        operator: FilterOperator::Contains,
        value: "%".into(),
    }];
    assert!(app.browse(r).await.unwrap().rows.is_empty());
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn cancellation_releases_a_waiting_server_query() {
    let app = Arc::new(Workbench::default());
    let info = app.connect(request()).await.unwrap();
    let (admin, task) = admin().await;
    admin
        .batch_execute("BEGIN; LOCK TABLE public.orders IN ACCESS EXCLUSIVE MODE")
        .await
        .unwrap();
    let request = query(&info.id, "SELECT * FROM public.orders");
    let id = request.request_id.clone();
    let worker = app.clone();
    let query_task = tokio::spawn(async move { worker.query(request).await });
    tokio::time::sleep(Duration::from_millis(200)).await;
    app.cancel(&info.id, &id).unwrap();
    let error = tokio::time::timeout(Duration::from_secs(4), query_task)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert_eq!(error.code, "CANCELLED");
    admin.batch_execute("ROLLBACK").await.unwrap();
    let again = app.query(query(&info.id, "SELECT 1")).await.unwrap();
    assert_eq!(again.rows[0][0].as_deref(), Some("1"));
    task.abort();
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn role_drift_is_checked_on_every_execution() {
    let app = Workbench::default();
    let info = app.connect(request()).await.unwrap();
    let (admin, task) = admin().await;
    admin
        .batch_execute("GRANT INSERT ON public.orders TO tablepor_reader")
        .await
        .unwrap();
    let result = app.query(query(&info.id, "SELECT 1")).await;
    // Restore fixture permissions before asserting, even when the behavior regresses.
    admin
        .batch_execute("REVOKE INSERT ON public.orders FROM tablepor_reader")
        .await
        .unwrap();
    assert_eq!(result.unwrap_err().code, "UNSAFE_ROLE");
    task.abort();
}

#[tokio::test]
async fn plaintext_remote_and_invalid_requests_are_rejected_without_network() {
    let app = Workbench::default();
    let mut r = request();
    r.host = "example.com".into();
    assert_eq!(app.connect(r).await.unwrap_err().code, "TLS_REQUIRED");
    let mut r = request();
    r.host = "localhost".into();
    assert_eq!(app.connect(r).await.unwrap_err().code, "TLS_REQUIRED");
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn disconnect_cancels_jobs_and_metadata_shares_the_concurrency_limit() {
    let app = Arc::new(Workbench::default());
    let info = app.connect(request()).await.unwrap();
    let (admin, task) = admin().await;
    admin
        .batch_execute("BEGIN; LOCK TABLE public.orders IN ACCESS EXCLUSIVE MODE")
        .await
        .unwrap();
    let request = query(&info.id, "SELECT * FROM public.orders");
    let worker = app.clone();
    let handle = tokio::spawn(async move { worker.query(request).await });
    tokio::time::sleep(Duration::from_millis(200)).await;
    let busy = app.overview(&info.id).await.unwrap_err();
    app.disconnect(&info.id).await.unwrap();
    let cancelled = tokio::time::timeout(Duration::from_secs(4), handle)
        .await
        .unwrap()
        .unwrap();
    admin.batch_execute("ROLLBACK").await.unwrap();
    assert_eq!(busy.code, "BUSY");
    assert_eq!(cancelled.unwrap_err().code, "CANCELLED");
    assert_eq!(
        app.overview(&info.id).await.unwrap_err().code,
        "NOT_CONNECTED"
    );
    task.abort();
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn verified_tls_does_not_fall_back_to_plaintext() {
    let app = Workbench::default();
    let mut r = request();
    r.tls = TlsMode::VerifyFull;
    // This fixture does not provide TLS. The app must reject it, not downgrade.
    assert_eq!(
        app.connect(r).await.unwrap_err().code,
        "TLS_CONNECTION_FAILED"
    );
}

fn editor_request() -> ConnectRequest {
    ConnectRequest {
        username: "tablepor_editor".into(),
        password: "local-editor-only".into(),
        manual_edits: true,
        ..request()
    }
}
async fn editable_rows(app: &Workbench, id: &str, table: &str) -> QueryResult {
    app.browse(BrowseRequest {
        database: None,
        schema: None,
        table: table.into(),
        ..browse(id)
    })
    .await
    .unwrap()
}
fn edit(result: &QueryResult, index: usize, column: &str, value: Option<&str>) -> RowChange {
    RowChange {
        insert: false,
        delete: false,
        original: result
            .columns
            .iter()
            .cloned()
            .zip(result.rows[index].clone())
            .collect(),
        changes: [(column.to_owned(), value.map(str::to_owned))].into(),
    }
}
fn save(id: &str, table: &str, rows: Vec<RowChange>) -> SaveRequest {
    SaveRequest {
        database: None,
        schema: None,
        connection_id: id.into(),
        request_id: Uuid::new_v4().to_string(),
        table: table.into(),
        rows,
    }
}
async fn reset_edits() {
    let (db, task) = admin().await;
    db.batch_execute("UPDATE public.edit_items SET label = CASE id WHEN 1 THEN 'First' ELSE 'Second' END, note = CASE id WHEN 1 THEN NULL ELSE 'original' END; UPDATE public.edit_composite SET label = CASE tenant WHEN 1 THEN 'one' ELSE 'two' END").await.unwrap();
    task.abort();
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn page_sizes_and_explicit_filtered_counts_are_bounded() {
    let app = Workbench::default();
    let info = app.connect(request()).await.unwrap();
    for size in [50, 100, 250, 500] {
        let mut req = browse(&info.id);
        req.page_size = size;
        let first = app.browse(req).await.unwrap();
        assert_eq!(first.rows.len(), size.min(450));
        assert_eq!(first.rows[0][0].as_deref(), Some("1"));
        let mut req = browse(&info.id);
        req.page_size = size;
        req.page = 1;
        let second = app.browse(req).await.unwrap();
        if size < 450 {
            assert_eq!(second.rows[0][0], Some((size + 1).to_string()));
        } else {
            assert!(second.rows.is_empty());
        }
    }
    assert_eq!(app.count(browse(&info.id)).await.unwrap().rows, "450");
    let mut req = browse(&info.id);
    req.filters.push(Filter {
        column: "id".into(),
        operator: FilterOperator::LessThan,
        value: "8".into(),
    });
    assert_eq!(app.count(req).await.unwrap().rows, "7");
    for (page_size, page) in [(0, 0), (51, 0), (500, 201)] {
        let mut req = browse(&info.id);
        req.page_size = page_size;
        req.page = page;
        assert_eq!(app.browse(req).await.unwrap_err().code, "POLICY_DENIED");
    }
    app.disconnect(&info.id).await.unwrap();
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn staged_deletes_and_updates_commit_atomically_and_recheck_permissions() {
    let (db, task) = admin().await;
    let schema = format!("deletes_{}", Uuid::new_v4().simple());
    db.batch_execute(&format!("CREATE SCHEMA {schema}; GRANT USAGE ON SCHEMA {schema} TO tablepor_editor, tablepor_reader;
        CREATE TABLE {schema}.items(id integer PRIMARY KEY, label text, payload jsonb);
        INSERT INTO {schema}.items VALUES (1,'one','{{\"a\":1}}'),(2,'two',NULL),(3,'three','[]');
        GRANT SELECT, UPDATE, DELETE ON {schema}.items TO tablepor_editor;
        GRANT SELECT ON {schema}.items TO tablepor_reader;")).await.unwrap();
    let app = Workbench::default();
    let mut profile = editor_request();
    profile.schema = schema.clone();
    let info = app.connect(profile).await.unwrap();
    assert!(info.tables[0].deletable);
    let original = editable_rows(&app, &info.id, "items").await;
    let deletion = |index: usize| RowChange {
        insert: false,
        delete: true,
        original: original
            .columns
            .iter()
            .cloned()
            .zip(original.rows[index].clone())
            .collect(),
        changes: Default::default(),
    };
    // A later stale row must roll back an earlier delete in the same batch.
    db.batch_execute(&format!(
        "UPDATE {schema}.items SET label='concurrent' WHERE id=2"
    ))
    .await
    .unwrap();
    assert_eq!(
        app.save(save(
            &info.id,
            "items",
            vec![deletion(0), edit(&original, 1, "label", Some("draft"))]
        ))
        .await
        .unwrap_err()
        .code,
        "EDIT_CONFLICT"
    );
    assert_eq!(editable_rows(&app, &info.id, "items").await.rows.len(), 3);
    let current = editable_rows(&app, &info.id, "items").await;
    let saved = app
        .save(save(
            &info.id,
            "items",
            vec![deletion(0), edit(&current, 1, "label", Some("saved"))],
        ))
        .await
        .unwrap();
    assert_eq!((saved.updated_rows, saved.deleted_rows), (1, 1));
    let current = editable_rows(&app, &info.id, "items").await;
    assert_eq!(current.rows.len(), 2);
    assert_eq!(current.rows[0][1].as_deref(), Some("saved"));
    db.batch_execute(&format!(
        "REVOKE DELETE ON {schema}.items FROM tablepor_editor"
    ))
    .await
    .unwrap();
    assert_eq!(
        app.save(save(&info.id, "items", vec![deletion(2)]))
            .await
            .unwrap_err()
            .code,
        "POLICY_DENIED"
    );
    let mut invalid = deletion(2);
    invalid.changes.insert("label".into(), Some("no".into()));
    assert_eq!(
        app.save(save(&info.id, "items", vec![invalid]))
            .await
            .unwrap_err()
            .code,
        "EDIT_LIMIT"
    );
    let mut reader = request();
    reader.schema = schema.clone();
    let read = app.connect(reader).await.unwrap();
    assert!(!read.tables[0].deletable);
    assert_eq!(
        app.save(save(&read.id, "items", vec![deletion(2)]))
            .await
            .unwrap_err()
            .code,
        "POLICY_DENIED"
    );
    app.disconnect(&read.id).await.unwrap();
    app.disconnect(&info.id).await.unwrap();
    db.batch_execute(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
    task.abort();
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn manual_save_persists_exact_text_null_and_composite_keys() {
    reset_edits().await;
    let app = Workbench::default();
    let info = app.connect(editor_request()).await.unwrap();
    let table = info.tables.iter().find(|t| t.name == "edit_items").unwrap();
    assert!(
        table
            .columns
            .iter()
            .find(|c| c.name == "label")
            .unwrap()
            .editable
    );
    assert!(
        !table
            .columns
            .iter()
            .find(|c| c.name == "id")
            .unwrap()
            .editable
    );
    assert!(
        !table
            .columns
            .iter()
            .find(|c| c.name == "label_copy")
            .unwrap()
            .editable
    );
    let original = editable_rows(&app, &info.id, "edit_items").await;
    let literal = "O'Reilly'; DELETE FROM public.edit_items; --\nภาษาไทย";
    let saved = app
        .save(save(
            &info.id,
            "edit_items",
            vec![
                edit(&original, 0, "label", Some(literal)),
                edit(&original, 1, "note", None),
            ],
        ))
        .await
        .unwrap();
    assert_eq!(saved.updated_rows, 2);
    let after = editable_rows(&app, &info.id, "edit_items").await;
    assert_eq!(after.rows.len(), 2);
    assert_eq!(after.rows[0][1].as_deref(), Some(literal));
    assert_eq!(after.rows[0][3], original.rows[0][3]);
    assert_eq!(after.rows[0][4].as_deref(), Some(literal));
    assert!(after.rows[1][2].is_none());
    let composite = editable_rows(&app, &info.id, "edit_composite").await;
    app.save(save(
        &info.id,
        "edit_composite",
        vec![edit(&composite, 0, "label", Some("changed"))],
    ))
    .await
    .unwrap();
    let after = editable_rows(&app, &info.id, "edit_composite").await;
    assert_eq!(after.rows[0][2].as_deref(), Some("changed"));
    assert_eq!(after.rows[1][2].as_deref(), Some("two"));
    reset_edits().await;
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn conflicts_and_database_errors_roll_back_the_entire_save() {
    reset_edits().await;
    let app = Workbench::default();
    let info = app.connect(editor_request()).await.unwrap();
    let original = editable_rows(&app, &info.id, "edit_items").await;
    let (db, task) = admin().await;
    db.batch_execute("UPDATE public.edit_items SET note = 'another writer' WHERE id = 2")
        .await
        .unwrap();
    let err = app
        .save(save(
            &info.id,
            "edit_items",
            vec![
                edit(&original, 0, "label", Some("must roll back")),
                edit(&original, 1, "note", Some("stale overwrite")),
            ],
        ))
        .await
        .unwrap_err();
    assert_eq!(err.code, "EDIT_CONFLICT");
    let after = editable_rows(&app, &info.id, "edit_items").await;
    assert_eq!(after.rows[0][1].as_deref(), Some("First"));
    assert_eq!(after.rows[1][2].as_deref(), Some("another writer"));
    let err = app
        .save(save(
            &info.id,
            "edit_items",
            vec![
                edit(&after, 0, "note", Some("must roll back")),
                edit(&after, 1, "label", Some("First")),
            ],
        ))
        .await
        .unwrap_err();
    assert_eq!(err.code, "DATABASE_ERROR");
    assert!(editable_rows(&app, &info.id, "edit_items").await.rows[0][2].is_none());
    task.abort();
    reset_edits().await;
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn manual_edits_cannot_bypass_scope_role_keys_or_sql_policy() {
    let app = Workbench::default();
    let reader = app.connect(request()).await.unwrap();
    let info = app.connect(editor_request()).await.unwrap();
    let original = editable_rows(&app, &info.id, "edit_items").await;
    for (id, column, value) in [
        (&reader.id, "label", Some("x")),
        (&info.id, "id", Some("99")),
        (&info.id, "label_copy", Some("x")),
        (&info.id, "missing", Some("x")),
        (&info.id, "label", None),
    ] {
        assert_eq!(
            app.save(save(
                id,
                "edit_items",
                vec![edit(&original, 0, column, value)]
            ))
            .await
            .unwrap_err()
            .code,
            "POLICY_DENIED"
        );
    }
    let no_key = editable_rows(&app, &info.id, "edit_no_key").await;
    assert_eq!(
        app.save(save(
            &info.id,
            "edit_no_key",
            vec![edit(&no_key, 0, "label", Some("x"))]
        ))
        .await
        .unwrap_err()
        .code,
        "POLICY_DENIED"
    );
    for sql in [
        "UPDATE public.edit_items SET label = 'x'",
        "DELETE FROM public.edit_items",
        "TRUNCATE public.edit_items",
        "SELECT public.dangerous()",
    ] {
        assert_eq!(
            app.query(query(&info.id, sql)).await.unwrap_err().code,
            "POLICY_DENIED"
        );
    }
    let mut readonly = request();
    readonly.manual_edits = true;
    let readonly = app.connect(readonly).await.unwrap();
    assert!(
        readonly
            .tables
            .iter()
            .flat_map(|t| &t.columns)
            .all(|c| !c.editable)
    );
    assert!(
        app.save(save(
            &readonly.id,
            "edit_items",
            vec![edit(&original, 0, "label", Some("x"))]
        ))
        .await
        .is_err()
    );
    assert_eq!(
        app.save(save(&info.id, "edit_items", vec![]))
            .await
            .unwrap_err()
            .code,
        "EDIT_LIMIT"
    );
    assert_eq!(
        app.save(save(
            &info.id,
            "edit_items",
            vec![
                edit(&original, 0, "label", Some("x")),
                edit(&original, 0, "label", Some("y"))
            ]
        ))
        .await
        .unwrap_err()
        .code,
        "POLICY_DENIED"
    );
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn revoked_column_permission_is_rechecked_at_save() {
    let app = Workbench::default();
    let info = app.connect(editor_request()).await.unwrap();
    let original = editable_rows(&app, &info.id, "edit_items").await;
    let (db, task) = admin().await;
    db.batch_execute("REVOKE UPDATE(note) ON public.edit_items FROM tablepor_editor")
        .await
        .unwrap();
    let outcome = app
        .save(save(
            &info.id,
            "edit_items",
            vec![edit(&original, 0, "note", Some("x"))],
        ))
        .await;
    db.batch_execute("GRANT UPDATE(note) ON public.edit_items TO tablepor_editor")
        .await
        .unwrap();
    task.abort();
    assert_eq!(outcome.unwrap_err().code, "POLICY_DENIED");
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn interrupted_save_reports_uncertainty_and_does_not_retry() {
    let app = Arc::new(Workbench::default());
    let info = app.connect(editor_request()).await.unwrap();
    let original = editable_rows(&app, &info.id, "edit_items").await;
    let request = save(
        &info.id,
        "edit_items",
        vec![edit(&original, 0, "note", Some("never committed"))],
    );
    let request_id = request.request_id.clone();
    let (db, task) = admin().await;
    db.batch_execute("BEGIN; SELECT id FROM public.edit_items WHERE id = 1 FOR UPDATE")
        .await
        .unwrap();
    let worker = app.clone();
    let saving = tokio::spawn(async move { worker.save(request).await });
    tokio::time::sleep(Duration::from_millis(200)).await;
    app.cancel(&info.id, &request_id).unwrap();
    let outcome = saving.await.unwrap();
    db.batch_execute("ROLLBACK").await.unwrap();
    task.abort();
    assert_eq!(outcome.unwrap_err().code, "SAVE_UNCERTAIN");
    assert_eq!(
        editable_rows(&app, &info.id, "edit_items").await.rows,
        original.rows
    );
}

fn filter(column: &str, operator: FilterOperator, value: &str) -> Filter {
    Filter {
        column: column.into(),
        operator,
        value: value.into(),
    }
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn multiple_filters_and_sorting_run_before_pagination() {
    let app = Workbench::default();
    let info = app.connect(request()).await.unwrap();
    let mut r = browse(&info.id);
    r.filters = vec![
        filter("id", FilterOperator::GreaterThan, "440"),
        filter("status", FilterOperator::Equals, "completed"),
    ];
    r.sort_column = Some("total".into());
    r.descending = true;
    let data = app.browse(r).await.unwrap();
    // Fixture status starts processing at id 1; completed occurs at 3,6,...450.
    assert_eq!(
        data.rows
            .iter()
            .map(|r| r[0].as_deref().unwrap())
            .collect::<Vec<_>>(),
        vec!["450", "447", "444", "441"]
    );
    let mut r = browse(&info.id);
    r.filters = vec![
        filter("id", FilterOperator::GreaterThan, "440"),
        filter("status", FilterOperator::Equals, "completed"),
    ];
    r.filter_match = FilterMatch::Any;
    r.page_size = 250;
    assert_eq!(app.browse(r).await.unwrap().rows.len(), 156);
    let mut r = browse(&info.id);
    r.sort_column = Some("total".into());
    r.descending = true;
    r.page = 1;
    let second = app.browse(r).await.unwrap();
    assert_eq!(second.rows[0][0].as_deref(), Some("400"));
    assert_eq!(second.rows[49][0].as_deref(), Some("351"));
    // Sort ties use the immutable key, keeping page boundaries deterministic.
    let mut r = browse(&info.id);
    r.sort_column = Some("country".into());
    let sorted = app.browse(r).await.unwrap();
    assert!(sorted.sql.contains("\"orders\".\"id\" ASC NULLS LAST"));
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn mixed_null_and_bound_filters_preserve_parameter_positions() {
    let app = Workbench::default();
    let info = app.connect(request()).await.unwrap();
    let mut r = browse(&info.id);
    r.table = "customers".into();
    r.filters = vec![
        filter("email", FilterOperator::IsNull, ""),
        filter("name", FilterOperator::Contains, "Yuna"),
        filter("id", FilterOperator::Equals, "3"),
    ];
    let data = app.browse(r).await.unwrap();
    assert_eq!(data.rows.len(), 1);
    assert_eq!(data.rows[0][0].as_deref(), Some("3"));
    assert!(data.sql.contains("ILIKE $1"));
    assert!(data.sql.contains("$2::pg_catalog.text"));
    let mut r = browse(&info.id);
    r.table = "customers".into();
    r.filters = vec![
        filter("email", FilterOperator::IsNull, ""),
        filter("name", FilterOperator::Equals, "' OR true; --"),
    ];
    r.filter_match = FilterMatch::Any;
    assert_eq!(app.browse(r).await.unwrap().rows.len(), 1);
    for descending in [true, false] {
        let mut r = browse(&info.id);
        r.table = "customers".into();
        r.sort_column = Some("email".into());
        r.descending = descending;
        assert_eq!(
            app.browse(r).await.unwrap().rows.last().unwrap()[0].as_deref(),
            Some("3")
        );
    }
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn filters_and_sort_columns_remain_bounded_and_catalog_checked() {
    let app = Workbench::default();
    let info = app.connect(request()).await.unwrap();
    let mut r = browse(&info.id);
    r.filters = vec![filter("id", FilterOperator::Equals, "1"); 21];
    assert_eq!(app.browse(r).await.unwrap_err().code, "POLICY_DENIED");
    let mut r = browse(&info.id);
    r.filters = vec![filter(
        "customer",
        FilterOperator::Equals,
        &"x".repeat(4097),
    )];
    assert_eq!(app.browse(r).await.unwrap_err().code, "POLICY_DENIED");
    let mut r = browse(&info.id);
    r.filters = vec![
        filter("id", FilterOperator::GreaterThan, "1"),
        filter("id); DROP TABLE orders; --", FilterOperator::Equals, "1"),
    ];
    assert_eq!(app.browse(r).await.unwrap_err().code, "POLICY_DENIED");
    let mut r = browse(&info.id);
    r.sort_column = Some("id; DELETE FROM orders".into());
    assert_eq!(app.browse(r).await.unwrap_err().code, "POLICY_DENIED");
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn generated_and_identity_columns_are_not_editable() {
    let app = Workbench::default();
    let info = app.connect(editor_request()).await.unwrap();
    let table = info
        .tables
        .iter()
        .find(|t| t.name == "generated_items")
        .unwrap();
    for name in ["sequence", "label_copy"] {
        assert!(
            !table
                .columns
                .iter()
                .find(|c| c.name == name)
                .unwrap()
                .editable
        );
    }
    assert!(
        table
            .columns
            .iter()
            .find(|c| c.name == "label")
            .unwrap()
            .editable
    );
    // Virtual columns must be excluded from both browsing and arbitrary SQL scope.
    if info.server_version.parse::<u32>().unwrap() >= 180_000 {
        assert!(
            info.tables
                .iter()
                .find(|t| t.name == "virtual_items")
                .unwrap()
                .read_blocked_reason
                .is_some()
        );
        assert!(
            app.query(query(&info.id, "SELECT * FROM public.virtual_items"))
                .await
                .is_err()
        );
        let mut request = browse(&info.id);
        request.table = "virtual_items".into();
        assert!(app.browse(request).await.is_err());
    }
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn multiple_connections_and_schemas_keep_reads_and_saves_isolated() {
    let app = Workbench::default();
    let first = app.connect(editor_request()).await.unwrap();
    let second = app.connect(request()).await.unwrap();
    let schemas = app.schemas(&first.id).await.unwrap();
    assert!(schemas.contains(&"public".into()) && schemas.contains(&"audit".into()));
    assert!(!schemas.contains(&"restricted".into()));
    assert!(app.schema_tables(&first.id, "restricted").await.is_err());
    assert!(
        app.schema_tables(&first.id, "audit'; DROP SCHEMA public; --")
            .await
            .is_err()
    );
    let columns = app.schema_tables(&first.id, "audit").await.unwrap();
    assert_eq!(columns[0].columns.len(), 2);
    let mut browse_audit = browse(&first.id);
    browse_audit.schema = Some("audit".into());
    browse_audit.table = "edit_items".into();
    let data = app.browse(browse_audit).await.unwrap();
    assert_eq!(data.rows[0][1].as_deref(), Some("Audit only"));
    let mut update = SaveRequest {
        database: None,
        connection_id: first.id.clone(),
        request_id: Uuid::new_v4().to_string(),
        schema: Some("audit".into()),
        table: "edit_items".into(),
        rows: vec![RowChange {
            insert: false,
            delete: false,
            original: [
                ("id".into(), Some("1".into())),
                ("label".into(), Some("Audit only".into())),
            ]
            .into(),
            changes: [("label".into(), Some("Audit edited".into()))].into(),
        }],
    };
    app.save(update).await.unwrap();
    let mut q = query(
        &second.id,
        "SELECT label FROM audit.edit_items WHERE id = 1",
    );
    q.schema = Some("audit".into());
    assert_eq!(
        app.query(q).await.unwrap().rows[0][0].as_deref(),
        Some("Audit edited")
    );
    let public = app
        .query(query(
            &second.id,
            "SELECT label FROM public.edit_items WHERE id = 1",
        ))
        .await
        .unwrap();
    assert_ne!(public.rows[0][0].as_deref(), Some("Audit edited"));
    let mut forbidden = query(&second.id, "SELECT * FROM public.edit_items");
    forbidden.schema = Some("audit".into());
    assert!(app.query(forbidden).await.is_err());
    assert!(
        !app.indexes_in_schema(&second.id, "edit_items", Some("audit"))
            .await
            .unwrap()
            .is_empty()
    );
    update = SaveRequest {
        database: None,
        connection_id: first.id.clone(),
        request_id: Uuid::new_v4().to_string(),
        schema: Some("audit".into()),
        table: "edit_items".into(),
        rows: vec![RowChange {
            insert: false,
            delete: false,
            original: [
                ("id".into(), Some("1".into())),
                ("label".into(), Some("Audit edited".into())),
            ]
            .into(),
            changes: [("label".into(), Some("Audit only".into()))].into(),
        }],
    };
    app.save(update).await.unwrap();
    app.disconnect(&first.id).await.unwrap();
    assert!(app.query(query(&first.id, "SELECT 1")).await.is_err());
    assert!(app.query(query(&second.id, "SELECT 1")).await.is_ok());
    app.disconnect(&second.id).await.unwrap();
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn typed_inserts_use_defaults_preserve_values_and_commit_atomically() {
    let (admin, task) = admin().await;
    admin.batch_execute("DROP TABLE IF EXISTS public.insert_fixture; CREATE TABLE public.insert_fixture (id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY, label text NOT NULL UNIQUE, payload json, amount numeric(30,10), note text DEFAULT 'default note', copied text GENERATED ALWAYS AS (label) STORED); GRANT SELECT ON public.insert_fixture TO tablepor_editor, tablepor_reader; GRANT INSERT(label,payload,amount,note), UPDATE(label), DELETE ON public.insert_fixture TO tablepor_editor;").await.unwrap();
    let app = Workbench::default();
    let mut settings = request();
    settings.username = "tablepor_editor".into();
    settings.password = "local-editor-only".into();
    settings.manual_edits = true;
    let info = app.connect(settings).await.unwrap();
    let table = info
        .tables
        .iter()
        .find(|t| t.name == "insert_fixture")
        .unwrap();
    assert!(table.insertable);
    assert!(!table.columns[0].insertable && table.columns[0].has_default);
    assert!(!table.columns.last().unwrap().insertable);
    let insert = |label: &str| RowChange {
        insert: true,
        delete: false,
        original: Default::default(),
        changes: [
            ("label".into(), Some(label.into())),
            (
                "payload".into(),
                Some("{\"n\":9007199254740993,\"n\":2}".into()),
            ),
            (
                "amount".into(),
                Some("12345678901234567890.1234567890".into()),
            ),
        ]
        .into(),
    };
    let mut null_note = insert("null note");
    null_note.changes.insert("note".into(), None);
    let saved = app
        .save(save(
            &info.id,
            "insert_fixture",
            vec![insert("default note"), null_note],
        ))
        .await
        .unwrap();
    assert_eq!(
        (saved.inserted_rows, saved.updated_rows, saved.deleted_rows),
        (2, 0, 0)
    );
    let mut b = browse(&info.id);
    b.table = "insert_fixture".into();
    let data = app.browse(b).await.unwrap();
    assert_eq!(
        data.rows[0][2].as_deref(),
        Some("{\"n\":9007199254740993,\"n\":2}")
    );
    assert_eq!(
        data.rows[0][3].as_deref(),
        Some("12345678901234567890.1234567890")
    );
    assert_eq!(data.rows[0][4].as_deref(), Some("default note"));
    assert_eq!(data.rows[1][4], None);
    assert_eq!(data.rows[0][5].as_deref(), Some("default note"));
    // A constraint failure after a successful update and insert rolls back all three.
    assert!(
        app.save(save(
            &info.id,
            "insert_fixture",
            vec![
                edit(&data, 0, "label", Some("changed")),
                insert("rolled back"),
                insert("null note")
            ]
        ))
        .await
        .is_err()
    );
    let rows = admin
        .query("SELECT label FROM public.insert_fixture ORDER BY id", &[])
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].get::<_, String>(0), "default note");
    let deletion = RowChange {
        insert: false,
        delete: true,
        original: data
            .columns
            .iter()
            .cloned()
            .zip(data.rows[1].clone())
            .collect(),
        changes: Default::default(),
    };
    let saved = app
        .save(save(
            &info.id,
            "insert_fixture",
            vec![
                edit(&data, 0, "label", Some("changed")),
                deletion,
                insert("new row"),
            ],
        ))
        .await
        .unwrap();
    assert_eq!(
        (saved.inserted_rows, saved.updated_rows, saved.deleted_rows),
        (1, 1, 1)
    );
    for name in ["id", "copied", "not_a_column"] {
        let mut row = insert("forbidden");
        row.changes.insert(name.into(), Some("1".into()));
        assert_eq!(
            app.save(save(&info.id, "insert_fixture", vec![row]))
                .await
                .unwrap_err()
                .code,
            "POLICY_DENIED"
        );
    }
    let mut invalid = insert("bad");
    invalid.delete = true;
    assert_eq!(
        app.save(save(&info.id, "insert_fixture", vec![invalid]))
            .await
            .unwrap_err()
            .code,
        "EDIT_LIMIT"
    );
    let mut invalid = insert("bad original");
    invalid.original.insert("id".into(), Some("1".into()));
    assert_eq!(
        app.save(save(&info.id, "insert_fixture", vec![invalid]))
            .await
            .unwrap_err()
            .code,
        "EDIT_LIMIT"
    );
    admin.batch_execute("REVOKE INSERT(label,payload,amount,note) ON public.insert_fixture FROM tablepor_editor").await.unwrap();
    assert_eq!(
        app.save(save(&info.id, "insert_fixture", vec![insert("revoked")]))
            .await
            .unwrap_err()
            .code,
        "POLICY_DENIED"
    );
    assert_eq!(
        admin
            .query_one("SELECT count(*) FROM public.insert_fixture", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        2
    );
    app.disconnect(&info.id).await.unwrap();
    admin
        .batch_execute("DROP TABLE public.insert_fixture")
        .await
        .unwrap();
    task.abort();
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn inserts_distinguish_default_null_and_required_keys_and_enforce_limits() {
    let (admin, task) = admin().await;
    admin.batch_execute("DROP TABLE IF EXISTS public.insert_defaults; CREATE TABLE public.insert_defaults (id bigint PRIMARY KEY DEFAULT 100, label text NOT NULL DEFAULT 'default', note text); GRANT SELECT, INSERT ON public.insert_defaults TO tablepor_editor, tablepor_reader;").await.unwrap();
    let app = Workbench::default();
    let mut settings = request();
    settings.username = "tablepor_editor".into();
    settings.password = "local-editor-only".into();
    settings.manual_edits = true;
    let info = app.connect(settings).await.unwrap();
    let insert = || RowChange {
        insert: true,
        delete: false,
        original: Default::default(),
        changes: Default::default(),
    };
    assert_eq!(
        app.save(save(&info.id, "insert_defaults", vec![insert()]))
            .await
            .unwrap()
            .inserted_rows,
        1
    );
    let row = admin
        .query_one("SELECT id,label,note FROM public.insert_defaults", &[])
        .await
        .unwrap();
    assert_eq!(row.get::<_, i64>(0), 100);
    assert_eq!(row.get::<_, String>(1), "default");
    assert_eq!(row.get::<_, Option<String>>(2), None);
    admin
        .batch_execute("ALTER TABLE public.insert_defaults ALTER COLUMN id DROP DEFAULT")
        .await
        .unwrap();
    assert!(
        app.save(save(&info.id, "insert_defaults", vec![insert()]))
            .await
            .is_err()
    );
    let mut row = insert();
    row.changes
        .insert("id".into(), Some("9007199254740993".into()));
    app.save(save(&info.id, "insert_defaults", vec![row]))
        .await
        .unwrap();
    let mut row = insert();
    row.changes.insert("id".into(), None);
    assert_eq!(
        app.save(save(&info.id, "insert_defaults", vec![row]))
            .await
            .unwrap_err()
            .code,
        "POLICY_DENIED"
    );
    assert_eq!(
        app.save(save(
            &info.id,
            "insert_defaults",
            (0..101).map(|_| insert()).collect()
        ))
        .await
        .unwrap_err()
        .code,
        "EDIT_LIMIT"
    );
    let mut row = insert();
    row.changes
        .insert("label".into(), Some("x".repeat(4 * 1024 * 1024)));
    assert_eq!(
        app.save(save(&info.id, "insert_defaults", vec![row]))
            .await
            .unwrap_err()
            .code,
        "EDIT_LIMIT"
    );
    let mut row = insert();
    row.changes = (0..501).map(|i| (format!("c{i}"), None)).collect();
    assert_eq!(
        app.save(save(&info.id, "insert_defaults", vec![row]))
            .await
            .unwrap_err()
            .code,
        "EDIT_LIMIT"
    );
    admin
        .batch_execute("REVOKE INSERT ON public.insert_defaults FROM tablepor_reader")
        .await
        .unwrap();
    let reader = app.connect(request()).await.unwrap();
    assert_eq!(
        app.save(save(&reader.id, "insert_defaults", vec![insert()]))
            .await
            .unwrap_err()
            .code,
        "POLICY_DENIED"
    );
    app.disconnect(&reader.id).await.unwrap();
    app.disconnect(&info.id).await.unwrap();
    admin
        .batch_execute("DROP TABLE public.insert_defaults")
        .await
        .unwrap();
    task.abort();
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn server_browsing_routes_reads_counts_indexes_and_saves_to_the_selected_database() {
    let app = Workbench::default();
    let mut req = request();
    req.database.clear();
    req.username = "tablepor_editor".into();
    req.password = "local-editor-only".into();
    req.manual_edits = true;
    let info = app.connect(req).await.unwrap();
    assert!(info.database.is_empty());
    assert!(
        info.tables.is_empty(),
        "server connect must not fetch every schema"
    );
    let names = app.databases(&info.id).await.unwrap();
    assert!(names.contains(&"tablepor_other".into()));
    assert!(names.contains(&"odd \" database".into()));
    for hidden in [
        "template0",
        "template1",
        "tablepor_hidden",
        "tablepor_disabled",
    ] {
        assert!(!names.contains(&hidden.into()));
    }
    let schemas = app
        .schemas_in_database(&info.id, Some("tablepor_other"))
        .await
        .unwrap();
    assert!(schemas.contains(&"audit".into()));
    let catalog = app
        .tables_in_database(&info.id, "public", Some("tablepor_other"))
        .await
        .unwrap();
    assert!(
        catalog.iter().any(|t| t.name == "multi_db_items"
            && t.columns.iter().any(|c| c.name == "label" && c.editable))
    );
    for (database, schema, label, count) in [
        ("tablepor_test", "public", "original database", "1"),
        ("tablepor_other", "public", "other database", "2"),
        ("odd \" database", "public", "quoted database", "1"),
        ("tablepor_other", "audit", "other audit", "1"),
    ] {
        let mut read = browse(&info.id);
        read.database = Some(database.into());
        read.schema = Some(schema.into());
        read.table = "multi_db_items".into();
        let result = app.browse(read).await.unwrap();
        assert_eq!(result.rows[0][1].as_deref(), Some(label));
        let mut sql = query(
            &info.id,
            &format!("SELECT label FROM {schema}.multi_db_items ORDER BY id LIMIT 1"),
        );
        sql.database = Some(database.into());
        sql.schema = Some(schema.into());
        let result = app.query(sql).await.unwrap();
        assert_eq!(result.rows[0][0].as_deref(), Some(label));
        assert!(result.timings.unwrap().connection_reused);
        let mut count_req = browse(&info.id);
        count_req.database = Some(database.into());
        count_req.schema = Some(schema.into());
        count_req.table = "multi_db_items".into();
        assert_eq!(app.count(count_req).await.unwrap().rows, count);
        assert!(
            !app.indexes_in_database(&info.id, "multi_db_items", Some(schema), Some(database))
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            app.overview_in_database(&info.id, Some(database))
                .await
                .unwrap()
                .database,
            database
        );
    }
    // Returning to either database reuses its own physical connection.
    for database in [
        "tablepor_test",
        "tablepor_other",
        "tablepor_test",
        "tablepor_other",
    ] {
        let mut r = query(&info.id, "SELECT 1");
        r.database = Some(database.into());
        assert!(
            app.query(r)
                .await
                .unwrap()
                .timings
                .unwrap()
                .connection_reused
        );
    }
    let mut change = save(
        &info.id,
        "multi_db_items",
        vec![RowChange {
            insert: false,
            delete: false,
            original: [
                ("id".into(), Some("1".into())),
                ("label".into(), Some("other database".into())),
            ]
            .into(),
            changes: [("label".into(), Some("edited other".into()))].into(),
        }],
    );
    change.database = Some("tablepor_other".into());
    change.schema = Some("public".into());
    assert_eq!(app.save(change).await.unwrap().updated_rows, 1);
    for (database, expected) in [
        ("tablepor_other", "edited other"),
        ("tablepor_test", "original database"),
    ] {
        let mut r = query(
            &info.id,
            "SELECT label FROM public.multi_db_items WHERE id = 1",
        );
        r.database = Some(database.into());
        assert_eq!(
            app.query(r).await.unwrap().rows[0][0].as_deref(),
            Some(expected)
        );
    }
    let mut restore = save(
        &info.id,
        "multi_db_items",
        vec![RowChange {
            insert: false,
            delete: false,
            original: [
                ("id".into(), Some("1".into())),
                ("label".into(), Some("edited other".into())),
            ]
            .into(),
            changes: [("label".into(), Some("other database".into()))].into(),
        }],
    );
    restore.database = Some("tablepor_other".into());
    app.save(restore).await.unwrap();
    // Named database errors never fall back to another database.
    for database in ["missing_database", "tablepor_hidden", "", "bad\0name"] {
        let mut r = query(&info.id, "SELECT 1");
        r.database = Some(database.into());
        assert!(app.query(r).await.is_err());
    }
    let mut r = query(&info.id, "SELECT 1");
    r.database = Some("tablepor_test".into());
    assert!(
        app.query(r)
            .await
            .unwrap()
            .timings
            .unwrap()
            .connection_reused
    );
    // Touch five databases: at most four idle server sessions survive.
    for database in [
        "postgres",
        "tablepor_test",
        "tablepor_other",
        "odd \" database",
        "tablepor_fallback",
    ] {
        let mut r = query(&info.id, "SELECT 1");
        r.database = Some(database.into());
        app.query(r).await.unwrap();
    }
    let (admin, task) = admin().await;
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let count: i64 = admin.query_one("SELECT count(*) FROM pg_stat_activity WHERE application_name = 'table-por' AND usename = 'tablepor_editor'", &[]).await.unwrap().get(0);
            if count <= 4 { break; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.unwrap();
    app.disconnect(&info.id).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let count: i64 = admin.query_one("SELECT count(*) FROM pg_stat_activity WHERE application_name = 'table-por' AND usename = 'tablepor_editor'", &[]).await.unwrap().get(0);
            if count == 0 { break; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.unwrap();
    task.abort();
}

#[tokio::test]
#[ignore = "requires the disposable Compose fixture"]
async fn optional_database_discovery_has_bounded_fallback_and_keeps_strict_reader_checks() {
    let app = Workbench::default();
    let mut req = request();
    req.database.clear();
    let info = app.connect(req).await.unwrap();
    assert!(
        app.databases(&info.id)
            .await
            .unwrap()
            .contains(&"tablepor_test".into())
    );
    let mut r = query(
        &info.id,
        "SELECT label FROM public.multi_db_items WHERE id = 1",
    );
    r.database = Some("tablepor_other".into());
    assert_eq!(
        app.query(r).await.unwrap().rows[0][0].as_deref(),
        Some("other database")
    );
    // This database still has TEMP privileges: switching must re-run reader policy.
    let mut r = query(&info.id, "SELECT 1");
    r.database = Some("tablepor_fallback".into());
    assert!(app.query(r).await.is_err());
    app.disconnect(&info.id).await.unwrap();
    let mut req = request();
    req.database.clear();
    req.username = "tablepor_fallback".into();
    req.password = "local-fallback-only".into();
    req.manual_edits = true;
    let info = app.connect(req).await.unwrap();
    assert_eq!(
        app.overview(&info.id).await.unwrap().database,
        "tablepor_fallback"
    );
    app.disconnect(&info.id).await.unwrap();
    let mut req = request();
    req.database.clear();
    req.username = "tablepor_no_bootstrap".into();
    req.password = "local-no-bootstrap".into();
    req.manual_edits = true;
    assert_eq!(
        app.connect(req.clone()).await.unwrap_err().code,
        "DATABASE_REQUIRED"
    );
    req.database = "tablepor_other".into();
    let info = app.connect(req).await.unwrap();
    app.disconnect(&info.id).await.unwrap();
    let mut req = request();
    req.database.clear();
    req.password = "incorrect-fixture-password".into();
    assert_eq!(
        app.connect(req).await.unwrap_err().code,
        "CONNECTION_FAILED"
    );
}
