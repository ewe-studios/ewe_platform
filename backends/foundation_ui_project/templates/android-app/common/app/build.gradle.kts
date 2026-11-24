import com.android.build.api.variant.ApplicationAndroidComponentsExtension
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.provider.Property
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.OutputDirectory
import org.gradle.process.ExecOperations
import javax.inject.Inject

plugins {
    id("com.android.application") version "9.5.1"
    id("org.jetbrains.kotlin.plugin.compose") version "2.4.21"
}
android {
    namespace = "{{ APP_ID }}"
    compileSdk { version = release(37) { minorApiLevel = 2 } }
{% if COMPOSED %}    val releaseStore = providers.environmentVariable("EWE_ANDROID_KEYSTORE")
    if (releaseStore.isPresent) {
        val releaseKey = signingConfigs.create("applicationRelease") {
            storeFile = rootProject.file(releaseStore.get())
            storePassword = providers.environmentVariable("EWE_ANDROID_STORE_PASSWORD").get()
            keyAlias = providers.environmentVariable("EWE_ANDROID_KEY_ALIAS").get()
            keyPassword = providers.environmentVariable("EWE_ANDROID_KEY_PASSWORD").get()
        }
        buildTypes.getByName("release").signingConfig = releaseKey
    }
{% endif %}    defaultConfig {
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

abstract class StageAppResources @Inject constructor(private val commands: ExecOperations) : DefaultTask() {
    @get:Internal abstract val packageDirectory: DirectoryProperty
    @get:Input abstract val cargoProfile: Property<String>
    @get:Internal abstract val producerDirectory: DirectoryProperty
    @get:OutputDirectory abstract val assets: DirectoryProperty

    @TaskAction
    fun stage() {
        commands.exec {
            workingDir(packageDirectory.get().asFile)
            environment("EWE_RESOURCE_TARGET", "android")
            environment("EWE_RESOURCE_DEST", assets.dir("EweResources").get().asFile)
            environment("CARGO_TARGET_DIR", producerDirectory.get().asFile)
            commandLine("cargo", "check", "--profile", cargoProfile.get(), "--manifest-path", "Cargo.toml", "--no-default-features", "--lib")
        }.assertNormalExitValue()
    }
}

extensions.getByType<ApplicationAndroidComponentsExtension>().onVariants { variant ->
    val task = tasks.register<StageAppResources>("stage${variant.name.replaceFirstChar { it.uppercaseChar() }}Resources") {
        cargoProfile.set(providers.gradleProperty("eweResourceCargoProfile").orElse(
            if (variant.buildType == "release") "release" else "dev"
        ))
        packageDirectory.set(layout.projectDirectory.dir("../{{ RESOURCE_PACKAGE }}"))
        producerDirectory.set(layout.buildDirectory.dir("resource-producer/${variant.name}"))
        assets.set(layout.buildDirectory.dir("generated/ewe-assets/${variant.name}"))
        // Cargo tracks the manifest, directories and generated inputs.
        outputs.upToDateWhen { false }
    }
    variant.sources.assets?.addGeneratedSourceDirectory(task, StageAppResources::assets)
}

{% if COMPOSED and HOST == "on-device" %}
extensions.getByType<ApplicationAndroidComponentsExtension>().onVariants { variant ->
    val variantName = variant.name.replaceFirstChar { it.uppercaseChar() }
    val native = tasks.register<Exec>("prepare${variantName}Rust") {
        workingDir(rootProject.projectDir.parentFile)
        commandLine(providers.environmentVariable("EWE_UI_BIN").getOrElse("ewe_ui"), "task", "run", "app:prepare", "--target", "android")
        if (variant.buildType == "release") args("--release")
    }
    tasks.matching { it.name == "pre${variantName}Build" }.configureEach { dependsOn(native) }
}
{% endif %}
