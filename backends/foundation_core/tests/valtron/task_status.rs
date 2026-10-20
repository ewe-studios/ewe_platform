//! Tests that a plain iterator of `TaskStatus` values behaves as expected.

use std::time;

use foundation_core::valtron::TaskStatus;

#[test]
fn vec_iterator_is_task_iterator() {
    let tasks: Vec<TaskStatus<(), ()>> = vec![
        TaskStatus::Delayed(time::Duration::from_secs(1)),
        TaskStatus::Pending(()),
        TaskStatus::Init,
        TaskStatus::Ready(()),
    ];

    let task_iterator = Box::new(tasks.into_iter());

    let collected_tasks: Vec<TaskStatus<(), ()>> = task_iterator.collect();
    assert_eq!(
        collected_tasks,
        vec![
            TaskStatus::Delayed(time::Duration::from_secs(1)),
            TaskStatus::Pending(()),
            TaskStatus::Init,
            TaskStatus::Ready(()),
        ]
    );
}
