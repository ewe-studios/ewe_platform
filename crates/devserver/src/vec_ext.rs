/// `VecStringExt` implements convenient methods to extract a Vec<String> from
/// any Vec<&str> type.
pub trait VecStringExt {
    fn to_vec_string(self) -> Vec<String>;
}

impl VecStringExt for Vec<&str> {
    fn to_vec_string(self) -> Vec<String> {
        self.iter().map(|item| String::from(*item)).collect()
    }
}
