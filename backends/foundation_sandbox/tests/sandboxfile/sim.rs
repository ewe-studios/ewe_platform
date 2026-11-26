//! The executor rows of feature 14, section 7: I4, I5, I6, I11, the cache
//! scenarios, the network row, the fault table and I12.

use std::collections::HashSet;
use std::sync::Arc;

use foundation_sandbox::native::host::NativeFs;
use foundation_sandbox::native::images::build::cache::StepCache;
use foundation_sandbox::native::images::build::executor::{BuildProgress, BuildStepLine};
use foundation_sandbox::native::images::build::keys::step_key;
use foundation_sandbox::native::images::build::BuildOptions;
use foundation_sandbox::native::images::sandboxfile::Sandboxfile;
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::host::HostFs;
use foundation_sandbox::shared::limits::BuildLimits;
use foundation_sandbox::shared::rpc::messages::ExecEventKind;

use super::support::key_of;
use super::support::{base_name, one_step, write_file, Harness, TempDir};

/// Counts the `CacheHit` values of one progress list.
fn cache_hits(progress: &[BuildProgress]) -> usize {
    progress
        .iter()
        .filter(|value| matches!(value, BuildProgress::CacheHit { .. }))
        .count()
}

/// The number of images in the store.
fn image_count(harness: &Harness) -> usize {
    crate::pool::block_on(harness.store.list())
        .expect("the store lists")
        .len()
}

/// The work directory is empty after a failed step.
fn work_is_empty(harness: &Harness) {
    let work = harness.host.config.cache_dir.join("images/work");
    let entries = harness.host.fs.read_dir(&work).unwrap_or_default();
    assert!(entries.is_empty(), "no work file stays: {entries:?}");
}

#[test]
fn build_sim_cache() {
    let _serial = crate::control::support::serial();
    let harness = Harness::new("cache");
    harness.base(&base_name());
    let steps = "RUN one\nRUN two\nRUN three\nRUN four\nRUN five\n";
    let text = Harness::file(&base_name(), steps);

    let (first, progress) = harness.build_with_progress(&text);
    let first = first.expect("the first build succeeds");
    assert_eq!(
        harness.driver.launch_calls(),
        5,
        "one sandbox for each step"
    );
    assert_eq!(cache_hits(&progress), 1, "only FROM is cached");

    // Invariant I4: an unchanged build starts no sandbox and gives one key.
    let (second, progress) = harness.build_with_progress(&text);
    let second = second.expect("the second build succeeds");
    assert_eq!(key_of(&first), key_of(&second), "the image key is the same");
    assert_eq!(
        harness.driver.launch_calls(),
        5,
        "the second build starts nothing"
    );
    assert_eq!(cache_hits(&progress), 6, "every step is a cache hit");

    // Invariant I5: only the changed step and the later ones run.
    let changed = Harness::file(
        &base_name(),
        "RUN one\nRUN two\nRUN three\nRUN four changed\nRUN five\n",
    );
    let (third, progress) = harness.build_with_progress(&changed);
    let third = third.expect("the changed build succeeds");
    assert_eq!(harness.driver.launch_calls(), 7, "steps 4 and 5 run");
    assert_eq!(cache_hits(&progress), 4, "FROM and steps 1 to 3 are cached");
    assert_ne!(key_of(&first), key_of(&third));

    // A touched file is not a change, and one changed byte is.
    let context = harness.cache.parent().expect("the context");
    write_file(context, "app/main.sh", b"echo one\n");
    let add_text = Harness::file(&base_name(), "ADD ./app /opt/app\nRUN later\n");
    harness.build(&add_text).expect("the ADD build succeeds");
    let launches = harness.driver.launch_calls();
    let file = context.join("app/main.sh");
    let opened = std::fs::OpenOptions::new()
        .write(true)
        .open(&file)
        .expect("the file opens");
    opened
        .set_times(
            std::fs::FileTimes::new().set_modified(
                std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(9),
            ),
        )
        .expect("the time changes");
    harness
        .build(&add_text)
        .expect("the touched build succeeds");
    assert_eq!(
        harness.driver.launch_calls(),
        launches,
        "a touched file is cached"
    );
    write_file(context, "app/main.sh", b"echo two\n");
    harness
        .build(&add_text)
        .expect("the changed build succeeds");
    assert_eq!(
        harness.driver.launch_calls(),
        launches + 2,
        "the ADD and later run"
    );

    // `no_cache` runs every step after FROM and still stores the results.
    let before = harness.driver.launch_calls();
    let options = BuildOptions {
        no_cache: true,
        ..BuildOptions::default()
    };
    harness
        .build_options(&text, options)
        .expect("the no-cache build succeeds");
    assert_eq!(harness.driver.launch_calls(), before + 5);
}

