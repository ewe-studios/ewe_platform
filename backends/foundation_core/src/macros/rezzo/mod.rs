//! Module implementing custom helpers for working with results.

/// [`on_result`] lets you inline the underline actions you want a result to
/// be handled both for it's ok state and error state.
///
/// # Example
///
/// ```rust
/// use foundation_core::*;
///
///
/// let good_result: Result<i32, &str> = Ok(10);
/// on_result!(good_result, val => println!("Success: {}", val), err => println!("Error: {}", err));
///
/// let bad_result: Result<i32, &str> = Err("Error occurred");
/// on_result!(bad_result, val => println!("Success: {}", val), err => println!("Error: {}", err));
///
/// ```
#[macro_export]
macro_rules! on_result {
    ($expr:expr, $ok_var:pat => $ok_body:expr, $err_var:pat => $err_body:expr) => {
        match $expr {
            Ok($ok_var) => $ok_body,
            Err($err_var) => $err_body,
        }
    };
}

#[cfg(test)]
mod test_on_result {
    #[test]
    fn test() {
        assert_eq!(
            on_result!(Ok::<usize, Box<dyn std::error::Error + 'static>>(10), val => val, _err => 20),
            10
        );
        assert_eq!(
            on_result!(Err::<usize, std::io::Error>(std::io::Error::from(std::io::ErrorKind::AlreadyExists)), val => val, _err => 20),
            20
        );
    }
}

/// [`unwrap_or_panic_log`] lets you unwrap the underlying OK value from a result, logging
/// and panicing on the error state.
///
/// # Example
///
/// ```rust
/// use foundation_core::*;
///
/// let result: Result<i32, String> = Ok(5);
/// let value = unwrap_or_panic_log!(result);
/// println!("Value: {}", value);
///
/// ```
#[macro_export]
macro_rules! unwrap_or_panic_log {
    ($expr:expr) => {
        match $expr {
            Ok(val) => val,
            Err(e) => {
                tracing::error!("Error unwrapping: {:?}", e);

                // You might want to panic!, return an early Err, or use a default value
                panic!("Unrecoverable error: {:?}", e);
            }
        }
    };
}

#[cfg(test)]
mod test_unwrap_or_panic_log {
    #[test]
    fn test() {
        assert_eq!(
            unwrap_or_panic_log!(Ok::<usize, Box<dyn std::error::Error + 'static>>(10)),
            10
        );
    }
}

/// [`is_ok`] lets you perform an equality on the Ok value returning
/// true or false if the values match or a definite false if its an Error
/// and not a Ok value.
///
/// # Example
///
/// ```rust
/// use foundation_core::*;
///
/// let result: Result<i32, String> = Ok(5);
/// assert!(is_ok!(result, 5));
///
///
/// ```
#[macro_export]
macro_rules! is_ok {
    ($expr:expr, $($ok_value:expr),+ $(,)?) => {
        match $expr {
            Ok(val) => {
                let values = [$($ok_value),*];

                let mut result = false;
                for v in values.iter() {
                    if val == *v {
                        result = true;
                        break;
                    }
                }
                result
            },
            Err(_) => false,
        }
    };
}

#[cfg(test)]
mod test_is_ok {
    #[test]
    fn test() {
        assert_eq!(
            is_ok!(Ok::<usize, Box<dyn std::error::Error + 'static>>(10), 10),
            true
        );
        assert_eq!(
            is_ok!(
                Ok::<usize, Box<dyn std::error::Error + 'static>>(10),
                20,
                10
            ),
            true
        );
        assert_eq!(
            is_ok!(Ok::<usize, Box<dyn std::error::Error + 'static>>(10), 20),
            false
        );
        assert_eq!(
            is_ok!(
                Err::<usize, std::io::Error>(std::io::Error::from(
                    std::io::ErrorKind::AlreadyExists
                )),
                10
            ),
            false
        );
    }
}
