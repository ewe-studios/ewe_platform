use foundation_core::{
    valtron::{NoAction, TaskIterator},
    wire::simple_http::client::{TlsHandshakeState, TlsHandshakeTask},
};

#[test]
fn test_tls_handshake_state_variants() {
    // Verify all state variants exist
    let init = TlsHandshakeState::Init;
    let handshaking = TlsHandshakeState::Handshaking;
    let complete = TlsHandshakeState::Complete;
    let error = TlsHandshakeState::Error;

    // Use the variants to avoid unused variable warnings
    assert_ne!(init, complete);
    assert_ne!(handshaking, error);
}

#[test]
fn test_tls_handshake_state_debug() {
    let state = TlsHandshakeState::Init;
    let debug_str = format!("{state:?}");
    assert!(debug_str.contains("Init"));
}

#[test]
fn test_tls_handshake_task_is_task_iterator() {
    // Compile-time check that TlsHandshakeTask implements TaskIterator
    fn assert_task_iterator<T: TaskIterator>() {}
    assert_task_iterator::<TlsHandshakeTask>();
}

#[test]
fn test_tls_handshake_task_associated_types() {
    // Verify associated types are correct

    fn check_types<T>()
    where
        T: TaskIterator<Pending = TlsHandshakeState, Ready = (), Spawner = NoAction>,
    {
    }

    check_types::<TlsHandshakeTask>();
}
