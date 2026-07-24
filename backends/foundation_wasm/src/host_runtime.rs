// Every public function: returns type-aliased Results; panics on Mutex poison.
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::must_use_candidate)]

use alloc::string::String;
use alloc::vec::Vec;
use foundation_nostd::comp::basic::Mutex;

use crate::{
    DoTask, FnDoTask, FnFrameCallback, FnIntervalCallback, FrameCallback, FrameCallbackList,
    FromBinary, Instructions, InternalCallback, InternalPointer, InternalReferenceRegistry,
    IntervalCallback, IntervalRegistry, MemoryAllocation, MemoryAllocationError, MemoryAllocations,
    MemoryId, MemoryReaderError, ReturnTypeHints, ReturnTypeId, Returns, ScheduleRegistry,
    TaskErrorCode, TaskResult, ThreeState, TickState,
};

// Imports only the `web` ABI module needs (gated to keep non-web builds warning-free).
#[cfg(feature = "web")]
use crate::{
    BinaryReadError, BinaryReaderResult, CompletedInstructions, ExternalPointer, JSEncoding,
    Params, ReturnValueError, ReturnValues, ToBinary,
};
#[cfg(feature = "web")]
use foundation_nostd::raw_parts::RawParts;

// Allocations for the memory management.
// `pub(crate)` so the relocated return-value parser in `protocol.rs` can read/free
// arena slots for array-buffer return values (feature 00 Layer 2).
pub(crate) static ALLOCATIONS: Mutex<MemoryAllocations> = Mutex::new(MemoryAllocations::create());

// All registered animation callback, a registered function must indicate when it should
// be removed and hence no direct removal/deletion is supported.
static ANIMATION_FRAME_CALLBACKS: Mutex<FrameCallbackList> =
    Mutex::new(FrameCallbackList::create());

// All operation callbacks registered for a giving result by by the host.
static INTERNAL_CALLBACKS: Mutex<InternalReferenceRegistry> = InternalReferenceRegistry::create();

// All registered interval callbacks (think: setInterval in JS) registry.
static RECURRING_INTERVAL_CALLBACKS: Mutex<IntervalRegistry> = IntervalRegistry::create();

// All registered scheduled callbacks (think: setTimeout in JS) registry.
static SCHEDULED_CALLBACKS: Mutex<ScheduleRegistry> = ScheduleRegistry::create();

/// [`internal_api`] are internal methods, structs, and surfaces that provide core functionalities
/// that we support or that allows making or preparing data to be sent-out or sent-across the API.
///
/// You should never place a function in here that needs to be exposed to the host or host function
/// we want to define but instead use the [`exposed_runtime`] or [`abi`] modules.
pub mod internal_api {
    #![allow(clippy::missing_errors_doc)]
    use alloc::boxed::Box;

    use crate::{FnCallback, MemoryAllocationResult, ReturnValues};

    use super::{
        internal_api, DoTask, FnDoTask, FnFrameCallback, FnIntervalCallback, FrameCallback,
        FromBinary, Instructions, InternalCallback, InternalPointer, IntervalCallback,
        MemoryAllocation, MemoryAllocationError, MemoryId, MemoryReaderError, ReturnTypeHints,
        ReturnTypeId, Returns, String, TaskErrorCode, TaskResult, ThreeState, TickState, Vec,
        ALLOCATIONS, ANIMATION_FRAME_CALLBACKS, INTERNAL_CALLBACKS, RECURRING_INTERVAL_CALLBACKS,
        SCHEDULED_CALLBACKS,
    };

    static FAILED_RETURN_HINT: ReturnTypeHints =
        ReturnTypeHints::One(ThreeState::One(ReturnTypeId::ErrorCode));

    // -- Instruction methods

    pub fn create_instructions(text_size: u64, operation_size: u64) -> Instructions {
        ALLOCATIONS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .batch_for(text_size, operation_size, true)
            .expect("should create allocated memory slot")
    }

    pub fn get_memory(memory_id: MemoryId) -> MemoryAllocation {
        ALLOCATIONS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .get(memory_id)
            .expect("should fetch related memory allocation")
    }

    /// [`with_global_allocations`] runs `f` with exclusive access to the GLOBAL
    /// arena — the one the `dispose_allocation`/`allocation_start_pointer` WASM
    /// exports operate on. Anything shipping message slots to the JS host MUST
    /// allocate them here: ids from a private [`super::MemoryAllocations`] are
    /// meaningless to JS's ACK path and would fail its generation check.
    pub fn with_global_allocations<R>(f: impl FnOnce(&mut super::MemoryAllocations) -> R) -> R {
        let mut guard = ALLOCATIONS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        f(&mut guard)
    }

    // Callback return parsers

