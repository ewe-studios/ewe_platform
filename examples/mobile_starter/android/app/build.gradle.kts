plugins {
    id("com.android.application") version "9.5.1"
    id("org.jetbrains.kotlin.plugin.compose") version "2.4.21"
}
android {
    namespace = "ewe.starter"
    compileSdk { version = release(37) { minorApiLevel = 2 } }
    defaultConfig { applicationId = "ewe.starter"; minSdk = 26; targetSdk = 36; versionCode = 1; versionName = "0.1.0" }
    buildFeatures { compose = true }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
}
dependencies {
    implementation("${providers.gradleProperty("eweGroup").get()}:ewe-ui:${providers.gradleProperty("eweVersion").get()}")
    implementation("androidx.activity:activity-compose:1.13.0")
}
