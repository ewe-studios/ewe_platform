//! NUMA strategy tests moved out of `src/llama_backend.rs` (spec 70
//! feature 59, rule 7). Every item the tests use is public.

use infrastructure_llama_bindings as bindings;
use infrastructure_llama_cpp::llama_backend::{InvalidNumaStrategy, NumaStrategy};

#[test]
fn numa_from_and_to() {
    let numas = [
        NumaStrategy::DISABLED,
        NumaStrategy::DISTRIBUTE,
        NumaStrategy::ISOLATE,
        NumaStrategy::NUMACTL,
        NumaStrategy::MIRROR,
        NumaStrategy::COUNT,
    ];

    for numa in &numas {
        let from = bindings::ggml_numa_strategy::from(*numa);
        let to = NumaStrategy::try_from(from).expect("Failed to convert from and to");
        assert_eq!(*numa, to);
    }
}

#[test]
fn check_invalid_numa() {
    let invalid = 800;
    let invalid = NumaStrategy::try_from(invalid);
    assert_eq!(invalid, Err(InvalidNumaStrategy(invalid.unwrap_err().0)));
}
