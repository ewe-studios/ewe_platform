#[macro_export]
macro_rules! field_method {
    ($field_name:ident, $type_name:ty) => {
        #[inline]
        pub fn $field_name(&self) -> &$type_name {
            &self.$field_name
        }
    };
}

#[macro_export]
macro_rules! field_method_as_mut {
    ($method_name:ident, $field_name:ident, $type_name:ty) => {
        #[inline]
        pub fn $method_name(&mut self) -> &mut $type_name {
            &mut self.$field_name
        }
    };
}
