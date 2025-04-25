#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_panics_doc)]

use alloc::string::String;
use alloc::vec::Vec;
use foundation_nostd::{raw_parts::RawParts, spin::Mutex};

use crate::{
    ExternalPointer, InternalCallback, InternalPointer, InternalReferenceRegistry, JSEncoding,
    MemoryAllocations,
};

static INTERNAL_CALLBACKS: Mutex<InternalReferenceRegistry> = InternalReferenceRegistry::create();

static ALLOCATIONS: Mutex<MemoryAllocations> = Mutex::new(MemoryAllocations::new());

pub type JSAllocationId = u64;

pub mod internal_api {
    use super::*;

    // -- callback methods

    #[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
    pub fn register_internal_callback<F>(f: F) -> InternalPointer
    where
        F: InternalCallback + 'static,
    {
        INTERNAL_CALLBACKS.lock().add(f)
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_arch = "wasm64")))]
    pub fn register_internal_callback<F>(f: F) -> InternalPointer
    where
        F: InternalCallback + Send + Sync + 'static,
    {
        INTERNAL_CALLBACKS.lock().add(f)
    }

    pub fn unregister_internal_callback(addr: InternalPointer) {
        INTERNAL_CALLBACKS
            .lock()
            .remove(addr)
            .expect("should be registered");
    }

    pub fn run_internal_callbacks(addr: InternalPointer, start_pointer: *const u8, length: u64) {
        let callback = INTERNAL_CALLBACKS
            .lock()
            .get(addr)
            .expect("should be registered");
        callback.receive(start_pointer, length);
    }

    // -- extract methods

    pub fn extract_vec_from_memory(allocation_id: u64) -> Vec<u8> {
        let allocations = ALLOCATIONS.lock();
        let mem = allocations
            .get(allocation_id.into())
            .expect("Allocation should be initialized");
        mem.clone_memory().expect("should clone memory")
    }

    pub fn extract_string_from_memory(allocation_id: u64) -> String {
        let allocations = ALLOCATIONS.lock();
        let mem = allocations
            .get(allocation_id.into())
            .expect("Allocation should be initialized");
        mem.string_from_memory()
            .expect("should convert into String")
    }
}

pub mod exposed_runtime {
    use super::*;

    #[no_mangle]
    pub extern "C" fn create_allocation(size: u64) -> u64 {
        let mem_id = ALLOCATIONS
            .lock()
            .allocate(size)
            .expect("should create requested allocation");
        mem_id.as_u64()
    }

    #[no_mangle]
    pub extern "C" fn allocation_start_pointer(mem_id: u64) -> *const u8 {
        let allocations = ALLOCATIONS.lock();
        let memory = allocations
            .get(mem_id.into())
            .expect("Allocation should be initialized");
        memory
            .get_pointer()
            .expect("should be able to get valid pointer")
    }

    #[no_mangle]
    pub extern "C" fn allocation_length(allocation_id: u64) -> u64 {
        let allocations = ALLOCATIONS.lock();
        let mem = allocations
            .get(allocation_id.into())
            .expect("Allocation should be initialized");
        mem.len().expect("should return allocation length")
    }

    #[no_mangle]
    pub extern "C" fn clear_allocation(allocation_id: u64) {
        let allocations = ALLOCATIONS.lock();
        let mem = allocations
            .get(allocation_id.into())
            .expect("Allocation should be initialized");
        mem.clear().expect("should clear memory");
    }

    #[no_mangle]
    pub extern "C" fn unregister_callback(addr: u64) {
        internal_api::unregister_internal_callback(addr.into());
    }

    #[no_mangle]
    pub extern "C" fn run_callback(addr: u64, start: u64, length: u64) {
        internal_api::run_internal_callbacks(addr.into(), start as *const u8, length);
    }
}

/// [`host_runtime`] is the expected interface which the JS/Host
/// must provide and related wrapper functions that make that interaction
/// simpler.
#[allow(unused)]
pub mod host_runtime {
    use super::*;

    // -- Data Information
    pub mod batch {
        #[link(wasm_import_module = "batch")]
        extern "C" {
            // [apply_batch] takes a location in memory that has a batch of operations
            // which match the [`crate::Operations`] outlined in the batching API the
            // runtime supports, allowing us amortize the cost of doing bulk processing on
            // the wasm and host boundaries.
            pub fn apply_batch(allocation_start: u64, allocation_end: u64);
        }
    }

    // -- Reference handling
    pub mod references {
        #[link(wasm_import_module = "refs")]
        extern "C" {
            //  Provides a way to inform the need to drop a outside cached reference
            //  used for execution, e.g JSFunction or some other referential type.
            pub fn drop_reference(external_reference_id: u64);
        }
    }

