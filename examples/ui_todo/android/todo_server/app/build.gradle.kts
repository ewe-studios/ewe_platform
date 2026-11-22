import com.android.build.api.variant.ApplicationAndroidComponentsExtension
import org.gradle.api.file.DirectoryProperty
import org.gradle.api.tasks.Internal
import org.gradle.api.tasks.OutputDirectory
import org.gradle.process.ExecOperations
import javax.inject.Inject

plugins {
    id("com.android.application") version "9.5.1"
    id("org.jetbrains.kotlin.plugin.compose") version "2.4.21"
}
android {
    namespace = "ewe.todo"
    compileSdk { version = release(37) { minorApiLevel = 2 } }
    defaultConfig {
        testInstrumentationRunner = "ewe.todo.ReplayCheck"
        applicationId = "ewe.todo"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        ndk { abiFilters += setOf("arm64-v8a", "x86_64") }
    }
    sourceSets["main"].kotlin.directories.add("../../generated")
    buildFeatures { compose = true }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
}
dependencies {
    implementation("${providers.gradleProperty("eweGroup").get()}:ewe-ui:${providers.gradleProperty("eweVersion").get()}")
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("net.java.dev.jna:jna:5.19.1@aar")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.11.0")
}

abstract class StageTodoResources @Inject constructor(private val commands: ExecOperations) : DefaultTask() {
    @get:Internal abstract val packageDirectory: DirectoryProperty
    @get:Internal abstract val producerDirectory: DirectoryProperty
    @get:OutputDirectory abstract val assets: DirectoryProperty

    @TaskAction
    fun stage() {
        commands.exec {
            workingDir(packageDirectory.get().asFile)
            environment("EWE_RESOURCE_TARGET", "android")
            environment("EWE_RESOURCE_DEST", assets.dir("EweResources").get().asFile)
            environment("CARGO_TARGET_DIR", producerDirectory.get().asFile)
            commandLine("cargo", "check", "--manifest-path", "Cargo.toml", "--no-default-features", "--lib")
        }.assertNormalExitValue()
    }
}

extensions.getByType<ApplicationAndroidComponentsExtension>().onVariants { variant ->
    val task = tasks.register<StageTodoResources>("stage${variant.name.replaceFirstChar { it.uppercaseChar() }}Resources") {
        packageDirectory.set(layout.projectDirectory.dir("../../.."))
        producerDirectory.set(layout.buildDirectory.dir("resource-producer/${variant.name}"))
        assets.set(layout.buildDirectory.dir("generated/ewe-assets/${variant.name}"))
        // Cargo tracks the manifest, directories and generated inputs.
        outputs.upToDateWhen { false }
    }
    variant.sources.assets?.addGeneratedSourceDirectory(task, StageTodoResources::assets)
}
