#![allow(unused_imports)]

use foundation_jsnostd::{
    self, exposed_runtime, host_runtime, internal_api, ExternalPointer, Params,
};

use foundation_nostd::*;

#[no_mangle]
extern "C" fn main() {
    let console_log = host_runtime::api_v1::register_function(
        r"
        function(message){
            this.mock.createDom('div');
            return {tag: 'div' };
        }",
    );

    console_log.invoke_for_dom(&[Params::Text8("Hello from intro")]);
}
