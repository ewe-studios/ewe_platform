//! Expand eligible action methods in the consumer of this runtime.

#[cfg(ewe_action_browser)]
#[doc(hidden)]
#[macro_export]
macro_rules! __ewe_action_browser {
    (@enabled) => { true };
    (@items $($item:item)*) => { $($item)* };
}

#[cfg(not(ewe_action_browser))]
#[doc(hidden)]
#[macro_export]
macro_rules! __ewe_action_browser {
    (@enabled) => {
        false
    };
    (@items $($item:item)*) => {};
}

#[cfg(ewe_action_mobile)]
#[doc(hidden)]
#[macro_export]
macro_rules! __ewe_action_mobile {
    (@enabled) => { true };
    (@items $($item:item)*) => { $($item)* };
}

#[cfg(not(ewe_action_mobile))]
#[doc(hidden)]
#[macro_export]
macro_rules! __ewe_action_mobile {
    (@enabled) => {
        false
    };
    (@items $($item:item)*) => {};
}

#[cfg(ewe_action_server)]
#[doc(hidden)]
#[macro_export]
macro_rules! __ewe_action_server {
    (@enabled) => { true };
    (@items $($item:item)*) => { $($item)* };
}

#[cfg(not(ewe_action_server))]
#[doc(hidden)]
#[macro_export]
macro_rules! __ewe_action_server {
    (@enabled) => {
        false
    };
    (@items $($item:item)*) => {};
}

#[cfg(ewe_action_host)]
#[doc(hidden)]
#[macro_export]
macro_rules! __ewe_action_host {
    (@enabled) => { true };
    (@items $($item:item)*) => { $($item)* };
}

#[cfg(not(ewe_action_host))]
#[doc(hidden)]
#[macro_export]
macro_rules! __ewe_action_host {
    (@enabled) => {
        false
    };
    (@items $($item:item)*) => {};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __ewe_action_all {
    (@enabled) => { true };
    (@items $($item:item)*) => { $($item)* };
}
