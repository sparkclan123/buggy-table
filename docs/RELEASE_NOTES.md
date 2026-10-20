buggy-table 0.1.10 — Preview

Faster keyboard workflows for table edits, more room for long values, and an explicit MySQL TLS compatibility option.

## Changes

- Use the mouse wheel to scroll an overflowing table-tab strip horizontally. Horizontal trackpad scrolling and tab dragging continue to work.
- Cmd/Ctrl+Z undoes unsaved cell edits, staged deletions and copied rows. Shift+Cmd/Ctrl+Z redoes them. History belongs to each table tab and clears after Save, Discard or reloading data.
- Select rows, press Cmd/Ctrl+C, then Cmd/Ctrl+V in the same table to append copies below the current page. They remain yellow drafts until Save. PostgreSQL requires a supported table with a primary key, a manual-edit session and INSERT permission. Database-generated keys and generated columns are omitted; fill Required primary-key cells before saving. MySQL and Redis remain read-only.
- PostgreSQL typed saves now support INSERT alongside UPDATE/DELETE in one transaction, using bound values and fresh permissions. Defaults, NULL, exact decimal/JSON text and generated columns are preserved. A failed save rolls back the whole batch; the existing 100-row, 500-cell and 4 MiB limits apply.
- Long cell values open in a wider multiline editor with a thin scrollbar. Enter finishes editing, Shift+Enter adds a newline, and Escape restores the previous value.
- Cmd/Ctrl+Enter runs the current SQL instead of invoking CodeMirror's default blank-line command.
- MySQL now offers **TLS · encrypted, certificate not verified** for explicitly chosen compatibility with self-signed/untrusted or hostname-mismatched certificates. Verified TLS remains the default and supports private CA PEM files. Compatibility mode still requires encryption and never falls back to plaintext, but does not authenticate the server's identity. Client-certificate authentication is not supported.

Validation and installer provenance are recorded in BUILD_INFO.txt and [VALIDATION.md](https://github.com/sparkclan123/buggy-table/blob/preview/v0.1.10/docs/VALIDATION.md). Browser database cases use IPC doubles; adapter tests use disposable database fixtures. The user's external MySQL server has not been independently verified.

## Install or replace an older preview

**This preview does not install updates automatically.** The Updates button opens the GitHub downloads page. Earlier previews also shipped with the updater disabled; publishing a release cannot enable it in an already-installed app. Download the appropriate installer and replace the previous version manually. Finish or discard unsaved edits and close the app before replacing it. Keep the existing app data folder to retain saved connection settings.

| Platform | Installer |
|---|---|
| Mac Apple Silicon (M series) | `buggy-table_0.1.10_aarch64.dmg` |
| Mac Intel | `buggy-table_0.1.10_x64.dmg` |
| Windows x64 | `buggy-table_0.1.10_x64-setup.exe` |

On Mac, open the DMG and drag buggy-table into Applications, replacing the old app. On Windows, run the installer using the existing installation location. Checksums are provided in SHA256SUMS.txt; build provenance is in BUILD_INFO.txt.

## Preview signing limits

The Mac apps have ad-hoc bundle signatures but **no Apple Developer ID or notarization**. macOS may still show “damaged” with only “Move to Trash”; Open Anyway is not guaranteed. If your Mac offers Open Anyway and you trust the download, follow [Apple's instructions](https://support.apple.com/en-us/102445). Do not disable Gatekeeper. A Developer ID signed and notarized release is required for normal Mac distribution.

Windows previews have **no Authenticode certificate** and may show an unknown-publisher or SmartScreen warning. Future in-app updates require installation of an updater-enabled signed release once the signing credentials are configured.
