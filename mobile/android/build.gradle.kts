plugins {
    alias(libs.plugins.jvm) apply false
    alias(libs.plugins.android.library) apply false
    alias(libs.plugins.compose) apply false
}
allprojects {
    group = providers.gradleProperty("mavenGroup").get()
    version = providers.gradleProperty("packageVersion").get()
}

subprojects {
    plugins.withId("maven-publish") {
        val reference = tasks.register<Jar>("apiReferenceJar") {
            archiveClassifier.set("javadoc")
            from("docs")
        }
        extensions.configure<PublishingExtension> {
            publications.withType<MavenPublication>().configureEach {
                artifact(reference)
                pom {
                    name.set("Ewe UI ${project.name}")
                    description.set("Ewe UI native ${project.name} for Android and Kotlin consumers.")
                    url.set("https://github.com/ewe-studios/ewe_platform")
                    licenses { license { name.set("Apache License, Version 2.0"); url.set("https://www.apache.org/licenses/LICENSE-2.0") } }
                    developers { developer { id.set("ewe-studios"); name.set("Ewe Studios") } }
                    scm {
                        url.set("https://github.com/ewe-studios/ewe_platform")
                        connection.set("scm:git:https://github.com/ewe-studios/ewe_platform.git")
                        developerConnection.set("scm:git:ssh://git@github.com/ewe-studios/ewe_platform.git")
                    }
                }
            }
        }
    }
}