    // -- Functions (Invocation & Registration)
    pub mod functions {
        #[link(wasm_import_module = "funcs")]
        extern "C" {
            // [`js_unregister_function`] provides a means to unregister a target function
            // from the WASM - Host runtime boundary.
            pub fn js_unregister_function(handle: u64);

            // [`js_preallocate_reference`] allows you to pre-allocate a specific reference
            // that can later be used for registration at a later point in time.
            pub fn js_preallocate_reference() -> u64;

            // [`js_register_function_at`] allows us to register a function at a
            // pre-allocated location, it should panics if that id does not location
            // was not pre-allocated via the [`js_preallocate_reference`].
            pub fn js_register_function_at(handle: u64, start: u64, len: u64, encoding: u8) -> u64;

            // registers a function via it's provided start and length
            // indicative of where the function body can be found
            // as utf-8 or utf-18 encoded byte (based on third argument)
            // from the start pointer in memory to the specified
            // length to be registered in the shared
            // function registry.
            pub fn js_register_function(start: u64, len: u64, encoding: u8) -> u64;

            // invokes a Javascript function across the WASM/RUST ABI
            // which then returns the allocation_id (as f64) that can
            // be used to get the related allocation vector
            // from the global allocations.
            pub fn js_invoke_function(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: usize,
            ) -> u64;
        }
    }

    // -- registration functions

    #[derive(Copy, Clone)]
    pub struct JSFunction {
        pub handler: u64,
    }

    #[macro_export]
    macro_rules! js {
        ($e:expr) => {{
            static mut FN: Option<u64> = None;
            unsafe {
                if FN.is_none() {
                    // store the handler for the related js function as a catch for later use.
                    FN = Some(
                        foundation_jsnostd::host_runtime::JSFunction::register_function($e).handler,
                    );
                }
                foundation_jsnostd::host_runtime::JSFunction {
                    handler: FN.unwrap(),
                }
            }
        }};
    }

    // expose the macro for outside use.
    pub use js;

    #[allow(clippy::cast_precision_loss)]
    impl JSFunction {
        /// [`register_function`] calls the underlying [`js_abi`] registration
        /// function to register a javascript code that can be called from memory
        /// allowing you define the underlying code we want executed.
        pub fn register_function(code: &str) -> JSFunction {
            let start = code.as_ptr() as usize;
            let len = code.len();
            unsafe {
                JSFunction {
                    handler: host_runtime::functions::js_register_function(
                        start as u64,
                        len as u64,
                        JSEncoding::UTF8.into(),
                    ), // precision loss here
                }
            }
        }

        /// [`register_function_utf16`] calls the underlying [`js_abi`] registration
        /// function to register a javascript code already encoded
        /// as UTF16 by the borrowed slice of u16 that can be called from memory
        /// allowing you define the underlying code we want executed.
        pub fn register_function_utf16(code: &[u16]) -> JSFunction {
            let start = code.as_ptr() as usize;
            let len = code.len();
            unsafe {
                JSFunction {
                    handler: host_runtime::functions::js_register_function(
                        start as u64,
                        len as u64,
                        JSEncoding::UTF16.into(),
                    ), // precision loss here
                }
            }
        }

        /// [`invoke`] invokes a javascript function registered at the given handle
        /// defined by the [`JSFunction::handler`] which then receives the set of parameters
        /// supplied to be invoked with.
        ///
        /// The `js_abi` will handle necessary conversion and execution of the function
        /// with the passed arguments.
        pub fn invoke(&self, params: &[InvocationParameter]) -> JSAllocationId {
            let param_bytes = InvocationParameter::param_to_bytes(params);
            let RawParts {
                ptr,
                length,
                capacity: _,
            } = RawParts::from_vec(param_bytes);
            unsafe { host_runtime::functions::js_invoke_function(self.handler, ptr, length) }
        }

        /// [`unregister_function`] calls the JS ABI on the host to de-register
        /// the target function.
        pub fn unregister_function(&self) {
            unsafe { host_runtime::functions::js_unregister_function(self.handler) }
        }
    }

    // -- JS dropping ExternalPointer

    pub const JS_UNDEFINED: JSExternalRef = JSExternalRef(ExternalPointer::pointer(0));
    pub const JS_NULL: JSExternalRef = JSExternalRef(ExternalPointer::pointer(1));
    pub const DOM_SELF: JSExternalRef = JSExternalRef(ExternalPointer::pointer(2));
    pub const DOM_DOCUMENT: JSExternalRef = JSExternalRef(ExternalPointer::pointer(3));
    pub const DOM_WINDOW: JSExternalRef = JSExternalRef(ExternalPointer::pointer(4));
    pub const DOM_BODY: JSExternalRef = JSExternalRef(ExternalPointer::pointer(5));
    pub const DOM_FALSE: JSExternalRef = JSExternalRef(ExternalPointer::pointer(6));
    pub const DOM_TRUE: JSExternalRef = JSExternalRef(ExternalPointer::pointer(7));

    pub struct JSExternalRef(ExternalPointer);

    impl JSExternalRef {
        pub fn number(&self) -> u64 {
            self.0.into_inner()
        }
    }

    impl Drop for JSExternalRef {
        fn drop(&mut self) {
            unsafe {
                host_runtime::references::drop_reference(self.0.into_inner());
            }
        }
    }

    impl From<u64> for JSExternalRef {
        fn from(value: u64) -> Self {
            Self(value.into())
        }
    }

    // --- Browser / WASM ABI

