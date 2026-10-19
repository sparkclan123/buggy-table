# Validation history

## Grid and issue fixes — 0.1.7 Preview, 2026-10-19

- Open source issues #3–#6 were read and addressed in code: actionable MySQL connection errors/private CA, PostgreSQL/MySQL snapshot graphs, Redis INFO graphs, and Redis SCAN key records. No issue comments or closures were posted.
- Playwright: **84 passed**, Chromium and WebKit. New coverage includes page sizes 50/100/250/500, explicit counts without automatic count requests, symbol filters, typed/calendar timestamp edits and filters with microseconds/offset preservation, local column drag/keyboard reorder with correct cell ownership, highlighted drafts, staged deletion/restore/save, Cmd+W dirty guards, collapsed controls, full values with variable-height virtualization, sticky row numbers, Redis cursor accumulation/deduplication/search/command scans and INFO permission errors. Database/Redis browser tests use explicit IPC doubles; real adapters are tested below.
- PostgreSQL integration: **26 passed on each of 15, 16, 17 and 18** against separate disposable loopback fixtures. New tests verify page boundaries, invalid page limits, exact filtered counts, mixed delete/update atomic commit, rollback after stale-row conflicts, revoked DELETE permission and reader rejection. Existing catalog completeness, typed JSON/arrays/enums, precision, cancellation and schema isolation checks still pass.
- Multi-engine integration: **10 passed** against MySQL 8.4 and Redis 8. MySQL tests include bound exact counts, real authentication/network/untrusted-TLS errors and a successful private-CA TLS handshake/authentication/catalog read through a loopback TLS front end to the disposable MySQL fixture. No public or production database was contacted.
- Frontend unit tests: **5 passed**. Rust policy/unit/transport checks and Clippy pass. Svelte/TypeScript reports no errors/warnings; Prettier, cargo fmt and git diff whitespace checks pass.
- Native macOS ARM64 debug bundle: connected through actual Tauri IPC to the local PostgreSQL fixture without remembering credentials/settings. Count exact returned a real count; Cmd+W showed Save/Discard/Keep editing for a draft, Discard & close removed that tab, and another Cmd+W closed the remaining tab while keeping the app window open. Test connection was disconnected and the app quit. No production bookmark was connected.
- Light/dark graph screenshots and the wrapped/sticky grid were inspected. Numeric graph labels use grouping; unknown metrics remain unavailable rather than zero.
- Final color checks cover actual computed cell backgrounds: selected pending deletions override selection blue in both themes, and active cell inputs retain the edit highlight. Focused grid/appearance tests were rerun after this CSS correction.
- macOS ARM64 release-mode `.app` built successfully (15.81 MiB) and passed `codesign --verify --deep --strict`. This is an ad-hoc-signed local build, not a notarized release or a published updater artifact. This pre-release validation build used version 0.1.6; the distribution version is 0.1.7. Final installer provenance and CI results are recorded in the release BUILD_INFO.txt.

Performance experiment: release-mode backend, 100 synthetic tables, one warmup then five samples, proxy adding 10 ms per forwarded TCP chunk in each direction. Values include connection/validation/transfer, excluding UI rendering. Browse uses the new default 50 rows; SQL returns 200 rows. These are controlled loopback measurements, not production-server guarantees or a general performance certification.

| Mode | Operation | Rows | Median | P95 |
| --- | --- | ---: | ---: | ---: |
| Manual | Browse | 50 | 242.0 ms | 247.0 ms |
| Manual | SELECT | 200 | 258.8 ms | 260.1 ms |
| Strict reader | Browse | 50 | 325.9 ms | 330.5 ms |
| Strict reader | SELECT | 200 | 341.5 ms | 342.4 ms |

Browse/count/save now fetch metadata only for their selected relation. Count exact is opt-in with a five-second server query timeout; it is never part of initial page loading. OFFSET paging and 4 MiB result limits remain; full values changes display wrapping only. Windows native execution and public-trust production endpoints were not exercised in this update.

