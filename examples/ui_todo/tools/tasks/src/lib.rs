//! Application-owned build and device checks for Todo and the release starters.
use foundation_tasks::{TaskError, TaskResult, TaskSet};
use foundation_ui_native::tasks::{output, stage, swift, sync, NativeBuildEnv};
mod android;
mod android_device;
mod android_report;
mod android_screen;
mod device;
mod ios_device;
mod seed;
mod todo;

#[derive(clap::Args)]
struct Seed {
    url: String,
}
/// Load the environment used by the example device tests.
#[must_use]
pub fn environment() -> NativeBuildEnv {
    let mut env = NativeBuildEnv::from_workspace();
    env.devices.extend_from_host(&[
        "EWE_ANDROID_BOOT_TIMEOUT",
        "EWE_ANDROID_VIEW_TIMEOUT",
        "EWE_ANDROID_VIEW_POLL_MS",
        "EWE_ANDROID_SERIAL",
        "EWE_THEME_CAPTURES",
        "EWE_TODO_RPC",
        "EWE_TODO_SOCKET",
        "EWE_TODO_BIN",
        "EWE_IOS_DEVICE",
        "EWE_SIMCTL_BIN",
        "EWE_AXE_BIN",
    ]);
    env
}

/// Register the historical task names in the repository runner.
///
/// # Errors
/// Returns task registration conflicts.
pub fn compatibility(set: &mut TaskSet) -> TaskResult {
    foundation_ui_native::tasks::add_apple_build(set)?;
    set.add(
        "build-android",
        "Build the Android native libraries",
        android::build,
    )?;
    set.add(
        "bindings:sync-android",
        "Update or check the Android bindings",
        android::sync,
    )?;

    set.add(
        "todo:prepare-ios",
        "Prepare the Apple Todo applications",
        todo::prepare,
    )?;
    set.add(
        "todo:test-ios",
        "Test the Apple Todo applications",
        ios_device::todo,
    )?;
    set.add(
        "todo:test-android",
        "Test the Android Todo applications",
        android_device::todo,
    )?;
    set.add(
        "release:test-ios-starter",
        "Test the Apple starter application",
        ios_device::starter,
    )?;
    set.add(
        "release:test-android-starter",
        "Test the Android starter application",
        android_device::starter,
    )?;
    Ok(())
}
/// Return the project's standalone task set.
///
/// # Errors
/// Returns task registration conflicts.
pub fn set() -> Result<TaskSet, TaskError> {
    let mut set = TaskSet::new("ui_todo_tasks").with_namespace("todo");
    set.add(
        "seed-starter",
        "Add one Todo through the server action endpoint",
        |ctx, args: Seed| async move { seed::run(&ctx, &args.url).await },
    )?;
    set.add(
        "peer-android",
        "Check the Android peer screen",
        android_screen::peer,
    )?;
    set.add("build-android", "Build Android libraries", android::build)?;
    set.add(
        "sync-android",
        "Update or check the Android example bindings",
        android::sync,
    )?;
    set.add(
        "prepare-ios",
        "Prepare the Apple Todo applications",
        todo::prepare,
    )?;
    set.add(
        "test-ios",
        "Test the Apple Todo applications",
        ios_device::todo,
    )?;
    set.add(
        "test-android",
        "Test the Android Todo applications",
        android_device::todo,
    )?;
    set.add(
        "test-ios-starter",
        "Test the Apple starter application",
        ios_device::starter,
    )?;
    set.add(
        "test-android-starter",
        "Test the Android starter application",
        android_device::starter,
    )?;
    Ok(set)
}
