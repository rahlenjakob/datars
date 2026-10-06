#!/usr/bin/env bash
# Build libdatars_ffi.so for Android ABIs into android/datars/src/main/jniLibs.
#   rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
set -euo pipefail
cd "$(dirname "$0")/.."
NDK="${ANDROID_NDK_HOME:-$(ls -d "$ANDROID_HOME"/ndk/* | sort -V | tail -1)}"
TOOLCHAIN="$NDK/toolchains/llvm/prebuilt/$(uname -s | tr A-Z a-z)-x86_64/bin"
API=24
# The sandbox (QuickJS, for T3 bundles) is in by default; the app's policy decides at runtime.
# Inter is compiled in for raw documents (bundles carry their own fonts). `gpu` draws frames with
# Vulkan or GLES on the view's Surface (wgpu); without it frames are CPU pixels.
FEATURES="${FEATURES-sandbox,bundled-fonts,gpu}"
# QuickJS bindings are generated for Android: bindgen needs libclang and the NDK sysroot.
export LIBCLANG_PATH="${LIBCLANG_PATH:-$(xcode-select -p 2>/dev/null)/Toolchains/XcodeDefault.xctoolchain/usr/lib}"
SYSROOT="$NDK/toolchains/llvm/prebuilt/$(uname -s | tr A-Z a-z)-x86_64/sysroot"
build() {
  local target=$1 abi=$2 clang=$3
  export CC_${target//-/_}="$TOOLCHAIN/$clang"
  export AR_${target//-/_}="$TOOLCHAIN/llvm-ar"
  local upper=$(echo "${target//-/_}" | tr a-z A-Z)
  export CARGO_TARGET_${upper}_LINKER="$TOOLCHAIN/$clang"
  export BINDGEN_EXTRA_CLANG_ARGS_${target//-/_}="--sysroot=$SYSROOT --target=$target$API"
  cargo build -p datars-ffi --release --target "$target" --no-default-features --features "$FEATURES"
  mkdir -p "android/datars/src/main/jniLibs/$abi"
  cp "target/$target/release/libdatars_ffi.so" "android/datars/src/main/jniLibs/$abi/"
  "$TOOLCHAIN/llvm-strip" "android/datars/src/main/jniLibs/$abi/libdatars_ffi.so"
}
build aarch64-linux-android arm64-v8a "aarch64-linux-android${API}-clang"
build x86_64-linux-android x86_64 "x86_64-linux-android${API}-clang"
echo "built android/datars/src/main/jniLibs/*/libdatars_ffi.so"
# Every `external fun` in Native.kt must have its JNI symbol (a missing one is a runtime crash).
for f in $(grep -o "external fun [a-zA-Z]*" android/datars/src/main/java/dev/datars/Native.kt | awk '{print $3}'); do
  "$TOOLCHAIN/llvm-nm" -D --defined-only android/datars/src/main/jniLibs/arm64-v8a/libdatars_ffi.so | grep -q "Java_dev_datars_Native_$f$" || { echo "missing JNI symbol for Native.$f" >&2; exit 1; }
done
echo "JNI: every Native.kt declaration is exported"
