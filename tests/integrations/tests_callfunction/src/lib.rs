#![allow(unused_imports)]

use foundation_jsnostd::{
    self, exposed_runtime, host_runtime, internal_api, ExternalPointer, Params, ReturnTypeHints,
    ReturnTypeId,
};

use foundation_nostd::*;

#[no_mangle]
extern "C" fn main() {
    let console_log = host_runtime::web::register_function(
        r"
        function(message){
            this.mock.logs(message);
            return this.ctx.Reply.asMemorySlice(0);
        }",
    );

    console_log.invoke(
        &[Params::Text8("Hello from intro")],
        ReturnTypeHints::One(ReturnTypeId::MemorySlice),
    );
}
