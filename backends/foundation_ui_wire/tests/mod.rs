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
mod sequencer_tests;
mod union_numbers_tests;
mod vocab_behaviour_tests;
mod vocab_dom_op_tests;
mod vocab_forms_tests;
mod vocab_js_tests;
mod vocab_props_tests;
mod vocab_tokens_tests;
mod vocab_version_tests;
mod writer_columnar_tests;
mod writer_error_tests;
mod writer_gate_tests;
mod writer_roundtrip_tests;
mod writer_table_tests;

#[cfg(feature = "reference")]
mod reference_tests;
