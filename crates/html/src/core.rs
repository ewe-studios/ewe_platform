pub struct Mutation {
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub content: Vec<u8>,
    pub removed: bool,
}
