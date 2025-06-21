#![allow(unused_imports)]

use foundation_jsnostd::{
    self, exposed_runtime, host_runtime, internal_api, ExternalPointer, Params, ReturnTypeHints,
};

use foundation_nostd::*;

#[no_mangle]
extern "C" fn main() {
    let console_log = host_runtime::web::register_function(
        r"
        function(message){
            console.log(message);
        }",
    );

    console_log.invoke(&[Params::Text8("Hello from intro")], ReturnTypeHints::None);

    std::panic::set_hook(Box::new(move |e| {
        console_log.invoke(
            &[Params::Text8(e.to_string().as_str())],
            ReturnTypeHints::None,
        );
    }));

    let _ = host_runtime::web::cache_text("alex");

    let console_log_id = host_runtime::web::allocate_function_reference();
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

    instructions
        .invoke_no_return_function(console_log_id, Some(&[Params::Text8("Hello from intro")]))
        .expect("should register call");

    host_runtime::web::send_instructions(instructions.complete().expect("complete instruction"));
}
