buggy-table 0.1.9 — Preview

Clearer tables and a more compact workspace. Installers are provided for Mac Apple Silicon, Mac Intel and Windows x64.

## Changes

- Alternating row backgrounds in Data and Row details, tuned for light and dark themes. Selected rows remain blue; edited cells remain yellow and pending deletions remain red.
- Hold or drag an open table tab to lift it, then drop before or after another tab. The strip scrolls automatically near its edges. Escape or dropping outside cancels. Alt+Left/Right also reorders tabs.
- Tab order is remembered with saved connections on this device. Reordering keeps the active table, unsaved edits and loaded results without querying again.
- A thinner 3 px tab scrollbar, light-blue table icons, and an icon-only Refresh schema button immediately before the schema selector.
- Browse 100 rows per page by default. The existing 50, 100, 250 and 500 choices remain available; rendering is still virtualized.
- Updates opens the GitHub downloads page in your default browser. Preview installers still require manual installation; pending edits are kept when opening the link.

Validation and installer provenance are recorded in BUILD_INFO.txt and [VALIDATION.md](https://github.com/sparkclan123/buggy-table/blob/preview/v0.1.9/docs/VALIDATION.md). Browser database cases use IPC doubles; CI adapter tests use disposable database fixtures.

## Install or replace an older preview

**This preview does not install updates automatically.** The Updates button opens the GitHub downloads page. Earlier previews also shipped with the updater disabled; publishing a release cannot enable it in an already-installed app. Download the appropriate installer and replace the previous version manually. Finish or discard unsaved edits and close the app before replacing it. Keep the existing app data folder to retain saved connection settings.

| Platform | Installer |
|---|---|
| Mac Apple Silicon (M series) | `buggy-table_0.1.9_aarch64.dmg` |
| Mac Intel | `buggy-table_0.1.9_x64.dmg` |
| Windows x64 | `buggy-table_0.1.9_x64-setup.exe` |

On Mac, open the DMG and drag buggy-table into Applications, replacing the old app. On Windows, run the installer and use the existing installation location. Checksums are provided in SHA256SUMS.txt; build provenance is in BUILD_INFO.txt.

## Preview signing limits

The Mac apps have ad-hoc bundle signatures but **no Apple Developer ID or notarization**. macOS may still show “damaged” with only “Move to Trash”; Open Anyway is not guaranteed. If your Mac offers Open Anyway and you trust the download, follow [Apple's instructions](https://support.apple.com/en-us/102445). Do not disable Gatekeeper. A Developer ID signed and notarized release is required for normal Mac distribution.

The Windows installer has **no Authenticode certificate** and may show an unknown-publisher or SmartScreen warning. Future in-app updates require installation of an updater-enabled signed release once the signing credentials are configured.
