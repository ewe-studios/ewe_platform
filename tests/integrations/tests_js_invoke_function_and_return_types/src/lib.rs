#![allow(unused_imports)]

use foundation_wasm::{
    self, exposed_runtime, host_runtime, internal_api, ExternalPointer, InternalPointer, MemoryId,
    MemoryLocation, Params, ReturnTypeHints, ReturnTypeId, ReturnValues, ThreeState, TypedSlice,
};

use foundation_nostd::*;

#[no_mangle]
extern "C" fn main() {
    let console_log = host_runtime::web::register_function(
        r"
        function(message){
            console.log('Error occurred: ',message);
        }",
    );

    std::panic::set_hook(Box::new(move |e| {
        console_log.invoke_no_return(&[Params::Text8(e.to_string().as_str())]);
    }));

    let hello_id = host_runtime::web::cache_text("hello");
    let return_arg = host_runtime::web::register_function(
        r"
        function(v1){
            const ret = this.mock.returnArg(v1);
            console.log('Mock returnArg returned: ', ret);
            return ret;
        }",
    );

    assert!(matches!(
        return_arg
            .invoke_for_replies(
                &[Params::TypedArraySlice(TypedSlice::Int8, &[5])],
                ReturnTypeHints::One(ThreeState::One(ReturnTypeId::TypedArraySlice))
            )
            .unwrap()
            .pop()
            .unwrap(),
        ReturnValues::TypedArraySlice(TypedSlice::Int8, _)
    ));

    assert!(return_arg.invoke_for_f64(&[Params::Float64(5.0)]) == 5.0);

    assert!(return_arg.invoke_for_f32(&[Params::Float32(5.0)]) == 5.0);

    assert!(return_arg.invoke_for_u64(&[Params::Uint64(5)]) == 5);

    assert!(return_arg.invoke_for_u32(&[Params::Uint32(5)]) == 5);

    assert!(return_arg.invoke_for_u16(&[Params::Uint16(5)]) == 5);

    assert!(return_arg.invoke_for_u8(&[Params::Uint8(5)]) == 5);

    assert!(return_arg.invoke_for_u64(&[Params::Int64(5)]) == 5);

    assert!(return_arg.invoke_for_u32(&[Params::Int32(5)]) == 5);

    assert!(return_arg.invoke_for_i16(&[Params::Int16(5)]) == 5);

    assert!(return_arg.invoke_for_u8(&[Params::Int8(5)]) == 5);

    assert!(
        return_arg
            .invoke_for_str(&[Params::Text8("alex")])
            .expect("is str")
            == *"alex"
    );

    assert!(
        return_arg
            .invoke_for_str(&[hello_id.into_param()])
            .expect("is str")
            == *"hello"
    );

    assert!(
        return_arg
            .invoke_for_replies(
                &[Params::ErrorCode(50)],
                ReturnTypeHints::One(ThreeState::One(ReturnTypeId::ErrorCode))
            )
            .unwrap()
            == vec![ReturnValues::ErrorCode(50)],
    );

    assert!(
        return_arg
            .invoke_for_replies(
                &[Params::Undefined],
                ReturnTypeHints::One(ThreeState::One(ReturnTypeId::None))
            )
            .unwrap()
            == vec![ReturnValues::None],
    );

    assert!(
        return_arg
            .invoke_for_replies(
                &[Params::InternalReference(0)],
                ReturnTypeHints::One(ThreeState::One(ReturnTypeId::InternalReference))
            )
            .unwrap()
            == vec![ReturnValues::InternalReference(InternalPointer::pointer(0))],
    );

    assert!(
        return_arg
            .invoke_for_replies(
                &[Params::ExternalReference(0)],
                ReturnTypeHints::One(ThreeState::One(ReturnTypeId::ExternalReference))
            )
            .unwrap()
            == vec![ReturnValues::ExternalReference(ExternalPointer::pointer(0))],
    );

    assert!(
        return_arg
            .invoke_for_replies(
                &[Params::Int128(10)],
                ReturnTypeHints::One(ThreeState::One(ReturnTypeId::Int128))
            )
            .unwrap()
            == vec![ReturnValues::Int128(10)],
    );

    assert!(
        return_arg
            .invoke_for_replies(
                &[Params::Uint128(10)],
                ReturnTypeHints::One(ThreeState::One(ReturnTypeId::Uint128))
            )
            .unwrap()
            == vec![ReturnValues::Uint128(10)],
    );
}
