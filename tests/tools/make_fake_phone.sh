#!/usr/bin/env bash
# Builds a fake Android phone for testing RecovMe without a real device:
#   <dir>/bin/adb      a fake adb (devices -l, shell, pull) serving <dir>/storage
#   <dir>/storage/...  the phone's files: trash, gallery bins, thumbnails, chats
# Use:  bash make_fake_phone.sh /path/fakephone
#       PATH=/path/fakephone/bin:$PATH recovme-cli adb:R58N123ABC <save-folder> - photos,videos
set -euo pipefail
F="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
rm -rf "$F"; mkdir -p "$F/bin"
cat > "$F/bin/adb" <<ADB
#!/bin/bash
FAKE="$F"
ADB
cat >> "$F/bin/adb" <<'ADB'
[ "$1" = "-s" ] && { serial=$2; shift 2; }
case "$1" in
  devices) printf 'List of devices attached\nR58N123ABC             device usb:1-1 product:x model:SM_S911B device:dm1q transport_id:1\nZY22XYZ                unauthorized usb:1-2 transport_id:2\n\n' ;;
  shell)
    script="${2//\/storage/$FAKE/storage}"
    getprop() { case "$1" in ro.product.manufacturer) echo samsung;; ro.product.model) echo SM-S911B;; ro.build.version.release) echo 14;; esac; }
    export -f getprop
    bash -c "$script" ;;
  pull)
    shift; [ "$1" = "-a" ] && shift
    args=("$@"); dest="${args[-1]}"; unset 'args[-1]'; rc=0
    for f in "${args[@]}"; do
      case "$f" in *FAIL*) echo "adb: error: failed to stat remote object '$f'" >&2; exit 1;; esac
      cp -p "$f" "$dest/" || rc=1
    done; exit $rc ;;
esac
ADB
chmod +x "$F/bin/adb"
jpg() { mkdir -p "$(dirname "$1")"; python3 -c "import sys; open(sys.argv[1],'wb').write(b'\xff\xd8\xff\xe0'+bytes([int(sys.argv[2])])*30000+b'\xff\xd9')" "$1" "$2"; }
R="$F/storage/emulated/0"
jpg "$R/DCIM/Camera/.trashed-1712345678-IMG_20240101.jpg" 1         # system trash, name restored
jpg "$R/DCIM/Camera/IMG_keep.jpg" 2                                 # only with "everything"
jpg "$R/Android/.Trash/com.sec.android.gallery3d/1651234567" 3      # no extension: sniffed as JPEG
jpg "$R/DCIM/.thumbnails/123.jpg" 4
touch "$R/DCIM/.thumbnails/.nomedia"                                # skipped
jpg "$R/Android/media/com.whatsapp/WhatsApp/Media/WhatsApp Images/IMG-1.jpg" 5
jpg "$R/Android/media/com.whatsapp/WhatsApp/Media/WhatsApp Images/FAIL.jpg" 6   # copy fails -> note
jpg "$R/WhatsApp/Media/WhatsApp Images/IMG-1.jpg" 7                 # same name, other folder: both kept
mkdir -p "$R/Android/media/com.whatsapp/WhatsApp/Media/.Statuses"
printf '\0\0\0\x18ftypmp42xxxxxxxx' > "$R/Android/media/com.whatsapp/WhatsApp/Media/.Statuses/s1.mp4"
jpg "$F/storage/ABCD-1234/DCIM/Camera/.trashed-1700000000-card.jpg" 8           # memory card in the phone
mkdir -p "$R/Download"; echo "%PDF-1.4 x" > "$R/Download/doc.pdf"
jpg "$R/Android/.Trash/com.sec.android.app.myfiles/abc123/1712345678901/storage/emulated/0/Download/.!%#@\$/invoice.jpg" 9
jpg "$R/DCIM/Camera/.pending-1712345678-IMG_half.jpg" 10           # unfinished photo
jpg "$R/Android/data/org.telegram.messenger/cache/-123456_99.jpg" 11 # Telegram cache
echo "fake phone ready in $F"
