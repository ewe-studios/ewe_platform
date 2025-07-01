#![allow(unused_imports)]

use foundation_wasm::{
    self, exposed_runtime, host_runtime, internal_api, ExternalPointer, Params, ReturnTypeHints,
    ReturnValues, Returns,
};

use foundation_nostd::*;

#[no_mangle]
extern "C" fn main() {
    let console_log = host_runtime::web::register_function(
        r"
        function(message){
            console.log('Error occurred: ',message);
        }",
    );

    std::panic::set_hook(Box::new(move |e| {
        console_log.invoke_no_return(&[Params::Text8(e.to_string().as_str())]);
    }));

    let console_log_id = host_runtime::web::allocate_function_reference();
    let instructions = internal_api::create_instructions(100, 100);
    instructions
        .register_function(
            console_log_id,
            "
        function(){
            const args = Array.from(arguments);
            this.mock.select(args);
            return null;
        }",
        )
        .expect("should encode correctly");

    instructions
        .invoke(
            console_log_id,
            Some(&[
                Params::Bool(true),
                Params::Bool(false),
                Params::Int8(10),
                Params::Int16(10),
                Params::Int32(10),
                Params::Int64(10),
                Params::Uint8(10),
                Params::Uint16(10),
                Params::Uint32(10),
                Params::Uint64(10),
                Params::Float32(10.0),
                Params::Float64(10.0),
            ]),
            ReturnTypeHints::None,
        )
        .expect("encode instruction");

    let result =
        host_runtime::web::batch_response(instructions.complete().expect("complete instruction"))
            .expect("should get back values");

    assert_eq!(
        result,
        vec![Returns::One(ReturnValues::ExternalReference(
            4294967296.into()
        ))]
    )
}
