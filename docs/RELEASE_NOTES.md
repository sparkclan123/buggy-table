buggy-table 0.1.11 — Preview

Use a separate OpenVPN connection for each database, with saved profiles, network health checks and reconnect controls.

## Changes

- Compact VPN switch and settings beside each database connection, including the new-connection form and connection manager.
- Import inline `.ovpn` client profiles, add/remove/rename them, and choose a default independently for each database. The enabled/default preference is saved with the local connection bookmark.
- When enabled, connect OpenVPN before connecting the database. Each active database owns an isolated Linux container, so different VPNs can use overlapping private hostnames and IP ranges without replacing the computer's routes or DNS.
- Show VPN state and the last route check. **Test route** checks private DNS/TCP reachability to the database port and reports latency; it does not run SQL or prove database authentication. **Reconnect** refreshes that database's VPN and database session.
- Reuse the running VPN for subsequent queries. Status polling reads local state without issuing extra SQL. A failed VPN never falls back to the device network. Direct connections do not require Docker.
- Keep database TLS verification against the original database hostname. Encrypt VPN configuration/private keys/passwords locally using a key in the system credential store. Stored VPN secrets are not sent back to the UI.
- Preserve the existing workspace if network replacement fails, and require saving/discarding pending edits before a network change or reconnect. VPN servers may independently restrict concurrent use of the same client certificate.

## VPN requirements and limits

Install and start **Docker Desktop with Linux containers** and use a local Docker context. First use downloads/builds the runtime and may take several minutes; Prepare in VPN settings can do this in advance. Later sessions reuse the image. No Docker installation is needed for connections with VPN disabled.

Supported: OpenVPN TUN profiles with inline CA/certificates/keys, username/password authentication and encrypted private-key passwords. Use the database's private hostname/IP; add VPN DNS IPs if the server does not supply them. TAP, external certificate/key files, `<connection>` blocks, user scripts/plugins, browser SSO and interactive MFA/challenges are not supported. Disconnect before editing/removing a profile currently in use. See the [connection guide](https://github.com/sparkclan123/buggy-table/blob/preview/v0.1.11/docs/CONNECTIONS.md).

Validation and installer provenance are recorded in BUILD_INFO.txt and [VALIDATION.md](https://github.com/sparkclan123/buggy-table/blob/preview/v0.1.11/docs/VALIDATION.md). Browser cases use IPC doubles; real isolated OpenVPN/PostgreSQL fixtures verify separate networks, private DNS, TLS identity and failure isolation. External user VPN profiles and native Windows VPN operation have not been independently verified. MySQL and Redis retain their existing read-only policies.

## Install or replace an older preview

**This preview does not install updates automatically.** The Updates button opens the GitHub downloads page. Earlier previews also shipped with the updater disabled; publishing a release cannot enable it in an already-installed app. Download the appropriate installer and replace the previous version manually. Finish or discard unsaved edits and close the app before replacing it. Keep the existing app data folder to retain saved connection settings.

| Platform | Installer |
|---|---|
| Mac Apple Silicon (M series) | `buggy-table_0.1.11_aarch64.dmg` |
| Mac Intel | `buggy-table_0.1.11_x64.dmg` |
| Windows x64 | `buggy-table_0.1.11_x64-setup.exe` |

On Mac, open the DMG and drag buggy-table into Applications, replacing the old app. On Windows, run the installer using the existing installation location. Checksums are provided in SHA256SUMS.txt; build provenance is in BUILD_INFO.txt.

## Preview signing limits

The Mac apps have ad-hoc bundle signatures but **no Apple Developer ID or notarization**. macOS may still show “damaged” with only “Move to Trash”; Open Anyway is not guaranteed. If your Mac offers Open Anyway and you trust the download, follow [Apple's instructions](https://support.apple.com/en-us/102445). Do not disable Gatekeeper. A Developer ID signed and notarized release is required for normal Mac distribution.

Windows previews have **no Authenticode certificate** and may show an unknown-publisher or SmartScreen warning. Future in-app updates require installation of an updater-enabled signed release once the signing credentials are configured.
