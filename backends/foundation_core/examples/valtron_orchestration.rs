//! Orchestrating several `#[valtron::task]` state machines to do one job:
//! a parallel fan-out (three independent cooks) and a sequential pipeline
//! (order → cook → serve), each stage a separate valtron function.
//!
//! Run with:
//!
//! ```sh
//! cargo run -p foundation_core --example valtron_orchestration
//! ```

use foundation_core::valtron;
use foundation_core::valtron::{execute, task, valtron, Ctx, StreamIteratorExt, TaskStatus};

/// --- Parallel fan-out: three enum-state cooks -----------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EggStage {
    Raw,
    Frying,
    Done,
}

#[derive(Debug, PartialEq)]
struct Eggs {
    fried: bool,
}

#[task]
fn fry_eggs(ctx: Ctx, stage: EggStage) -> Option<(EggStage, TaskStatus<Eggs, ()>)> {
    let _ = ctx;
    match stage {
        EggStage::Raw => Some((EggStage::Frying, TaskStatus::Ignore)),
        EggStage::Frying => Some((EggStage::Done, TaskStatus::Ready(Eggs { fried: true }))),
        EggStage::Done => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CoffeeStage {
    Beans,
    Brewing,
    Done,
}

#[derive(Debug, PartialEq)]
struct Coffee {
    cups: u32,
}

#[task]
fn brew_coffee(ctx: Ctx, stage: CoffeeStage) -> Option<(CoffeeStage, TaskStatus<Coffee, ()>)> {
    let _ = ctx;
    match stage {
        CoffeeStage::Beans => Some((CoffeeStage::Brewing, TaskStatus::Ignore)),
        CoffeeStage::Brewing => Some((CoffeeStage::Done, TaskStatus::Ready(Coffee { cups: 2 }))),
        CoffeeStage::Done => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToastStage {
    Bread,
    Toasting,
    Done,
}

#[derive(Debug, PartialEq)]
struct Toast {
    slices: u32,
}

#[task]
fn toast_bread(ctx: Ctx, stage: ToastStage) -> Option<(ToastStage, TaskStatus<Toast, ()>)> {
    let _ = ctx;
    match stage {
        ToastStage::Bread => Some((ToastStage::Toasting, TaskStatus::Ignore)),
        ToastStage::Toasting => Some((ToastStage::Done, TaskStatus::Ready(Toast { slices: 2 }))),
        ToastStage::Done => None,
    }
}

/// --- Sequential pipeline: enum states feeding stage to stage --------------

/// A struct state: the order to cook, and how many steps of cooking remain.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Order {
    dish: String,
    steps_left: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Meal {
    dish: String,
}

/// A one-shot state: `New` runs the single step, `Taken` ends the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ordering {
    New,
    Taken,
}

/// A one-shot task: take the order, emit it as the single `Ready` value, then
/// move to `Taken`, whose arm (`Taken => None`) terminates.
#[task]
fn take_order(ctx: Ctx, state: Ordering) -> Option<(Ordering, TaskStatus<Order, ()>)> {
    let _ = ctx;
    match state {
        Ordering::Taken => None,
        Ordering::New => Some((
            Ordering::Taken,
            TaskStatus::Ready(Order { dish: "pancakes".to_string(), steps_left: 2 }),
        )),
    }
}

/// A multi-step state: `Cooking(order)` still has steps left, `Done` ends it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Cooking {
    Cooking(Order),
    Done,
}

#[task]
fn cook(ctx: Ctx, state: Cooking) -> Option<(Cooking, TaskStatus<Meal, ()>)> {
    let _ = ctx;
    match state {
        Cooking::Done => None,
        Cooking::Cooking(order) => {
            if order.steps_left == 0 {
                Some((Cooking::Done, TaskStatus::Ready(Meal { dish: order.dish })))
            } else {
                Some((
                    Cooking::Cooking(Order { steps_left: order.steps_left - 1, ..order }),
                    TaskStatus::Ignore,
                ))
            }
        }
    }
}

/// A one-shot state: `ToServe(meal)` runs the single step, `Done` ends it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Serving {
    ToServe(Meal),
    Done,
}

#[task]
fn serve(ctx: Ctx, state: Serving) -> Option<(Serving, TaskStatus<String, ()>)> {
    let _ = ctx;
    match state {
        Serving::Done => None,
        Serving::ToServe(meal) => {
            Some((Serving::Done, TaskStatus::Ready(format!("served {}", meal.dish))))
        }
    }
}

#[valtron]
async fn main() {
    // Fan-out: all three cooks start on the pool at once.
    let eggs = execute(fry_eggs::new(EggStage::Raw), None).expect("engine live");
    let coffee = execute(brew_coffee::new(CoffeeStage::Beans), None).expect("engine live");
    let toast = execute(toast_bread::new(ToastStage::Bread), None).expect("engine live");

    // Join: await each at the boundary — the others kept cooking meanwhile.
    // `into_ready_future().await` resolves to Option<(value, rest)>: the first
    // `Ready` payload plus the remaining iterator. `_` drops the (now-finished)
    // rest — each cook emits a single `Ready`, then returns `None`.
    let (eggs, _) = eggs.into_ready_future().await.expect("eggs fried");
    let (coffee, _) = coffee.into_ready_future().await.expect("coffee brewed");
    let (toast, _) = toast.into_ready_future().await.expect("toast done");

    // Pipeline: each stage's `Ready` seeds the next stage's `new(...)`.
    let (order, _) = execute(take_order::new(Ordering::New), None)
        .expect("engine live")
        .into_ready_future()
        .await
        .expect("order taken");
    let (meal, _) = execute(cook::new(Cooking::Cooking(order)), None)
        .expect("engine live")
        .into_ready_future()
        .await
        .expect("meal cooked");
    let (ticket, _) = execute(serve::new(Serving::ToServe(meal)), None)
        .expect("engine live")
        .into_ready_future()
        .await
        .expect("meal served");

    println!("fan-out  -> eggs={eggs:?} coffee={coffee:?} toast={toast:?}");
    println!("pipeline -> {ticket}");
}
