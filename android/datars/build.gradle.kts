// datars-android — the datars runtime for Android apps. Install once; charts arrive as bundles by
// URL and update without a Play Store release. Native libraries come from scripts/build-android.sh
// (cargo + the NDK) into src/main/jniLibs/<abi>/libdatars_ffi.so.
plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "dev.datars"
    compileSdk = 34
    defaultConfig { minSdk = 24 }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
    kotlinOptions { jvmTarget = "17" }
}

// No dependencies: the view uses a background thread for network work and the main looper for
// everything that touches the engine.
