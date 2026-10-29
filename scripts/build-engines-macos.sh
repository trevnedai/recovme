#!/usr/bin/env bash
# Builds the two recovery engines for macOS (Apple Silicon) from source and
# drops them into src-tauri/resources/engines so the app bundle carries them.
#   PhotoRec   – CGSecurity testdisk repo, pinned commit (7.3-WIP, newest file signatures)
#   tsk_recover – The Sleuth Kit release tarball
# Both are linked only against macOS system libraries (libjpeg is linked statically).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/src-tauri/resources/engines"
TESTDISK_COMMIT="${TESTDISK_COMMIT:-f9e57dc98be47a952078e6a7f54cb134bddfb141}"
TSK_VERSION="${TSK_VERSION:-4.15.0}"
WORK="${RUNNER_TEMP:-/tmp}/recovme-engines"
JOBS="$(sysctl -n hw.ncpu 2>/dev/null || echo 4)"
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-11.0}"

# Keep a log, and on failure post its last lines as a GitHub annotation
# (annotations are readable through the API even when raw logs are not).
LOG="${RUNNER_TEMP:-/tmp}/recovme-engines.log"
: > "$LOG"
exec 3>&1
exec > >(tee -a "$LOG") 2>&1
on_err() {
  local line=$1 code=$2
  sleep 1
  if [ -n "${GITHUB_ACTIONS:-}" ]; then
    {
      printf '::error title=Engine build failed (line %s, exit %s)::' "$line" "$code"
      tail -n 45 "$LOG" | sed 's/%/%25/g' | awk '{printf "%s%%0A", $0}'
      printf '\n'
    } >&3
  fi
}
trap 'on_err $LINENO $?' ERR

rm -rf "$WORK" "$OUT/photorec" "$OUT/tsk"
mkdir -p "$WORK/static" "$OUT/photorec" "$OUT/tsk"

brew list autoconf >/dev/null 2>&1 || brew install autoconf
brew list automake >/dev/null 2>&1 || brew install automake
brew list libtool >/dev/null 2>&1 || brew install libtool
brew list pkg-config >/dev/null 2>&1 || brew install pkg-config
brew list jpeg-turbo >/dev/null 2>&1 || brew install jpeg-turbo
JPEG="$(brew --prefix jpeg-turbo)"
cp "$JPEG/lib/libjpeg.a" "$WORK/static/"

echo "── PhotoRec ($TESTDISK_COMMIT) ──"
git clone --quiet https://github.com/cgsecurity/testdisk.git "$WORK/testdisk"
cd "$WORK/testdisk"
git checkout --quiet "$TESTDISK_COMMIT"
autoreconf --install --force -W all -I config
./configure --disable-qt --without-ntfs --without-ntfs3g --without-ewf --without-ext2fs \
  --without-reiserfs --without-iconv --without-uuid --enable-missing-uuid-ok \
  --with-jpeg-includes="$JPEG/include" --with-jpeg-lib="$WORK/static"
make -j"$JOBS" -C src photorec || make -j"$JOBS"
cp src/photorec "$OUT/photorec/photorec"
cp COPYING "$OUT/photorec/COPYING.txt"

echo "── The Sleuth Kit $TSK_VERSION ──"
cd "$WORK"
curl -fsSL -o tsk.tgz "https://github.com/sleuthkit/sleuthkit/releases/download/sleuthkit-$TSK_VERSION/sleuthkit-$TSK_VERSION.tar.gz"
tar xzf tsk.tgz
cd "sleuthkit-$TSK_VERSION"
./configure --disable-java --disable-cppunit --disable-shared --enable-static \
  --without-afflib --without-libewf --without-libvhdi --without-libvmdk --without-libvslvm --without-libbfio
make -j"$JOBS"
cp tools/autotools/tsk_recover "$OUT/tsk/tsk_recover"
[ -d licenses ] && cp -R licenses "$OUT/tsk/licenses" || true

chmod 755 "$OUT/photorec/photorec" "$OUT/tsk/tsk_recover"

echo "── Checking the engines are self-contained ──"
for b in "$OUT/photorec/photorec" "$OUT/tsk/tsk_recover"; do
  otool -L "$b"
  if otool -L "$b" | tail -n +2 | grep -vE "^[[:space:]]*/(usr/lib|System)/" | grep -q .; then
    echo "ERROR: $b links a library that won't exist on users' Macs" >&2
    exit 1
  fi
done
"$OUT/photorec/photorec" /version
"$OUT/tsk/tsk_recover" -V
echo "Engines ready in $OUT"
