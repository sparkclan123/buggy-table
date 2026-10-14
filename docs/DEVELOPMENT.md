# Development notes

## Principles

1. Prefer a readable function and explicit types over a framework. Split modules when there is a real boundary.
2. The frontend cannot open a database, change credentials on an existing session or execute arbitrary Tauri commands.
3. Inputs from the UI and database are untrusted. Never render their HTML, launch URLs or log their raw contents.
4. Human cell edits use only the typed manual-save endpoint. SQL cannot execute writes. Future AI must use a separate strict-reader session and must never receive the manual-save tool or an editor session ID.
5. Keep the sample workspace honest: its data is fictional, metrics are not simulated, SQL requires a real desktop connection.

## Deliberate differences from the long-term plan

- One Rust crate and plain modules; no speculative plugin host, dynamic loading or trait hierarchy.
- `tokio-postgres` replaces the tentative SQLx choice for the first PostgreSQL adapter. Native cancellation and simple-query text streaming keep dynamic values lossless without a large decoder.
- A dedicated connection per operation avoids pool state leaks. This costs connection/preflight latency and is a measured trade-off to revisit after session-reset and cancellation tests exist for pooling.
- Session-only credentials rather than incomplete secret persistence. The Rust password holder zeroizes its owned buffer on drop, but driver/OS/JS copies and process memory are not claimed to be perfectly erased.
- 200 rows per browser page; raw SELECT results stop after 1,000 rows or 4 MiB. The UI receives one bounded result, not a complete streaming IPC protocol yet.
- CodeMirror loads only when the SQL tab is opened.
- Monitoring is manual refresh, not a background collector. No history is persisted.
- Local AI remains unavailable. MySQL/Redis are read-only adapters with their own policy and integration fixtures; see SECURITY_REVIEW.md.

## PostgreSQL security boundary

Only PostgreSQL 17 is enabled in this first build because it is the version exercised by the integration fixture. Expand the version gate only after running the same security/cancellation suite on the added version.

Queries use an isolated connection, `BEGIN READ ONLY`, fixed `search_path=pg_catalog`, `standard_conforming_strings=on`, 10-second statement timeout, 1.5-second lock timeout and 10-second idle-in-transaction timeout. Application deadlines cover connection/preflight/execution and cancellation closes the dedicated connection. A cancel request is not presented as proof the server has already finished stopping work.

With manual editing disabled, strict-reader preflight rejects elevated role attributes, role memberships, database CREATE/TEMP, schema CREATE, application relation write privileges, column-level writes, sequence mutation privileges and executable non-system routines. Public grants count. The PostgreSQL `pg_settings` system view is excluded from the relation-write check because PostgreSQL exposes session-setting updates there to ordinary roles; user SQL cannot access this view or execute SET.

The application does not modify grants. A DBA must provision a dedicated role for strict-reader mode; broad default PUBLIC privileges can require deliberate server configuration. Do not blindly copy fixture revocations to a shared production database. The fixture shows a self-contained setup for an empty, disposable database only.

Only ordinary base tables in the selected schema, readable in full by the role, with approved built-in scalar types are listed. Tables with RLS, inherited children under a parent, custom/JSON/array types, foreign tables, partitioned parents and views are excluded. A currently approved table can disappear after a schema change; reconnect to refresh the explorer.

SQL policy parses PostgreSQL AST, recursively checks expressions and reconstructs a canonical query from supported clauses. The canonical form must match the parsed input, so extra clauses cannot be silently ignored. Comments and redundant syntax may be normalized; `LIMIT ALL` is equivalent to an omitted limit, and the result cap still applies. The exact original SQL is never sent through the SQL editor path.

Supported: one SELECT, explicit schema-qualified base table, plain columns/literals, basic comparisons/arithmetic/boolean expressions, LIKE/ILIKE, IN, BETWEEN, NULL checks, ORDER BY, numeric LIMIT/OFFSET. Rejected: joins, CTEs, unions, functions, casts, locks, custom operators, dynamic SQL, COPY and all administration/writes. Estimated EXPLAIN is added by the backend after validation; actual EXPLAIN ANALYZE is unavailable.

The filter builder is a separate, narrow typed compiler: column names and types come from the same approved catalog; values are bound parameters, never string-interpolated. A maximum of 20 conditions is combined with a typed all/any (AND/OR) choice; NULL checks consume no bind slot. Filtering and sorting run in PostgreSQL before LIMIT/OFFSET, with primary-key tie breakers and NULLS LAST. Text containment escapes `%`, `_` and backslash. Its internal built-in casts are not exposed as a generic SQL escape hatch.

## Manual table editing

`manualEdits` is an explicit connection choice (UI defaults on; missing API field defaults off). Human sessions accept normal database accounts without the strict-reader role audit. SELECT parsing, approved-table scope, read-only transactions, TLS and resource limits still apply to reads. Database grants remain authoritative. A reader account in manual mode has no editable cells.

