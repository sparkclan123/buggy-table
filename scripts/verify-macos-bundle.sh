#!/bin/bash
# Check the actual app inside the DMG as well as the build output.
set -euo pipefail

bundle_dir=${1:?Usage: verify-macos-bundle.sh BUNDLE_DIR preview|signed}
mode=${2:?Choose preview or signed}
case "$mode" in
  preview|signed) ;;
  *) echo "Expected preview or signed" >&2; exit 1 ;;
esac

verify_app() {
  local app=$1
  codesign --verify --deep --strict --verbose=2 "$app"
  # A linker-only signature does not seal the bundle's resources.
  test -f "$app/Contents/_CodeSignature/CodeResources"
  # Exercise the actual packaged sidecar without developer PATH or Docker.
  env -i PATH=/usr/bin:/bin "$app/Contents/MacOS/buggy-vpn" --self-test
  if otool -L "$app/Contents/MacOS/buggy-vpn" | grep -E '/opt/homebrew|/usr/local|vcpkg|artifacts'; then
    echo "VPN engine contains a developer-machine library dependency" >&2
    exit 1
  fi
  if [ "$mode" = signed ]; then
    codesign --display --verbose=4 "$app" 2>&1 | grep -q '^Authority=Developer ID Application:'
    xcrun stapler validate "$app"
    spctl --assess --type execute --verbose=4 "$app"
    # Available on macOS 14+. Checks more than signature integrity alone.
    if [ -x /usr/bin/syspolicy_check ]; then
      /usr/bin/syspolicy_check distribution "$app"
    fi
  fi
}

verify_app "$bundle_dir/macos/buggy-table.app"
shopt -s nullglob
images=("$bundle_dir"/dmg/*.dmg)
if [ "${#images[@]}" -ne 1 ]; then
  echo "Expected exactly one DMG in $bundle_dir/dmg" >&2
  exit 1
fi

mount_dir=$(mktemp -d "${TMPDIR:-/tmp}/buggy-verify.XXXXXX")
mounted=false
cleanup() {
  if [ "$mounted" = true ]; then
    hdiutil detach "$mount_dir" || return 1
  fi
  rmdir "$mount_dir"
}
trap cleanup EXIT
hdiutil attach "${images[0]}" -readonly -nobrowse -mountpoint "$mount_dir"
mounted=true
verify_app "$mount_dir/buggy-table.app"
if [ "$mode" = preview ]; then
  echo "Preview integrity checks passed for the bundle and packaged app."
  echo "NOT distribution approval: ad-hoc apps may be blocked with only Move to Trash."
else
  echo "Signed macOS bundle and packaged app passed distribution policy checks."
fi
