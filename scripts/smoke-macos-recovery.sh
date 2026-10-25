#!/bin/bash
set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "Usage: smoke-macos-recovery.sh <disk-image.dmg> <work-directory>" >&2
  exit 2
fi

dmg="$1"
work_dir="$2"
mount_point="$work_dir/mount"
config_dir="$work_dir/config"
report="$work_dir/recovery.log"
capture="$work_dir/opened-path.txt"
handler_app="$work_dir/RecoveryCapture.app"
handler_script="$work_dir/recovery-capture.applescript"
handler_bundle_id="com.bdmv-remuxer.recovery-capture"
damaged_settings='{ malformed preferences'
older_backup_contents='older recovery copy'

test -f "$dmg" || { echo "The macOS disk image is missing: $dmg" >&2; exit 1; }
command -v duti >/dev/null || { echo "duti is required to register the smoke document handler." >&2; exit 1; }
mkdir -p "$mount_point" "$config_dir"
hdiutil attach -nobrowse -readonly -mountpoint "$mount_point" "$dmg" >/dev/null

cleanup() {
  hdiutil detach "$mount_point" -quiet || hdiutil detach -force "$mount_point" -quiet || true
}
trap cleanup EXIT

app="$(find "$mount_point" -maxdepth 2 -type d -name '*.app' -print -quit)"
binary="$app/Contents/MacOS/bdmv-remuxer"
test -n "$app" && test -x "$binary" || {
  echo "The mounted disk image does not contain a runnable BDMV Remuxer app bundle." >&2
  exit 1
}

# Register a temporary document handler so /usr/bin/open delivers the exact file
# URL to a capture file instead of launching a user-facing text editor.
cat >"$handler_script" <<APPLESCRIPT
on open droppedItems
  set backupPath to POSIX path of (item 1 of droppedItems)
  set captureFile to POSIX file "$capture"
  set captureHandle to open for access captureFile with write permission
  try
    set eof captureHandle to 0
    write backupPath to captureHandle
    close access captureHandle
  on error errorMessage number errorNumber
    try
      close access captureHandle
    end try
    error errorMessage number errorNumber
  end try
  quit
end open
APPLESCRIPT
touch "$capture"
osacompile -o "$handler_app" "$handler_script"

info_plist="$handler_app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleIdentifier $handler_bundle_id" "$info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleDocumentTypes array" "$info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleDocumentTypes:0 dict" "$info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleDocumentTypes:0:CFBundleTypeName string BDMV-Recovery-Smoke-Backup" "$info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleDocumentTypes:0:CFBundleTypeExtensions array" "$info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleDocumentTypes:0:CFBundleTypeExtensions:0 string 1" "$info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleDocumentTypes:0:CFBundleTypeRole string Editor" "$info_plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleDocumentTypes:0:LSHandlerRank string Owner" "$info_plist"

lsregister="/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister"
"$lsregister" -f "$handler_app"
duti -s "$handler_bundle_id" .1 all
handler_info="$(duti -x settings.json.corrupt.1)"
registered_handler="${handler_info%%$'\n'*}"
if [ "$registered_handler" != "$handler_bundle_id" ]; then
  echo "The recovery smoke document handler was not registered for .1 files: $registered_handler" >&2
  exit 1
fi

settings="$config_dir/settings.json"
older_backup="$config_dir/settings.json.corrupt"
expected_backup="$config_dir/settings.json.corrupt.1"
printf '%s' "$damaged_settings" >"$settings"
printf '%s' "$older_backup_contents" >"$older_backup"
older_backup_hash="$(shasum -a 256 "$older_backup" | awk '{print $1}')"
rm -f "$report" "$capture"
touch "$capture"

open -n "$app" --args --smoke-settings-recovery "$config_dir" "$report"
for _ in {1..60}; do
  [ -f "$report" ] && break
  sleep 1
done
test -f "$report" || { echo "The app bundle did not create its recovery report." >&2; exit 1; }
cat "$report"
grep -Fq "Opened recovered preferences backup without changing it" "$report" || {
  echo "The app bundle's recovery/open smoke reported a failure." >&2
  exit 1
}
grep -Fq "$expected_backup" "$report" || {
  echo "The app bundle did not report the newly preserved backup path: $expected_backup" >&2
  exit 1
}

handler_received_path=false
for _ in {1..30}; do
  if [ -f "$capture" ] && [ "$(cat "$capture")" = "$expected_backup" ]; then
    handler_received_path=true
    break
  fi
  sleep 1
done
if [ "$handler_received_path" != true ]; then
  actual_path="$(cat "$capture" 2>/dev/null || echo 'The registered macOS document handler was never launched.')"
  echo "The mounted app did not pass the preserved backup path to /usr/bin/open. Expected '$expected_backup'; received '$actual_path'." >&2
  exit 1
fi

if [ "$(shasum -a 256 "$older_backup" | awk '{print $1}')" != "$older_backup_hash" ]; then
  echo "Opening the recovered backup changed the pre-existing backup." >&2
  exit 1
fi
if [ "$(cat "$expected_backup")" != "$damaged_settings" ]; then
  echo "The newly preserved backup contents changed during the open check." >&2
  exit 1
fi

echo "The app bundle opened the newly preserved backup through /usr/bin/open, and both backup files remain unchanged."
