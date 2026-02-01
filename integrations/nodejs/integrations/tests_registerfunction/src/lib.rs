#![allow(unused_imports)]

use foundation_wasm::{self, exposed_runtime, host_runtime, internal_api, ExternalPointer, Params};

use foundation_nostd::*;

#[no_mangle]
extern "C" fn main() {
    let _ = host_runtime::web::register_function(
        r"
        function(message){
            this.mock.runtime.logs(message);
        }",
    );
}
