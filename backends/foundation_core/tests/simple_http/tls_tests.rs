use foundation_core::{
    valtron::{NoAction, TaskIterator},
    wire::simple_http::client::{TlsHandshakeState, TlsHandshakeTask},
};

#[test]
fn test_tls_handshake_state_variants() {
    // Verify all state variants exist
    let _init = TlsHandshakeState::Init;
    let _handshaking = TlsHandshakeState::Handshaking;
    let _complete = TlsHandshakeState::Complete;
    let _error = TlsHandshakeState::Error;
}

#[test]
fn test_tls_handshake_state_debug() {
    let state = TlsHandshakeState::Init;
    let debug_str = format!("{:?}", state);
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
