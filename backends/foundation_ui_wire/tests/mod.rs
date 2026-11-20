// The generated owned tables require default construction outside their crate.
macro_rules! table {
    ($table:ty { $($field:ident: $value:expr),+ $(,)? }) => {{
        let mut table = <$table>::default();
        $(table.$field = $value;)+
        table
    }};
}

mod align_tests;
mod error_ext_tests;
mod fixtures_tests;
mod read_tests;
mod roundtrip_tests;
mod union_numbers_tests;
mod vocab_behaviour_tests;
mod vocab_dom_op_tests;
mod vocab_forms_tests;
mod vocab_js_tests;
mod vocab_props_tests;
mod vocab_tokens_tests;
mod vocab_version_tests;
