/// VecExt implements convenient methods to extract a that can be applied to
/// Vec<T> objects for special methods.
pub trait VecExt {
    fn to_vec_string(self) -> Vec<String>;
}

impl VecExt for Vec<&str> {
    fn to_vec_string(self) -> Vec<String> {
        self.iter().map(|item| String::from(*item)).collect()
    }
}
