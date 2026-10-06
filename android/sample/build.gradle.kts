// The sample app as an app developer builds it: depends on the datars library module (an AAR in a
// real app), one activity that loads a chart by URL. scripts/build-android-sample.sh builds the
// same sources without Gradle.
plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "dev.datars.sample"
    compileSdk = 34
    defaultConfig {
        applicationId = "dev.datars.sample"
        minSdk = 24
        targetSdk = 34
        versionCode = 1
        versionName = "0.1"
    }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
    kotlinOptions { jvmTarget = "17" }
}

dependencies {
    implementation(project(":datars"))
}