Environment: macOS ARM64, Rust 1.99.0, Node 22.22.2. Database tests use the disposable PostgreSQL 17 Compose fixture.

## Automated checks

- Rust policy tests: allowed SELECT subset and denied destructive/indirect SQL corpus.
- PostgreSQL integration: query/browse, multiple bound AND/OR filters, NULL mixed with numbered binds, numeric sorting before pagination, stable key tie breakers, filter count/value limits, identifier rejection, literal bound filters, precise values, NULL, empty-result columns, estimated plan, indexes and overview.
- Security integration: strict-reader mode rejects admin credentials, reject mutation SQL, verify the server role denies writes even without the app policy, detect privilege drift and cancel a blocked query.
- Manual-save integration: persisted literal/Unicode text and NULL, composite keys, preserved decimals, immutable/generated columns, conflict and constraint-error atomic rollback, column permission revocation, cancellation without retry, read-only role enforcement and no SQL write bypass.
- Frontend: strict TypeScript/Svelte checks and Vitest tests for sample filtering/paging and safe display helpers.
- Playwright: Chromium and WebKit; sample paging/filtering, multi-table state, Save/Discard, inline edits/Escape, NULL edits, keyboard save/refresh without document reload, pointer/keyboard column resizing with per-tab widths, full-dataset sorting, multiple filters, dirty-tab close choices, honest unavailable features, password clearing/no browser storage, modal behavior, layout screenshot and no unexpected browser requests on the tested sample flow.
- Rust formatting and Clippy with warnings treated as errors.

## Scope of evidence

The tests are specific to this preview, not certification of every PostgreSQL feature. Native macOS packaging/smoke-test status and any remaining issues are recorded below after validation. Windows packaging is configured but has not been executed on this Mac. No production database or real credentials were used.

Performance budgets in `DATABASE_TOOL_PLAN.md` remain targets. Bundle size alone does not establish startup latency, idle memory or query overhead. Full network/OS egress verification, fuzzing, long-running soak tests, signed distribution and a Windows native run remain future release work.

## MySQL / Redis update — 2026-10-14

- 61 tests passed: 6 Rust policy/resource tests, 18 PostgreSQL integration tests, 8 multi-engine transport/security/integration tests, 3 frontend unit tests and 26 Chromium/WebKit UI tests.
- MySQL fixture: version 8.4, direct SELECT-only role, browse/SQL/EXPLAIN, structure/indexes/overview, exact decimals, bound filters, sort, empty-result columns, denied functions/mutations/indirection, server-enforced write denial and privilege-drift rejection.
- Redis fixture: Redis 8, SCAN, strings/hash, TTL, binary-as-hex, bounded GET, nonexistent keys, denied writes even under an admin fixture account, rejected cross-engine APIs and an oversized hash response interrupted at the wire budget. PING still works after that rejection.
- Both new adapters reject remote plaintext and manual-write flags. Verified TLS rejects the local untrusted/non-TLS fixtures without downgrade. A successful connection to a public-trust TLS endpoint was not tested.
- Browser tests include engine selection/default ports, clearing passwords, readonly mode, Redis key inspection/TTL, text-safe rendering, command rejection, refresh shortcuts and INFO navigation. Redis browser tests use a labeled IPC stub; real server coverage is in Rust and the native smoke test.
- Native macOS ARM64 smoke test through actual Tauri IPC: selected MySQL, connected to the isolated reader fixture, opened items and executed the generated backtick-quoted SELECT with exact decimal output. Switched to Redis, scanned 6 fixture keys, inspected fixture:hello with its type/TTL, ran PING → PONG, and opened INFO server from Database overview. No real credentials or production databases were used. The final text-only INFO display adjustment passed both browser engines.
- Release macOS `.app` built successfully (approximately 12.05 MiB). New adapters added no frontend runtime packages. CSS is approximately 14.58 kB minified; these sizes do not measure RAM. Windows native packaging/run remains unverified on this Mac.
- Svelte/TypeScript, formatting and Clippy checks pass. Runtime adapters add mysql_async and redis-rs; no new frontend runtime dependency.
- Dependency findings: `pnpm audit` reports zero advisories. `time` was updated to fix its advisory. Two quick-xml advisories remain through plist/Tauri; full details and reachability assessment are in [SECURITY_REVIEW.md](SECURITY_REVIEW.md). No audit suppression was added.

