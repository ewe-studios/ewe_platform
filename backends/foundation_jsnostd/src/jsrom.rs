#![allow(clippy::missing_doc_code_examples)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::missing_panics_doc)]

use foundation_nostd::{raw_parts::RawParts, spin::Mutex};

static ALLOCATIONS: Mutex<Vec<Option<Vec<u8>>>> = Mutex::new(Vec::new());

pub type JSAllocationId = f64;

#[no_mangle]
pub extern "C" fn create_allocation(size: usize) -> usize {
    let mut buf = Vec::with_capacity(size as usize);
    buf.resize(size, 0);
    let mut allocations = ALLOCATIONS.lock();
    let index = allocations.len();
    allocations.push(Some(buf));
    index
}

#[no_mangle]
pub extern "C" fn allocation_start_pointer(allocation_id: usize) -> *const u8 {
    let allocations = ALLOCATIONS.lock();
    let allocation = allocations
        .get(allocation_id)
        .expect("Allocation should be initialized");
    let vec = allocation.as_ref().unwrap();
    vec.as_ptr()
}

#[no_mangle]
pub extern "C" fn allocation_length(allocation_id: usize) -> f64 {
    let allocations = ALLOCATIONS.lock();
    let allocation = allocations
        .get(allocation_id)
        .expect("Allocation should be initialized");
    let vec = allocation.as_ref().unwrap();
    vec.len() as f64
}

#[no_mangle]
pub extern "C" fn clear_allocation(allocation_id: usize) {
    let mut allocations = ALLOCATIONS.lock();
    allocations[allocation_id] = None;
}

// -- extract methods

pub fn extract_vec_from_memory(allocation_id: usize) -> Vec<u8> {
    let allocations = ALLOCATIONS.lock();
    let allocation = allocations.get(allocation_id).expect("should be allocated");
    let vec = allocation.as_ref().unwrap();
    vec.clone()
}

pub fn extract_string_from_memory(allocation_id: usize) -> String {
    let allocations = ALLOCATIONS.lock();
    let allocation = allocations.get(allocation_id).expect("should be allocated");
    let vec = allocation.as_ref().unwrap();
    String::from_utf8(vec.clone()).unwrap()
}

/// `JSABI` is the expected interface which the JS portion or
/// using runtime needs to provide to ensure the underlying
/// code can correctly work and function.
///
/// These are functions that need to be provided.
#[allow(unused)]
pub mod js_abi {

    // -- Data Information
    pub mod data {
        #[link(wasm_import_module = "data")]
        extern "C" {
            // this support the need to send a value that will later
            // be picked up for processing, e.g callbacks registered for
            // a specific respoonse or batch data that should be processed.
            // It requires the relevant ABI runtime side to know how to handle such things.
            pub fn process_data(allocation_start: i64, allocation_end: i64);
        }
    }

    // -- Reference handling
    pub mod references {
        #[link(wasm_import_module = "refs")]
        extern "C" {
            //  Provides a way to inform the need to drop a outside cached reference
            //  used for execution, e.g JSFunction or some other referential type.
            pub fn drop_reference(external_reference_id: i64);
        }
    }

    // -- Functions (Invocation & Registration)
    pub mod functions {
        #[link(wasm_import_module = "funcs")]
        extern "C" {
            // registers a function via it's provided start and length
            // indicative of where the function body can be found
            // as utf-8 or utf-18 encoded byte (based on third argument)
            // from the start pointer in memory to the specified
            // length to be registered in the shared
            // function registry.
            pub fn js_register_function(start: f64, len: f64, encoding: u8) -> f64;

            // invokes a Javascript function across the WASM/RUST ABI
            // which then returns the allocation_id (as f64) that can
            // be used to get the related allocation vector
            // from the global allocations.
            pub fn js_invoke_function(
                handler: f64,
                parameters_start: *const u8,
                parameters_length: usize,
            ) -> f64;
        }
    }
}

// -- registration functions

#[derive(Copy, Clone)]
pub struct JSFunction {
    pub handler: f64,
}