    /// Parse the replies encoded in the memory location referenced by the
    /// provided [`MemoryId`].
    ///
    /// # Errors
    ///
    /// Returns [`MemoryAllocationError`] if the allocation is invalid or
    /// the reply binary cannot be deserialized.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn parse_callback_replies(
        memory_id: MemoryId,
        returns: ReturnTypeHints,
    ) -> MemoryAllocationResult<Returns> {
        let memory = internal_api::get_memory(memory_id);
        let result_container = memory.into_with(|mem| {
            (
                returns.clone().from_binary(mem.as_ref()),
                FAILED_RETURN_HINT.clone().from_binary(mem.as_ref()),
            )
        });

        if result_container.is_none() {
            return Err(MemoryAllocationError::FailedAllocationReading(memory_id));
        }

        let (result, failed_result) = result_container.unwrap();

        if let Ok(mut failed_returns) = failed_result {
            if let Some(ReturnValues::ErrorCode(code)) = failed_returns.pop() {
                return Err(MemoryAllocationError::TaskFailure(TaskErrorCode::new(code)));
            }
        }

        if let Err(err) = result {
            return Err(MemoryReaderError::NotValidReplyBinary(err).into());
        }

        let mut replies = result.unwrap();

        match &returns {
            ReturnTypeHints::One(_) => {
                if replies.len() != 1 {
                    return Err(MemoryReaderError::ReturnValueError(
                        crate::ReturnValueError::ExpectedOne(replies),
                    )
                    .into());
                }

                Ok(Returns::One(replies.pop().expect("should have value")))
            }
            ReturnTypeHints::List(_) => Ok(Returns::List(replies)),
            ReturnTypeHints::Multi(_) => Ok(Returns::Multi(replies)),
            ReturnTypeHints::None => unreachable!(
                "ReturnTypeHints::None should never be returned, its a bug at the host"
            ),
        }
    }

    // animation function registration with the host.

    /// [`get_total_animation_callbacks`] returns total count of registered callbacks.
    pub fn get_total_animation_callbacks() -> usize {
        ANIMATION_FRAME_CALLBACKS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .len()
    }

    /// [`run_animation_frames`] provides a method that will automatically
    /// convert any type that implements the [`Fn`] trait.
    pub fn run_animation_frames(tick: f64) -> u8 {
        ANIMATION_FRAME_CALLBACKS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .call(tick)
            .into()
    }

    /// [`register_animation_hook`] provides a method that will automatically
    /// convert any type that implements the [`Fn`] trait.
    #[cfg(not(target_family = "wasm"))]
    pub fn register_animation_hook<F>(f: F)
    where
        F: Fn(f64) -> TickState + Send + Sync + 'static,
    {
        ANIMATION_FRAME_CALLBACKS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .add(Box::new(FnFrameCallback::new(Box::new(f))));
    }

    /// [`register_callback`] provides a method that will automatically
    /// convert any type that implements the [`Fn`] trait.
    #[cfg(target_family = "wasm")]
    pub fn register_animation_hook<F>(f: F)
    where
        F: Fn(f64) -> TickState + 'static,
    {
        ANIMATION_FRAME_CALLBACKS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .add(Box::new(FnFrameCallback::new(Box::new(f))));
    }

    /// [`register_animation_interval_callback`] provides a more direct method for
    /// registering a type that implements the [`FrameCallback`] trait.
    #[cfg(target_family = "wasm")]
    pub fn register_animation_interval_callback<F>(f: F)
    where
        F: FrameCallback + 'static,
    {
        ANIMATION_FRAME_CALLBACKS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .add(Box::new(f));
    }

    /// [`register_animation_interval_callback`] provides a more direct method for
    /// registering a type that implements the [`FrameCallback`] trait.
    #[cfg(not(target_family = "wasm"))]
    pub fn register_animation_interval_callback<F>(f: F)
    where
        F: FrameCallback + Send + Sync + 'static,
    {
        ANIMATION_FRAME_CALLBACKS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .add(Box::new(f));
    }

    // schedule function registration with the host.

    /// [`unregister_schedule_callback`] unregisters the underlying callback
    /// in the schedule callback list.
    pub fn unregister_schedule_callback(addr: InternalPointer) {
        SCHEDULED_CALLBACKS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .delete(addr)
            .expect("should be registered");
    }

    /// Run a registered scheduled callback by ID (oneshot timeout).
    ///
    /// # Errors
    ///
    /// Returns an error if the callback is not found or has already fired.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn run_schedule_callback(id: InternalPointer) -> crate::WasmRequestResult<()> {
        match SCHEDULED_CALLBACKS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .call(id)
        {
            Some(()) => Ok(()),
            None => Err(crate::WASMErrors::GuestError(
                crate::GuestOperationError::UnknownInternalPointer(id),
            )),
        }
    }

    /// [`register_callback`] provides a method that will automatically
    /// convert any type that implements the [`Fn`] trait.
    #[cfg(not(target_family = "wasm"))]
    pub fn register_schedule<F>(f: F) -> InternalPointer
    where
        F: Fn() + Send + Sync + 'static,
    {
        SCHEDULED_CALLBACKS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .add(Box::new(FnDoTask::new(Box::new(f))))
    }

    /// [`register_callback`] provides a method that will automatically
    /// convert any type that implements the [`Fn`] trait.
    #[cfg(target_family = "wasm")]
    pub fn register_schedule<F>(f: F) -> InternalPointer
    where
        F: Fn() + 'static,
    {
        SCHEDULED_CALLBACKS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .add(Box::new(FnDoTask::new(Box::new(f))))
    }

    /// [`register_schedule_callback`] provides a more direct method for
    /// registering a type that implements the [`InternalCallback`] trait.
    #[cfg(target_family = "wasm")]
    pub fn register_schedule_callback<F>(f: F) -> InternalPointer
    where
        F: DoTask + 'static,
    {
        SCHEDULED_CALLBACKS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .add(Box::new(f))
    }

    /// [`register_schedule_callback`] provides a more direct method for
    /// registering a type that implements the [`InternalCallback`] trait.
    #[cfg(not(target_family = "wasm"))]
    pub fn register_schedule_callback<F>(f: F) -> InternalPointer
    where
        F: DoTask + Send + Sync + 'static,
    {
        SCHEDULED_CALLBACKS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .add(Box::new(f))
    }

    // interval function registration with the host.

    /// Run a registered interval callback by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the callback is not found or has been deregistered.
    ///
    /// # Panics
    ///
    /// Panics if the internal mutex is poisoned.
    pub fn run_interval_callback(id: InternalPointer) -> crate::WasmRequestResult<TickState> {
        match RECURRING_INTERVAL_CALLBACKS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .call(id)
        {
            Some(inner) => Ok(inner),
            None => Err(crate::WASMErrors::GuestError(
                crate::GuestOperationError::UnknownInternalPointer(id),
            )),
        }
    }

    /// [`register_callback`] provides a method that will automatically
    /// convert any type that implements the [`Fn`] trait.
    #[cfg(not(target_family = "wasm"))]
    pub fn register_interval<F>(f: F) -> InternalPointer
    where
        F: Fn() -> TickState + Send + Sync + 'static,
    {
        RECURRING_INTERVAL_CALLBACKS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .add(Box::new(FnIntervalCallback::new(Box::new(f))))
    }

    /// [`unregister_interval_callback`] unregisters the underlying callback
    /// in the interval callback list.
    pub fn unregister_interval_callback(addr: InternalPointer) {
        RECURRING_INTERVAL_CALLBACKS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .delete(addr)
            .expect("should be registered");
    }

    /// [`register_callback`] provides a method that will automatically
    /// convert any type that implements the [`Fn`] trait.
    #[cfg(target_family = "wasm")]
    pub fn register_interval<F>(f: F) -> InternalPointer
    where
        F: Fn() -> TickState + 'static,
    {
        RECURRING_INTERVAL_CALLBACKS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .add(Box::new(FnIntervalCallback::new(Box::new(f))))
    }

    /// [`register_interval_callback`] provides a more direct method for
    /// registering a type that implements the [`InternalCallback`] trait.
    #[cfg(target_family = "wasm")]
    pub fn register_interval_callback<F>(f: F) -> InternalPointer
    where
        F: IntervalCallback + 'static,
    {
        RECURRING_INTERVAL_CALLBACKS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .add(Box::new(f))
    }

    /// [`register_interval_callback`] provides a more direct method for
    /// registering a type that implements the [`InternalCallback`] trait.
    #[cfg(not(target_family = "wasm"))]
    pub fn register_interval_callback<F>(f: F) -> InternalPointer
    where
        F: IntervalCallback + Send + Sync + 'static,
    {
        RECURRING_INTERVAL_CALLBACKS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .add(Box::new(f))
    }

    // -- callback methods

    /// [`register_callback`] provides a method that will automatically
    /// convert any type that implements the [`Fn`] trait.
    #[cfg(not(target_family = "wasm"))]
    pub fn register_callback<F>(returns: ReturnTypeHints, f: F) -> InternalPointer
    where
        F: Fn(TaskResult<Returns>) + Send + Sync + 'static,
    {
        INTERNAL_CALLBACKS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .add(returns, Box::new(FnCallback::new(Box::new(f))))
    }

    /// [`register_callback`] provides a method that will automatically
    /// convert any type that implements the [`Fn`] trait.
    #[cfg(target_family = "wasm")]
    pub fn register_callback<F>(returns: ReturnTypeHints, f: F) -> InternalPointer
    where
        F: Fn(TaskResult<Returns>) + 'static,
    {
        INTERNAL_CALLBACKS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .add(returns, Box::new(FnCallback::new(Box::new(f))))
    }

    /// [`register_internal_callback`] provides a more direct method for
    /// registering a type that implements the [`InternalCallback`] trait.
    #[cfg(not(target_family = "wasm"))]
    pub fn register_internal_callback<F>(returns: ReturnTypeHints, f: F) -> InternalPointer
    where
        F: InternalCallback + Send + Sync + 'static,
    {
        INTERNAL_CALLBACKS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .add(returns, Box::new(f))
    }

    /// [`register_internal_callback`] provides a more direct method for
    /// registering a type that implements the [`InternalCallback`] trait.
    #[cfg(target_family = "wasm")]
    pub fn register_internal_callback<F>(returns: ReturnTypeHints, f: F) -> InternalPointer
    where
        F: InternalCallback + 'static,
    {
        INTERNAL_CALLBACKS
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .add(returns, Box::new(f))
    }

    pub fn unregister_internal_callback(addr: InternalPointer) {
        INTERNAL_CALLBACKS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .delete(addr)
            .expect("should be registered");
    }

    pub fn run_internal_callbacks(addr: InternalPointer, value: MemoryId) {
        let returns = INTERNAL_CALLBACKS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
            .get_type(addr)
            .expect("should be registered");

        match internal_api::parse_callback_replies(value, returns) {
            Ok(values) => {
                INTERNAL_CALLBACKS
                    .lock()
                    .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                    .call(addr, Ok(values))
                    .expect("should have called callback");
            }
            Err(err) => match err {
                MemoryAllocationError::TaskFailure(code) => {
                    INTERNAL_CALLBACKS
                        .lock()
                        .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                        .call(addr, Err(code))
                        .expect("should have called callback");
                }
                _ => {
                    panic!("Runtime memory bug, please investigate, this should not fail: {err:?}")
                }
            },
        }
    }

    // -- extract methods

    pub fn extract_vec_from_memory(allocation_id: u64) -> Vec<u8> {
        let allocations = ALLOCATIONS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        // Return empty vec for invalid allocation ids (0 = uninitialized).
        match allocations.get(allocation_id.into()) {
            Ok(mem) => mem.clone_memory().unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    pub fn extract_string_from_memory(allocation_id: u64) -> String {
        let allocations = ALLOCATIONS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        match allocations.get(allocation_id.into()) {
            Ok(mem) => mem.string_from_memory().unwrap_or_default(),
            Err(_) => String::new(),
        }
    }
}

/// [`exposed_runtime`] are the underlying functions we expose to the host from
/// the system. These are functions the runtime exposes to the host to be able
/// to make calls into the system or triggering processes.
pub mod exposed_runtime {
    #![allow(clippy::missing_errors_doc)]
    use super::{internal_api, InternalPointer, MemoryId, ALLOCATIONS};

    #[no_mangle]
    pub extern "C" fn create_allocation(size: u64) -> u64 {
        let mut arena = ALLOCATIONS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        arena.seed(); // idempotent — ensures slot 0 is consumed, first real id is 1
        arena
            .allocate(size)
            .expect("should create requested allocation")
            .as_u64()
    }

    #[no_mangle]
    pub extern "C" fn allocation_start_pointer(mem_id: u64) -> *const u8 {
        let allocations = ALLOCATIONS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        let memory = allocations
            .get(mem_id.into())
            .expect("Allocation should be initialized");
        memory
            .get_pointer()
            .expect("should be able to get valid pointer")
    }

    #[no_mangle]
    pub extern "C" fn allocation_length(allocation_id: u64) -> u64 {
        let allocations = ALLOCATIONS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        let mem = allocations
            .get(allocation_id.into())
            .expect("Allocation should be initialized");
        mem.len().expect("should return allocation length")
    }

    #[no_mangle]
    pub extern "C" fn dispose_allocation(allocation_id: u64) {
        let mut allocations = ALLOCATIONS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        // Silently ignore invalid / already-freed allocation ids.
        let _ = allocations.deallocate(allocation_id.into());
    }

    #[no_mangle]
    pub extern "C" fn clear_allocation(allocation_id: u64) {
        let allocations = ALLOCATIONS
            .lock()
            .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner);
        let mem = allocations
            .get(allocation_id.into())
            .expect("Allocation should be initialized");
        mem.clear().expect("should clear memory");
    }

    // schedule function registration with the host.

    #[no_mangle]
    pub extern "C" fn run_scheduled_callback(id: u64) {
        internal_api::run_schedule_callback(InternalPointer::pointer(id))
            .expect("should trigger handler");
    }

    #[no_mangle]
    pub extern "C" fn run_interval_callback(internal_pointer: u64) -> u8 {
        let state = internal_api::run_interval_callback(InternalPointer::pointer(internal_pointer))
            .expect("should have executed");
        state.into_u8()
    }

    #[no_mangle]
    pub extern "C" fn trigger_animation_callbacks(tick: f64) -> u8 {
        internal_api::run_animation_frames(tick)
    }

    #[no_mangle]
    pub extern "C" fn get_total_animation_callbacks() -> usize {
        internal_api::get_total_animation_callbacks()
    }

    #[no_mangle]
    pub extern "C" fn unregister_callback(addr: u64) {
        internal_api::unregister_internal_callback(addr.into());
    }

    #[no_mangle]
    pub extern "C" fn invoke_callback(internal_pointer: u64, allocation_id: u64) {
        internal_api::run_internal_callbacks(
            InternalPointer::pointer(internal_pointer),
            MemoryId::from_u64(allocation_id),
        );
    }

    #[cfg(feature = "wasi")]
    #[no_mangle]
    pub extern "C" fn wasi_tick() -> u8 {
        crate::wasi_host::tick().into_u8()
    }

    #[cfg(feature = "wasi")]
    #[no_mangle]
    pub extern "C" fn wasi_poll_blocking() -> u8 {
        crate::wasi_host::poll_blocking().into_u8()
    }

    #[cfg(feature = "wasi")]
    #[no_mangle]
    pub extern "C" fn wasi_time_until_next_event_ms() -> u64 {
        crate::wasi_host::time_until_next_event()
            .map(|d| d.as_millis() as u64)
            .unwrap_or(u64::MAX)
    }

    #[cfg(feature = "wasi")]
    #[no_mangle]
    pub extern "C" fn wasi_has_pending_work() -> u8 {
        u8::from(crate::wasi_host::has_pending_work())
    }
}