## Previous PostgreSQL UI build results

- Rust: 3 policy tests and 18 integration/connection/save tests passed (including the opt-in fixture tests).
- Frontend unit tests: 3 passed.
- Browser UI: 22 passed across Chromium and WebKit.
- Svelte/TypeScript: no errors or warnings. Clippy: no warnings. Rust/Prettier formatting checks passed.
- macOS ARM64 release `.app` bundle built successfully, approximately 10 MiB.
- Packaged macOS app: connected through the actual Tauri IPC to the disposable PostgreSQL fixture, opened customers/edit_items in separate tabs, preserved a pending edit across tab switches, blocked native window-close with pending work, saved `Native save verified`, refreshed, and confirmed the same persisted value independently through psql. Generated-column output updated too. No production data was used.
- Rebuilt compact UI native smoke test: opened customers/edit_items as separate tabs, toggled header sorting to descending, edited a cell directly inside the grid, saved `Native inline verified` with Command-S while the input was focused, and refreshed with Command-R without losing the connection or tabs. Independent psql output confirmed the persisted value and generated column. Added and applied two filter conditions through the native connection and inspected the resulting layout.
- Native automation initially entered only whitespace using synthetic typing; clipboard paste plus an explicit input-value check worked. The successful save evidence uses the inspected pasted value. Earlier ScreenCaptureKit access failures did not recur in this native run.
- Small-window UI coverage also verifies sidebar controls remain visible and per-table grid scroll survives tab switching.
- Initial frontend JavaScript is approximately 131.52 kB minified (44.50 kB gzip). The SQL editor is a separate lazy chunk, approximately 432.62 kB minified. Packaged CSS decreased from approximately 20.51 kB to 13.26 kB (3.33 kB gzip). These are asset sizes, not process-memory measurements.
- Windows CI build configuration was added; it has not been run here.

- A concurrent frontend production build and Vite development-server UI run produced two blank-page test failures with a Svelte module initialization error. Running the UI suite separately after the build passed all 12 then-current cases; do not run these operations simultaneously against the same Vite cache.

## Dark appearance, signed distribution and buggy-table branding — 2026-10-14

- Renamed the displayed app, package/binary, installer and release assets to `buggy-table`. Generated macOS `.icns` and Windows `.ico` from the user-supplied logo using Tauri's contain fit; preserved the complete image. Bundle ID and preference/key paths stay stable.
- Passed 38 UI cases across Chromium and WebKit, including dark/system preference persistence, inline controls/SQL editor, update offer/error/progress, disabled automatic checks, and dirty-tab installation guards. Native macOS dark appearance and real updater IPC were checked; the unpublished feed returns a handled error.
- Passed 7 Rust unit tests, 18 PostgreSQL and 8 multi-engine integration tests, 3 Vitest tests and 1 release-manifest test. Svelte/TypeScript, Clippy, rustfmt, Prettier and actionlint pass locally.
- Built local Mac ARM `.app`, `.dmg` and signed updater archive; validated DMG checksum and updater signature/public key/bound version. A corrupted copy was rejected with `InvalidSignature`. OS certificate/notarization and two-version installation are still pending owner-provided signing secrets.
- GitHub Windows checks exposed CRLF checkout differences; added `.gitattributes` to keep source text LF across platforms. Fresh CI runs validate the renamed installers.
- JavaScript dependency audit: 0 advisories. Cargo still reports the two existing quick-xml advisories documented in SECURITY_REVIEW.md; no advisory suppression.

## PostgreSQL version compatibility — 0.1.2, 2026-10-14

Separate loopback-only disposable Docker projects were used, leaving the existing development database in place. The integration test asserts the server major matches the requested matrix version.

