buggy-table 0.1.13 — Preview

Improve OpenVPN runtime preparation and make VPN switches clearer in both themes.

## Changes

- Find Docker Desktop credential helpers when the app starts from Finder with a limited PATH. The app adds Docker's executable directories to its child process environment without changing the computer's settings.
- Build against the selected local Docker engine and explicitly load the image there. A custom/cloud Buildx builder, BuildKit host or default platform no longer redirects runtime setup.
- Refresh the selected Docker context when checking or preparing setup, including after Docker restarts or its context changes.
- Replace the generic Prepare error with specific guidance for missing tools, credential access, download certificates, registry limits, disk space, network failures and timeout. Diagnostics are bounded and do not expose raw registry/proxy credentials.
- Allow up to 10 minutes for first-time setup, with an elapsed-time indicator, a visible ready state and **Check Docker again**. Subsequent connections reuse the runtime.
- Redesign the compact VPN switch with a properly contained thumb, larger click area, keyboard focus and clear Light/Dark states. Use the same control in connection lists and VPN settings.
- Retain the `push-peer-info` import fix from 0.1.12.

The original generic error was reported on another Mac. Its exact Docker failure is not known from that message alone. These changes address setup compatibility and make any remaining external network, certificate or Docker configuration issue actionable; they do not disable certificate verification.

## VPN requirements and limits

Install and start **Docker Desktop with Linux containers** and use a local Docker context. First use downloads/builds the runtime and may take several minutes; Prepare in VPN settings can do this in advance. Later sessions reuse the image. No Docker installation is needed for connections with VPN disabled.

Supported: OpenVPN TUN profiles with inline CA/certificates/keys, username/password authentication and encrypted private-key passwords. Use the database's private hostname/IP; add VPN DNS IPs if the server does not supply them. TAP, external certificate/key files, `<connection>` blocks, user scripts/plugins, browser SSO and interactive MFA/challenges are not supported. Disconnect before editing/removing a profile currently in use. See the [connection guide](https://github.com/sparkclan123/buggy-table/blob/preview/v0.1.13/docs/CONNECTIONS.md).

Validation and installer provenance are recorded in BUILD_INFO.txt and [VALIDATION.md](https://github.com/sparkclan123/buggy-table/blob/preview/v0.1.13/docs/VALIDATION.md). Browser cases use IPC doubles; real isolated OpenVPN/PostgreSQL fixtures verify separate networks, private DNS, TLS identity and failure isolation. External user VPN profiles and native Windows VPN operation have not been independently verified. MySQL and Redis retain their existing read-only policies.

## Install or replace an older preview

**This preview does not install updates automatically.** The Updates button opens the GitHub downloads page. Earlier previews also shipped with the updater disabled; publishing a release cannot enable it in an already-installed app. Download the appropriate installer and replace the previous version manually. Finish or discard unsaved edits and close the app before replacing it. Keep the existing app data folder to retain saved connection settings.

| Platform | Installer |
|---|---|
| Mac Apple Silicon (M series) | `buggy-table_0.1.13_aarch64.dmg` |
| Mac Intel | `buggy-table_0.1.13_x64.dmg` |
| Windows x64 | `buggy-table_0.1.13_x64-setup.exe` |

On Mac, open the DMG and drag buggy-table into Applications, replacing the old app. On Windows, run the installer using the existing installation location. Checksums are provided in SHA256SUMS.txt; build provenance is in BUILD_INFO.txt.

## Preview signing limits

The Mac apps have ad-hoc bundle signatures but **no Apple Developer ID or notarization**. macOS may still show “damaged” with only “Move to Trash”; Open Anyway is not guaranteed. If your Mac offers Open Anyway and you trust the download, follow [Apple's instructions](https://support.apple.com/en-us/102445). Do not disable Gatekeeper. A Developer ID signed and notarized release is required for normal Mac distribution.

Windows previews have **no Authenticode certificate** and may show an unknown-publisher or SmartScreen warning. Future in-app updates require installation of an updater-enabled signed release once the signing credentials are configured.
