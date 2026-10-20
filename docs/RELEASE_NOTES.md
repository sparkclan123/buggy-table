buggy-table 0.1.14 — Preview

Connect to a MySQL server without entering a database name, then expand databases in the sidebar.

## Changes

- Make MySQL's default database optional in new and saved connections. The sidebar lists accessible databases with compact +/− controls, per-database refresh and a database-list refresh button.
- Load each database catalog on expansion and reuse it within the workspace. Search covers database names and loaded tables. Table tabs, filters, counts and indexes keep their own database identity, including identical table names in different databases.
- Allow MySQL reader, writer, role and administrator accounts to connect. MySQL remains read-only in this app: dedicated read-only transactions, guarded SELECT reconstruction and blocked mutation/indirection still apply. No server permissions are changed.
- Fetch only the selected table's metadata for browse/count/index and SQL reads instead of scanning all columns in a database on every operation.
- Keep views, generated-column tables, unsupported types and other storage engines visible with a structure-only reason. Ordinary defaults such as CURRENT_TIMESTAMP no longer incorrectly block table reads. Add JSON and binary-column browsing; non-UTF-8 binary data uses a placeholder.
- Improve TLS verification guidance. Verified TLS remains the default; encrypted TLS without identity verification still requires explicit selection. There is no plaintext fallback.
- Retain per-database isolated OpenVPN support and the Docker preparation/switch improvements from 0.1.13.

Validation uses disposable MySQL fixtures for read-only enforcement with privileged accounts, multiple databases, permission boundaries, same-name tables, JSON, TLS and connection errors. Chromium/WebKit cover the new tree and retry behavior. Installer checks and CI results are recorded in BUILD_INFO.txt.

## VPN requirements and limits

Install and start **Docker Desktop with Linux containers** and use a local Docker context. First use downloads/builds the runtime and may take several minutes; Prepare in VPN settings can do this in advance. Later sessions reuse the image. No Docker installation is needed for connections with VPN disabled.

Supported: OpenVPN TUN profiles with inline CA/certificates/keys, username/password authentication and encrypted private-key passwords. Use the database's private hostname/IP; add VPN DNS IPs if the server does not supply them. TAP, external certificate/key files, `<connection>` blocks, user scripts/plugins, browser SSO and interactive MFA/challenges are not supported. Disconnect before editing/removing a profile currently in use. See the [connection guide](https://github.com/sparkclan123/buggy-table/blob/preview/v0.1.14/docs/CONNECTIONS.md).

Validation and installer provenance are recorded in BUILD_INFO.txt and [VALIDATION.md](https://github.com/sparkclan123/buggy-table/blob/preview/v0.1.14/docs/VALIDATION.md). Browser cases use IPC doubles; real isolated OpenVPN/PostgreSQL fixtures verify separate networks, private DNS, TLS identity and failure isolation. External user VPN profiles and native Windows VPN operation have not been independently verified. MySQL and Redis retain their existing read-only policies.

## Install or replace an older preview

**This preview does not install updates automatically.** The Updates button opens the GitHub downloads page. Earlier previews also shipped with the updater disabled; publishing a release cannot enable it in an already-installed app. Download the appropriate installer and replace the previous version manually. Finish or discard unsaved edits and close the app before replacing it. Keep the existing app data folder to retain saved connection settings.

| Platform | Installer |
|---|---|
| Mac Apple Silicon (M series) | `buggy-table_0.1.14_aarch64.dmg` |
| Mac Intel | `buggy-table_0.1.14_x64.dmg` |
| Windows x64 | `buggy-table_0.1.14_x64-setup.exe` |

On Mac, open the DMG and drag buggy-table into Applications, replacing the old app. On Windows, run the installer using the existing installation location. Checksums are provided in SHA256SUMS.txt; build provenance is in BUILD_INFO.txt.

## Preview signing limits

The Mac apps have ad-hoc bundle signatures but **no Apple Developer ID or notarization**. macOS may still show “damaged” with only “Move to Trash”; Open Anyway is not guaranteed. If your Mac offers Open Anyway and you trust the download, follow [Apple's instructions](https://support.apple.com/en-us/102445). Do not disable Gatekeeper. A Developer ID signed and notarized release is required for normal Mac distribution.

Windows previews have **no Authenticode certificate** and may show an unknown-publisher or SmartScreen warning. Future in-app updates require installation of an updater-enabled signed release once the signing credentials are configured.