/// [`abi`] is the expected interface which the JS/Host
/// must provide for use with wrapper functions that make it simple
/// and easier to interact with.
///
/// Gated behind the `web` feature: it declares the `wasm_import_module = "abi"` host
/// imports, so only consumers talking to a JS/web host compile it in. Non-web hosts
/// (WASI, native, custom WASM hosts) use the rest of the ABI (memory, encoding,
/// registries, WASM exports) without being forced to supply these imports.
#[cfg(feature = "web")]
#[allow(unused)]
pub mod abi {
    use super::{
        abi, internal_api, BinaryReadError, BinaryReaderResult, CompletedInstructions, DoTask,
        ExternalPointer, FrameCallback, FromBinary, InternalCallback, InternalPointer,
        IntervalCallback, JSEncoding, MemoryAllocationError, MemoryId, Params, RawParts,
        ReturnTypeHints, ReturnTypeId, ReturnValueError, ReturnValues, Returns, String, ThreeState,
        TickState, ToBinary, Vec, ALLOCATIONS,
    };
    // GroupReturnTypeHints now lives in `protocol.rs` (feature 00 Layer 2).
    use crate::GroupReturnTypeHints;

    // DOM reference constants (DOM_SELF/THIS/WINDOW/DOCUMENT/BODY) moved to
    // `foundation_wasm_ui::wasm::dom::constants` (feature 00 — no DOM in the ABI crate).

    // -- Functions (Invocation & Registration)
    pub mod web {
        use crate::{CachedText, MemoryAllocationResult, MemoryReaderError, WasmRequestResult};

        use super::{
            abi, internal_api, BinaryReadError, BinaryReaderResult, CompletedInstructions, DoTask,
            ExternalPointer, FrameCallback, FromBinary, InternalCallback, InternalPointer,
            IntervalCallback, JSEncoding, MemoryAllocationError, MemoryId, Params, RawParts,
            ReturnTypeHints, ReturnTypeId, ReturnValueError, ReturnValues, Returns, String,
            ThreeState, TickState, ToBinary, Vec, ALLOCATIONS,
        };
        // GroupReturnTypeHints now lives in `protocol.rs` (feature 00 Layer 2).
        use crate::GroupReturnTypeHints;

