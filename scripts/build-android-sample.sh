#!/usr/bin/env bash
# Build the sample app APK without Gradle: kotlinc (the library + the app) → d8 → aapt2 → the native
# libraries from scripts/build-android.sh → zipalign → apksigner (a local debug key). Uses the kotlinc
# and JDK bundled with Android Studio (or KOTLINC / JAVA_HOME).
set -euo pipefail
cd "$(dirname "$0")/.."
STUDIO="/Applications/Android Studio.app/Contents"
export JAVA_HOME="${JAVA_HOME:-$STUDIO/jbr/Contents/Home}"
export PATH="$JAVA_HOME/bin:$PATH"
KOTLINC="${KOTLINC:-$STUDIO/plugins/Kotlin/kotlinc}"
PLATFORM="$(ls -d "$ANDROID_HOME"/platforms/android-34 2>/dev/null || ls -d "$ANDROID_HOME"/platforms/android-* | sort -V | tail -1)"
BT="$(ls -d "$ANDROID_HOME"/build-tools/* | sort -V | tail -1)"
OUT=out/android-sample
rm -rf "$OUT" && mkdir -p "$OUT/classes" "$OUT/dex" "$OUT/apk/lib"
bash "$KOTLINC/bin/kotlinc" -cp "$PLATFORM/android.jar" -d "$OUT/classes" android/datars/src/main/java/dev/datars/*.kt android/sample/src/main/java/dev/datars/sample/*.kt 2>&1 | grep -v "^warning" || true
"$BT/d8" --release --min-api 24 --lib "$PLATFORM/android.jar" --output "$OUT/dex" $(find "$OUT/classes" -name '*.class') "$KOTLINC/lib/kotlin-stdlib.jar"
"$BT/aapt2" link -I "$PLATFORM/android.jar" --manifest android/sample/src/main/AndroidManifest.xml -o "$OUT/base.apk"
cp -r android/datars/src/main/jniLibs/* "$OUT/apk/lib/"
cp "$OUT/dex/classes.dex" "$OUT/apk/"
(cd "$OUT/apk" && zip -qr ../base.apk classes.dex lib)
"$BT/zipalign" -f -p 4 "$OUT/base.apk" "$OUT/aligned.apk"
# One debug key, kept across builds (a new key per build makes `adb install -r` fail).
KEY="${DATARS_DEBUG_KEYSTORE:-out/android-debug.keystore}"
[ -f "$KEY" ] || keytool -genkeypair -keystore "$KEY" -storepass android -keypass android -alias debug -keyalg RSA -validity 3650 -dname "CN=datars debug" >/dev/null 2>&1
"$BT/apksigner" sign --ks "$KEY" --ks-pass pass:android --out "$OUT/datars-sample.apk" "$OUT/aligned.apk"
echo "$OUT/datars-sample.apk ($(du -h "$OUT/datars-sample.apk" | cut -f1))"
