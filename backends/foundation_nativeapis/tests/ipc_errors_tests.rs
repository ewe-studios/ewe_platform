//! Tests extracted from shared/ipc/errors.rs
mod tests {
    use foundation_nativeapis::shared::ipc::*;
    

    #[test]
    fn error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::Other, "test");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::IoError(_)));
    }

    #[test]
    fn join_error_to_send_error() {
        let join = JoinError::Timeout;
        let send: SendError = join.into();
        assert!(matches!(send, SendError::Timeout));
    }

    #[test]
    fn join_error_to_recv_error() {
        let join = JoinError::TokenMismatch;
        let recv: RecvError = join.into();
        assert!(matches!(recv, RecvError::TokenMismatch));
    }
}
