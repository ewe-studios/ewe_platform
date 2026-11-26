plugins { alias(libs.plugins.android.library); alias(libs.plugins.compose); `maven-publish` }
android {
    namespace = "io.github.ewe_studios.ui.ui"
    compileSdk { version = release(37) { minorApiLevel = 2 } }
    sourceSets["androidTest"].assets.srcDir("../../../backends/foundation_ui_gpu/tests/shaders")
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

// Compile the independent Vulkan host only for the device test APK.
val vulkanTestLibs = layout.buildDirectory.dir("generated/vulkanTestJni")
android.sourceSets["androidTest"].jniLibs.srcDir(vulkanTestLibs.get().asFile)
val vulkanTestHost = tasks.register("compileVulkanTestHost") {
    val source = file("src/androidTest/cpp/host_vulkan.cpp")
    val headers = project(":binding").file("src/main/assets/include")
    inputs.file(source)
    inputs.dir(headers)
    outputs.dir(vulkanTestLibs)
    doLast {
        val ndk = System.getenv("ANDROID_NDK_HOME")?.let { file(it) } ?: androidComponents.sdkComponents.sdkDirectory.get().asFile.resolve("ndk").listFiles()?.filter { it.isDirectory }?.maxByOrNull { it.name }
        requireNotNull(ndk) { "The Vulkan device test needs the Android NDK" }
        val toolchain = ndk.resolve("toolchains/llvm/prebuilt").listFiles()!!.first { it.isDirectory }.resolve("bin")
        for ((abi, triple) in mapOf("arm64-v8a" to "aarch64-linux-android", "x86_64" to "x86_64-linux-android")) {
            val output = vulkanTestLibs.get().dir(abi).asFile
            output.mkdirs()
            val windows = System.getProperty("os.name").startsWith("Windows")
            val compiler = toolchain.resolve(triple + "26-clang++" + if (windows) ".cmd" else "")
            val args = listOf(compiler.path, "-std=c++17", "-shared", "-fPIC", "-static-libstdc++", "-DVK_USE_PLATFORM_ANDROID_KHR", "-I"+headers.path,
                source.path, "-lvulkan", "-landroid", "-ldl", "-o", output.resolve("libewe_gpu_test_host.so").path)
            check(ProcessBuilder(args).inheritIO().start().waitFor() == 0) { "Failed to build Vulkan test host for $abi" }
        }
    }
}
tasks.matching { it.name == "mergeDebugAndroidTestJniLibFolders" }.configureEach { dependsOn(vulkanTestHost) }
