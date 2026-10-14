# Validation — 0.1.0 preview

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
