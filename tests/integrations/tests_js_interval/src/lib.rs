#![allow(unused_imports)]

use foundation_wasm::{
    self, exposed_runtime, host_runtime, internal_api, ExternalPointer, Params, ReturnTypeHints,
    TickState,
};

use foundation_nostd::*;

#[no_mangle]
extern "C" fn main() {
    let console_handle = host_runtime::web::register_function(
        r"
        function(message){
            console.log('Error occurred: ',message);
        }",
    );

    std::panic::set_hook(Box::new(move |e| {
        console_handle.invoke_no_return(&[Params::Text8(e.to_string().as_str())]);
    }));

    let console_log = host_runtime::web::register_function(
        r"
        function(message){
            this.mock.logs(message);
        }",
    );

    host_runtime::web::register_interval(300.0, move || {
        console_log.invoke(&[Params::Text8("Hello from intro")], ReturnTypeHints::None);
        TickState::REQUEUE
    });
}
