buggy-table 0.1.6 — Preview

Faster PostgreSQL queries and more complete schema discovery, with installers for Mac Apple Silicon, Mac Intel and Windows x64.

## Changes

- Reduce PostgreSQL catalog loading from one metadata query per table to three batched queries, while checking permissions on every operation. This substantially reduces waiting on remote connections with many tables.
- In a local latency simulation with 100 tables and 200 returned rows, median SELECT time fell from 6,593 ms to 257 ms in manual-edit mode, and from 6,666 ms to 341 ms in strict-reader mode. These are controlled test results, not a guarantee for every server or query.
- Show tables containing JSON/JSONB, supported arrays and enums. Support browsing, filtering and manual saves for these types without losing bigint/decimal precision.
- List views and other restricted relations in the schema browser with a visible explanation in Structure, instead of silently hiding them. Execution restrictions still apply.
- Load schemas beyond the previous 500-table cap. Catalogs exceeding 5,000 relations or 50,000 columns now fail explicitly instead of returning an incomplete list.
- Add Refresh schema and preserve unsaved edits when refreshing or switching schemas.
- Show total PostgreSQL operation time and a breakdown for connection, validation and query execution.

Validated with PostgreSQL 15, 16, 17 and 18 integration tests, Chromium/WebKit UI tests and a native Mac smoke test against disposable local fixtures. The performance measurements did not use a production database.

## Install or replace an older preview

**This preview has no in-app updates.** Versions 0.1.4 and 0.1.5 also shipped with the updater disabled; publishing this release cannot enable it in an already-installed app. Download the appropriate installer and replace the previous version manually. Keep the existing app data folder to retain saved connection settings. Finish or discard unsaved edits and close the app before replacing it.

| Platform | Installer |
|---|---|
| Mac Apple Silicon (M series) | `buggy-table_0.1.6_aarch64.dmg` |
| Mac Intel | `buggy-table_0.1.6_x64.dmg` |
| Windows x64 | `buggy-table_0.1.6_x64-setup.exe` |

On Mac, open the DMG and drag buggy-table into Applications, replacing the old app. On Windows, run the installer and use the existing installation location. Checksums are provided in `SHA256SUMS.txt`; build provenance is in `BUILD_INFO.txt`.

## Preview signing limits

The Mac apps have ad-hoc bundle signatures but **no Apple Developer ID or notarization**. macOS may still show “damaged” with only “Move to Trash”; Open Anyway is not guaranteed. If your Mac offers Open Anyway and you trust the download, follow [Apple's instructions](https://support.apple.com/en-us/102445). Do not disable Gatekeeper. A Developer ID signed and notarized release is required for normal Mac distribution.

The Windows installer has **no Authenticode certificate** and may show an unknown-publisher or SmartScreen warning. This release is a Preview, not an OS-certified production build. Future in-app updates require installation of an updater-enabled signed release once the signing credentials are configured.