| PostgreSQL server | Integration tests | Encrypted authentication/query proxy test |
| --- | --- | --- |
| 15.19 | 19 passed | 1 passed |
| 16.15 | 19 passed | 1 passed |
| 17.11 | 19 passed | 1 passed |
| 18.6 | 19 passed | 1 passed |

Coverage includes connect, schema/column discovery, browse/filter/sort/paging, exact values, SQL/EXPLAIN, indexes/status, save/rollback/conflict, privilege drift, cancellation/disconnect and plaintext rejection for TLS modes. The new test verifies generated and identity columns remain non-editable even with table-level UPDATE grants; PostgreSQL 18 virtual columns are excluded from browsing and SQL scope.

The upper version gate is removed. Unit coverage accepts 15–18 and future version numbers while rejecting older/malformed versions. Browser IPC stubs verify 15–18 connect without a notice and a hypothetical 19 shows an untested-compatibility notice. This is not a real PostgreSQL 19 compatibility test.

52 Chromium/WebKit UI tests, 12 Rust unit tests, 2 non-fixture integration validation tests, 3 frontend tests and the release-manifest test passed. Svelte/TypeScript, Clippy, rustfmt, Prettier, version consistency and actionlint passed. The new CI matrix repeats real database tests for 15–18. No production credentials or production database queries were used; the user's remote database has not been authenticated by this test run.

Native macOS ARM64 0.1.2 `.app` and `.dmg` built successfully (15.42 MiB / 8.27 MiB). DMG checksum verification passed. In the packaged Tauri app, connected to the isolated PostgreSQL 18.6 fixture, opened customers and edit_items as separate tabs, edited a cell inline, saved with Command-S, refreshed with Command-R and viewed the live server/status snapshot. Independent psql confirmed both the saved value and stored generated column. The smoke test used only disposable fixture credentials/data. Windows 0.1.2 installer/native execution is not validated locally.

## Saved connections and schema workspaces — 0.1.3, 2026-10-14

- 64 Chromium/WebKit cases pass, including 54-bookmark search, offline startup, restored tabs, same-name tables in two schemas, schema-bound saves, drafts surviving connection switches, global close protection, Redis inspection state and failed reconnect recovery. Browser cases use labeled IPC doubles.
- Real PostgreSQL 15/16/17/18: 20 integration tests pass on each version. New coverage validates schema USAGE, inaccessible-schema rejection, separate reads/saves for public/audit tables, and disconnecting one session without affecting another. Existing MySQL 8.4 / Redis 8 integration suite: 8 pass.
- 14 non-fixture Rust unit tests and 3 frontend unit tests pass. Local metadata roundtrip verifies restart, password exclusion, Unix file permissions and non-overwrite of corrupt files. An opt-in native Mac login Keychain test created/read/deleted one disposable credential successfully.
- Svelte/TypeScript, Clippy, formatting, version/release-manifest checks and actionlint pass. Windows CI includes a real credential roundtrip and an assertion of Local persistence; its result must be checked separately from local Mac evidence.
- No production credentials or production database queries were used. Password-free connection metadata is stored locally; query results/SQL/drafts are not persisted. OS backups, swap and other local processes are outside this guarantee.

Packaged Mac ARM64 smoke test: simultaneously connected to PostgreSQL 18, MySQL 8.4 and Redis 8 through actual Tauri IPC. Opened public/edit_items and audit/edit_items in separate tabs, retained an audit draft while switching to MySQL, then saved/refreshed it. Independent psql confirmed only audit changed. Redis key/type/TTL/value remained visible after switching away/back. Quit/reopened: all three bookmarks were offline, PostgreSQL reconnected using login Keychain with its audit tab restored, and MySQL (password not saved) reopened the prefilled form with an empty password. The local JSON contained neither fixture passwords nor saved/preview row values and had mode 0600. The manager's search receives keyboard focus immediately; dialogs share the app's theme styling. A regression test also ensures changing a bookmark's database/account clears stale catalog tabs while a name-only edit preserves them (15 non-fixture Rust unit tests total).