#[test]
fn build_sim_run_fails() {
    let _serial = crate::control::support::serial();
    let harness = Harness::new("run-fails");
    harness.base(&base_name());
    harness.slave.with_exec_events(vec![
        ExecEventKind::Started { pid: 7 },
        ExecEventKind::Stderr {
            data: b"oops\n".to_vec(),
        },
        ExecEventKind::Exit {
            code: Some(3),
            signal: None,
        },
    ]);
    let (result, progress) = harness.build_with_progress(&one_step("RUN fail"));
    let error = result.expect_err("the step fails");
    match error.current_context() {
        SandboxError::BuildStepFailed { line, exit, signal } => {
            assert_eq!(*line, 2);
            assert_eq!(*exit, Some(3));
            assert_eq!(*signal, None);
        }
        other => panic!("the error is not BuildStepFailed: {other:?}"),
    }
    let stderr: Vec<u8> = progress
        .iter()
        .filter_map(|value| match value {
            BuildProgress::Output {
                stderr: true, data, ..
            } => Some(data.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(stderr, b"oops\n");
    assert_eq!(
        image_count(&harness),
        1,
        "only the base image is in the store"
    );
    work_is_empty(&harness);
}

#[test]
fn build_network_needs_driver() {
    let _serial = crate::control::support::serial();
    let harness = Harness::without_network("network");
    harness.base(&base_name());
    let (result, progress) = harness.build_with_progress(&one_step("RUN --network host echo hi"));
    let error = result.expect_err("no driver gives a guest network");
    assert!(matches!(
        error.current_context(),
        SandboxError::UnsupportedNetwork
    ));
    assert_eq!(
        error.downcast_ref::<BuildStepLine>().map(|line| line.0),
        Some(2),
        "the error names the step line"
    );
    assert_eq!(harness.driver.launch_calls(), 0, "no sandbox started");
    assert!(progress.is_empty(), "the build reports nothing");
}

#[test]
fn build_fault_matrix() {
    let _serial = crate::control::support::serial();

    // Probe: no usable driver. Nothing starts.
    let harness = Harness::unusable("fault-nodriver");
    let error = harness
        .build(&one_step("RUN one"))
        .expect_err("no driver is usable");
    assert!(matches!(
        error.current_context(),
        SandboxError::NoDriver { .. }
    ));

    // The base fails: a Docker source with no Engine.
    let harness = Harness::new("fault-base");
    let error = harness
        .build("FROM docker:missing:1\nRUN one\n")
        .expect_err("the Docker source needs an Engine");
    assert!(matches!(
        error.current_context(),
        SandboxError::DriverUnavailable { .. }
    ));
    assert_eq!(harness.driver.launch_calls(), 0);

    // The ADD source leaves the context: no sandbox for that step.
    let harness = Harness::new("fault-add-path");
    harness.base(&base_name());
    let error = harness
        .build(&Harness::file(&base_name(), "ADD ../secret /x\n"))
        .expect_err("the source leaves the context");
    match error.current_context() {
        SandboxError::InvalidSpec { field, reason } => {
            assert_eq!(field, "sandboxfile");
            assert!(
                reason.starts_with("line 2: "),
                "the reason names the line: {reason}"
            );
        }
        other => panic!("the error is not InvalidSpec: {other:?}"),
    }
    assert_eq!(harness.driver.launch_calls(), 0);
    work_is_empty(&harness);

    // The ADD entry limit fails with the line.
    let harness = Harness::new("fault-add-limit");
    harness.base(&base_name());
    write_file(
        harness.cache.parent().expect("the context"),
        "app/one",
        b"1",
    );
    write_file(
        harness.cache.parent().expect("the context"),
        "app/two",
        b"2",
    );
    let options = BuildOptions {
        limits: BuildLimits::default().with_max_context_entries(2),
        ..BuildOptions::default()
    };
    let error = harness
        .build_options(
            &Harness::file(&base_name(), "ADD ./app /opt/app\n"),
            options,
        )
        .expect_err("the entry limit stops the ADD");
    assert!(matches!(
        error.current_context(),
        SandboxError::ArtifactTooLarge { .. }
    ));
    assert_eq!(
        error.downcast_ref::<BuildStepLine>().map(|line| line.0),
        Some(2)
    );

    // The build sandbox does not start: the work file is removed.
    let harness = Harness::new("fault-launch");
    harness.base(&base_name());
    harness.driver.set_launch_error(SandboxError::Io {
        op: "launch".to_string(),
        message: "the test driver cannot start".to_string(),
    });
    let error = harness
        .build(&one_step("RUN one"))
        .expect_err("the launch fails");
    assert!(matches!(error.current_context(), SandboxError::Io { .. }));
    work_is_empty(&harness);
    assert_eq!(image_count(&harness), 1);
}

#[test]
fn build_concurrent_same_key() {
    let _serial = crate::control::support::serial();
    let directory = TempDir::new("concurrent");
    let first = Harness::shared(&directory);
    let second = Harness::shared(&directory);
    first.base(&base_name());
    let text = Harness::file(&base_name(), "RUN one\n");
    let file = Sandboxfile::parse(&text).expect("the file parses");
    let context = first.cache.parent().expect("the context").to_path_buf();
    let one = first
        .builder
        .build(&file, &context, BuildOptions::default());
    let two = second
        .builder
        .build(&file, &context, BuildOptions::default());
    let (first_result, second_result) =
        crate::pool::block_on(async { futures::future::join(one, two).await });
    let first_image = first_result.expect("the first build succeeds");
    let second_image = second_result.expect("the second build succeeds");
    assert_eq!(
        key_of(&first_image),
        key_of(&second_image),
        "one content gives one key"
    );

    // The snapshot of the step is one complete file, not two mixed files.
    let previous = foundation_sandbox::native::images::build::keys::base_key(
        &"a".repeat(32),
        foundation_sandbox::shared::spec::GuestArch::host(),
        4096,
    );
    let step = step_key(&previous, "RUN one", None);
    let snapshot = directory
        .path()
        .join("cache/images/steps")
        .join(format!("{step}.ext4"));
    let bytes = std::fs::read(&snapshot).expect("the step snapshot exists");
    assert_eq!(bytes, b"base-rootfs", "the snapshot is one complete copy");
    let images = crate::pool::block_on(first.store.list()).expect("the store lists");
    assert_eq!(images.len(), 2, "the base and the built image");
}

#[test]
fn cache_evicts_oldest() {
    let directory = TempDir::new("evict");
    let (ctx, _clock) = crate::control::support::ctx(9);
    let fs: Arc<dyn HostFs> = Arc::new(NativeFs);
    let cache = StepCache::new(
        ctx,
        fs,
        &directory.path().join("cache"),
        BuildLimits::default().with_cache_max_bytes(25),
    );
    for (key, used) in [("a", 1_u64), ("b", 2_u64), ("c", 3_u64)] {
        std::fs::write(cache.snapshot_path(key), vec![0_u8; 10]).expect("the snapshot is written");
        std::fs::write(
            cache.steps_dir().join(format!("{key}.used")),
            used.to_string(),
        )
        .expect("the use time is written");
    }
    cache.evict(&HashSet::new());
    assert!(
        !cache.snapshot_path("a").exists(),
        "the oldest snapshot goes"
    );
    assert!(cache.snapshot_path("b").exists());
    assert!(cache.snapshot_path("c").exists());
}

#[test]
fn cache_keeps_running_build() {
    let directory = TempDir::new("evict-protected");
    let (ctx, _clock) = crate::control::support::ctx(10);
    let fs: Arc<dyn HostFs> = Arc::new(NativeFs);
    let cache = StepCache::new(
        ctx,
        fs,
        &directory.path().join("cache"),
        BuildLimits::default().with_cache_max_bytes(25),
    );
    for (key, used) in [("a", 1_u64), ("b", 2_u64), ("c", 3_u64)] {
        std::fs::write(cache.snapshot_path(key), vec![0_u8; 10]).expect("the snapshot is written");
        std::fs::write(
            cache.steps_dir().join(format!("{key}.used")),
            used.to_string(),
        )
        .expect("the use time is written");
    }
    let protected: HashSet<String> = ["a".to_string()].into_iter().collect();
    cache.evict(&protected);
    assert!(
        cache.snapshot_path("a").exists(),
        "the running build keeps its snapshot"
    );
    assert!(!cache.snapshot_path("b").exists(), "the next oldest goes");
    assert!(cache.snapshot_path("c").exists());
}

#[test]
fn run_output_streams() {
    let _serial = crate::control::support::serial();
    let harness = Harness::new("stream");
    harness.base(&base_name());
    let chunk = vec![b'x'; 1024 * 1024 - 1];
    let total_chunks = 100_u64;
    let mut events = vec![ExecEventKind::Started { pid: 3 }];
    for _ in 0..total_chunks {
        events.push(ExecEventKind::Stderr {
            data: chunk.clone(),
        });
    }
    events.push(ExecEventKind::Exit {
        code: Some(0),
        signal: None,
    });
    harness.slave.with_exec_events(events);
    let text = one_step("RUN big");
    let file = Sandboxfile::parse(&text).expect("the file parses");
    let mut stream = harness.builder.build_with_progress(
        file,
        harness.cache.parent().expect("the context").to_path_buf(),
        BuildOptions::default(),
    );
    let (result, total, largest) = crate::pool::block_on(async {
        let mut total = 0_u64;
        let mut largest = 0_usize;
        while let Some(event) = stream.next().await {
            match event {
                foundation_sandbox::native::images::build::executor::BuildEvent::Progress(
                    BuildProgress::Output { data, .. },
                ) => {
                    total = total.saturating_add(data.len() as u64);
                    largest = largest.max(data.len());
                }
                foundation_sandbox::native::images::build::executor::BuildEvent::Progress(_) => {}
                foundation_sandbox::native::images::build::executor::BuildEvent::Done(result) => {
                    return (*result, total, largest);
                }
            }
        }
        panic!("the stream ended without a result");
    });
    result.expect("the build succeeds");
    assert_eq!(total, total_chunks * (1024 * 1024 - 1));
    assert!(
        largest < 1024 * 1024,
        "the builder holds less than 1 MiB of one chunk: {largest}"
    );
}
