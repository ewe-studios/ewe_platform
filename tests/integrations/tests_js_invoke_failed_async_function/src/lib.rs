#![allow(unused_imports)]

use foundation_wasm::{
    self, exposed_runtime, host_runtime, internal_api, ExternalPointer, Params, ReturnTypeHints,
    ReturnTypeId, ReturnValues, Returns, TaskErrorCode, ThreeState,
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
            assert_eq!(result, Err(TaskErrorCode(101)));
        },
    );

    let task = host_runtime::web::register_function(
        r"
        function(message){
            const result = Promise.reject(this.asReplyError(101));
            this.mock.logs(message, result);
            return result;
        }",
    );

    task.invoke_async(
        callback_handle,
        &[Params::Text8("Hello from intro")],
        ReturnTypeHints::One(ThreeState::One(ReturnTypeId::Uint8)),
    );
}