macOS ARM64 `.app` and `.dmg` build successfully (15.75 MiB / 8.39 MiB); DMG checksum verification passes. Windows/Intel native installer results are separate CI artifacts, not a claim of native execution on this Mac.


## Sidebar controls and public preview — 0.1.4, 2026-10-14

The PostgreSQL schema selector is now a native dropdown at the bottom of the sidebar. Theme preferences remain System/Light/Dark, accessed through a compact icon beside the app/version. The obsolete schema modal and styles were removed. Schema options load when connecting; query/save scope continues to belong to each table tab.

64 Chromium/WebKit UI cases pass, including real dropdown selection through IPC fixtures and persistent theme preferences. Native Mac 0.1.4 opens the theme menu from the footer icon. No production database was connected during this UI smoke test.

GitHub Check run 38496464089 passed every job: PostgreSQL 15–18, integration/UI, macOS and Windows. Windows Credential Manager roundtrip succeeded and asserted Local persistence. Installer run 38496464150 built Mac ARM64, Mac Intel and Windows x64 from b46a10861a4225ddf474daa98c9df99e30f9124e, tagged preview/v0.1.4 and merged into main through PR #2. Both downloaded DMGs pass checksum verification and contain version 0.1.4 with the expected arm64/x86_64 architecture. Windows installer build succeeds; interactive Windows installation has not been tested on this Mac.

Public distribution: https://github.com/sparkclan123/buggy-table-releases/releases/tag/v0.1.4. The release includes SHA-256 checksums and build provenance. It remains an unsigned preview with in-app updates disabled; no Apple/Azure signing credentials were supplied.

## macOS packaging fix 0.1.5 — 2026-10-14

- Reproduced 0.1.4 DMG failures: ARM `codesign --verify --deep --strict` reported `code has no resources but signature indicates they must be present`; Intel reported `code object is not signed at all`. DMG CRC itself passed. The earlier installer checks did not cover bundle signatures.
- 0.1.5 uses Tauri ad-hoc bundle signing; native ARM and cross-compiled Intel release builds completed locally. Both build outputs and mounted DMG apps passed strict/deep codesign validation with sealed resources, correct bundle ID/version and expected architecture.
- Regression checks: the new verifier rejects both old 0.1.4 apps; rejects a temporary copy with a modified Info.plist; signed-production mode rejects the ad-hoc preview.
- Native ARM fresh launch displayed v0.1.5 and existing disconnected profiles. No production DB connection was made. Intel interactive launch is untested (this ARM host has no Rosetta).
- `pnpm release:check`, frontend type check/build, `pnpm test` (3 tests), format checks, `bash -n`, actionlint and `git diff --check` passed. Database/UI integration suites were not rerun for this packaging-only change.
- Source commit `cf7d1bc206d3dfa97e6b7e4a30c55d2b9c0981bd`, tag `preview/v0.1.5`. GitHub Check run `38506595822` was blocked before execution by account payment/spending limits; it is not a passing CI run.
- Gatekeeper still rejects ad-hoc as untrusted: no Developer ID identity or notarization credentials are configured. This release does not claim automatic acceptance on other Macs. The standard per-app Open Anyway flow has not been tested on a clean second Mac. Managed-device policy may disallow it.
- Staged ARM DMG SHA-256: `e9006414e0c21e2572e4979ef688b5790ad8d12d07b88e9b7e99a0aeaf438976`; Intel: `cac0fe69b4a8d7aa7dafaedf513438f29adc96d502caa8085d2a916648469809`. All four draft asset digests matched local files before publishing. Windows remains on 0.1.4.

## Follow-up: Gatekeeper still blocks 0.1.5

