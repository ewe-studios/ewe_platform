use foundation_core::{is_ok, on_result, unwrap_or_panic_log};

#[test]
fn test_on_result() {
    assert_eq!(
        on_result!(Ok::<usize, Box<dyn std::error::Error + 'static>>(10), val => val, _err => 20),
        10
    );
    assert_eq!(
        on_result!(Err::<usize, std::io::Error>(std::io::Error::from(std::io::ErrorKind::AlreadyExists)), val => val, _err => 20),
        20
    );
}

#[test]
fn test_unwrap_or_panic_log() {
    assert_eq!(
        unwrap_or_panic_log!(Ok::<usize, Box<dyn std::error::Error + 'static>>(10)),
        10
    );
}

#[test]
fn test_is_ok() {
    assert!(is_ok!(
        Ok::<usize, Box<dyn std::error::Error + 'static>>(10),
        10
    ));
    assert!(is_ok!(
        Ok::<usize, Box<dyn std::error::Error + 'static>>(10),
        20,
        10
    ));
    assert!(!is_ok!(
        Ok::<usize, Box<dyn std::error::Error + 'static>>(10),
        20
    ));
    assert!(!is_ok!(
        Err::<usize, std::io::Error>(std::io::Error::from(std::io::ErrorKind::AlreadyExists)),
        10
    ));
}
