//! Generated wire types (`proto/ewe/proxy/registration/v1/registration.proto`).
//!
//! WHY: the registration contract is defined once in protobuf (root rule 5).
//! This crate generates its client from the same file that the proxy's
//! registration authority serves, and does not depend on the proxy crate
//! (spec 82 feature 01, section 4.6).
//!
//! WHAT: [`v1`] holds the messages and the two enums;
//! [`v1::registration_service`] and [`v1::registrar_enrollment_service`] hold
//! each service's trait, `register_*` function, typed client and procedure
//! paths.
//!
//! HOW: `include!` of the `OUT_DIR` files that `build.rs` writes.

/// `ewe.proxy.registration.v1`.
#[allow(
    clippy::all,
    clippy::pedantic,
    missing_docs,
    unused_qualifications,
    rustdoc::all
)]
pub mod v1 {
    include!(concat!(
        env!("OUT_DIR"),
        "/ewe.proxy.registration.v1.mod.rs"
    ));

    /// `RegistrationService`: register, report, deregister, renew.
    pub mod registration_service {
        #[allow(unused_imports)]
        use super::*;
        include!(concat!(env!("OUT_DIR"), "/registration_service.rs"));
    }

    /// `RegistrarEnrollmentService`: enrollment token to client certificate.
    pub mod registrar_enrollment_service {
        #[allow(unused_imports)]
        use super::*;
        include!(concat!(env!("OUT_DIR"), "/registrar_enrollment_service.rs"));
    }
}