- Downloaded the public ARM DMG again; SHA-256 matched `e9006414e0c21e2572e4979ef688b5790ad8d12d07b88e9b7e99a0aeaf438976`. Extracted to an isolated test directory and added quarantine to that copy only.
- Strict/deep codesign validation passes, but `syspolicy_check distribution --json` exits 70: `Adhoc Signed App` and fatal `Notary Ticket Missing`. `spctl --assess --type execute` exits 3 (`rejected`). This is not a distribution-ready release.
- GUI launch on the development Mac was inconclusive (automation timed out and a translocated process started). The exact other-machine dialog has not been reproduced on a clean Mac. Do not treat a developer-machine launch as a Gatekeeper acceptance test. No quarantine attributes were removed and no Gatekeeper settings were disabled.
- Signed-release verification now runs syspolicy_check when available, in addition to Developer ID authority, stapler and spctl checks. Signing configuration rejects ad-hoc/development identities before building.
- Release documentation now explicitly states that Open Anyway may be unavailable. A new signed release is blocked by the absence of Apple Developer ID credentials; rebuilding with ad-hoc signing cannot fulfill normal Mac distribution.

## PostgreSQL latency and complete schema discovery — 2026-10-19

The baseline at `22a14c1` reopened PostgreSQL on each operation and queried column metadata separately for every approved table. With 100 tables, catalog loading issued 102 SQL statements before the SELECT (plus preflight/startup). Each prepared catalog request also incurred protocol round trips. The displayed time omitted connection and catalog work: the baseline reported 37 ms for a read taking 6,593 ms overall.

The updated implementation batches all column metadata in one OID-array query, uses typed one-shot catalog/filter requests, and keeps every permission/type check fresh. Catalog statement count is three regardless of table count. PostgreSQL result timing now includes connection/check/query phases. A LIMIT-201 pagination sentinel does not open another cancellation connection.

Measured release builds on this Mac, PostgreSQL 17 Docker fixture, synthetic 100-table schema, 200 returned rows, one discarded warmup followed by five samples per case. The local TCP proxy delays each forwarded chunk by 10 ms per direction; it is an approximate network-latency experiment, not a measurement of the user's remote server. Both versions use the same harness/data/limits and a fresh database connection per operation. Five samples are a small regression probe; p95 is the largest sample, not a production latency SLO.

| Operation | Before median | After median | Before p95 | After p95 |
|---|---:|---:|---:|---:|
| SELECT, manual-edit session | 6,593 ms | 257 ms | 6,606 ms | 259 ms |
| Browse, manual-edit session | 6,624 ms | 258 ms | 6,631 ms | 259 ms |
| SELECT, strict reader | 6,666 ms | 341 ms | 6,717 ms | 342 ms |
| Browse, strict reader | 6,726 ms | 338 ms | 6,754 ms | 342 ms |

Without injected latency, the 100-table manual SELECT median changed from 77.2 to 9.6 ms; strict-reader SELECT from 97.0 to 29.2 ms. The main observed improvement is removal of catalog network round trips. Fresh connection/TLS setup and strict-reader global privilege audits remain deliberate costs. Query plans, server locks, index quality, large fields, OFFSET depth and UI/IPC costs are separate and are not ruled out for a different workload. Harness and reproduction commands are in DEVELOPMENT.md and `src-tauri/examples/benchmark_postgres.rs`.

Discovery now shows all table/view kinds with an explicit read restriction where required. JSON/JSONB, enum and supported arrays no longer hide an entire table. Tests exercise browse/SELECT/filter/save, exact bigint-array values, JSON text conflict detection, invalid JSON rollback, enum names with spaces, enum-array casts, materialized-view reads, and continued rejection of view/RLS/custom-domain execution. A 520-table schema loads completely, beyond the former 500-table limit. Hard limits of 5,000 relations / 50,000 columns fail explicitly instead of returning a partial catalog. Refresh schema discovers new objects while preserving drafts.

Validation: 24 PostgreSQL integration cases on each of versions 15, 16, 17 and 18; 68 Chromium/WebKit UI cases; frontend type check/build and three unit tests; Rust unit tests, strict Clippy, formatting and diff checks. UI fixtures are separate from the real database tests. Native Mac debug bundle builds and opens: connected only to the local disposable fixture without saving credentials, confirmed the formerly hidden JSONB table, browsed 200 rows, ran SELECT through real Tauri IPC, inspected timing phases and a listed view's restriction, then disconnected and closed the test app. No production database connection or Windows native smoke test was performed.
