plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.ewe.platform.capability"
    compileSdk = 36
    defaultConfig {
        minSdk = 24
    }
    kotlinOptions {
        jvmTarget = "11"
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
}

dependencies {
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.webkit:webkit:1.14.0")
    // The app's tauri-android module provides app.tauri.annotation.* and app.tauri.plugin.*
    compileOnly(project(":tauri-android"))
}
