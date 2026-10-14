buggy-table 0.1.5 — Preview

- Fix incomplete macOS bundle signatures that could cause “app is damaged” on downloaded builds. Both Apple Silicon and Intel previews now use complete ad-hoc signatures.
- Verify signatures and sealed resources in both the build output and the app inside each DMG before uploading installers.
- Signed production builds additionally require Developer ID, a stapled notarization ticket and Gatekeeper acceptance.

This is a **macOS-only packaging fix**. Windows users can continue using [0.1.4](https://github.com/sparkclan123/buggy-table-releases/releases/tag/v0.1.4); no Windows 0.1.5 installer is published.

## Included from 0.1.4

- Move PostgreSQL schema selection to a native dropdown at the bottom of the sidebar. Move appearance selection into a compact icon beside the app version; System/Light/Dark preferences remain available.
- Add saved connection bookmarks, pin/recent/search/group filters and explicit Reconnect after restarting. Optional passwords stay in Mac login Keychain / Windows Local Credential Manager.
- Keep PostgreSQL/MySQL/Redis sessions and table tabs separate when switching; preserve unsaved cells and Redis inspection state.
- Add searchable PostgreSQL schema selection; bind browse/SQL/index/save to the owning table schema.
- Restore only table identities on startup and limit clean result caches; no query text, row data or drafts written to connection settings.
- Remove the PostgreSQL 17-only gate. Test PostgreSQL 15, 16, 17 and 18 with connection, browse, SQL, index/status, save/rollback, cancellation and TLS coverage.
- Allow newer PostgreSQL majors to attempt connection with an untested-compatibility notice; retain all safety checks.
- Exclude PostgreSQL 18 virtual generated columns from approved table scope; protect stored generated and identity columns from edits.
- Add an explicit PostgreSQL TLS mode that requires encryption without verifying the certificate/hostname, for compatibility with private/self-signed servers. Verified TLS remains the default. Neither TLS mode falls back to plaintext.
- Support optional locally saved custom CA PEM bundles for verified PostgreSQL connections.
- Distinguish TLS, network and authentication errors with actionable messages.
- Keep PostgreSQL-only TLS options isolated from MySQL/Redis.

This preview has no Apple Developer ID/notarization or Windows Authenticode certificate, and its in-app updater is disabled. Install manually. Unverified TLS encrypts traffic but does not prevent server impersonation; choose it only for an endpoint you trust. Database contents and credentials are not included in update requests.

## Install on Mac

Download the DMG for your chip, open it, and drag buggy-table to Applications (replace the old app). This preview is ad-hoc signed, **not Apple Developer ID signed or notarized**. On first launch, macOS may block it as an unverified developer. If you trust this release, open System Settings → Privacy & Security → Open Anyway after attempting to launch it, then confirm Open. Managed Macs may prohibit this. Do not disable Gatekeeper. See [Apple’s opening instructions](https://support.apple.com/en-us/102445).

This fixes bundle integrity; it does not promise warning-free installation. Warning-free distribution requires Apple Developer ID and notarization. Saved connection settings remain in the existing app data location; do not delete that folder when replacing the app.
