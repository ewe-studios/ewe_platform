#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_panics_doc)]

use alloc::string::String;
use alloc::vec::Vec;
use foundation_nostd::{raw_parts::RawParts, spin::Mutex};

use crate::{
    CompletedInstructions, ExternalPointer, Instructions, InternalCallback, InternalPointer,
    InternalReferenceRegistry, JSEncoding, MemoryAllocation, MemoryAllocations, MemoryId, Params,
    ToBinary,
};

static INTERNAL_CALLBACKS: Mutex<InternalReferenceRegistry> = InternalReferenceRegistry::create();

static ALLOCATIONS: Mutex<MemoryAllocations> = Mutex::new(MemoryAllocations::new());

/// [`internal_api`] are internal methods, structs, and surfaces that provide core functionalities
/// that we support or that allows making or preparing data to be sent-out or sent-across the API.
///
/// You should never place a function in here that needs to be exposed to the host or host function
/// we want to define but instead use the [`exposed_runtime`] or [`host_runtime`] modules.
pub mod internal_api {
    use super::*;

    // -- Instruction methods

    pub fn create_instructions(text_size: u64, operation_size: u64) -> Instructions {
        ALLOCATIONS
            .lock()
            .batch_for(text_size, operation_size, true)
            .expect("should create allocated memory slot")
    }

    pub fn get_memory(memory_id: MemoryId) -> MemoryAllocation {
        ALLOCATIONS
            .lock()
            .get(memory_id)
            .expect("should fetch related memory allocation")
    }

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

/// [`exposed_runtime`] are the underlying functions we expose to the host from
/// the system. These are functions the runtime exposes to the host to be able
/// to make calls into the system or triggering processes.
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
/// must provide for use with wrapper functions that make it simple
/// and easier to interact with.
#[allow(unused)]
pub mod host_runtime {
    use super::*;

    pub const DOM_SELF: ExternalPointer = ExternalPointer::pointer(0);
    pub const DOM_THIS: ExternalPointer = ExternalPointer::pointer(1);
    pub const DOM_WINDOW: ExternalPointer = ExternalPointer::pointer(2);
    pub const DOM_DOCUMENT: ExternalPointer = ExternalPointer::pointer(3);
    pub const DOM_BODY: ExternalPointer = ExternalPointer::pointer(4);

    // -- Data Information
    pub mod api_v2 {
        use super::*;

        #[link(wasm_import_module = "v2")]
        extern "C" {
            // [`apply_instructions`] takes a location in memory that has a batch of operations
            // which match the [`crate::Operations`] outlined in the batching API the
            // runtime supports, allowing us amortize the cost of doing bulk processing on
            // the wasm and host boundaries.
            pub fn apply_instructions(
                operation_pointer: u64,
                operation_length: u64,
                text_pointer: u64,
                text_length: u64,
            );

            /// [`function_allocate_external_pointer`] allows you to ahead of time request the
            /// allocation of an external reference id unique for a function and unreusable by anyone else
            /// you the owner. This allows you get an id you would use later in the future to register
            /// for usage later.
            pub fn function_allocate_external_pointer() -> u64;

            /// [`object_allocate_external_pointer`] allows you to ahead of time request the
            /// allocation of an external reference id unique for an object and unreusable by anyone else
            /// you the owner. This allows you get an id you would use later in the future to register
            /// for usage later.
            pub fn object_allocate_external_pointer() -> u64;

            /// [`dom_allocate_external_pointer`] allows you to ahead of time request the
            /// allocation of an external reference id unique for a dom node and unreusable by anyone else
            /// you the owner. This allows you get an id you would use later in the future to register
            /// for usage later.
            pub fn dom_allocate_external_pointer() -> u64;
        }

        /// [`preallocate_dom_external_reference`] requests the host runtime to pre-allocate
        /// a target external reference for usage by the caller for a dom node.
        pub fn preallocate_dom_external_reference() -> ExternalPointer {
            unsafe {
                ExternalPointer::pointer(host_runtime::api_v2::dom_allocate_external_pointer())
            }
        }

        /// [`preallocate_func_external_reference`] requests the host runtime to pre-allocate
        /// a target external reference for usage by the caller for a function.
        pub fn preallocate_func_external_reference() -> ExternalPointer {
            unsafe {
                ExternalPointer::pointer(host_runtime::api_v2::function_allocate_external_pointer())
            }
        }

        /// [`preallocate_object_external_reference`] requests the host runtime to pre-allocate
        /// a target external reference for usage by the caller for an object.
        pub fn preallocate_object_external_reference() -> ExternalPointer {
            unsafe {
                ExternalPointer::pointer(host_runtime::api_v2::object_allocate_external_pointer())
            }
        }

