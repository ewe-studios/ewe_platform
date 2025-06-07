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
        function(){
            const args = Array.from(arguments);
            this.mock.select(args);
        }",
        )
        .expect("should encode correctly");

    instructions
        .invoke_no_return_function(
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
        )
        .expect("encode instruction");

    host_runtime::api_v2::send_instructions(instructions.complete().expect("complete instruction"));
}
