plugins { alias(libs.plugins.android.library);  `maven-publish` }
android {
    namespace = "io.github.ewe_studios.ui.binding"
    compileSdk { version = release(37) { minorApiLevel = 2 } }
    sourceSets["main"].kotlin.directories.add("src/generated/kotlin")
    defaultConfig { minSdk = 26 }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }

    publishing { singleVariant("release") { withSourcesJar() } }
}
kotlin { compilerOptions.jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17) }
dependencies {
    api(project(":core"))
    implementation("net.java.dev.jna:jna:${libs.versions.jna.get()}@aar")
}
afterEvaluate { publishing { publications { create<MavenPublication>("library") { artifactId = "ewe-ui-binding"; from(components["release"]) } } } }
