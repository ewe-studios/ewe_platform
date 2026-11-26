use super::{check_shared, receive};
use foundation_signals::{
    Context, HubGone, MultiThreadedRuntime, Runtime, SignalHub, SingleThreadedRuntime,
};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::{mpsc, Arc};
use std::thread;

#[test]
fn both_runtimes_keep_their_own_graph_and_callback_bounds() {
    let local: Rc<Runtime> = Rc::new(SingleThreadedRuntime::new());
    let local_scope = local.scope();
    let (local_read, local_write) = local_scope.signal(0);
    let observed = Rc::new(Cell::new(0));
    let output = Rc::clone(&observed);
    local_scope.effect(move || output.set(local_read.get()));

    let shared = Arc::new(MultiThreadedRuntime::new());
    let scope = shared.scope();
    let (read, write) = scope.signal(1);
    check_shared(&shared);
    check_shared(&scope);
    check_shared(&read);
    check_shared(&write);

    local_write.set(3);
    write.set(2);
    shared.stabilize();
    assert_eq!(observed.get(), 0);
    assert_eq!(read.get(), 2);
    local.stabilize();
    assert_eq!(observed.get(), 3);
}

#[test]
fn hub_worker_receives_each_queued_mutation_result() {
    let owner = Rc::new(Runtime::new());
    let scope = Context::new(Rc::clone(&owner));
    let hub = SignalHub::new(&owner);
    let (read, write) = scope.signal(Vec::<u32>::new());
    let remote = hub.expose_setter(&write);
    let worker_writer = remote.clone();
    let (first_reply, first) = mpsc::channel();
    let (second_reply, second) = mpsc::channel();
    thread::spawn(move || {
        worker_writer
            .update(move |items| {
                items.push(11);
                first_reply.send(items.clone()).unwrap();
            })
            .unwrap();
        worker_writer
            .update(move |items| {
                items.push(22);
                second_reply.send(items.clone()).unwrap();
            })
            .unwrap();
    })
    .join()
    .unwrap();

    assert_eq!(read.get(), Vec::<u32>::new());
    assert_eq!(first.try_recv(), Err(mpsc::TryRecvError::Empty));
    assert_eq!(second.try_recv(), Err(mpsc::TryRecvError::Empty));
    assert_eq!(hub.pump().commands_applied, 2);
    assert_eq!(receive(&first), [11]);
    assert_eq!(receive(&second), [11, 22]);
    drop(hub);
    assert_eq!(remote.set(vec![33]), Err(HubGone));
}
