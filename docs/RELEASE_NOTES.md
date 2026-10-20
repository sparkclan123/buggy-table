buggy-table 0.1.12 — Preview

Fix importing OpenVPN profiles containing `push-peer-info`.

## Changes

- Fix a profile-import bug in 0.1.11: the app rejected the standard OpenVPN `push-peer-info` directive before attempting a VPN connection. Import now accepts and preserves it, including `--push-peer-info`, commented lines, and Windows-exported files with CRLF/BOM. You do not need to remove the directive from your `.ovpn`.
- Explain unsupported options accurately instead of suggesting that every rejected option is a script, plugin or external file reference.
- Validate that `push-peer-info` has no arguments; restrictions on imported scripts, plugins and external files remain enforced.
- Add regression coverage for encrypted profile save/reload and real OpenVPN/PostgreSQL connections using profiles with `push-peer-info`.

Includes the per-database VPN switches, saved profiles/defaults, status, **Test route** and **Reconnect** introduced in 0.1.11. Each enabled database uses an isolated Docker network; normal connections use the device network. VPN sessions are reused, database TLS keeps the original hostname, and a failed VPN never falls back to a direct connection.

## VPN requirements and limits

Install and start **Docker Desktop with Linux containers** and use a local Docker context. First use downloads/builds the runtime and may take several minutes; Prepare in VPN settings can do this in advance. Later sessions reuse the image. No Docker installation is needed for connections with VPN disabled.

Supported: OpenVPN TUN profiles with inline CA/certificates/keys, username/password authentication and encrypted private-key passwords. Use the database's private hostname/IP; add VPN DNS IPs if the server does not supply them. TAP, external certificate/key files, `<connection>` blocks, user scripts/plugins, browser SSO and interactive MFA/challenges are not supported. Disconnect before editing/removing a profile currently in use. See the [connection guide](https://github.com/sparkclan123/buggy-table/blob/preview/v0.1.12/docs/CONNECTIONS.md).

Validation and installer provenance are recorded in BUILD_INFO.txt and [VALIDATION.md](https://github.com/sparkclan123/buggy-table/blob/preview/v0.1.12/docs/VALIDATION.md). Browser cases use IPC doubles; real isolated OpenVPN/PostgreSQL fixtures verify separate networks, private DNS, TLS identity and failure isolation. External user VPN profiles and native Windows VPN operation have not been independently verified. MySQL and Redis retain their existing read-only policies.

## Install or replace an older preview

**This preview does not install updates automatically.** The Updates button opens the GitHub downloads page. Earlier previews also shipped with the updater disabled; publishing a release cannot enable it in an already-installed app. Download the appropriate installer and replace the previous version manually. Finish or discard unsaved edits and close the app before replacing it. Keep the existing app data folder to retain saved connection settings.

| Platform | Installer |
|---|---|
| Mac Apple Silicon (M series) | `buggy-table_0.1.12_aarch64.dmg` |
| Mac Intel | `buggy-table_0.1.12_x64.dmg` |
| Windows x64 | `buggy-table_0.1.12_x64-setup.exe` |

On Mac, open the DMG and drag buggy-table into Applications, replacing the old app. On Windows, run the installer using the existing installation location. Checksums are provided in SHA256SUMS.txt; build provenance is in BUILD_INFO.txt.

## Preview signing limits

The Mac apps have ad-hoc bundle signatures but **no Apple Developer ID or notarization**. macOS may still show “damaged” with only “Move to Trash”; Open Anyway is not guaranteed. If your Mac offers Open Anyway and you trust the download, follow [Apple's instructions](https://support.apple.com/en-us/102445). Do not disable Gatekeeper. A Developer ID signed and notarized release is required for normal Mac distribution.

Windows previews have **no Authenticode certificate** and may show an unknown-publisher or SmartScreen warning. Future in-app updates require installation of an updater-enabled signed release once the signing credentials are configured.
