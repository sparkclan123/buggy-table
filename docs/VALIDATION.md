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

## Recorded results

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
