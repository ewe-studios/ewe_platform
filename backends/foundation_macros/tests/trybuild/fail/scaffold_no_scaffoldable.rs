use foundation_macros::Scaffold;

struct PlainInner {
    value: u32,
}

// No #[scaffoldable] on PlainInner's impl — derive(Scaffold) should fail
impl PlainInner {
    pub fn get_value(&self) -> u32 {
        self.value
    }
}

#[derive(Scaffold)]
pub struct Outer {
    #[scaffold(field)]
    inner: PlainInner,
}

fn main() {}
