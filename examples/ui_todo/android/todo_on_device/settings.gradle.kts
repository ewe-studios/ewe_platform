pluginManagement { repositories { google(); mavenCentral(); gradlePluginPortal() } }
dependencyResolutionManagement {
    repositories {
        providers.gradleProperty("eweRepository").orNull?.let { maven(url = uri(it)) }
        google()
        mavenCentral()
    }
}
rootProject.name = "TodoOnDevice"
include(":app")
