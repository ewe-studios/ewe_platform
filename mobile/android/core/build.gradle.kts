plugins { alias(libs.plugins.jvm); `java-library`; `maven-publish` }
val isolatedSources = tasks.register<Sync>("isolateFlatBuffers") {
    from("src/generated/kotlin") { into("kotlin") }
    from("src/vendor/java") {
        into("java")
        eachFile { path = path.replace("com/google/flatbuffers", "ewe/ui/flatbuffers") }
    }
    filter { line: String -> line.replace("com.google.flatbuffers", "ewe.ui.flatbuffers") }
    into(layout.buildDirectory.dir("isolated-flatbuffers"))
}
kotlin {
    jvmToolchain(25)
    compilerOptions.jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
    sourceSets.main { kotlin.srcDir(layout.buildDirectory.dir("isolated-flatbuffers/kotlin")) }
}
java { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17; withSourcesJar() }
sourceSets.main { java.srcDir(layout.buildDirectory.dir("isolated-flatbuffers/java")) }
tasks.named("compileKotlin") { dependsOn(isolatedSources) }
tasks.named("sourcesJar") { dependsOn(isolatedSources) }
tasks.processResources { from("src/vendor/LICENSE-flatbuffers.txt") { into("META-INF") } }
dependencies {

    api(libs.runtime)
    api(libs.coroutines)
    api(libs.json)
    testImplementation(libs.junit)
    testImplementation("com.google.flatbuffers:flatbuffers-java:25.2.10")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}
tasks.test { useJUnitPlatform() }
publishing { publications { create<MavenPublication>("library") { artifactId = "ewe-ui-core"; from(components["java"]) } } }
