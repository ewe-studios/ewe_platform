//! A timed state machine — a traffic light — driven by `TaskStatus::Delayed`.
//!
//! Run with:
//!
//! ```sh
//! cargo run -p foundation_core --example valtron_traffic_light
//! ```
//!
//! Because `Ready` is **not** terminal, a single `#[valtron::task]` machine
//! streams many values — one `Ready` per phase — and ends when it returns
//! `None`. `Delayed(d)` hands control back to the executor with a promise to
//! re-poll after `d`, so the phases are spaced out in time.

use std::time::Duration;

use foundation_core::valtron;
use foundation_core::valtron::{
    execute, task, valtron, Ctx, StreamExt, StreamIteratorExt, TaskStatus,
};

/// An enum state: the light currently showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Light {
    Red,
    Green,
    Yellow,
}

impl Light {
    fn next(self) -> Self {
        match self {
            Light::Red => Light::Green,
            Light::Green => Light::Yellow,
            Light::Yellow => Light::Red,
        }
    }
}

/// The state machine: either pausing (`Pause`, emits `Delayed`) or showing the
/// current light (`Show`, emits `Ready`). `remaining` counts the phases left.
#[derive(Debug, Clone, Copy)]
enum Phase {
    Pause { light: Light, remaining: u32 },
    Show { light: Light, remaining: u32 },
}

#[task]
fn traffic_light(ctx: Ctx, phase: Phase) -> Option<(Phase, TaskStatus<Light, ()>)> {
    let _ = ctx;
    match phase {
        // No phases left: end the stream.
        Phase::Pause { remaining: 0, .. } => None,
        // Sleep a moment, then show the current light.
        Phase::Pause { light, remaining } => Some((
            Phase::Show { light, remaining },
            TaskStatus::Delayed(Duration::from_millis(1)),
        )),
        // Show the light, then advance to the next colour.
        Phase::Show { light, remaining } => Some((
            Phase::Pause { light: light.next(), remaining: remaining - 1 },
            TaskStatus::Ready(light),
        )),
    }
}

#[valtron]
async fn main() {
    // Stream two full cycles (6 phases), skipping the `Delayed` gaps, via
    // `into_next_stream`.
    let mut stream = execute(
        traffic_light::new(Phase::Pause { light: Light::Red, remaining: 6 }),
        None,
    )
    .expect("engine live")
    .into_next_stream();

    let mut phases = Vec::new();
    while let Some(light) = stream.next().await {
        phases.push(light);
    }

    assert_eq!(
        phases,
        vec![
            Light::Red,
            Light::Green,
            Light::Yellow,
            Light::Red,
            Light::Green,
            Light::Yellow,
        ]
    );
    println!("traffic light -> {phases:?}");
}
