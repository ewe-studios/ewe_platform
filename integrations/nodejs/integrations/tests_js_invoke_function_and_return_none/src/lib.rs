#![allow(unused_imports)]

use foundation_wasm::{self, exposed_runtime, host_runtime, internal_api, ExternalPointer, Params};

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

    let handler = host_runtime::web::register_function(
        r"
        function(v1){
            this.mock.is_sample(v1);
            return this.asNone();
        }",
    );

    assert!(handler.invoke_for_none(&[Params::Bool(true)]));
}
