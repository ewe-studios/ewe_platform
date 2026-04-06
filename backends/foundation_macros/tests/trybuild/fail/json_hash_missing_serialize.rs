use foundation_macros::JsonHash;

// Missing Serialize - should fail
#[derive(JsonHash)]
pub struct InvalidStruct {
    pub name: String,
}

fn main() {}
