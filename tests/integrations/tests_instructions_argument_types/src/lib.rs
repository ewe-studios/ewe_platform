#![allow(unused_imports)]

use foundation_jsnostd::{
    self, exposed_runtime, host_runtime, internal_api, ExternalPointer, Params,
};

use foundation_nostd::*;

#[no_mangle]
extern "C" fn main() {
    let _ = host_runtime::api_v1::register_function(
        r"
        function(message){
            console.log(message);
        }",
    );

    let console_log_id = host_runtime::api_v2::preallocate_func_external_reference();
    let instructions = internal_api::create_instructions(100, 100);
    instructions
        .register_function(
            console_log_id,
            "
        function(message){
            console.log(message);
        }",
        )
        .expect("should encode correctly");

    host_runtime::api_v2::send_instructions(instructions.complete().expect("complete instruction"));
}
