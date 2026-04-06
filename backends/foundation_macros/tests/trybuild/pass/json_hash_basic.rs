use foundation_macros::JsonHash;
use serde::Serialize;

#[derive(JsonHash, Serialize)]
pub struct ValidStruct {
    pub name: String,
    pub value: i32,
}

fn main() {
    let s = ValidStruct {
        name: "test".into(),
        value: 42,
    };
    let _hash = s.struct_hash();
}
