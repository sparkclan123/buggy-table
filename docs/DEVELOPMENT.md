# Development notes

## Principles

1. Prefer a readable function and explicit types over a framework. Split modules when there is a real boundary.
2. The frontend cannot open a database, change credentials on an existing session or execute arbitrary Tauri commands.
3. Inputs from the UI and database are untrusted. Never render their HTML, launch URLs or log their raw contents.
4. Human cell edits and staged row deletions use only the typed manual-save endpoint. SQL cannot execute writes. Future AI must use a separate strict-reader session and must never receive the manual-save tool or an editor session ID.
5. Keep the sample workspace honest: its data is fictional, metrics are not simulated, SQL requires a real desktop connection.

## Deliberate differences from the long-term plan

- One Rust crate and plain modules; no speculative plugin host, dynamic loading or trait hierarchy.
- `tokio-postgres` replaces the tentative SQLx choice for the first PostgreSQL adapter. Native cancellation and simple-query text streaming keep dynamic values lossless without a large decoder.
- PostgreSQL keeps one reusable read transport per connected workspace (maximum eight workspaces). Successful reads consume their result completely and leave no open transaction. Error/cancelled/truncated-by-byte-limit transports are discarded. Exact counts and saves use separate, non-reusable connections; save triggers cannot contaminate a read session. MySQL/Redis still use their existing dedicated transports.
- Password-free local bookmarks plus optional platform credential storage (Mac login Keychain / Windows Local persistence). The Rust password holder zeroizes its owned buffer on drop, but driver/OS/JS copies and process memory are not claimed to be perfectly erased.
- 100 rows by default, selectable 50/100/250/500 per browser page; raw SELECT results stop after 1,000 rows or 4 MiB. The UI receives one bounded result, not a complete streaming IPC protocol yet.
- CodeMirror loads only when the SQL tab is opened.
- Monitoring is manual refresh, not a background collector. No history is persisted.
- Local AI remains unavailable. MySQL/Redis are read-only adapters with their own policy and integration fixtures; see SECURITY_REVIEW.md.

## PostgreSQL security boundary

PostgreSQL 15–18 are covered by the same integration/security/cancellation suite in the `postgres-compatibility` CI matrix. PostgreSQL 15 is the minimum; there is no upper version gate. Newer majors can attempt a connection and receive a visible compatibility notice, not a guarantee of support. Catalog validation, strict-reader checks, SQL policy and transaction restrictions are still enforced on every version. Tables with virtual or unknown generated-column kinds are excluded from the approved scope; stored generated and identity columns remain non-editable.

To validate a future stable major, add it to `.github/workflows/check.yml`, run the suite below, inspect catalog/protocol changes and then update the tested-version notice and documentation. Do not mark a version tested based only on a successful connection.

