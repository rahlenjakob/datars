#!/usr/bin/env bash
# Run the C-ABI golden test (crates/datars-ffi/tests/goldens.rs) on an Android device or emulator:
# P1 on Android. Boots the first AVD headless if no device is attached.
set -euo pipefail
cd "$(dirname "$0")/.."
NDK="${ANDROID_NDK_HOME:-$(ls -d "$ANDROID_HOME"/ndk/* | sort -V | tail -1)}"
HOSTTAG="$(uname -s | tr A-Z a-z)-x86_64"
TOOLCHAIN="$NDK/toolchains/llvm/prebuilt/$HOSTTAG/bin"
ADB="$ANDROID_HOME/platform-tools/adb"
API=24
TARGET=aarch64-linux-android
export CC_aarch64_linux_android="$TOOLCHAIN/${TARGET}${API}-clang" AR_aarch64_linux_android="$TOOLCHAIN/llvm-ar"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$TOOLCHAIN/${TARGET}${API}-clang"
export LIBCLANG_PATH="${LIBCLANG_PATH:-$(xcode-select -p 2>/dev/null)/Toolchains/XcodeDefault.xctoolchain/usr/lib}"
export BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android="--sysroot=$NDK/toolchains/llvm/prebuilt/$HOSTTAG/sysroot --target=$TARGET$API"
BIN=$(cargo test --release -p datars-ffi --test goldens --target $TARGET --no-run --message-format=json 2>/dev/null | grep '"executable":"' | sed 's/.*"executable":"\([^"]*\)".*/\1/' | tail -1)
echo "test binary: $BIN"
if [ -z "$("$ADB" devices | sed 1d | grep -w device)" ]; then
  AVD=$("$ANDROID_HOME/emulator/emulator" -list-avds | head -1)
  echo "booting $AVD …"
  nohup "$ANDROID_HOME/emulator/emulator" -avd "$AVD" -no-window -no-audio -no-boot-anim -no-snapshot-save >/dev/null 2>&1 &
  "$ADB" wait-for-device
  until [ "$("$ADB" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]; do sleep 2; done
fi
D=/data/local/tmp/datars
"$ADB" shell "rm -rf $D && mkdir -p $D/tests"
"$ADB" push "$BIN" "$D/goldens" >/dev/null
"$ADB" push examples "$D/" >/dev/null
"$ADB" push tests/golden "$D/tests/" >/dev/null
"$ADB" shell "cd $D && chmod +x goldens && DATARS_ROOT=$D ./goldens --nocapture"
