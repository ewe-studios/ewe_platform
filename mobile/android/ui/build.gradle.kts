plugins { alias(libs.plugins.android.library); alias(libs.plugins.compose); `maven-publish` }
android {
    namespace = "io.github.ewe_studios.ui.ui"
    compileSdk { version = release(37) { minorApiLevel = 2 } }
    testOptions.unitTests.all { it.useJUnitPlatform() }
    sourceSets["main"].kotlin.directories.add("src/generated/kotlin")
    defaultConfig {
        minSdk = 26
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
    buildFeatures { compose = true }
    publishing { singleVariant("release") { withSourcesJar() } }
}
kotlin { compilerOptions.jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17) }
dependencies {
    api(project(":core"))
    api(project(":binding"))
    api(platform("androidx.compose:compose-bom:${libs.versions.compose.get()}"))
    api("androidx.compose.foundation:foundation")
    api("androidx.compose.material3:material3")
    api(libs.okhttp)
    implementation(libs.coroutines.android)
    androidTestImplementation(platform("androidx.compose:compose-bom:${libs.versions.compose.get()}"))
    androidTestImplementation("androidx.compose.ui:ui-test-junit4")
    androidTestImplementation(libs.androidx.test.runner)
    debugImplementation("androidx.compose.ui:ui-test-manifest")
    testImplementation(libs.junit)
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}
afterEvaluate { publishing { publications { create<MavenPublication>("library") { artifactId = "ewe-ui"; from(components["release"]) } } } }