PostgreSQL read transports set `default_transaction_read_only=on`, fixed `search_path=pg_catalog`, `standard_conforming_strings=on`, 10-second statement timeout, 1.5-second lock timeout and 10-second idle-in-transaction timeout at startup. Approved SELECTs and fixed catalog reads use PostgreSQL implicit read-only transactions: there is no separate BEGIN/ROLLBACK network exchange and no idle transaction retaining locks or snapshots. SQL policy rejects session commands, functions and indirect reads. Counts start an explicit read-only transaction with a local five-second statement timeout on a dedicated connection. Saves alone start an explicit read-write transaction on a dedicated connection; neither path returns that transport to the read slot. Application deadlines cover validation/execution and cancellation closes the active transport. A cancel request is not presented as proof the server has already finished stopping work. See [PostgreSQL implicit transactions](https://www.postgresql.org/docs/current/tutorial-transactions.html) and [transaction defaults](https://www.postgresql.org/docs/current/runtime-config-client.html).

With manual editing disabled, strict-reader preflight rejects elevated role attributes, role memberships, database CREATE/TEMP, schema CREATE, application relation write privileges, column-level writes, sequence mutation privileges and executable non-system routines. Public grants count. The PostgreSQL `pg_settings` system view is excluded from the relation-write check because PostgreSQL exposes session-setting updates there to ordinary roles; user SQL cannot access this view or execute SET.

The application does not modify grants. A DBA must provision a dedicated role for strict-reader mode; broad default PUBLIC privileges can require deliberate server configuration. Do not blindly copy fixture revocations to a shared production database. The fixture shows a self-contained setup for an empty, disposable database only.

Catalog discovery lists ordinary/partitioned/foreign tables, views and materialized views in an accessible schema, including objects without SELECT privilege. `readBlockedReason` separates discovery from execution: structure remains inspectable while data/SQL/save paths require a freshly approved relation. Views, RLS, foreign/inherited/partitioned parents, virtual generated columns and unsupported domain/composite/extension types remain blocked with an explicit reason. Ordinary tables and materialized views support the scalar allowlist, JSON/JSONB, bytea, enums and built-in arrays of supported scalars/enums. Materialized views are read-only. Enum/array casts use quoted catalog-derived type namespaces, never a user-provided SQL type expression.

Browse/count/save and SQL with one table validate only the referenced relation, reducing metadata transfer on large schemas. SQL is parsed to identify its relation, then fully approved against fresh metadata before execution; the lookup hint never authorizes SQL. Literal-only SELECTs do not fetch a schema catalog. Schema discovery uses three bounded, typed queries (schema permission, relation list, scoped columns), independent of table count. These independent catalog reads and strict-reader checks are [pipelined](https://docs.rs/tokio-postgres/latest/tokio_postgres/) on the same connection; user SQL is sent only after all checks succeed. Limits are 5,000 relations and 50,000 columns; exceeding either returns an explicit error rather than a partial list. Privileges and schema are rechecked on every operation, with no TTL authorization cache. Schema selection reloads the list; Refresh schema explicitly discovers new/removed tables and preserves unsaved drafts.

PostgreSQL `elapsedMs` is the total backend read operation, including connection startup, validation/catalog reads, and query transfer. Optional `timings` separates those phases, records connection reuse and includes local cleanup. Server version is cached only for the lifetime of its physical connection; permissions, relation metadata and query results are not cached. A dead idle socket may reconnect once during fixed pre-execution catalog checks; an already-sent user SELECT or save is never replayed. Disconnect closes the idle transport. Transport recovery shares the existing 15-second validation/execution deadline. This is not server-only execution time and excludes IPC serialization and rendering. Typed extended-protocol queries avoid separate prepare/execute/close round trips for fixed catalog commands and bound browse filters. Pagination uses LIMIT page_size + 1; the sentinel row does not need a new cancellation connection. Cancellation remains enabled for byte caps, unbounded SQL row caps, deadlines and user cancellation.

`examples/benchmark_postgres.rs` measures the complete Workbench query/browse path against the disposable Compose fixture. It creates/removes a unique synthetic schema, returns 200 SQL rows / 50 browse rows, discards one warmup and reports five samples by default, including phase medians and actual physical connection counts. `BUGGY_BENCH_SAMPLES` changes the sample count. `BUGGY_BENCH_TABLES` varies schema size; `BUGGY_BENCH_DELAY_MS` adds delay to each forwarded TCP chunk in each direction. This is a reproducible latency experiment, not a production WAN/TLS or UI-render benchmark. Run with `BUGGY_BENCH_TABLES=100 BUGGY_BENCH_DELAY_MS=10 cargo run --release --manifest-path src-tauri/Cargo.toml --no-default-features --example benchmark_postgres` after starting the PostgreSQL fixture.

JSONB filters/conflicts use native equality. JSON and JSON[] have no native equality, so UI comparisons/sorting and optimistic conflict checks use their exact text representation, including whitespace and duplicate keys; raw SQL retains PostgreSQL semantics. Contains searches the text representation with a bound, escaped pattern. Arrays/enums keep their database ordering for typed comparison/sorting.

SQL policy parses PostgreSQL AST, recursively checks expressions and reconstructs a canonical query from supported clauses. The canonical form must match the parsed input, so extra clauses cannot be silently ignored. Comments and redundant syntax may be normalized; `LIMIT ALL` is equivalent to an omitted limit, and the result cap still applies. The exact original SQL is never sent through the SQL editor path.

Supported: one SELECT, explicit schema-qualified base table, plain columns/literals, basic comparisons/arithmetic/boolean expressions, LIKE/ILIKE, IN, BETWEEN, NULL checks, ORDER BY, numeric LIMIT/OFFSET. Rejected: joins, CTEs, unions, functions, casts, locks, custom operators, dynamic SQL, COPY and all administration/writes. Estimated EXPLAIN is added by the backend after validation; actual EXPLAIN ANALYZE is unavailable.

The filter builder is a separate, narrow typed compiler: column names and types come from the same approved catalog; values are bound parameters, never string-interpolated. A maximum of 20 conditions is combined with a typed all/any (AND/OR) choice; NULL checks consume no bind slot. Filtering and sorting run in PostgreSQL before LIMIT/OFFSET, with primary-key tie breakers and NULLS LAST. Text containment escapes `%`, `_` and backslash. Its internal built-in casts are not exposed as a generic SQL escape hatch.

## Manual table editing

`manualEdits` is an explicit connection choice (UI defaults on; missing API field defaults off). Human sessions accept normal database accounts without the strict-reader role audit. SELECT parsing, approved-table scope, read-only transactions, TLS and resource limits still apply to reads. Database grants remain authoritative. A reader account in manual mode has no editable cells.

`save_table_changes` accepts the table name, full original row and changed column/value pairs, never SQL. Catalog metadata controls identifiers, types and column UPDATE privileges. Values use bound text parameters with built-in casts. Primary keys are required and immutable, including composite keys; generated/identity columns are not editable. `UPDATE ONLY` and `DELETE FROM ONLY` exclude inherited descendants. A row with `delete: true` must have an empty changes map and fresh table DELETE permission. Deletes and edits share one atomic transaction; all original-column conflict checks apply to both. ACCESS SHARE prevents structural DDL racing column checks without requiring table-level UPDATE for column-grant users. UPDATE acquires its usual write locks. References: [PostgreSQL UPDATE](https://www.postgresql.org/docs/17/sql-update.html), [explicit locking](https://www.postgresql.org/docs/17/explicit-locking.html).

All original columns participate in `IS NOT DISTINCT FROM` predicates, including NULL. Exactly one matched row is required. Conflict, invalid value, revoked permission or constraint error rolls back the entire transaction. This detects different current values, not an ABA change that returned every field to its original value. Saves are capped at 100 rows, 500 changed cells and 4 MiB. Reads/saves share the one-operation-per-connection gate; saves have a 20-second application deadline plus server statement/lock timeouts.

A lost COMMIT response cannot prove rollback. Cancellation/disconnect/deadline during save returns `SAVE_UNCERTAIN`; the frontend retains drafts, disables editing/resubmission and requires discard/reload. No automatic retry. Successful commits clear drafts before refreshing, so a failed refresh cannot resubmit an acknowledged save. Constraints, triggers and rules still run for explicit human UPDATE/DELETE; their external/nontransactional side effects cannot be undone by this client. Do not promise that manual writes have no indirect server-side effects.

Each table tab owns filter drafts/applied conditions, match mode, sort direction, column widths, page, scroll, results, SQL, indexes and edits in `workspace.ts`. Switching tabs retains drafts. Inline editors write only to drafts on input; Enter/blur ends editing, Escape restores the value when that cell was opened, and Shift+Enter inserts a newline. Acknowledged save/new results close the active editor. Cmd/Ctrl+R and F5 refresh only the current Data view; Cmd/Ctrl+S saves including a focused editor. Modal dialogs suppress global shortcuts. Refresh/filter/sort/page changes are blocked while drafts exist. Closing dirty tabs asks Save/Discard/Keep editing; connection changes and native close requests are blocked until edits are resolved. Force quit, OS shutdown or a crash can lose in-memory drafts. Sample saves affect only fictional in-memory data and are labeled accordingly.

## Limits that remain

- This is a preview, not a proof of non-interference or a production security certification. The database administrator, OS, app binary and native dependencies remain trusted.
- DBA changes to functions, operators, privileges or schema can race metadata checks. The app cannot defend against a malicious database administrator or compromised device.
- Row/byte caps bound retained results, not arbitrary server work or the driver's allocation of one oversized protocol message/field. Network-level frame limits and worker isolation remain hardening work. A single huge cell can exceed the UI/result budget before it is discarded.
- Ordinary reads affect internal counters/caches and can acquire read locks. “Read-only” means no application data/schema mutation commands.
- TLS defaults to system certificate trust and hostname checks. PostgreSQL can add a CA PEM bundle (saved only with a remembered connection) or explicitly select `require` (encrypted, certificate/hostname not verified). Both TLS modes reject servers without TLS; there is no plaintext fallback. MySQL accepts the same private CA PEM input for verified TLS. MySQL/Redis reject PostgreSQL-only unverified TLS at the backend. A local plaintext connection requires a literal loopback IP; `localhost` is intentionally insufficient.
- No application telemetry/update/AI endpoints exist, but there is no OS-wide outbound firewall policy in this preview. Browser request tests do not certify all native/OS network traffic.
- SQL/history/results are not persisted. Connection metadata/tab identities use atomic local JSON writes; opt-in passwords use the OS credential store. OS swap, crash handling, backups, clipboard and malicious local processes are outside that claim.
- Pagination is OFFSET-based and not a cross-page snapshot. Concurrent database changes can cause skipped/repeated rows. Primary-key ordering is used where available; tables without a primary key have no stable paging guarantee.
- Index scan counters are cumulative, can reset and are not grounds for deleting an index. Activity visibility is limited by the role. No claim to capture every database query.

## Adding a feature

1. Define a small request/result DTO in `models.rs` and its TypeScript counterpart in `src/lib/types.ts`.
2. Put orchestration in `service.rs`; keep Tauri handlers thin. Register the command in `build.rs`, `commands.rs`, `lib.rs` and the explicit main-window capability.
3. Validate inputs in Rust. Use only fixed SQL with bound parameters or the SELECT policy path.
4. Add tests that exercise real failure modes: privilege bypass, identifier/value injection, cancellation, resource bounds or result precision.
5. Add UI in a focused component. Preserve an understandable unavailable state rather than inventing data.
6. Run formatting, checks, relevant tests and a native smoke test when IPC/platform behavior changes.

Sessions now use an explicit Postgres/Mysql/Redis enum. Shared job limits and cancellation remain in service.rs; each adapter owns protocol behavior. Redis has a separate typed command endpoint/UI instead of pretending keys are SQL tables. No dynamic plugin system or blanket raw-command method exists. Security policy and credential ownership stay in the mandatory core.

## Test fixture

`compose.yaml` binds PostgreSQL/MySQL/Redis only to loopback ports 15432/13306/16379. PostgreSQL/MySQL use tmpfs; Redis persistence is disabled. All data and accounts are disposable fixtures. The committed credentials are fixture credentials, not deployment secrets. Integration tests deliberately grant/revoke permissions and acquire locks on these fixture tables. They must never be pointed at another database.

Run a specific PostgreSQL version from the repository root (requires Docker and Rust):

```sh
export BUGGY_TEST_POSTGRES_VERSION=18
export BUGGY_TEST_POSTGRES_PORT=25432
docker compose -p buggy-pg-compat -f tests/compose.postgres-compat.yml up -d --wait
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --test postgres_integration -- --include-ignored --test-threads=1
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --lib encrypted_connection_authenticates_and_queries_through_tls -- --ignored
docker compose -p buggy-pg-compat -f tests/compose.postgres-compat.yml down
```

Use 15, 16, 17 or 18. Bring the fixture down before changing versions. This separate Compose project does not replace the default development database. The test verifies the actual server major matches the requested matrix entry. `PGDATA` is explicit for compatibility with the PostgreSQL 18 image layout. New fixture tables require recreating an existing disposable test database, never applying the fixture to a real database.

Read-only and mutation tests operate at two levels: application policy and the server role directly without the application. Cancellation is tested with a blocked query, and precision tests cover bigint beyond JavaScript's integer range, exact decimal strings and NULL.

For native release testing, open the packaged app, connect to the fixture, browse, filter, execute SELECT, inspect indexes, open the overview and disconnect. Confirm the sample banner disappears when connected and the UI never falls back to fictional data on errors.

## Compact UI and resource use

The result grid has neutral text and fixed 28 px rows. Column widths (72-800 px) use colgroup/table-layout:fixed, pointer capture for drag resizing, keyboard arrow controls and double-click reset. One ResizeObserver determines the visible row range plus overscan. Column metadata is indexed once per table instead of searched for each rendered cell. No grid library or new runtime dependency was added. The old modal cell editor and duplicate/decorative CSS were removed. CSS asset size is measured in validation; this is not a measured RAM reduction. Actual memory remains dependent on WebView, loaded results and open table tabs.

## Additional database adapters

See [SECURITY_REVIEW.md](SECURITY_REVIEW.md) for MySQL/Redis boundaries, dependency findings and residual limits. MySQL uses mysql_async with native TLS and prepared filter values; numeric/decimal values remain strings. Redis uses redis-rs with an 8 MiB wire-budget wrapper, explicit driver-task ownership and no automatic redirect/reconnect. MySQL normalizes its session SQL mode and requires direct read grants. These engines reject manual writes regardless of IPC flags.

UI tests start a production preview on port 1420 (close any dev server on that port first). This tests the bundled lazy SQL editor without Vite dev dependency reloads. Failure traces are uploaded by CI.


## Connection workspace ownership

`connections.ts` owns one UI workspace per backend session/bookmark; `workspace.ts` keys tabs by JSON `[schema, table]`. Schema travels with every browse/query/index/save request. PostgreSQL rechecks role privileges and approved tables for that schema on each operation. Switching never mutates a backend session's credentials or disconnects another session.

`profiles.rs` owns a versioned, bounded metadata format, atomic replacement and device-local vault adapters. `connection_commands.rs` keeps native credential I/O off the async executor and never returns a stored secret to JavaScript. Reconnect is explicit. UI layout writes serialize per bookmark. Settings restore table identities lazily; draft cells, filters, SQL text and results stay in memory only. Do not add them to `ProfileLayout`.

The OS credential roundtrip test is opt-in because it writes/deletes a disposable credential: `cargo test --manifest-path src-tauri/Cargo.toml --lib local_credential_roundtrip -- --ignored`. Run against a normal unlocked login session. Windows CI additionally asserts Local persistence; there is no mock credential-store fallback in production.

## Grid controls and explicit counts

The default browse page has 100 rows; Rust validates 50/100/250/500 and caps OFFSET at 100,000. Result bytes still stop at 4 MiB. When this byte cap cuts a page short, Next is disabled to avoid silently skipping unread rows. Full values changes wrapping only; it never removes the byte/row caps. Variable-height virtualization measures visible rows and retains overscan; the default remains fixed 28 px rows. The first row-number column stays sticky while horizontal scrolling.

Catalog estimates render with the first page. Only Count exact calls `count_table_rows`, using the same table/filters, bound values, fresh role validation, operation gate and cancellation. A 5-second server statement timeout bounds counting; connection/validation time is additional. A completed count is labeled as a snapshot, cleared on filter/save changes, and is never a paging prerequisite.

Cell drafts have a theme-aware background, with pending deletion taking priority over edits and row selection. The first cell activation selects the whole row, including immutable/read-only cells; another activation in an already-selected row opens an eligible editor. Enter follows the same two-step path; Space and ordinary typing do not start editing. Cell/row-selector clicks explicitly focus their button on macOS WebKit. Delete/Backspace stages deletion from a focused selected cell or row selector, never from an editor or inspector. Cmd/Ctrl+click toggles multiple rows and Shift selects a range. Restore deleted rows keeps any cell drafts; Discard removes all drafts. Native Cmd/Ctrl+W is routed through a custom Tauri menu item so macOS does not close the whole window. Save/Discard/Keep editing guards apply to deleted rows too.

The read-only Row details inspector uses the selected row already in memory, including staged edits. It never issues a query and formats only the inspected row. The default is enabled with 10% of the table area; its edge supports pointer capture, Left/Right resizing, Home/double-click reset and End expansion (8–60%). Closing hides it until the next row selection; the checkbox beside Full values disables it persistently. Only enabled/widthPercent are stored in `buggy-table.row-inspector.v1`; defaults do not write storage. Long values wrap and scroll without additional truncation. JSON/JSONB formatting validates syntax then indents original tokens, preserving numeric lexemes, duplicate keys, order and escapes. Other types remain exact database strings, including timestamp timezone/fractions, arrays and numerics. NULL, empty text and JSON null remain distinct. Database HTML is always rendered as text.

Header drag and Alt+Left/Right reorder displayed columns without changing query/result indices. LocalStorage contains only bounded column-order preferences keyed by connection/table identity (maximum 200 layouts); no row values, SQL or secrets. Timestamp editing/filtering uses free text plus a native date/time dialog. String composition preserves the displayed UTC offset and up to six fractional digits instead of converting through the computer's timezone.

Overview graphs retain up to 30 manual snapshots per connection in memory; no polling or persisted metric history. Unknown metrics display unavailable. Redis INFO is parsed separately for server/memory/stats/clients/keyspace; denied INFO does not draw zero graphs. Redis SCAN results are virtualized records, accumulated/deduplicated across cursors, with an explicit 10,000-key display limit. An empty batch with nonzero cursor still allows Next scan. Binary key names appear as hex records with text inspection disabled.


### Table appearance and tab arrangement (0.1.9)

Grid stripes follow the absolute row index so virtual scrolling does not flip their parity. Row details alternates complete fields. Edit/delete colors override stripes and selection in both themes. Table tabs support pointer capture with a lifted preview, insertion marker, edge scrolling and cancellation; Alt+Left/Right provides a keyboard alternative. Reordering existing workspace objects keeps drafts/results/selection/scroll and persists only identities through the existing per-connection layout writer. The tab scrollbar is 3 px on supported desktop WebViews.

The Updates button invokes `open_download_page`, whose Rust implementation opens a fixed HTTPS GitHub releases URL with the [Tauri opener](https://v2.tauri.app/plugin/opener/). No URL or application argument is accepted from the WebView. Generic opener commands and JavaScript link interception are not enabled. Signed builds retain separate Update options; previews link to downloads and require manual replacement.
