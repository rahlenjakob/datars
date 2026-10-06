#!/usr/bin/env bash
# Compile-check the Android library's Kotlin against android.jar without Gradle, using the kotlinc
# and JDK bundled with Android Studio (or KOTLINC / JAVA_HOME). Catches API mistakes in the view
# and the JNI declarations; the JNI symbols themselves are checked by scripts/build-android.sh.
set -euo pipefail
cd "$(dirname "$0")/.."
STUDIO="/Applications/Android Studio.app/Contents"
export JAVA_HOME="${JAVA_HOME:-$STUDIO/jbr/Contents/Home}"
KOTLINC="${KOTLINC:-$STUDIO/plugins/Kotlin/kotlinc}"
PLATFORM="$(ls -d "$ANDROID_HOME"/platforms/android-* | sort -V | tail -1)"
OUT="$(mktemp -d)"
PATH="$JAVA_HOME/bin:$PATH" bash "$KOTLINC/bin/kotlinc" -Werror -cp "$PLATFORM/android.jar" -d "$OUT" android/datars/src/main/java/dev/datars/*.kt
echo "kotlin ok ($(find "$OUT" -name '*.class' | wc -l | tr -d ' ') classes, $(basename "$PLATFORM"))"
rm -rf "$OUT"