        #[cfg(target_family = "wasm")]
        #[link(wasm_import_module = "abi")]
        extern "C" {

            /// [`hook_into_animation_frames`] registers a our interest with the host to be called
            ///  by the host animation runloop (e.g requestAnimationFrame in JS) to get
            ///  triggered (calling [`exposed_runtime::trigger_animation_callbacks`]) when
            ///  the next animation frame triggers.
            ///  Generally, the host will ask for how many animation callbacks are registered and not
            ///  trigger anything if its 0 via [`exposed_runtime::get_total_animation_callbacks`].
            pub fn hook_up_animation_frames();

            /// [`schedule_timeout`] registers a function to be called after
            /// a giving duration in milliseconds.
            pub fn schedule_timeout(timing: f64, callback: u64);

            /// [`unschedule_timeout`] unregisters a function timeout registration.
            pub fn unschedule_timeout(callback: u64);

            /// [`schedule_interval`] registers a function to be polled every [`timing`]
            /// and calls the relevant function when the timing is met recurringly.
            /// Think of it as setInterval in JS Host.
            pub fn schedule_interval(timing: f64, callback: u64);

            /// [`unschedule_interval`] unregisters a function interval registration.
            pub fn unschedule_interval(callback: u64);

            /// [`host_batch_apply`] takes a location in memory that has a batch of operations
            /// which match the [`crate::Operations`] outlined in the batching API the
            /// runtime supports, allowing us amortize the cost of doing bulk processing on
            /// the wasm and host boundaries.
            ///
            /// This batch apply returns no value and no underlying result will
            /// be written to memory.
            pub fn host_batch_apply(
                operation_pointer: u64,
                operation_length: u64,
                text_pointer: u64,
                text_length: u64,
            );

            /// [`host_batch_returning_apply`] takes a location in memory that has a batch of operations
            /// which match the [`crate::Operations`] outlined in the batching API the
            /// runtime supports, allowing us amortize the cost of doing bulk processing on
            /// the wasm and host boundaries.
            pub fn host_batch_returning_apply(
                operation_pointer: u64,
                operation_length: u64,
                text_pointer: u64,
                text_length: u64,
            ) -> u64;

            /// [`host_apply`] is the uniform, protocol-agnostic transport import: it
            /// applies the message held in arena slot `mem_id` at `(ptr, len)`. The
            /// slot's payload begins with the 14-byte `WasmEnvelope`
            /// (`[protocol][version][memory_id][length]`), so JS reads the protocol
            /// byte to dispatch and `dispose_allocation(mem_id)` to ACK. One slot, one
            /// ACK, for Arrow / Custom Binary / JSON alike (decision 028, G44).
            pub fn host_apply(mem_id: u64, ptr: u64, len: u64);

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

            // `dom_allocate_external_pointer` moved to
            // `foundation_wasm_ui::wasm::dom::element` (DOM-specific FFI, feature 00).

            /// [`host_object_drop_external_pointer`] retires an object-heap external
            /// reference (allocated via [`object_allocate_external_pointer`] or returned
            /// by an Object-hinted invocation), letting the host free the slot.
            pub fn host_object_drop_external_pointer(handle: u64);

            /// [`host_string_cache_drop_external_pointer`] evicts an interned string
            /// (a handle from [`host_cache_string`]) from the host string cache.
            pub fn host_string_cache_drop_external_pointer(handle: u64);

            /// [`host_cache_string`] provides a way to cache dynamic utf8 strings that
            /// will be interned into a map of a u64 key representing the string, this allows
            /// us to pay the cost of conversion once for these types of strings
            /// whilst limiting the overall cost since only a reference is ever passed around.
            pub fn host_cache_string(start: u64, len: u64, encoding: u8) -> u64;

            /// [`host_report`] delivers a `#[wasm_test]` case outcome to the host
            /// runner (feature 13's result protocol): `status` ∈ {0 = pass, 1 = fail,
            /// 2 = ignored}; `(ptr, len)` an optional UTF-8 message (assertion /
            /// panic text) read directly from linear memory.
            pub fn host_report(status: u32, ptr: u64, len: u64);

            // [`host_unregister_function`] provides a means to unregister a target function
            // from the WASM - Host runtime boundary.
            pub fn host_unregister_function(handle: u64);

            // registers a function via it's provided start and length
            // indicative of where the function body can be found
            // as utf-8 or utf-18 encoded byte (based on third argument)
            // from the start pointer in memory to the specified
            // length to be registered in the shared
            // function registry.
            pub fn host_register_function(start: u64, len: u64, encoding: u8) -> u64;

            /// [`host_invoke_function_as_object`] invokes a Host function across the WASM/RUST
            /// ABI expecting the result to be a host OBJECT: the host interns the returned
            /// value in its object heap and returns the heap handle (an external reference id)
            /// NAKED — no reply encoding (megatron `as_object` parity).
            pub fn host_invoke_function_as_object(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> u64;

            /// [`host_invoke_function_as_i64`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a [`i64`] as result. We do this to optimize any need to
            /// allocate memory explicitly for the result though wasm will do
            /// this for us since the type is basically supported.
            pub fn host_invoke_function_as_i64(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> i64;

            /// [`host_invoke_function_as_i32`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a [`i32`] as result. We do this to optimize any need to
            /// allocate memory explicitly for the result though wasm will do
            /// this for us since the type is basically supported.
            pub fn host_invoke_function_as_i32(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> i32;

            /// [`host_invoke_function_as_i16`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a [`i16`] as result. We do this to optimize any need to
            /// allocate memory explicitly for the result though wasm will do
            /// this for us since the type is basically supported.
            pub fn host_invoke_function_as_i16(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> i16;

            /// [`host_invoke_function_as_i8`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a [`i8`] as result. We do this to optimize any need to
            /// allocate memory explicitly for the result though wasm will do
            /// this for us since the type is basically supported.
            pub fn host_invoke_function_as_i8(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> i8;

            /// [`host_invoke_function_as_u64`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a [`u64`] as result. We do this to optimize any need to
            /// allocate memory explicitly for the result though wasm will do
            /// this for us since the type is basically supported.
            pub fn host_invoke_function_as_u64(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> u64;

            /// [`host_invoke_function_as_u32`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a [`u32`] as result. We do this to optimize any need to
            /// allocate memory explicitly for the result though wasm will do
            /// this for us since the type is basically supported.
            pub fn host_invoke_function_as_u32(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> u32;

            /// [`host_invoke_function_as_u16`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a [`u16`] as result. We do this to optimize any need to
            /// allocate memory explicitly for the result though wasm will do
            /// this for us since the type is basically supported.
            pub fn host_invoke_function_as_u16(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> u16;

            /// [`host_invoke_function_as_u8`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a [`u8`] as result. We do this to optimize any need to
            /// allocate memory explicitly for the result though wasm will do
            /// this for us since the type is basically supported.
            pub fn host_invoke_function_as_u8(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> u8;

            /// [`host_invoke_function_as_bool`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a [`u8`] as result. We do this to optimize any need to
            /// allocate memory explicitly for the result though wasm will do
            /// this for us since the type is basically supported.
            pub fn host_invoke_function_as_bool(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> u8;

            /// [`host_invoke_function_as_f64`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a f32 as result. We do this to optimize any need to allocate memory
            /// explicitly for the result though wasm will do this for us since the type is basically
            /// supported.
            pub fn host_invoke_function_as_f64(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> f64;

            /// [`host_invoke_function_as_f32`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the arguments to be read from specified memory location
            /// (pointer) and length of the content.
            ///
            /// It always expects to return a f32 as result. We do this to optimize any need to allocate memory
            /// explicitly for the result though wasm will do this for us since the type is basically
            /// supported.
            pub fn host_invoke_function_as_f32(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
            ) -> f32;

            /// [`host_invoke_callback_function`] invokes a Host function across the WASM/RUST ABI
            /// which must respond with result via a callback registered on the WASM side and
            /// referenced by a internal [`u64`] number.
            ///
            /// The host will issue a request to pass relevant response to the callback via
            /// the [`super::exposed_runtime::invoke_callback`] with the memory location
            /// containing the result.
            pub fn host_invoke_async_function(
                handler: u64,
                callback_handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
                returns_start: *const u8,
                returns_length: u64,
            );

            /// [`host_invoke_function`] invokes a Host function across the WASM/RUST ABI
            /// allowing you to specify the memory location for both outgoing
            // parameters and also return type expectation which
            // then returns the allocation_id (as f64) that can
            // be used to get the related allocation vector
            // from the global allocations.
            pub fn host_invoke_function(
                handler: u64,
                parameters_start: *const u8,
                parameters_length: u64,
                returns_start: *const u8,
                returns_length: u64,
            ) -> u64;

            // ── F28: Stream FFI (WASM → JS) ────────────────────────────
            ///
            /// WASM creates streams, pushes chunks, and ends them.
            /// JS binds callbacks via `FoundationWasm` methods (not via
            /// WASM FFI — callback binding is a JS-side concern).
            ///
            /// Buffered chunks live on the JS heap. Always bind or end
            /// the stream within a bounded time window.

            /// Create a host-side stream. Returns a u64 stream ID.
            pub fn host_stream_create() -> u64;

            /// Push a chunk to a host-side stream by ID.
            pub fn host_sender_send(stream_id: u64, data_start: *const u8, data_len: u32, seq: u64);

            /// Signal end-of-stream on a host-side stream by ID.
            pub fn host_sender_end(stream_id: u64);
        }

        #[cfg(not(target_family = "wasm"))]
        mod stubs {
            pub fn hook_up_animation_frames() {}
            pub fn schedule_timeout(_timing: f64, _callback: u64) {}
            pub fn unschedule_timeout(_callback: u64) {}
            pub fn schedule_interval(_timing: f64, _callback: u64) {}
            pub fn unschedule_interval(_callback: u64) {}
            pub fn host_batch_apply(
                _operation_pointer: u64,
                _operation_length: u64,
                _text_pointer: u64,
                _text_length: u64,
            ) {
            }
            pub fn host_batch_returning_apply(
                _operation_pointer: u64,
                _operation_length: u64,
                _text_pointer: u64,
                _text_length: u64,
            ) -> u64 {
                0
            }
            pub fn host_apply(_mem_id: u64, _ptr: u64, _len: u64) {}
            pub fn function_allocate_external_pointer() -> u64 {
                0
            }
            pub fn object_allocate_external_pointer() -> u64 {
                0
            }
            // `dom_allocate_external_pointer` stub moved with its FFI to
            // `foundation_wasm_ui::wasm::dom::element` (feature 00).
            pub fn host_object_drop_external_pointer(_handle: u64) {}
            pub fn host_string_cache_drop_external_pointer(_handle: u64) {}
            pub fn host_invoke_function_as_object(
                _handler: u64,
                _parameters_start: *const u8,
                _parameters_length: u64,
            ) -> u64 {
                0
            }
            pub fn host_cache_string(_start: u64, _len: u64, _encoding: u8) -> u64 {
                0
            }
            pub fn host_report(_status: u32, _ptr: u64, _len: u64) {}
            pub fn host_unregister_function(_handle: u64) {}
            pub fn host_register_function(_start: u64, _len: u64, _encoding: u8) -> u64 {
                0
            }
            pub fn host_invoke_function_as_i64(
                _handler: u64,
                _parameters_start: *const u8,
                _parameters_length: u64,
            ) -> i64 {
                0
            }
            pub fn host_invoke_function_as_i32(
                _handler: u64,
                _parameters_start: *const u8,
                _parameters_length: u64,
            ) -> i32 {
                0
            }
            pub fn host_invoke_function_as_i16(
                _handler: u64,
                _parameters_start: *const u8,
                _parameters_length: u64,
            ) -> i16 {
                0
            }
            pub fn host_invoke_function_as_i8(
                _handler: u64,
                _parameters_start: *const u8,
                _parameters_length: u64,
            ) -> i8 {
                0
            }
            pub fn host_invoke_function_as_u64(
                _handler: u64,
                _parameters_start: *const u8,
                _parameters_length: u64,
            ) -> u64 {
                0
            }
            pub fn host_invoke_function_as_u32(
                _handler: u64,
                _parameters_start: *const u8,
                _parameters_length: u64,
            ) -> u32 {
                0
            }
            pub fn host_invoke_function_as_u16(
                _handler: u64,
                _parameters_start: *const u8,
                _parameters_length: u64,
            ) -> u16 {
                0
            }
            pub fn host_invoke_function_as_u8(
                _handler: u64,
                _parameters_start: *const u8,
                _parameters_length: u64,
            ) -> u8 {
                0
            }
            pub fn host_invoke_function_as_bool(
                _handler: u64,
                _parameters_start: *const u8,
                _parameters_length: u64,
            ) -> u8 {
                0
            }
            pub fn host_invoke_function_as_f64(
                _handler: u64,
                _parameters_start: *const u8,
                _parameters_length: u64,
            ) -> f64 {
                0.0
            }
            pub fn host_invoke_function_as_f32(
                _handler: u64,
                _parameters_start: *const u8,
                _parameters_length: u64,
            ) -> f32 {
                0.0
            }
            pub fn host_invoke_async_function(
                _handler: u64,
                _callback_handler: u64,
                _parameters_start: *const u8,
                _parameters_length: u64,
                _returns_start: *const u8,
                _returns_length: u64,
            ) {
            }
            pub fn host_invoke_function(
                _handler: u64,
                _parameters_start: *const u8,
                _parameters_length: u64,
                _returns_start: *const u8,
                _returns_length: u64,
            ) -> u64 {
                0
            }

            // ── F28: Stream FFI ────────────────────────────────────────

            pub fn host_stream_create() -> u64 {
                0
            }
            pub fn host_sender_send(
                _stream_id: u64,
                _data_start: *const u8,
                _data_len: u32,
                _seq: u64,
            ) {
            }
            pub fn host_sender_end(_stream_id: u64) {}
        }

        // Re-export stubs with same names as extern functions for non-wasm targets
        #[cfg(not(target_family = "wasm"))]
        pub use stubs::*;

        // `allocate_dom_reference` moved to `foundation_wasm_ui::wasm::dom::element`
        // (DOM concept — keeps the ABI crate DOM-free, feature 00).

        /// [`allocate_function_reference`] requests the host runtime to pre-allocate
        /// a target external reference for usage by the caller for a function.
        pub fn allocate_function_reference() -> ExternalPointer {
            unsafe { ExternalPointer::pointer(abi::web::function_allocate_external_pointer()) }
        }

        /// [`allocate_object_reference`] requests the host runtime to pre-allocate
        /// a target external reference for usage by the caller for an object.
        pub fn allocate_object_reference() -> ExternalPointer {
            unsafe { ExternalPointer::pointer(abi::web::object_allocate_external_pointer()) }
        }

        /// [`batch`] sends a [`CompletedInstructions`] batch over to the host runtime
        /// to be applied and expects no responses/returned values to be provided.
        pub fn batch(instruction: CompletedInstructions) {
            let operations_memory = internal_api::get_memory(instruction.ops_id);
            let text_memory = internal_api::get_memory(instruction.text_id);

            let (ops_pointer, ops_length) =
                operations_memory.as_address().expect("get ops address");
            let (text_pointer, text_length) = text_memory.as_address().expect("get text address");

            unsafe {
                abi::web::host_batch_apply(
                    ops_pointer as u64,
                    ops_length,
                    text_pointer as u64,
                    text_length,
                );
            };
        }

        /// [`batch_response`] sends a [`CompletedInstructions`] batch over to the host runtime
        /// and expects the returned values of these execution to be returned to it.
        ///
        /// Note: Instructions that call callbacks will generally return None or
        /// [`ReturnTypeHints::None`] as their returned values.
        ///
        /// We ensure to keep the order of instructions to returned values through
        /// group returns.
        pub fn batch_response(
            instruction: CompletedInstructions,
        ) -> BinaryReaderResult<Vec<Returns>> {
            let operations_memory = internal_api::get_memory(instruction.ops_id);
            let text_memory = internal_api::get_memory(instruction.text_id);

            let (ops_pointer, ops_length) =
                operations_memory.as_address().expect("get ops address");
            let (text_pointer, text_length) = text_memory.as_address().expect("get text address");

            let return_id = unsafe {
                abi::web::host_batch_returning_apply(
                    ops_pointer as u64,
                    ops_length,
                    text_pointer as u64,
                    text_length,
                )
            };

            let mem_id = MemoryId::from_u64(return_id);
            let memory_result = ALLOCATIONS
                .lock()
                .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                .get(mem_id);
            assert!(
                memory_result.is_ok(),
                "batch_response: failed to get memory location: {mem_id:?} from {return_id:?} with {memory_result:?}"
            );

            if let Err(err) = memory_result {
                return Err(err.into());
            }
            let memory = memory_result.unwrap();

            let result = match memory.into_with(|item| {
                let group: GroupReturnTypeHints = Default::default();
                group.from_binary(item.as_ref())
            }) {
                Some(value) => value,
                None => Err(BinaryReadError::MemoryError(String::from(
                    "unable to read return values",
                ))),
            };

            if let Err(err) = ALLOCATIONS
                .lock()
                .unwrap_or_else(foundation_nostd::comp::basic::PoisonError::into_inner)
                .deallocate(mem_id)
            {
                return Err(err.into());
            }

            result
        }

        // animation function registration with the host.

        /// [`register_animation_hook`] provides a method that will automatically
        /// convert any type that implements the [`Fn`] trait.
        #[cfg(not(target_family = "wasm"))]
        pub fn register_animation_hook<F>(f: F)
        where
            F: Fn(f64) -> TickState + Send + Sync + 'static,
        {
            internal_api::register_animation_hook(f);

            // notify the host we are interested in animation frames
            // generally host should ignore if we are already registered.
            unsafe {
                abi::web::hook_up_animation_frames();
            };
        }

        /// [`register_animation_hook`] provides a method that will automatically
        /// convert any type that implements the [`Fn`] trait.
        #[cfg(target_family = "wasm")]
        pub fn register_animation_hook<F>(f: F)
        where
            F: Fn(f64) -> TickState + 'static,
        {
            internal_api::register_animation_hook(f);

            // notify the host we are interested in animation frames
            // generally host should ignore if we are already registered.
            unsafe {
                abi::web::hook_up_animation_frames();
            };
        }

        /// [`register_animation_hook_callback`] provides a more direct method for
        /// registering a type that implements the [`FrameCallback`] trait.
        #[cfg(target_family = "wasm")]
        pub fn register_animation_hook_callback<F>(f: F)
        where
            F: FrameCallback + Send + Sync + 'static,
        {
            internal_api::register_animation_interval_callback(f);

            // notify the host we are interested in animation frames
            // generally host should ignore if we are already registered.
            unsafe {
                abi::web::hook_up_animation_frames();
            };
        }

        /// [`register_animation_hook_callback`] provides a more direct method for
        /// registering a type that implements the [`FrameCallback`] trait.
        #[cfg(not(target_family = "wasm"))]
        pub fn register_animation_hook_callback<F>(f: F)
        where
            F: FrameCallback + Send + Sync + 'static,
        {
            internal_api::register_animation_interval_callback(f);

            // notify the host we are interested in animation frames
            // generally host should ignore if we are already registered.
            unsafe {
                abi::web::hook_up_animation_frames();
            };
        }

        // schedule function registration with the host.

        pub fn unregister_schedule<F>(id: InternalPointer) {
            unsafe {
                abi::web::unschedule_timeout(id.into_inner());
            };
            internal_api::unregister_schedule_callback(id);
        }

        /// [`register_callback`] provides a method that will automatically
        /// convert any type that implements the [`Fn`] trait.
        #[cfg(not(target_family = "wasm"))]
        pub fn register_schedule<F>(timing: f64, f: F) -> InternalPointer
        where
            F: Fn() + Send + Sync + 'static,
        {
            let id = internal_api::register_schedule(f);
            unsafe {
                abi::web::schedule_timeout(timing, id.into_inner());
            };
            id
        }

        /// [`register_callback`] provides a method that will automatically
        /// convert any type that implements the [`Fn`] trait.
        #[cfg(target_family = "wasm")]
        pub fn register_schedule<F>(timing: f64, f: F) -> InternalPointer
        where
            F: Fn() + 'static,
        {
            let id = internal_api::register_schedule(f);
            unsafe {
                abi::web::schedule_timeout(timing, id.into_inner());
            };
            id
        }

        /// [`register_schedule_callback`] provides a more direct method for
        /// registering a type that implements the [`InternalCallback`] trait.
        #[cfg(target_family = "wasm")]
        pub fn register_schedule_callback<F>(timing: f64, f: F) -> InternalPointer
        where
            F: DoTask + 'static,
        {
            let id = internal_api::register_schedule_callback(f);
            unsafe {
                abi::web::schedule_timeout(timing, id.into_inner());
            };
            id
        }

        /// [`register_schedule_callback`] provides a more direct method for
        /// registering a type that implements the [`InternalCallback`] trait.
        #[cfg(not(target_family = "wasm"))]
        pub fn register_schedule_callback<F>(timing: f64, f: F) -> InternalPointer
        where
            F: DoTask + Send + Sync + 'static,
        {
            let id = internal_api::register_schedule_callback(f);
            unsafe {
                abi::web::schedule_timeout(timing, id.into_inner());
            };
            id
        }

        // interval function registration with the host.

        pub fn unregister_interval<F>(id: InternalPointer) {
            unsafe {
                abi::web::unschedule_interval(id.into_inner());
            };
            internal_api::unregister_interval_callback(id);
        }

        /// [`register_callback`] provides a method that will automatically
        /// convert any type that implements the [`Fn`] trait.
        #[cfg(not(target_family = "wasm"))]
        pub fn register_interval<F>(timing: f64, f: F) -> InternalPointer
        where
            F: Fn() -> TickState + Send + Sync + 'static,
        {
            let id = internal_api::register_interval(f);
            unsafe {
                abi::web::schedule_interval(timing, id.into_inner());
            };
            id
        }

        /// [`register_callback`] provides a method that will automatically
        /// convert any type that implements the [`Fn`] trait.
        #[cfg(target_family = "wasm")]
        pub fn register_interval<F>(timing: f64, f: F) -> InternalPointer
        where
            F: Fn() -> TickState + 'static,
        {
            let id = internal_api::register_interval(f);
            unsafe {
                abi::web::schedule_interval(timing, id.into_inner());
            };
            id
        }

        /// [`register_interval_callback`] provides a more direct method for
        /// registering a type that implements the [`InternalCallback`] trait.
        #[cfg(target_family = "wasm")]
        pub fn register_interval_callback<F>(timing: f64, f: F) -> InternalPointer
        where
            F: IntervalCallback + Send + Sync + 'static,
        {
            let id = internal_api::register_interval_callback(f);
            unsafe {
                abi::web::schedule_interval(timing, id.into_inner());
            };
            id
        }

        /// [`register_interval_callback`] provides a more direct method for
        /// registering a type that implements the [`InternalCallback`] trait.
        #[cfg(not(target_family = "wasm"))]
        pub fn register_interval_callback<F>(timing: f64, f: F) -> InternalPointer
        where
            F: IntervalCallback + Send + Sync + 'static,
        {
            let id = internal_api::register_interval_callback(f);
            unsafe {
                abi::web::schedule_interval(timing, id.into_inner());
            };
            id
        }

        // Other methods

        /// [`cache_text`] provides a way to have the host runtime cache an expense
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
                CachedText::pointer(abi::web::host_cache_string(
                    start as u64,
                    len as u64,
                    JSEncoding::UTF8.into(),
                ))
            }
        }

        /// [`register_function`] calls the underlying [`js_abi`] registration
        /// function to register a host code that can be called from memory
        /// allowing you define the underlying code we want executed.
        pub fn register_function(code: &str) -> HostFunction {
            let start = code.as_ptr() as usize;
            let len = code.len();
            unsafe {
                HostFunction {
                    handler: abi::web::host_register_function(
                        start as u64,
                        len as u64,
                        JSEncoding::UTF8.into(),
                    ),
                }
            }
        }

        /// [`register_function_utf16`] calls the underlying [`js_abi`] registration
        /// function to register a host code already encoded
        /// as UTF16 by the borrowed slice of u16 that can be called from memory
        /// allowing you define the underlying code we want executed.
        pub fn register_function_utf16(code: &[u16]) -> HostFunction {
            let start = code.as_ptr() as usize;
            let len = code.len();
            unsafe {
                HostFunction {
                    handler: abi::web::host_register_function(
                        start as u64,
                        len as u64,
                        JSEncoding::UTF16.into(),
                    ), // precision loss here
                }
            }
        }

        /// [`invoke_as_f64`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`f64`].
        pub fn invoke_as_f64(handler: u64, params: &[Params]) -> f64 {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                abi::web::host_invoke_function_as_f64(handler, param_raw.ptr, param_raw.length)
            }
        }

        /// [`invoke_as_f32`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`f32`].
        pub fn invoke_as_f32(handler: u64, params: &[Params]) -> f32 {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                abi::web::host_invoke_function_as_f32(handler, param_raw.ptr, param_raw.length)
            }
        }

        /// [`invoke_as_i64`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`i64`].
        pub fn invoke_as_i64(handler: u64, params: &[Params]) -> i64 {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                abi::web::host_invoke_function_as_i64(handler, param_raw.ptr, param_raw.length)
            }
        }

        /// [`invoke_as_i32`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`i32`].
        pub fn invoke_as_i32(handler: u64, params: &[Params]) -> i32 {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                abi::web::host_invoke_function_as_i32(handler, param_raw.ptr, param_raw.length)
            }
        }

        /// [`invoke_as_i16`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`i16`].
        pub fn invoke_as_i16(handler: u64, params: &[Params]) -> i16 {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                abi::web::host_invoke_function_as_i16(handler, param_raw.ptr, param_raw.length)
            }
        }

        /// [`invoke_as_i8`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`i8`].
        pub fn invoke_as_i8(handler: u64, params: &[Params]) -> i8 {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                abi::web::host_invoke_function_as_i8(handler, param_raw.ptr, param_raw.length)
            }
        }

        /// [`invoke_as_u64`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`u64`].
        pub fn invoke_as_u64(handler: u64, params: &[Params]) -> u64 {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                abi::web::host_invoke_function_as_u64(handler, param_raw.ptr, param_raw.length)
            }
        }

        /// [`invoke_as_object`] invokes a host function registered at the given handle
        /// expecting the host to intern the returned value in its OBJECT heap and hand
        /// back the heap handle naked (no reply encoding) — wrapped as an
        /// [`ExternalPointer`] into that heap.
        pub fn invoke_as_object(handler: u64, params: &[Params]) -> ExternalPointer {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            ExternalPointer::pointer(unsafe {
                abi::web::host_invoke_function_as_object(handler, param_raw.ptr, param_raw.length)
            })
        }

        /// [`drop_object_reference`] retires an object-heap external reference so the
        /// host can free the slot (stale handles fail the generation check host-side).
        pub fn drop_object_reference(handle: ExternalPointer) {
            unsafe { abi::web::host_object_drop_external_pointer(handle.into_inner()) };
        }

        /// [`drop_cached_string`] evicts an interned string (a [`host_cache_string`]
        /// handle) from the host string cache.
        pub fn drop_cached_string(handle: u64) {
            unsafe { abi::web::host_string_cache_drop_external_pointer(handle) };
        }

        /// [`invoke_as_u32`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`u32`].
        pub fn invoke_as_u32(handler: u64, params: &[Params]) -> u32 {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                abi::web::host_invoke_function_as_u32(handler, param_raw.ptr, param_raw.length)
            }
        }

        /// [`invoke_as_u16`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`u16`].
        pub fn invoke_as_u16(handler: u64, params: &[Params]) -> u16 {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                abi::web::host_invoke_function_as_u16(handler, param_raw.ptr, param_raw.length)
            }
        }

