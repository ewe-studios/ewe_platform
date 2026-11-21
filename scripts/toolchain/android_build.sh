#!/usr/bin/env bash
set -euo pipefail

: "${ANDROID_HOME:?mise must resolve the android-sdk tool first}"
: "${ANDROID_COMPILE_SDK:?run this script with mise}"
: "${ANDROID_BUILD_TOOLS:?run this script with mise}"
: "${ANDROID_GRADLE_PLUGIN:?run this script with mise}"
: "${ANDROID_KOTLIN_VERSION:?mise must resolve the kotlin tool first}"

build_tools="$ANDROID_HOME/build-tools/$ANDROID_BUILD_TOOLS"
if [[ ! -x "$build_tools/aapt" || ! -x "$build_tools/apksigner" ]]; then
  printf 'FAIL android build: run mise run toolchain:android to install build tools\n' >&2
  exit 1
fi

project=$(mktemp -d "${TMPDIR:-/tmp}/ewe-android-build.XXXXXX")
trap 'find "$project" -depth -delete' EXIT
source="$project/app/src/main"
mkdir -p "$source/kotlin/org/eweplatform/toolchain"
cat > "$project/settings.gradle" <<'GRADLE'
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
    }
}
rootProject.name = 'toolchain-smoke'
include 'app'
GRADLE

# Use the same Kotlin release for the standalone compiler and Gradle plugin.
cat > "$project/build.gradle" <<GRADLE
buildscript {
    repositories {
        google()
        mavenCentral()
    }
    dependencies {
        classpath 'com.android.tools.build:gradle:$ANDROID_GRADLE_PLUGIN'
        classpath 'org.jetbrains.kotlin:kotlin-gradle-plugin:$ANDROID_KOTLIN_VERSION'
    }
}
GRADLE

cat > "$project/app/build.gradle" <<GRADLE
plugins {
    id 'com.android.application'
}

android {
    namespace = 'org.eweplatform.toolchain'
    compileSdk = $ANDROID_COMPILE_SDK
    buildToolsVersion = '$ANDROID_BUILD_TOOLS'

    defaultConfig {
        applicationId = 'org.eweplatform.toolchain'
        minSdk = 26
        targetSdk = $ANDROID_COMPILE_SDK
    }
}
GRADLE

cat > "$source/AndroidManifest.xml" <<'XML'
<manifest xmlns:android="http://schemas.android.com/apk/res/android">
    <application android:label="Toolchain Smoke Test">
        <activity android:name=".MainActivity" android:exported="true">
            <intent-filter>
                <action android:name="android.intent.action.MAIN"/>
                <category android:name="android.intent.category.LAUNCHER"/>
            </intent-filter>
        </activity>
    </application>
</manifest>
XML

cat > "$source/kotlin/org/eweplatform/toolchain/MainActivity.kt" <<'KOTLIN'
package org.eweplatform.toolchain

import android.app.Activity
import android.os.Bundle
import android.widget.TextView

class MainActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(TextView(this).apply { text = "Android toolchain smoke test" })
    }
}
KOTLIN

gradle --no-daemon --console=plain --warning-mode all --project-dir "$project" :app:assembleDebug

apk="$project/app/build/outputs/apk/debug/app-debug.apk"
badging=$("$build_tools/aapt" dump badging "$apk")
if [[ "$badging" != *"package: name='org.eweplatform.toolchain'"* ||
      "$badging" != *"launchable-activity: name='org.eweplatform.toolchain.MainActivity'"* ]]; then
  printf 'FAIL android build: the package or launcher is absent\n' >&2
  exit 1
fi
unzip -t "$apk" AndroidManifest.xml classes.dex
"$build_tools/apksigner" -J-enable-native-access=ALL-UNNAMED verify "$apk"
printf 'PASS android build (Android Gradle Plugin %s, platform %s)\n' \
  "$ANDROID_GRADLE_PLUGIN" "$ANDROID_COMPILE_SDK"
