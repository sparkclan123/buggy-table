buggy-table 0.1.8 — Preview

Faster PostgreSQL reads, deliberate row selection before editing, and a compact read-only row inspector. Installers are provided for Mac Apple Silicon, Mac Intel and Windows x64.

## PostgreSQL performance

- Reuse one read connection per connected workspace instead of reconnecting for every operation. Successful reads finish without leaving an open transaction.
- Remove separate BEGIN/ROLLBACK round trips from approved reads and pipeline independent catalog/permission checks. SQL checks only the referenced relation; permissions and metadata are still checked freshly for every operation.
- Keep Save and Count exact on dedicated connections. Failed, cancelled or byte-truncated reads discard their connection. An interrupted query or save is never automatically replayed.
- Show connection reuse and cleanup time in the timing breakdown. No query-result cache or automatic count query was introduced.

In a controlled release-mode experiment with 100 synthetic tables and 25 ms delay per forwarded TCP chunk in each direction, median manual SQL time fell from 545.43 ms to 138.81 ms and 50-row browsing from 514.20 ms to 139.24 ms. These include backend connection/checks/transfer, exclude UI rendering, and are not a latency guarantee for a real external database. See the methodology and limits in [VALIDATION.md](https://github.com/sparkclan123/buggy-table/blob/preview/v0.1.8/docs/VALIDATION.md).

## Row selection and details

- Click any cell once to highlight its whole row. Click an editable cell in that selected row again to edit. Ordinary typing, including Space, cannot accidentally start editing; Enter supports explicit keyboard activation.
- Unsaved-cell highlights and red pending-deletion backgrounds take priority over the selected-row highlight in light and dark themes. Delete/Backspace on a focused selected cell or row number stages deletion; Save remains explicit.
- Enable or disable **Row details** using the checkbox beside **Full values**. The right-hand panel is read-only, starts at 10% of the table area, has a close button and a draggable/keyboard-accessible edge. Its enabled state and width are remembered on this device.
- Inspect complete values vertically with scrolling, including staged edits. JSON/JSONB is indented without rounding large numbers or changing decimal tokens. Other database strings retain their original timestamps, offsets, fractions, array syntax and NULL/empty distinctions. Selecting a row makes no extra database request and does not persist its values.

Validated locally with 96 Chromium/WebKit UI cases and 8 frontend unit cases. The PostgreSQL read-transport changes passed 29 integration cases on each of PostgreSQL 15–18, TLS reuse checks, 17 Rust unit cases and static checks. Final installer provenance and CI results are recorded in BUILD_INFO.txt. Browser database cases use IPC doubles; real database adapter tests use disposable local fixtures.

## Install or replace an older preview

**This preview has no in-app updates.** Earlier previews also shipped with the updater disabled; publishing a release cannot enable it in an already-installed app. Download the appropriate installer and replace the previous version manually. Finish or discard unsaved edits and close the app before replacing it. Keep the existing app data folder to retain saved connection settings.

| Platform | Installer |
|---|---|
| Mac Apple Silicon (M series) | `buggy-table_0.1.8_aarch64.dmg` |
| Mac Intel | `buggy-table_0.1.8_x64.dmg` |
| Windows x64 | `buggy-table_0.1.8_x64-setup.exe` |

On Mac, open the DMG and drag buggy-table into Applications, replacing the old app. On Windows, run the installer and use the existing installation location. Checksums are provided in SHA256SUMS.txt; build provenance is in BUILD_INFO.txt.

## Preview signing limits

The Mac apps have ad-hoc bundle signatures but **no Apple Developer ID or notarization**. macOS may still show “damaged” with only “Move to Trash”; Open Anyway is not guaranteed. If your Mac offers Open Anyway and you trust the download, follow [Apple's instructions](https://support.apple.com/en-us/102445). Do not disable Gatekeeper. A Developer ID signed and notarized release is required for normal Mac distribution.

The Windows installer has **no Authenticode certificate** and may show an unknown-publisher or SmartScreen warning. Future in-app updates require installation of an updater-enabled signed release once the signing credentials are configured.
