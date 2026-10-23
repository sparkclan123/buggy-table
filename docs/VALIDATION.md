# Validation history

## Draft history, copied rows, keyboard shortcuts and MySQL TLS — 0.1.10 Preview, 2026-10-20

- 120 distinct Chromium/WebKit cases passed locally: 116 in the full run, followed by 32 relevant cases after the final changes, including four added cases. Coverage includes per-cell undo/redo, active-editor undo, staged deletion, copied rows/default keys, multi-row copies, table isolation, required manual keys, deleting unsaved copies without DELETE permission, Save/Discard behavior, expanded editors, thin scrollbars, SQL run shortcuts and mouse-wheel tab scrolling. Browser database cases use IPC doubles; sample saves remain local.
- 12 frontend unit cases passed. History coalesces typing per cell activation, preserves NULL versus DEFAULT, handles SQL column names that match JavaScript object properties, bounds retained history and prevents undo after uncertain commits.
- PostgreSQL 17: all 30 integration cases passed, followed by the additional default/required-key/size-limit case. Both new INSERT cases also passed on PostgreSQL 15, 16 and 18. Real saves cover identity/generated columns, exact numeric and JSON text, defaults versus explicit NULL, manual primary keys, atomic mixed INSERT/UPDATE/DELETE, constraint rollback, fresh column INSERT permissions, revoked privileges and row/cell/byte limits.
- All 17 Rust library/transport cases and 10 multi-engine cases passed, followed by the additional MySQL plaintext-rejection case. The MySQL TLS proxy verifies trusted-CA success, rejection of untrusted and hostname-mismatched certificates, and explicit encrypted/unverified success. A separate test removes the server TLS capability and confirms no login credentials are sent. Redis restrictions remain unchanged. The user's external MySQL endpoint was not tested.
- Actual macOS ARM64 Tauri app: Cmd+Z and Cmd+Shift+Z worked on completed edits and while a cell editor was active. Cmd+Enter executed SELECT 42 through native IPC. Cmd+C/Cmd+V appended a draft, undo/redo restored it, and Cmd+S inserted it into a disposable PostgreSQL table with a new generated primary key. No saved external connection was opened; the test table and session were cleaned up.
- Svelte/TypeScript, frontend production build, Prettier/rustfmt, release consistency checks, whitespace checks and Rust Clippy with warnings denied passed. A debug macOS app bundle built successfully. Cross-platform release-mode installer provenance and final CI results are recorded in the release's BUILD_INFO.txt; native Windows/Intel execution was not tested locally.

## Table appearance and tab arrangement — 0.1.9 Preview, 2026-10-19