    // --- Invocations

    //convert invoke parameters into bytes
    //assuming each parameter is preceded by a 32 bit integer indicating its type
    //0 = undefined
    //1 = null
    //2 = float-64
    //3 = bigint
    //4 = string (followed by 32-bit start and size of string in memory)
    //5 = extern ref
    //6 = array of float-64 (followed by 32-bit start and size of string in memory)

    pub enum InvocationParameter<'a> {
        Undefined,
        Null,
        Float64(f64),
        BigInt(i64),
        String(&'a str),
        Float32Array(&'a [f32]),
        Float64Array(&'a [f64]),
        Bool(bool),
        Uint32Array(&'a [u32]),
        ExternalReference(&'a JSExternalRef),
    }

    impl From<f64> for InvocationParameter<'_> {
        fn from(f: f64) -> Self {
            InvocationParameter::Float64(f)
        }
    }

    impl From<i32> for InvocationParameter<'_> {
        fn from(i: i32) -> Self {
            InvocationParameter::Float64(f64::from(i))
        }
    }

    impl From<usize> for InvocationParameter<'_> {
        fn from(i: usize) -> Self {
            InvocationParameter::Float64(i as f64)
        }
    }

    impl From<i64> for InvocationParameter<'_> {
        fn from(i: i64) -> Self {
            InvocationParameter::BigInt(i)
        }
    }

    impl<'a> From<&'a str> for InvocationParameter<'a> {
        fn from(s: &'a str) -> Self {
            InvocationParameter::String(s)
        }
    }

    impl<'a> From<&'a JSExternalRef> for InvocationParameter<'a> {
        fn from(i: &'a JSExternalRef) -> Self {
            InvocationParameter::ExternalReference(i)
        }
    }

    impl<'a> From<&'a [f32]> for InvocationParameter<'a> {
        fn from(a: &'a [f32]) -> Self {
            InvocationParameter::Float32Array(a)
        }
    }

    impl<'a> From<&'a [f64]> for InvocationParameter<'a> {
        fn from(a: &'a [f64]) -> Self {
            InvocationParameter::Float64Array(a)
        }
    }

    impl From<bool> for InvocationParameter<'_> {
        fn from(b: bool) -> Self {
            InvocationParameter::Bool(b)
        }
    }

    impl<'a> From<&'a [u32]> for InvocationParameter<'a> {
        fn from(a: &'a [u32]) -> Self {
            InvocationParameter::Uint32Array(a)
        }
    }

    impl<'a> InvocationParameter<'a> {
        pub fn param_to_bytes(params: &'a [InvocationParameter<'a>]) -> Vec<u8> {
            let mut encoded_params: Vec<u8> = Vec::new();
            for param in params {
                match param {
                    InvocationParameter::Undefined => {
                        encoded_params.push(0);
                    }
                    InvocationParameter::Null => {
                        encoded_params.push(1);
                    }
                    InvocationParameter::Float64(f) => {
                        encoded_params.push(2);
                        encoded_params.extend_from_slice(&f.to_le_bytes());
                    }
                    InvocationParameter::BigInt(i) => {
                        encoded_params.push(3);
                        encoded_params.extend_from_slice(&i.to_le_bytes());
                    }
                    InvocationParameter::String(s) => {
                        encoded_params.push(4);
                        let start = s.as_ptr() as usize;
                        let len = s.len();
                        encoded_params.extend_from_slice(&start.to_le_bytes());
                        encoded_params.extend_from_slice(&len.to_le_bytes());
                    }
                    InvocationParameter::ExternalReference(i) => {
                        encoded_params.push(5);
                        encoded_params.extend_from_slice(&i.number().to_le_bytes());
                    }
                    InvocationParameter::Float32Array(a) => {
                        encoded_params.push(6);
                        let start = a.as_ptr() as usize;
                        let len = a.len();
                        encoded_params.extend_from_slice(&start.to_le_bytes());
                        encoded_params.extend_from_slice(&len.to_le_bytes());
                    }
                    InvocationParameter::Bool(b) => {
                        if *b {
                            encoded_params.push(7);
                        } else {
                            encoded_params.push(8);
                        }
                    }
                    InvocationParameter::Float64Array(a) => {
                        encoded_params.push(9);
                        let start = a.as_ptr() as usize;
                        let len = a.len();
                        encoded_params.extend_from_slice(&start.to_le_bytes());
                        encoded_params.extend_from_slice(&len.to_le_bytes());
                    }
                    InvocationParameter::Uint32Array(a) => {
                        encoded_params.push(10);
                        let start = a.as_ptr() as usize;
                        let len = a.len();
                        encoded_params.extend_from_slice(&start.to_le_bytes());
                        encoded_params.extend_from_slice(&len.to_le_bytes());
                    }
                }
            }
            encoded_params
        }
    }
}

pub mod js_runtime {
    use foundation_jsmacros::embed_file_as;

    #[embed_file_as("../runtime/js/runtime.js")]
    pub struct RuntimeCore;

    #[embed_file_as("../runtime/js/packer.js")]
    pub struct PackerCore;
}