`save_table_changes` accepts the table name, full original row and changed column/value pairs, never SQL. Catalog metadata controls identifiers, types and column UPDATE privileges. Values use bound text parameters with built-in casts. Primary keys are required and immutable, including composite keys; generated/identity columns are not editable. `UPDATE ONLY` excludes inherited descendants. ACCESS SHARE prevents structural DDL racing column checks without requiring table-level UPDATE for column-grant users. UPDATE acquires its usual write locks. References: [PostgreSQL UPDATE](https://www.postgresql.org/docs/17/sql-update.html), [explicit locking](https://www.postgresql.org/docs/17/explicit-locking.html).

All original columns participate in `IS NOT DISTINCT FROM` predicates, including NULL. Exactly one matched row is required. Conflict, invalid value, revoked permission or constraint error rolls back the entire transaction. This detects different current values, not an ABA change that returned every field to its original value. Saves are capped at 100 rows, 500 changed cells and 4 MiB. Reads/saves share the one-operation-per-connection gate; saves have a 20-second application deadline plus server statement/lock timeouts.

A lost COMMIT response cannot prove rollback. Cancellation/disconnect/deadline during save returns `SAVE_UNCERTAIN`; the frontend retains drafts, disables editing/resubmission and requires discard/reload. No automatic retry. Successful commits clear drafts before refreshing, so a failed refresh cannot resubmit an acknowledged save. Constraints, triggers and rules still run for explicit human UPDATE; their external/nontransactional side effects cannot be undone by this client. Do not promise that manual writes have no indirect server-side effects.

Each table tab owns filter drafts/applied conditions, match mode, sort direction, column widths, page, scroll, results, SQL, indexes and edits in `workspace.ts`. Switching tabs retains drafts. Inline editors write only to drafts on input; Enter/blur ends editing, Escape restores the value when that cell was opened, and Shift+Enter inserts a newline. Acknowledged save/new results close the active editor. Cmd/Ctrl+R and F5 refresh only the current Data view; Cmd/Ctrl+S saves including a focused editor. Modal dialogs suppress global shortcuts. Refresh/filter/sort/page changes are blocked while drafts exist. Closing dirty tabs asks Save/Discard/Keep editing; connection changes and native close requests are blocked until edits are resolved. Force quit, OS shutdown or a crash can lose in-memory drafts. Sample saves affect only fictional in-memory data and are labeled accordingly.

## Limits that remain

- This is a preview, not a proof of non-interference or a production security certification. The database administrator, OS, app binary and native dependencies remain trusted.
- DBA changes to functions, operators, privileges or schema can race metadata checks. The app cannot defend against a malicious database administrator or compromised device.
- Row/byte caps bound retained results, not arbitrary server work or the driver's allocation of one oversized protocol message/field. Network-level frame limits and worker isolation remain hardening work. A single huge cell can exceed the UI/result budget before it is discarded.
- Ordinary reads affect internal counters/caches and can acquire read locks. “Read-only” means no application data/schema mutation commands.
- TLS defaults to system certificate trust and hostname checks. PostgreSQL can add a session-only CA PEM bundle or explicitly select `require` (encrypted, certificate/hostname not verified). Both TLS modes reject servers without TLS; there is no plaintext fallback. MySQL/Redis reject PostgreSQL-only TLS options at the backend. A local plaintext connection requires a literal loopback IP; `localhost` is intentionally insufficient.
- No application telemetry/update/AI endpoints exist, but there is no OS-wide outbound firewall policy in this preview. Browser request tests do not certify all native/OS network traffic.
- Credentials/history/results are not intentionally persisted by application code. OS swap, crash handling, backups, clipboard and malicious local processes are outside that claim.
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

Read-only and mutation tests operate at two levels: application policy and the server role directly without the application. Cancellation is tested with a blocked query, and precision tests cover bigint beyond JavaScript's integer range, exact decimal strings and NULL.

For native release testing, open the packaged app, connect to the fixture, browse, filter, execute SELECT, inspect indexes, open the overview and disconnect. Confirm the sample banner disappears when connected and the UI never falls back to fictional data on errors.

## Compact UI and resource use

The result grid has neutral text and fixed 28 px rows. Column widths (72-800 px) use colgroup/table-layout:fixed, pointer capture for drag resizing, keyboard arrow controls and double-click reset. One ResizeObserver determines the visible row range plus overscan. Column metadata is indexed once per table instead of searched for each rendered cell. No grid library or new runtime dependency was added. The old modal cell editor and duplicate/decorative CSS were removed. CSS asset size is measured in validation; this is not a measured RAM reduction. Actual memory remains dependent on WebView, loaded results and open table tabs.

## Additional database adapters

See [SECURITY_REVIEW.md](SECURITY_REVIEW.md) for MySQL/Redis boundaries, dependency findings and residual limits. MySQL uses mysql_async with native TLS and prepared filter values; numeric/decimal values remain strings. Redis uses redis-rs with an 8 MiB wire-budget wrapper, explicit driver-task ownership and no automatic redirect/reconnect. MySQL normalizes its session SQL mode and requires direct read grants. These engines reject manual writes regardless of IPC flags.

UI tests start a production preview on port 1420 (close any dev server on that port first). This tests the bundled lazy SQL editor without Vite dev dependency reloads. Failure traces are uploaded by CI.
