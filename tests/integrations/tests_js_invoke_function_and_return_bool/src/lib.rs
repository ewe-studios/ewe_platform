#![allow(unused_imports)]

use foundation_wasm::{
    self, exposed_runtime, host_runtime, internal_api, ExternalPointer, Params,
};

use foundation_nostd::*;

#[no_mangle]
extern "C" fn main() {
    let console_log = host_runtime::web::register_function(
        r"
        function(v1){
            return this.mock.is_sample(v1);
        }",
    );

    console_log.invoke_for_bool(&[Params::Bool(true)]);
}
