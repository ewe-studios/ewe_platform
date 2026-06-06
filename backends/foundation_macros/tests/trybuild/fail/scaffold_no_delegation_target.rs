use foundation_macros::scaffold_impl;
use foundation_nostd::macros::scaffold;

trait Ops {
    fn do_thing(&self) -> u32;
}

struct Wrapper {
    inner: u32,
}

// scaffold!() with no via/call/scaffold_method — should fail
#[scaffold_impl]
impl Ops for Wrapper {
    fn do_thing(&self) -> u32 {
        scaffold!()
    }
}

fn main() {}