        /// [`invoke_as_u8`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`u8`].
        pub fn invoke_as_u8(handler: u64, params: &[Params]) -> u8 {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                abi::web::host_invoke_function_as_u8(handler, param_raw.ptr, param_raw.length)
            }
        }

        /// [`invoke_as_bool`] invokes a host function registered at the given handle
        /// defined by the [`HostFunction::handler`] which then returns a [`u8`]
        /// which represents the state to be returned (1- true, 0 - false).
        pub fn invoke_as_bool(handler: u64, params: &[Params]) -> bool {
            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                abi::web::host_invoke_function_as_bool(handler, param_raw.ptr, param_raw.length)
                    == 1
            }
        }

        /// [`invoke_as_callback`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side which will when executed
        /// must have it's returned result communicated via a callback handle provided.
        ///
        /// This provides support for cases where Promises or async function or function
        /// whoes means of result communication is callback only.
        ///
        /// A [`u64`]
        ///
        /// When called we expect the return of a [`u64`] which actually points to a
        /// [`MemoryId`] registered in [`ALLOCATIONS`] which can be retrieved to get
        /// the actual result and is expected to be a binary of [`ReturnValues`]
        /// which match the return hints [`ReturnTypeHints`].
        pub fn invoke_as_async(
            handler: u64,
            callback: InternalPointer,
            params: &[Params],
            returns: ReturnTypeHints,
        ) {
            let return_hints_bytes = returns.to_binary();
            let return_raw = RawParts::from_vec(return_hints_bytes);

            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                abi::web::host_invoke_async_function(
                    handler,
                    callback.into_inner(),
                    param_raw.ptr,
                    param_raw.length,
                    return_raw.ptr,
                    return_raw.length,
                );
            };
        }

        /// [`invoke_for_str`] invokes a host function registered at the given handle
        /// defined by the [`HostFunction::handler`] which then returns a [`u64`]
        /// which represents the allocation id of the contents.
        pub fn invoke_for_str(handler: u64, params: &[Params]) -> WasmRequestResult<String> {
            match abi::web::invoke_for_replies(
                handler,
                params,
                ReturnTypeHints::One(ThreeState::One(ReturnTypeId::Text8)),
            ) {
                Ok(mut values) => match values.pop().unwrap() {
                    ReturnValues::Text8(content) => Ok(content),
                    _ => Err(ReturnValueError::UnexpectedReturnType.into()),
                },
                Err(err) => Err(err),
            }
        }

        /// [`invoke`] invokes a host function registered at the given handle
        /// which points to a registered function on the host side.
        ///
        /// When called we expect the return of a [`u64`] which actually points to a
        /// [`MemoryId`] registered in [`ALLOCATIONS`] which can be retrieved to get
        /// the actual result and is expected to be a binary of [`ReturnValues`]
        /// which match the return hints [`ReturnTypeHints`].
        pub fn invoke(handler: u64, params: &[Params], returns: ReturnTypeHints) -> u64 {
            let return_hints_bytes = returns.to_binary();
            let return_raw = RawParts::from_vec(return_hints_bytes);

            let param_bytes = params.to_binary();
            let param_raw = RawParts::from_vec(param_bytes);

            unsafe {
                abi::web::host_invoke_function(
                    handler,
                    param_raw.ptr,
                    param_raw.length,
                    return_raw.ptr,
                    return_raw.length,
                )
            }
        }

        /// [`invoke_for_replies`] invokes a host function registered at the given handle
        /// defined by the [`HostFunction::handler`] which then returns a [`u64`]
        /// which represents the allocation id of the contents which are to be encoded in
        /// the new Reply binary format implemented via [`FromBinary`] for [`ReturnTypeHints`].
        pub fn invoke_for_replies(
            handler: u64,
            params: &[Params],
            returns: ReturnTypeHints,
        ) -> WasmRequestResult<Vec<ReturnValues>> {
            let value = abi::web::invoke(handler, params, returns.clone());
            let memory_id = MemoryId::from_u64(value);

            let memory = internal_api::get_memory(memory_id);
            let result_container =
                memory.into_with(|mem| returns.clone().from_binary(mem.as_ref()));

            if result_container.is_none() {
                return Err(MemoryAllocationError::FailedAllocationReading(memory_id).into());
            }

            let result = result_container.unwrap();
            if let Err(err) = result {
                return Err(MemoryReaderError::NotValidReplyBinary(err).into());
            }

            let replies = result.unwrap();

            match &returns {
                ReturnTypeHints::One(_) => {
                    if replies.len() != 1 {
                        return Err(MemoryReaderError::ReturnValueError(
                            crate::ReturnValueError::ExpectedOne(replies),
                        )
                        .into());
                    }

                    Ok(replies)
                }
                ReturnTypeHints::List(_) => Ok(replies),
                ReturnTypeHints::Multi(_) => Ok(replies),
                ReturnTypeHints::None => unreachable!(
                    "a reply is always expected you cant use this in situation where None is given"
                ),
            }
        }

        // --- Browser / WASM ABI

        #[derive(Copy, Clone)]
        pub struct HostFunction {
            pub handler: u64,
        }

        #[allow(clippy::cast_precision_loss)]
        impl HostFunction {
            /// [`invoke`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then receives the set of parameters
            /// supplied to be invoked with.
            ///
            /// The `js_abi` will handle necessary conversion and execution of the function
            /// with the passed arguments.
            pub fn invoke(&self, params: &[Params], returns: ReturnTypeHints) -> MemoryId {
                MemoryId::from_u64(abi::web::invoke(self.handler, params, returns))
            }

            /// [`invoke_for_memory`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a [`u64`]
            /// which represents the allocation id of the contents.
            pub fn invoke_no_return(&self, params: &[Params]) {
                _ = abi::web::invoke(self.handler, params, ReturnTypeHints::None);
            }

            /// [`invoke_for_none`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns confirmation
            /// that it's return value is of type [`ReturnTypeId::None`] by applying
            /// return type checks.
            pub fn invoke_for_none(&self, params: &[Params]) -> bool {
                abi::web::invoke_for_replies(
                    self.handler,
                    params,
                    ReturnTypeHints::One(ThreeState::One(ReturnTypeId::None)),
                )
                .is_ok()
            }

            pub fn invoke_for_replies(
                &self,
                params: &[Params],
                expected: ReturnTypeHints,
            ) -> WasmRequestResult<Vec<ReturnValues>> {
                abi::web::invoke_for_replies(self.handler, params, expected)
            }

            /// [`invoke_for_bool`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a bool indicating the
            /// result.
            ///
            /// Internal true is when the returned number is >= 1 and False if 0.
            pub fn invoke_for_bool(&self, params: &[Params]) -> bool {
                abi::web::invoke_as_bool(self.handler, params)
            }

            /// [`invoke_for_i8`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a u8.
            pub fn invoke_for_i8(&self, params: &[Params]) -> i8 {
                unsafe { abi::web::invoke_as_i8(self.handler, params) }
            }

            /// [`invoke_for_i16`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a u16.
            pub fn invoke_for_i16(&self, params: &[Params]) -> i16 {
                abi::web::invoke_as_i16(self.handler, params)
            }

            /// [`invoke_for_i32`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a u32.
            pub fn invoke_for_i32(&self, params: &[Params]) -> i32 {
                abi::web::invoke_as_i32(self.handler, params)
            }

            /// [`invoke_for_i64`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a [`ExternalPointer`]
            /// representing the DOM node instance via an `ExternalPointer` that points to that object in the
            /// hosts object heap.
            pub fn invoke_for_i64(&self, params: &[Params]) -> i64 {
                abi::web::invoke_as_i64(self.handler, params)
            }

            /// [`invoke_for_u8`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a u8.
            pub fn invoke_for_u8(&self, params: &[Params]) -> u8 {
                unsafe { abi::web::invoke_as_u8(self.handler, params) }
            }

            /// [`invoke_for_u16`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a u16.
            pub fn invoke_for_u16(&self, params: &[Params]) -> u16 {
                abi::web::invoke_as_u16(self.handler, params)
            }

            /// [`invoke_for_u32`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a u32.
            pub fn invoke_for_u32(&self, params: &[Params]) -> u32 {
                abi::web::invoke_as_u32(self.handler, params)
            }

            /// [`invoke_for_u64`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a [`ExternalPointer`]
            /// representing the DOM node instance via an `ExternalPointer` that points to that object in the
            /// hosts object heap.
            pub fn invoke_for_u64(&self, params: &[Params]) -> u64 {
                abi::web::invoke_as_u64(self.handler, params)
            }

            /// [`invoke_for_float64`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a f64.
            pub fn invoke_for_f64(&self, params: &[Params]) -> f64 {
                abi::web::invoke_as_f64(self.handler, params)
            }

            /// [`invoke_for_float32`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a f32.
            pub fn invoke_for_f32(&self, params: &[Params]) -> f32 {
                abi::web::invoke_as_f32(self.handler, params)
            }

            /// [`invoke_for_str`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a [`u64`]
            /// which represents the allocation id of the contents.
            pub fn invoke_for_str(&self, params: &[Params]) -> WasmRequestResult<String> {
                abi::web::invoke_for_str(self.handler, params)
            }

            // `invoke_for_dom` moved to `foundation_wasm_ui::wasm::dom::element` as the
            // `DomInvoke` extension trait on `HostFunction` (DOM concept, feature 00).

            /// [`invoke_for_object`] invokes a host function registered at the given handle
            /// defined by the [`HostFunction::handler`] which then returns a [`ExternalPointer`]
            /// representing the object via an `ExternalPointer` that points to that object in the
            /// hosts object heap (the naked `as_object` fast-path — the handle crosses raw,
            /// no reply encoding).
            pub fn invoke_for_object(&self, params: &[Params]) -> ExternalPointer {
                abi::web::invoke_as_object(self.handler, params)
            }

            /// [`invoke_async`] invokes an async function which is registered and will be
            /// invoked and expected to return a Promise that is then used to collect the result or
            /// error.
            pub fn invoke_async(
                &self,
                callback_id: InternalPointer,
                params: &[Params],
                returns: ReturnTypeHints,
            ) {
                abi::web::invoke_as_async(self.handler, callback_id, params, returns);
            }

            /// [`unregister_function`] calls the JS ABI on the host to de-register
            /// the target function.
            pub fn unregister(self) {
                unsafe { abi::web::host_unregister_function(self.handler) }
            }
        }
    }
}

