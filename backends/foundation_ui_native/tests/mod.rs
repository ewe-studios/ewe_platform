mod core_tests;

#[cfg(feature = "tasks")]
mod tasks;

#[cfg(feature = "tasks")]
mod release_tasks;
mod resource_tests;

#[cfg(feature = "tasks")]
mod android_library;
#[cfg(feature = "storage")]
mod storage;