- 106 browser cases passed across Chromium/WebKit (104 in the full run, then the two corrected stripe assertions passed in a targeted rerun). Coverage includes lifted tab dragging in both directions, keyboard reordering, Escape/outside cancellation, overflow edge scrolling, saved order, preservation of drafts and no additional database queries.
- Alternating Data rows and Row details fields were checked visually in light/dark screenshots; virtual scrolling retains stripe colors, including the sticky row selector. Existing selection/edit/delete precedence cases passed. The tab scrollbar is 3 px and Refresh schema shares the selector row.
- The default is now 100 rows; all 50/100/250/500 options, pagination, sorting and full-value virtualization passed. Preview Updates invokes the fixed downloads command while keeping drafts; browser-open failures show the address. Existing signed-update guards and recovery tests passed.
- Frontend unit cases (8), Svelte/TypeScript checks, Rust clippy with warnings denied, and non-database Rust tests passed. UI database cases use IPC doubles. Cross-platform installer and CI provenance is recorded in BUILD_INFO.txt.
- The native PostgreSQL transport is unchanged except for the omitted-page-size default. The running older native app and its local fixture were left untouched.
- Both 0.1.9 Mac architectures were built locally from tag `preview/v0.1.9`; build-output and DMG-contained apps passed strict bundle signature checks, version/architecture checks and DMG integrity verification. The ARM64 release app launched and its Updates button opened the exact public downloads page in the OS browser through native IPC. Saved external connections remained disconnected; the test instance was closed afterward.
- At initial Mac publication, GitHub Actions was affected by [an active service incident](https://www.githubstatus.com/incidents/8tzbpcnl75jp). [Installer run 39284299677](https://github.com/sparkclan123/buggy-table/actions/runs/39284299677) and the Check workflows were queued. Windows 0.1.9 was not available; no completed CI result is claimed for this version at that point.

## Row selection and read-only inspector — 0.1.8 Preview, 2026-10-19

- First activation selects the entire row; a subsequent activation within that selected row opens an eligible editor. Ordinary typing, including Space, cannot accidentally open an editor. Immutable/read-only cells also select rows. Edited cells and pending deletions override selection colors in both themes, including hover and active editors.
- Row details sits on the right, starts at 10% of the table area, and supports pointer/keyboard resizing, close/reopen and a persistent checkbox beside Full values. It renders the selected row vertically with complete scrollable values and staged drafts. JSON formatting preserves original number tokens, decimal precision, key order, duplicate keys and escapes; timestamp/array/other database strings are not converted. Only display preferences persist, not row data.
- Final production-preview Playwright run: **96 passed**, Chromium and WebKit. The 12 new browser cases cover the two-stage mouse/keyboard interaction, accidental typing, immutable cells, multi-selection, theme priority, deletion/restore, inspector close/reopen, 10% width, pointer/keyboard resizing and persistence, disabled/storage-failure behavior, refresh clearing stale selections, long JSON scrolling, exact numerics/timestamps/NULL/empty values, safe HTML-as-text rendering and no additional browse/query/count calls during inspection. Typed database UI cases use explicit IPC doubles. Existing save/filter/sort/page/tab/connection cases also pass.
- **8 frontend unit tests passed**, including precision-preserving JSON formatting, invalid drafts, type-specific raw strings and bounded indentation for deeply nested JSON. Svelte/TypeScript has zero errors/warnings; the production frontend build, Prettier, rustfmt and whitespace checks pass. Compact/expanded JSON and light/dark screenshots were visually inspected.
- No backend code was changed for this feature. The preceding PostgreSQL performance changes are included in 0.1.8; their database evidence is recorded below. The running native app and disposable PostgreSQL fixture were left untouched during UI validation. Final cross-platform installer provenance and CI results are recorded in the release's BUILD_INFO.txt.

## PostgreSQL transport performance — 0.1.8 Preview, 2026-10-19

Changes: reuse one read connection per workspace, use implicit read-only transactions with fixed startup defaults, pipeline independent fresh catalog/privilege checks, and load SQL metadata only for the referenced relation. Version metadata is cached per physical connection; table/permission/result data is not cached. Saves and exact counts use separate connections, and a session touched by either path cannot enter the read slot. Dead reused transports may reconnect once before user SQL is sent; interrupted SQL and saves are never replayed.

Comparison: release-mode baseline built from published source commit `e4bd428f9f79e56ba0bc4510d61c31a389895e2c` (0.1.7), versus the working tree after this change. Both use PostgreSQL 17, 100 synthetic tables, the same loopback TCP proxy and fixture data, one warmup then nine samples. The delay is injected per forwarded TCP chunk in each direction, not an exact WAN RTT model. Times include backend connection/validation/execution/result conversion; IPC and UI rendering are excluded. These synthetic measurements do not establish the latency of the user's external database. Raw JSONL evidence is retained locally in `artifacts/performance-session-reuse-2026-10-19/` (ignored generated artifacts).

| Delay per direction/chunk | Mode | Operation | Before median | After median | Before P95 | After P95 |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| 25 ms | manual | query (200 rows) | 545.43 ms | 138.81 ms | 551.81 ms | 166.26 ms |
| 25 ms | manual | browse (50 rows) | 514.20 ms | 139.24 ms | 519.02 ms | 165.62 ms |
| 25 ms | strict | query (200 rows) | 696.50 ms | 142.87 ms | 701.81 ms | 145.69 ms |
| 25 ms | strict | browse (50 rows) | 665.94 ms | 143.55 ms | 669.00 ms | 144.97 ms |
| 0 ms | manual | query (200 rows) | 9.76 ms | 1.64 ms | 10.52 ms | 2.83 ms |
| 0 ms | manual | browse (50 rows) | 6.20 ms | 1.40 ms | 7.28 ms | 1.65 ms |
| 0 ms | strict | query (200 rows) | 29.45 ms | 2.67 ms | 30.55 ms | 2.79 ms |
| 0 ms | strict | browse (50 rows) | 28.62 ms | 2.45 ms | 39.78 ms | 2.65 ms |

All nine measured samples reused the same physical connection in each mode, with one transport opened across connection setup plus both operation loops. At the 25 ms setting, initial connection/catalog discovery took 275 ms (manual) / 290 ms (strict); warm connection acquisition and local cleanup each rounded to 0 ms. On loopback without injected delay, warm backend reads took 1.4–2.7 ms. P95 with nine samples is the maximum and is a regression probe, not a production SLO.

Validation:

- PostgreSQL 15/16/17/18: 28 integration cases passed on each version, followed by the additional save-trigger session-isolation case on each (29 cases per major). Coverage includes reuse without an idle transaction, disconnect cleanup, reconnect after backend termination, disposal after failed/cancelled reads, permission drift, fresh schema metadata, count isolation and unchanged atomic saves. A fixture trigger changes session settings during Save; subsequent reads retain the original clean backend.
- Verified TLS reuse test passed on each PostgreSQL major using a short-lived private CA and a local proxy accepting exactly one physical connection; a second read reuses it and a direct write remains server-denied.
- 17 Rust unit/transport tests and 5 frontend unit tests passed. Clippy with warnings denied, rustfmt, Prettier and Svelte/TypeScript checks pass.
- 16 Chromium/WebKit connection-workspace UI cases passed, including total timing, reused-connection detail, schema refresh, draft retention and connection/schema isolation. These browser cases use explicit IPC doubles; real database behavior is covered by the Rust tests above.
- macOS ARM64 release-mode app built (15.84 MiB) and passed strict/deep ad-hoc codesign verification. Through actual Tauri IPC, a session-only connection to the loopback fixture opened 50 order rows, refreshed with connection reuse visible (4–6 ms backend total), and counted the correct 450 rows in 12 ms. The build and disposable PostgreSQL fixture remain open for local inspection; saved production connections were not connected. Native SQL execution itself was covered by Rust integration tests, not a completed SQL-editor smoke action in this run.

This work is not yet part of the published 0.1.7 installers. No production database or saved user credential was used. MySQL/Redis transport optimizations, keyset pagination, real WAN profiling and OS-wide egress/resource soak testing are outside this change.

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

## Per-connection OpenVPN — 0.1.11 Preview, 2026-10-20

- Real local Docker/OpenVPN fixtures connect two independent tunnels to databases sharing `vpn-db.test / 10.77.0.1`. PostgreSQL returns the correct A/B marker through verified TLS. Wrong database certificate identity is rejected, a failed DB-auth replacement preserves the previous session, and stopping server A breaks A while B remains usable. Session shutdown removes the owned containers.
- The latest run established both fixture tunnels and their private-DNS/TCP preflights in about 412 ms total; five reused PostgreSQL reads took 3–6 ms each. These are local fixture measurements, not results for the user's external VPN or a promised latency.
- 132 Chromium/WebKit UI cases passed, including 12 VPN cases covering per-database defaults, import/rename/delete, no browser secret storage, route checks, refresh, failed replacement, unsaved-edit protection, canceled-new-connection cleanup and both themes at a 1000×700 viewport. UI cases use IPC doubles. The VPN subset was rerun after the final layout change.
- Existing real database regression suites passed: 31 PostgreSQL tests and 11 multi-engine/validation tests. Frontend unit tests: 12 passed. Final Rust unit run: 19 passed, one fixture-only TLS test ignored (that test passed separately in the full integration run). Clippy with warnings denied, formatting, frontend build/type checks and version/release asset checks passed.
- A debug macOS ARM64 app bundle built and ran with a separate test bundle identifier/app-data directory. Actual Tauri IPC imported the disposable `.ovpn`, encrypted its private key using login Keychain, connected to fixture B with private-CA verified TLS, displayed table data, ran Test route, and reconnected VPN+DB successfully. Reopening showed the saved VPN enabled/default choice with the DB offline. Closing the app left no running app VPN containers. Both local metadata files had mode 0600 and contained no plaintext fixture password or private key.
- No production DB or user OpenVPN profile was used. Native Windows behavior and MySQL/Redis end-to-end through a real VPN have not been validated in this local run; their direct database regressions passed. The statements above record local validation before packaging. CI and installer provenance for publication are recorded separately in the release BUILD_INFO.txt.

## OpenVPN profile import hotfix — 0.1.12 Preview, 2026-10-20

- Reproduced the reported `Unsupported OpenVPN option: push-peer-info` failure with a regression test before changing the validator. The fix accepts and preserves that standard client metadata directive, including the `--` prefix, trailing comments and CRLF/BOM exports; unexpected arguments, scripts, plugins and external-file directives still fail validation.
- Local Rust library tests: 21 passed, one unrelated disposable PostgreSQL fixture test ignored. Coverage includes encrypted save/reload of a profile containing `push-peer-info`.
- Added `push-peer-info` to both generated OpenVPN client fixtures. The real Docker/OpenVPN/PostgreSQL integration test passed: two separate VPNs with overlapping private DNS/IP, verified database TLS, session reuse, failed replacement preserving the old session, and failure isolation when one VPN stops.
- These tests use disposable local VPN servers, not the user's external profile. Cross-platform CI results and packaged installer provenance are recorded in the release BUILD_INFO.txt.

## OpenVPN preparation and controls — 0.1.13 Preview, 2026-10-20

- The reported failure occurred on a different Mac; its exact Docker stderr and elapsed time were unavailable. Local clean `--no-cache --pull` runtime setup with a minimal GUI-style PATH succeeded, so this does not claim to reproduce or diagnose that remote machine's specific failure.
- Added a real runtime-build regression with a fresh temporary image tag, a nonexistent custom builder, a bogus BuildKit endpoint and an invalid default platform. It passed with the compiled Rust test launched under `/usr/bin:/bin:/usr/sbin:/sbin`, confirming local builder selection, helper path setup and a loaded image. CI now runs this case before VPN integration.
- Rust library tests: 26 passed, one unrelated database fixture case ignored. New cases cover bounded/drained build output, successful/failed/timed-out child processes, helper discovery without shell configuration, and cause-specific errors that do not echo credential canaries.
- Real two-VPN PostgreSQL integration passed after the process changes: private DNS, verified database TLS, reused queries, failed replacement preserving the prior session and independent VPN shutdown.
- VPN UI cases: 16 passed across Chromium/WebKit. They include unavailable Docker → recheck → actionable failure → retry → ready, disabled setup controls while busy, per-connection behavior, keyboard switches and thumb bounds in both themes. Light/Dark screenshots were inspected at a 1000×700 viewport; sidebar status remains readable.
- The existing 0.1.12 native app was opened only to inspect setup status and then closed with all saved databases offline. No production connection or user VPN profile was tested. The new UI tests use IPC doubles; real process/build and VPN tests are separate. Release CI and installer evidence are recorded in BUILD_INFO.txt.


## MySQL database navigation — 0.1.14

MySQL connections may omit the default database, including saved bookmarks. The sidebar loads accessible database names first, expands tables on demand and reuses each loaded catalog. Browse, count, indexes and SQL use the requested database independently; SQL still requires a qualified table name. Views and unsupported relations remain visible with a structure-only reason. Per-table metadata lookup avoids rescanning unrelated columns on every read.

Unlike earlier previews, MySQL permits accounts with elevated privileges. SQL remains a reconstructed read-only subset and each dedicated connection begins a read-only transaction. Regression coverage includes reader, writer and root accounts, blocked mutation/indirection, server rejection of an UPDATE in the app's root transaction, privilege changes, permission-limited database discovery, same-name tables in two databases, counts/indexes, JSON, verified/private-CA TLS, explicit encrypted-only TLS and no plaintext fallback. These write-denial checks use disposable Compose fixtures only.

Local validation: 28 Rust library tests passed with fixtures enabled, 12 MySQL/Redis integration tests passed, 12 frontend unit tests passed, and 24 focused Chromium/WebKit cases passed (database tree, retry/empty states, TLS and existing overview behavior). Svelte/TypeScript and release consistency checks passed. Final CI and packaged-installer results are recorded in BUILD_INFO.txt attached to the release. Browser tests use IPC doubles; do not interpret them as native Windows MySQL validation.

Private endpoint diagnostics were performed only through ephemeral stdin without writing connection inputs to source, fixtures, bookmarks or logs. Public release materials contain no endpoint, account, database/table names or secrets from that diagnostic. No application row data was read or changed on that endpoint.

## MySQL transport reuse and VPN switching audit — 0.1.15, 2026-10-20

The 0.1.14 MySQL adapter established a new TCP/authentication/TLS session for each operation. PostgreSQL already retained a transport per workspace. MySQL's old displayed duration excluded connection/catalog setup, whereas PostgreSQL reported total backend time; those labels were not directly comparable.

The working-tree change retains at most one MySQL connection per workspace and a bounded 32-statement driver cache. Successful reads run in implicit read-only transactions (`transaction_read_only=1`, `autocommit=1`), so no snapshot or metadata lock is held between operations. Table metadata and permissions are still checked on every table read; result rows and permission decisions are not cached. Idle connections are probed after 30 seconds. A disconnected cached socket may reconnect once during fixed catalog lookup, before user SQL; user execution is never replayed. Cancelled, errored and incompletely consumed reads discard the socket. Generated pagination consumes its single lookahead row and terminator, allowing full pages to reuse the transport. Counts remain dedicated so their shorter execution timeout does not leak into ordinary reads. MySQL now reports connection, validation, execution/transfer and cleanup stages with total backend duration.

Reproducible client-overhead benchmark: `docker compose up -d --wait`, then `cargo run --manifest-path src-tauri/Cargo.toml --no-default-features --example benchmark_connections`. Repeat with `BUGGY_BENCH_DELAY_MS=20`. Both runs use the same Mac ARM64 host, debug build, Docker fixtures (MySQL 8.4 / PostgreSQL 17), no database TLS on the loopback benchmark, one unmeasured warmup then 30 samples per operation. Both SQL queries select the same three scalar columns and two rows; browse loads each fixture's two rows with page size 100. The schemas/checks differ between engines, so this measures the application's connection overhead, not which database engine is universally faster. The relay delays each read in each direction by 20 ms; this approximates latency, not real WAN packet timing, bandwidth, jitter or loss. Wall time is measured around the entire Workbench call, excluding UI rendering.

| Scenario | MySQL before p50 / p95 | MySQL after p50 / p95 | PostgreSQL after p50 / p95 |
| --- | --- | --- | --- |
| Local SQL, 2 rows | 4.96 / 7.32 ms | 1.60 / 3.22 ms | 2.80 / 3.73 ms |
| Local browse, 2 rows | 4.50 / 6.51 ms | 1.03 / 1.22 ms | 2.50 / 2.70 ms |
| Delayed SQL, 2 rows | 454.20 / 461.22 ms | 95.63 / 98.06 ms | 123.07 / 141.72 ms |
| Delayed browse, 2 rows | 439.82 / 462.25 ms | 95.08 / 99.13 ms | 121.41 / 126.74 ms |

MySQL's delayed p50 improved by 78–79%. Each operation previously opened 31 new transports including warmup; the new implementation opens zero additional transports after initial connection and reuses all 30 measured reads. PostgreSQL before-change delayed p50 was 118.86 ms (SQL), 117.82 ms (browse); its code was unchanged and run-to-run variation remains visible. These results do not predict a user's production query time or certify a performance standard.

VPN switching: selecting an already-connected workspace retains its tunnel and database transport. Changing VPN profile/reconnecting creates and validates a replacement before swapping; failed replacement preserves the prior session. Independent startups now share a read lock rather than serializing behind one global mutex; profile mutations/removal still take the write lock through startup and session insertion. The actual two-network OpenVPN fixture passed 30 alternating verified-TLS PostgreSQL reads (p50 3.22 ms, p95 5.74 ms) with every transport reused and correct A/B rows despite overlapping private IPs and DNS. Concurrent reads, private DNS, certificate mismatch rejection, failed replacement and shutting down A while B remains usable also passed. Two concurrent tunnel startups plus DNS/TCP preflights took 494 ms with the runtime already prepared. These are local Docker measurements, not external VPN latency or GUI switching measurements. The helper still runs userspace OpenVPN through Docker and a TCP relay; first-time runtime preparation, authentication, VM overhead and network RTT remain real costs.

Validation: 33 Rust library cases (including fixture-only reuse, read-only root sessions, committed-data freshness, no retained metadata locks, page/result-byte limits, cancellation, stale socket recovery, dedicated count settings and privilege revocation); 12 MySQL/Redis integration cases; the real two-VPN integration case; 12 frontend unit cases. Desktop/all-target Clippy and Svelte/TypeScript checks passed. Production databases and user VPN profiles were not accessed. Release CI and packaged installer evidence are recorded in BUILD_INFO.txt attached to v0.1.15. Raw local logs and measurements are in ignored `artifacts/performance-*` and `artifacts/mysql-reuse-*` files.

Design references: [MySQL session transaction scope](https://dev.mysql.com/doc/refman/8.4/en/set-transaction.html), [MySQL autocommit](https://dev.mysql.com/doc/refman/8.4/en/commit.html), [OpenVPN data-channel offload](https://community.openvpn.net/DataChannelOffload). An independent performance/security certification, native Windows VPN measurement and representative production workload benchmarking have not been performed.


## Bundled native OpenVPN — 0.1.16, 2026-10-22

The app no longer uses Docker. The native OpenVPN 3.11.8/lwIP 2.2.1 sidecar was tested on macOS ARM64 against disposable local OpenVPN 2 servers. Docker hosts only the test servers. Client tests run with an empty PATH, proving that no Docker/OpenVPN CLI or shell is required. The bundled engine uses ordinary user privileges; no OS TUN interface or route/DNS mutation is implemented.

- A uses UDP and certificate authentication; B uses TCP with explicitly requested AES-256-CBC/SHA256, certificates and username/password. Both expose the same database name and private IPv4 address. The real Workbench integration verifies private DNS, original-hostname database TLS, 30 alternating/reused reads, concurrent reads, a 1 MB TLS result, successful and failed session replacement, heartbeat/timer survival and stopping A while B remains usable.
- Native contract tests also cover IPv6 DNS and literal IPv6 destinations, relay token rejection, encrypted private keys, incorrect VPN password, missing DNS without system/public fallback, failed lookup, prohibited loopback destination, wrong VPN certificate identity and child shutdown on parent stdin EOF.
- ASan/UBSan validation caught and corrected the 64-bit IPv6 reassembly configuration and DNS table/request capacity mismatch during development. The subsequent two-network regression ran without sanitizer diagnostics. Client-owned buffer queues are bounded and apply TCP backpressure; result sizes above the TCP window are checked byte-for-byte.
- One local sanitizer run established two independent tunnels plus DNS/TCP preflights in about 36 ms, streamed a 1 MB TLS result in 30 ms, and measured alternating two-row reads at p50 2.95 ms / p95 3.69 ms. These are local fixture measurements with a small workload, not production benchmarks or an apples-to-apples speedup against a previous release.
- Frontend checks: zero Svelte/TypeScript errors/warnings, 12 unit tests and 16 Chromium/WebKit VPN cases passed. Rust unit/contract tests, all-target Clippy and version/release checks passed locally. Platform CI and final installer provenance are recorded in the release BUILD_INFO.txt.
- Packaging uses checksum-pinned OpenSSL 3.5.9 LTS compiled statically from source. Both Mac app and helper target macOS 11.0; bundle checks enforce that minimum and reject developer-library dependencies. MSVC packet layouts are checked at compile time. Every platform's packaged self-test includes a real DNS packet/response round trip through lwIP entirely in memory, without host networking.

No production database, supplied database password or user VPN profile was used. Native Windows VPN interoperability with external servers has not been exercised locally. Cross-platform builds run the packaged engine self-test; Mac bundle validation rejects developer-machine dynamic-library paths. OpenVPN 2/3 option and legacy-cipher compatibility can differ; unsupported profiles are not silently routed directly.
