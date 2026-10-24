#!/usr/bin/env bash
# Builds Lumen-<version>.apk without Gradle: cargo-ndk for the Rust code, javac+d8 for the two tiny Java files,
# aapt2 for resources, zipalign + apksigner to finish.
# Signing: set LUMEN_KS, LUMEN_KS_PASS, LUMEN_KEY_ALIAS, LUMEN_KEY_PASS (release), otherwise the debug key is used.
set -euo pipefail
cd "$(dirname "$0")/.."
SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/sdk}}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$(ls -d "$SDK"/ndk/* | sort -V | tail -1)}"
BT="$(ls -d "$SDK"/build-tools/* | sort -V | tail -1)"
JAR="$SDK/platforms/android-35/android.jar"
VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
CODE="$(echo "$VERSION" | awk -F. '{print $1*10000+$2*100+$3}')"
OUT="Lumen-$VERSION.apk"

rm -rf build && mkdir -p build/classes build/dex build/apk
cargo ndk -t arm64-v8a -o build/jniLibs build --release
javac --release 11 -Xlint:-options -cp "$JAR" -d build/classes java/dev/voer/lumen/*.java
"$BT/d8" --lib "$JAR" --release --min-api 26 --output build/dex build/classes/dev/voer/lumen/*.class
"$BT/aapt2" compile --dir res -o build/res.zip
"$BT/aapt2" link -o build/base.apk -I "$JAR" --manifest AndroidManifest.xml --min-sdk-version 26 --target-sdk-version 35 \
  --version-code "$CODE" --version-name "$VERSION" build/res.zip

cp build/base.apk build/unaligned.apk
cp build/dex/classes.dex build/apk/classes.dex
mkdir -p build/apk/lib/arm64-v8a && cp build/jniLibs/arm64-v8a/liblumen.so build/apk/lib/arm64-v8a/
(cd build/apk && zip -q -X -0 ../unaligned.apk classes.dex lib/arm64-v8a/liblumen.so)
"$BT/zipalign" -f -P 16 4 build/unaligned.apk build/aligned.apk

if [ -n "${LUMEN_KS:-}" ]; then
  "$BT/apksigner" sign --ks "$LUMEN_KS" --ks-pass "pass:$LUMEN_KS_PASS" --ks-key-alias "$LUMEN_KEY_ALIAS" --key-pass "pass:$LUMEN_KEY_PASS" --out "$OUT" build/aligned.apk
else
  DEBUG_KS="$HOME/.android/debug.keystore"
  [ -f "$DEBUG_KS" ] || { mkdir -p "$HOME/.android"; keytool -genkeypair -keystore "$DEBUG_KS" -storepass android -keypass android -alias androiddebugkey -dname "CN=Android Debug" -keyalg RSA -keysize 2048 -validity 10000 >/dev/null 2>&1; }
  "$BT/apksigner" sign --ks "$DEBUG_KS" --ks-pass pass:android --ks-key-alias androiddebugkey --key-pass pass:android --out "$OUT" build/aligned.apk
fi
"$BT/apksigner" verify "$OUT"
echo "built $OUT ($(du -h "$OUT" | cut -f1))"
