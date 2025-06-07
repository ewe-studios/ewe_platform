#![allow(unused_imports)]

use foundation_jsnostd::{
    self, exposed_runtime, host_runtime, internal_api, ExternalPointer, Params,
};

use foundation_nostd::*;

#[no_mangle]
extern "C" fn main() {
    let console_log = host_runtime::api_v1::register_function(
        r"
        function(v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14){
            return this.mock.calculateAge(v1, v2, v3, v4, v5, v6, v7, v8, v9, v10, v11, v12, v13, v14);
        }",
    );

    console_log.invoke_for_u64(&[
        Params::Int32(5),
        Params::Int64(5),
        Params::Int16(10),
        Params::Int8(10),
        Params::Uint32(5),
        Params::Uint64(5),
        Params::Uint16(10),
        Params::Uint8(10),
        Params::Uint128(10),
        Params::Int128(10),
        Params::Bool(true),
        Params::Bool(false),
        Params::Float32(10.2),
        Params::Float64(10.4),
    ]);
}
