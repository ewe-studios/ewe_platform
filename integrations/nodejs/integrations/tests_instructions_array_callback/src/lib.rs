#![allow(unused_imports)]

use foundation_wasm::{
    self, exposed_runtime, host_runtime, internal_api, ExternalPointer, Params, ReturnTypeHints,
    ReturnTypeId, ReturnValues, Returns, ThreeState, TypedSlice,
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

    let callback_handle = internal_api::register_callback(
        ReturnTypeHints::One(ThreeState::One(ReturnTypeId::Uint8)),
        |result| {
            assert_eq!(result, Ok(Returns::One(ReturnValues::Uint8(10))));
        },
    );

    let console_log_id = host_runtime::web::allocate_function_reference();
    let instructions = internal_api::create_instructions(100, 100);
    instructions
        .register_function(
            console_log_id,
            "
        function(){
            const args = Array.from(arguments);
            this.mock.select(args);
            return Promise.resolve(this.asUint8(10));
        }",
        )
        .expect("should encode correctly");

    let cached_id = host_runtime::web::cache_text("alex");
    instructions
        .invoke_async(
            console_log_id,
            callback_handle,
            Some(&[
                cached_id.into_param(),
                Params::ExternalReference(1),
                Params::InternalReference(2),
                Params::Uint8Array(&[1, 1]),
                Params::Int8Array(&[1, 1]),
                Params::Uint16Array(&[1, 1]),
                Params::Int16Array(&[1, 1]),
                Params::Uint32Array(&[1, 1]),
                Params::Int32Array(&[1, 1]),
                Params::Int64Array(&[2, 2]),
                Params::Uint64Array(&[3, 3]),
                Params::Float32Array(&[1.0, 1.0]),
                Params::Float64Array(&[1.0, 1.0]),
                Params::TypedArraySlice(TypedSlice::Uint8, &[4, 4]),
            ]),
            ReturnTypeHints::One(ThreeState::One(ReturnTypeId::Uint8)),
        )
        .expect("encode instruction");

    let items =
        host_runtime::web::batch_response(instructions.complete().expect("complete instruction"))
            .expect("got no response");

    assert_eq!(items.len(), 1);
}
