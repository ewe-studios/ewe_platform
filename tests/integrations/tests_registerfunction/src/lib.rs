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
            this.mock.runtime.logs(message);
        }",
    );
}
