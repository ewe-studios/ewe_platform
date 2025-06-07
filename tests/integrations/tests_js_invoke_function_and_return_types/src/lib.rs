#![allow(unused_imports)]

use foundation_jsnostd::{
    self, exposed_runtime, host_runtime, internal_api, ExternalPointer, Params,
};

use foundation_nostd::*;

#[no_mangle]
extern "C" fn main() {
    let console_log = host_runtime::api_v1::register_function(
        r"
        function(v1){
            return this.mock.returnArg(v1);
        }",
    );

    assert!(console_log.invoke_for_f64(&[Params::Float64(5.0)]) == 5.0);

    assert!(console_log.invoke_for_f32(&[Params::Float32(5.0)]) == 5.0);

    assert!(console_log.invoke_for_u64(&[Params::Uint64(5)]) == 5);

    assert!(console_log.invoke_for_u32(&[Params::Uint32(5)]) == 5);

    assert!(console_log.invoke_for_u16(&[Params::Uint16(5)]) == 5);

    assert!(console_log.invoke_for_u8(&[Params::Uint8(5)]) == 5);

    assert!(console_log.invoke_for_u64(&[Params::Int64(5)]) == 5);

    assert!(console_log.invoke_for_u32(&[Params::Int32(5)]) == 5);

    assert!(console_log.invoke_for_u16(&[Params::Int16(5)]) == 5);

    assert!(console_log.invoke_for_u8(&[Params::Int8(5)]) == 5);
}
