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
        schema: None,
        connection_id: connection_id.into(),
        request_id: Uuid::new_v4().to_string(),
        sql: sql.into(),
        explain: false,
    }
}

fn browse(connection_id: &str) -> BrowseRequest {
    BrowseRequest {
        schema: None,
        connection_id: connection_id.into(),
        request_id: Uuid::new_v4().to_string(),
        table: "orders".into(),
        filters: vec![],
        filter_match: FilterMatch::All,
        sort_column: None,
        descending: false,
        page: 0,
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
    assert!(!info.tables.iter().any(|t| {
        [
            "orders_view",
            "rls_table",
            "custom_payload",
            "virtual_items",
        ]
        .contains(&t.name.as_str())
    }));
    let page = app.browse(browse(&info.id)).await.unwrap();
    assert_eq!(page.rows.len(), 200);
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
        schema: None,
        table: table.into(),
        ..browse(id)
    })
    .await
    .unwrap()
}
fn edit(result: &QueryResult, index: usize, column: &str, value: Option<&str>) -> RowChange {
    RowChange {
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
    assert_eq!(app.browse(r).await.unwrap().rows.len(), 156);
    let mut r = browse(&info.id);
    r.sort_column = Some("total".into());
    r.descending = true;
    r.page = 1;
    let second = app.browse(r).await.unwrap();
    assert_eq!(second.rows[0][0].as_deref(), Some("250"));
    assert_eq!(second.rows[199][0].as_deref(), Some("51"));
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
        assert!(!info.tables.iter().any(|t| t.name == "virtual_items"));
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
        connection_id: first.id.clone(),
        request_id: Uuid::new_v4().to_string(),
        schema: Some("audit".into()),
        table: "edit_items".into(),
        rows: vec![RowChange {
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
        connection_id: first.id.clone(),
        request_id: Uuid::new_v4().to_string(),
        schema: Some("audit".into()),
        table: "edit_items".into(),
        rows: vec![RowChange {
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