#[macro_export]
macro_rules! js {
    ($e:expr) => {{
        static mut FN: Option<f64> = None;
        unsafe {
            if FN.is_none() {
                // store the handler for the related js function as a catch for later use.
                FN = Some(foundation_jsnostd::JSFunction::register_function($e).handler);
            }
            JSFunction {
                handler: FN.unwrap(),
            }
        }
    }};
}

/// [`JSEncoding`] defines a defining type to help indicate the
/// underlying encoding for a giving text body.
pub enum JSEncoding {
    UTF8,
    UTF16,
}

impl Into<f32> for JSEncoding {
    fn into(self) -> f32 {
        match self {
            JSEncoding::UTF8 => 8.0,
            JSEncoding::UTF16 => 16.0,
        }
    }
}

impl Into<u8> for JSEncoding {
    fn into(self) -> u8 {
        match self {
            JSEncoding::UTF8 => 8,
            JSEncoding::UTF16 => 16,
        }
    }
}

impl Into<u32> for JSEncoding {
    fn into(self) -> u32 {
        match self {
            JSEncoding::UTF8 => 8,
            JSEncoding::UTF16 => 16,
        }
    }
}

impl From<u8> for JSEncoding {
    fn from(value: u8) -> Self {
        if value == 8 {
            return JSEncoding::UTF8;
        }
        if value == 16 {
            return JSEncoding::UTF16;
        }
        JSEncoding::UTF8
    }
}

impl From<u16> for JSEncoding {
    fn from(value: u16) -> Self {
        if value == 8 {
            return JSEncoding::UTF8;
        }
        if value == 16 {
            return JSEncoding::UTF16;
        }
        JSEncoding::UTF8
    }
}

impl From<u32> for JSEncoding {
    fn from(value: u32) -> Self {
        if value == 8 {
            return JSEncoding::UTF8;
        }
        if value == 16 {
            return JSEncoding::UTF16;
        }
        JSEncoding::UTF8
    }
}

impl From<f32> for JSEncoding {
    fn from(value: f32) -> Self {
        if value == 8.0 {
            return JSEncoding::UTF8;
        }
        if value == 16.0 {
            return JSEncoding::UTF16;
        }
        JSEncoding::UTF8
    }
}

impl From<f64> for JSEncoding {
    fn from(value: f64) -> Self {
        if value == 8.0 {
            return JSEncoding::UTF8;
        }
        if value == 16.0 {
            return JSEncoding::UTF16;
        }
        JSEncoding::UTF8
    }
}

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
                handler: js_abi::functions::js_register_function(
                    start as f64,
                    len as f64,
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
                handler: js_abi::functions::js_register_function(
                    start as f64,
                    len as f64,
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
        unsafe { js_abi::functions::js_invoke_function(self.handler, ptr, length) }
    }
}

// -- ExternalReference

pub const JS_UNDEFINED: ExternalReference = ExternalReference { value: 0 };
pub const JS_NULL: ExternalReference = ExternalReference { value: 1 };
pub const DOM_SELF: ExternalReference = ExternalReference { value: 2 };
pub const DOM_WINDOW: ExternalReference = ExternalReference { value: 2 };
pub const DOM_DOCUMENT: ExternalReference = ExternalReference { value: 3 };
pub const DOM_BODY: ExternalReference = ExternalReference { value: 4 };

pub struct ExternalReference {
    pub value: i64,
}

impl Drop for ExternalReference {
    fn drop(&mut self) {
        unsafe {
            js_abi::references::drop_reference(self.value);
        }
    }
}

impl From<i64> for ExternalReference {
    fn from(value: i64) -> Self {
        Self { value }
    }
}

// --- Browser / WASM ABI

/// `ReturnTypes`  represent the allocations type value being
/// communicated by the contents of an allocation, a byte is
/// allocated in each allocation to define what return type it
/// represent.
#[repr(usize)]
pub enum ReturnTypes {
    Undefined = 0,
    NULL = 1,
    String = 2,
    Float64 = 3,
    BigInt = 4,
    Vector = 5,
    ExternalReference = 6,
}

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
    ExternalReference(&'a ExternalReference),
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

impl<'a> From<&'a ExternalReference> for InvocationParameter<'a> {
    fn from(i: &'a ExternalReference) -> Self {
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
                    encoded_params.extend_from_slice(&i.value.to_le_bytes());
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
