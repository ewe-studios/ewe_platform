#![allow(unused_imports)]

use foundation_jsnostd::{self, exposed_runtime, host_runtime};

use foundation_nostd::*;

#[no_mangle]
pub extern "C" fn main() {
    let console_log = foundation_jsnostd::host_runtime::js!(
        r"
        function(message){
            console.log(message);
        }"
    );

    console_log.invoke(&[host_runtime::InvocationParameter::String(
        "Hello from intro",
    )]);
}
