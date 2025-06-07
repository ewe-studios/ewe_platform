#![allow(unused_imports)]

use foundation_jsnostd::{
    self, exposed_runtime, host_runtime, internal_api, ExternalPointer, Params, TypedSlice,
};

use foundation_nostd::*;

#[no_mangle]
extern "C" fn main() {
    let cached_id = host_runtime::api_v1::cache_text("alex");

    let select_ref = host_runtime::api_v1::register_function(
        r"
        function(){
            const args = Array.from(arguments);
            this.mock.select(args);
        }",
    );

    select_ref.invoke(&[
        cached_id.into_param(),
        Params::ExternalReference(1),
        Params::InternalReference(2),
        Params::Uint8Array(&[1, 1]),
        Params::Int8Array(&[1, 1]),
        Params::Uint16Array(&[1, 1]),
        Params::Int16Array(&[1, 1]),
        Params::Uint32Array(&[1, 1]),
        Params::Int32Array(&[1, 1]),
        Params::Int64Array(&[2, 2]),
        Params::Uint64Array(&[3, 3]),
        Params::Float32Array(&[1.0, 1.0]),
        Params::Float64Array(&[1.0, 1.0]),
        Params::TypedArraySlice(TypedSlice::Uint8, &[4, 4]),
    ]);
}
