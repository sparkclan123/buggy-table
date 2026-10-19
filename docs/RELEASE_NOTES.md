buggy-table 0.1.7 — Preview

More capable table editing and browsing, clearer MySQL connection errors, and PostgreSQL/MySQL/Redis snapshot charts. Includes installers for Mac Apple Silicon, Mac Intel and Windows x64.

## Table editing and navigation

- Highlight unsaved cells, including the cell currently being edited, in both light and dark themes.
- Keep the row-number column visible while scrolling sideways. Select rows from their row numbers and press Delete/Backspace to stage deletion; pending deletions appear red and can be restored or discarded. PostgreSQL changes are committed only on Save, with permission and conflict checks and atomic mixed updates/deletions.
- Drag column headers to reorder columns, or use Alt+Left/Right on a focused header. The layout is saved only on your device.
- Type timestamp values directly or use a date/time picker when editing and filtering, preserving fractional seconds and explicit offsets.
- Use symbolic filter operators with explanatory labels and tooltips.
- Close the active table tab with Cmd/Ctrl+W, with Save/Discard/Keep editing choices for pending changes.
- Choose 50 (default), 100, 250 or 500 rows per page. Show the estimated total immediately and request an exact filtered count with Count exact; page loading does not wait for counting.
- Collapse the table controls using the button beside Refresh, use slimmer scrollbars, and optionally enable Full values to wrap complete cell contents on the current page. Full values is off by default and retains existing result-size limits.

## Query performance and issue fixes

- Browse, exact-count and save operations load metadata for the selected PostgreSQL relation instead of the entire schema, retaining fresh permission checks. Exact counts are opt-in and have a five-second server query timeout.
- Retain the complete schema discovery and typed JSON/JSONB, supported arrays and enum handling introduced in 0.1.6.
- [Issue #3](https://github.com/sparkclan123/buggy-table/issues/3): distinguish MySQL network, TLS, authentication, permission and unsupported-server errors; support a custom CA certificate with verified TLS.
- [Issue #4](https://github.com/sparkclan123/buggy-table/issues/4): add PostgreSQL/MySQL overview charts from real manually refreshed snapshots, with unavailable metrics shown explicitly.
- [Issue #5](https://github.com/sparkclan123/buggy-table/issues/5): add Redis INFO snapshot charts without fabricated history or background polling.
- [Issue #6](https://github.com/sparkclan123/buggy-table/issues/6): display Redis SCAN results as key records, accumulate and deduplicate cursor pages, support searching and command scans, and keep Next available for an empty batch with a nonzero cursor.

Validated locally with 84 Chromium/WebKit UI cases, 26 PostgreSQL integration cases on each of versions 15–18, 10 MySQL/Redis integration cases, unit/static checks and native macOS IPC/keyboard smoke tests. Final highlight changes passed 22 focused UI cases. Controlled latency measurements and their limitations are recorded in [VALIDATION.md](https://github.com/sparkclan123/buggy-table/blob/preview/v0.1.7/docs/VALIDATION.md); they are not a guarantee for every server or query. Final installer build provenance and CI results are included in BUILD_INFO.txt.

## Install or replace an older preview

**This preview has no in-app updates.** Versions 0.1.4–0.1.6 also shipped with the updater disabled; publishing a release cannot enable it in an already-installed app. Download the appropriate installer and replace the previous version manually. Keep the existing app data folder to retain saved connection settings. Finish or discard unsaved edits and close the app before replacing it.

| Platform | Installer |
|---|---|
| Mac Apple Silicon (M series) | `buggy-table_0.1.7_aarch64.dmg` |
| Mac Intel | `buggy-table_0.1.7_x64.dmg` |
| Windows x64 | `buggy-table_0.1.7_x64-setup.exe` |

On Mac, open the DMG and drag buggy-table into Applications, replacing the old app. On Windows, run the installer and use the existing installation location. Checksums are provided in SHA256SUMS.txt; build provenance is in BUILD_INFO.txt.

## Preview signing limits

The Mac apps have ad-hoc bundle signatures but **no Apple Developer ID or notarization**. macOS may still show “damaged” with only “Move to Trash”; Open Anyway is not guaranteed. If your Mac offers Open Anyway and you trust the download, follow [Apple's instructions](https://support.apple.com/en-us/102445). Do not disable Gatekeeper. A Developer ID signed and notarized release is required for normal Mac distribution.

The Windows installer has **no Authenticode certificate** and may show an unknown-publisher or SmartScreen warning. Future in-app updates require installation of an updater-enabled signed release once the signing credentials are configured.