// ── F41: IPC ABI module ───────────────────────────────────────────────────
///
/// Every host (Tauri, Deno, browser, WASI) implements these imports.
/// Same memory-arena pattern as `abi::web::batch()` — allocate, pass
/// (ptr, len) to host, host reads/writes via `exposed_runtime`.

pub mod ipc {
    use super::{MemoryAllocation, ALLOCATIONS};

    /// WASM imports — host implements these.
    #[cfg(target_family = "wasm")]
    #[link(wasm_import_module = "abi")]
    extern "C" {
        /// Dispatch an IPC request. `callback_id` identifies the callback in
        /// the WASM-side registry. The host MUST call `ipc_resolve(callback_id,
        /// alloc_id)` with the response — synchronously or asynchronously.
        pub fn host_ipc_invoke(request_ptr: *const u8, request_len: u32, callback_id: u64);

        /// Open a host→WASM stream. Returns stream ID (0 = error).
        pub fn host_ipc_stream_open(request_ptr: *const u8, request_len: u32) -> u64;

        /// Read next chunk from host-created stream. Returns allocation ID (0 = closed/error).
        pub fn host_ipc_stream_read(stream_id: u64) -> u64;

        /// Close a host→WASM stream.
        pub fn host_ipc_stream_close(stream_id: u64);
    }

    /// Stubs for non-wasm targets — never called, only for compilation.
    #[cfg(not(target_family = "wasm"))]
    mod stubs {
        pub fn host_ipc_invoke(_ptr: *const u8, _len: u32, _cb: u64) {}
        pub fn host_ipc_stream_open(_ptr: *const u8, _len: u32) -> u64 {
            0
        }
        pub fn host_ipc_stream_read(_id: u64) -> u64 {
            0
        }
        pub fn host_ipc_stream_close(_id: u64) {}
    }

    #[cfg(not(target_family = "wasm"))]
    pub use stubs::*;

}
