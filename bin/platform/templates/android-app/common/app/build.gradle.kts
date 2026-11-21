plugins {
    id("com.android.application") version "9.5.1"
    id("org.jetbrains.kotlin.plugin.compose") version "2.4.21"
}
android {
    namespace = "{{ APP_ID }}"
    compileSdk { version = release(37) { minorApiLevel = 2 } }
    defaultConfig {
        applicationId = "{{ APP_ID }}"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "{{ VERSION }}"
        ndk { abiFilters += setOf("arm64-v8a", "x86_64") }
    }
{% if HOST == "on-device" %}    sourceSets["main"].kotlin.directories.add("generated/kotlin")
    sourceSets["main"].jniLibs.directories.add("generated/jniLibs")
{% endif %}    buildFeatures { compose = true }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
}
dependencies {
    implementation("${providers.gradleProperty("eweGroup").get()}:ewe-ui:${providers.gradleProperty("eweVersion").get()}")
{% if HOST == "on-device" %}    implementation("net.java.dev.jna:jna:5.19.1@aar")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.11.0")
{% endif %}    implementation("androidx.activity:activity-compose:1.13.0")
}