        /// [`send_instructions`] sends a [`CompletedInstructions`] batch over to the host runtime.
        pub fn send_instructions(instruction: CompletedInstructions) {
            let operations_memory = internal_api::get_memory(instruction.ops_id);
            let text_memory = internal_api::get_memory(instruction.text_id);

            let (ops_pointer, ops_length) =
                operations_memory.as_address().expect("get ops address");
            let (text_pointer, text_length) = text_memory.as_address().expect("get text address");

            unsafe {
                host_runtime::api_v2::apply_instructions(
                    ops_pointer as u64,
                    ops_length,
                    text_pointer as u64,
                    text_length,
                )
            }
        }

        /// [`complete_instructions`] completes and sends a [`Instructions`] batch over to the host runtime.
        pub fn complete_instructions(instruction: Instructions) {
            let completed = instruction.complete().expect("complete instruction");
            host_runtime::api_v2::send_instructions(completed);
        }
    }

    // -- Functions (Invocation & Registration)
    pub mod api_v1 {
        use crate::CachedText;

        use super::*;

        #[link(wasm_import_module = "v1")]
        extern "C" {
            //  [`js_drop_reference`] Provides a way to inform the need to drop a outside cached reference
            //  used for execution, e.g JSFunction or some other referential type.
            pub fn js_drop_reference(external_reference_id: u64);

            /// [`js_cache_string`] provides a way to cache dynamic utf8 strings that
            /// will be interned into a map of a u64 key representing the string, this allows
            /// us to pay the cost of conversion once for these types of strings
            /// whilst limiting the overall cost since only a reference is ever passed around.
            pub fn js_cache_string(start: u64, len: u64, encoding: u8) -> u64;

            // [`js_unregister_function`] provides a means to unregister a target function
            // from the WASM - Host runtime boundary.
            pub fn js_unregister_function(handle: u64);

            // registers a function via it's provided start and length
            // indicative of where the function body can be found
            // as utf-8 or utf-18 encoded byte (based on third argument)
            // from the start pointer in memory to the specified
            // length to be registered in the shared
            // function registry.
            pub fn js_register_function(start: u64, len: u64, encoding: u8) -> u64;

            // invokes a javascript function across the host boundary ABI and returns
            // a u64 [`ExternalPointer`] that can be used to retrieve or
            // manipulate`] for manipulating the given
            // dom element.
            pub fn js_invoke_function_and_return_dom(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: usize,
            ) -> u64;

            // invokes a javascript function across the host boundary ABI and returns
            // a u64 [`ExternalPointer`] that can be used to retrieve or
            // manipulate`] for manipulating the given
            // object element.
            pub fn js_invoke_function_and_return_object(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: usize,
            ) -> u64;

            // invokes a javascript function across the host boundary ABI and returns
            // a u64 that is the result.
            pub fn js_invoke_function_and_return_bigint(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: usize,
            ) -> u64;

            // invokes a javascript function across the host boundary ABI and returns
            // a bool (represented as a u8) contains either 0 for false or 1 for true.
            pub fn js_invoke_function_and_return_bool(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: usize,
            ) -> u8;

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

        // [`Droppable`] creates a reference that when drops will
        // also drop the related reference on the host JS runtime.
        pub struct Droppable(ExternalPointer);

        impl Droppable {
            pub fn number(&self) -> u64 {
                self.0.into_inner()
            }
        }

        impl Drop for Droppable {
            fn drop(&mut self) {
                unsafe {
                    host_runtime::api_v1::js_drop_reference(self.0.into_inner());
                }
            }
        }

        impl From<ExternalPointer> for Droppable {
            fn from(value: ExternalPointer) -> Self {
                Self(value)
            }
        }

        impl From<u64> for Droppable {
            fn from(value: u64) -> Self {
                Self(value.into())
            }
        }

        /// [`cached_text`] provides a way to have the host runtime cache an expense
        /// text for you which allows you perform multiple re-use of the same text.
        ///
        /// Remember that UTF-8 top UTF-16 is an expensive operation and when you have
        /// a string you plan to re-use over and over then there is benefit in simply
        /// caching these string but also understand we are using more memory both in
        /// the rust (guest wasm) side and the host side and you really only benefit from
        /// that overhead when that string really is very much reused alot.
        ///
        /// Additionally, what if you end up sending the same UTF-16 text over to the host
        /// often, there is also benefit in just sending it once, caching it and referencing
        /// it by the cache id.
        ///
        /// Be aware this is immediate and instead of keeping the memory slot owned on the
        /// wasm side, we defer to the host runtime to always maintain a reference id for us
        /// and must guarantee that id will hold for the lifetime of the program.
        pub fn cache_text(code: &str) -> CachedText {
            let start = code.as_ptr() as usize;
            let len = code.len();
            unsafe {
                CachedText::pointer(host_runtime::api_v1::js_cache_string(
                    start as u64,
                    len as u64,
                    JSEncoding::UTF8.into(),
                ))
            }
        }

        /// [`register_function`] calls the underlying [`js_abi`] registration
        /// function to register a javascript code that can be called from memory
        /// allowing you define the underlying code we want executed.
        pub fn register_function(code: &str) -> JSFunction {
            let start = code.as_ptr() as usize;
            let len = code.len();
            unsafe {
                JSFunction {
                    handler: host_runtime::api_v1::js_register_function(
                        start as u64,
                        len as u64,
                        JSEncoding::UTF8.into(),
                    ),
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
                    handler: host_runtime::api_v1::js_register_function(
                        start as u64,
                        len as u64,
                        JSEncoding::UTF16.into(),
                    ), // precision loss here
                }
            }
        }

        // --- Browser / WASM ABI

        #[derive(Copy, Clone)]
        pub struct JSFunction {
            pub handler: u64,
        }

        #[allow(clippy::cast_precision_loss)]
        impl JSFunction {
            /// [`invoke`] invokes a javascript function registered at the given handle
            /// defined by the [`JSFunction::handler`] which then receives the set of parameters
            /// supplied to be invoked with.
            ///
            /// The `js_abi` will handle necessary conversion and execution of the function
            /// with the passed arguments.
            pub fn invoke(&self, params: &[Params]) -> u64 {
                let param_bytes = params.to_binary();
                let RawParts {
                    ptr,
                    length,
                    capacity: _,
                } = RawParts::from_vec(param_bytes);
                unsafe { host_runtime::api_v1::js_invoke_function(self.handler, ptr, length) }
            }

            /// [`invoke_for_bool`] invokes a javascript function registered at the given handle
            /// defined by the [`JSFunction::handler`] which then returns a bool indicating the
            /// result.
            ///
            /// Internal true is when the returned number is >= 1 and False if 0.
            pub fn invoke_for_bool(&self, params: &[Params]) -> bool {
                let param_bytes = params.to_binary();
                let RawParts {
                    ptr,
                    length,
                    capacity: _,
                } = RawParts::from_vec(param_bytes);
                unsafe {
                    match host_runtime::api_v1::js_invoke_function_and_return_bool(
                        self.handler,
                        ptr,
                        length,
                    ) {
                        1.. => true,
                        0 => false,
                    }
                }
            }

            /// [`invoke_for_dom`] invokes a javascript function registered at the given handle
            /// defined by the [`JSFunction::handler`] which then returns a [`ExternalPointer`]
            /// representing the DOM node instance via an ExternalPointer that points to that object in the
            /// hosts object heap.
            pub fn invoke_for_bigint(&self, params: &[Params]) -> u64 {
                let param_bytes = params.to_binary();
                let RawParts {
                    ptr,
                    length,
                    capacity: _,
                } = RawParts::from_vec(param_bytes);
                unsafe {
                    host_runtime::api_v1::js_invoke_function_and_return_bigint(
                        self.handler,
                        ptr,
                        length,
                    )
                }
            }

            /// [`invoke_for_dom`] invokes a javascript function registered at the given handle
            /// defined by the [`JSFunction::handler`] which then returns a [`ExternalPointer`]
            /// representing the DOM node instance via an ExternalPointer that points to that object in the
            /// hosts object heap.
            pub fn invoke_for_dom(&self, params: &[Params]) -> ExternalPointer {
                let param_bytes = params.to_binary();
                let RawParts {
                    ptr,
                    length,
                    capacity: _,
                } = RawParts::from_vec(param_bytes);
                unsafe {
                    host_runtime::api_v1::js_invoke_function_and_return_dom(
                        self.handler,
                        ptr,
                        length,
                    )
                    .into()
                }
            }

            /// [`invoke_for_object`] invokes a javascript function registered at the given handle
            /// defined by the [`JSFunction::handler`] which then returns a [`ExternalPointer`]
            /// representing the object via an ExternalPointer that points to that object in the
            /// hosts object heap.
            pub fn invoke_for_object(&self, params: &[Params]) -> ExternalPointer {
                let param_bytes = params.to_binary();
                let RawParts {
                    ptr,
                    length,
                    capacity: _,
                } = RawParts::from_vec(param_bytes);
                unsafe {
                    host_runtime::api_v1::js_invoke_function_and_return_object(
                        self.handler,
                        ptr,
                        length,
                    )
                    .into()
                }
            }

            /// [`unregister_function`] calls the JS ABI on the host to de-register
            /// the target function.
            pub fn unregister(self) {
                unsafe { host_runtime::api_v1::js_unregister_function(self.handler) }
            }
        }
    }
}
