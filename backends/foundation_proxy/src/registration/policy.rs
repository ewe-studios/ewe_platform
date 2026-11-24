//! The registration address and route policy.
//!
//! WHY: a valid connection is not enough. A sidecar must not register an
//! address outside the operator's networks (server-side request forgery (SSRF)
//! and traffic theft, section 5.4), a route host that the operator did not
//! allow, or a service that its host may not join.
//!
//! WHAT: [`RegistrationPolicy`]'s checks. [`RegistrationPolicy`] itself lives in
//! [`crate::registration::config`].
//!
//! HOW: an Internet Protocol (IP) literal is checked against the allowed
//! networks. A Domain Name System (DNS) name is resolved with the injected
//! [`DnsResolver`], and every result must match. The asynchronous check parks
//! the task; it never runs the blocking lookup on a worker.

use std::net::IpAddr;

use foundation_netio::shared::client::DnsResolver;

use super::config::RegistrationPolicy;

/// Check a backend address against the policy.
///
/// `host` is an IP literal or a DNS name; `port` is used only for the lookup.
///
/// # Errors
/// A message that names the address and the rule that refused it.
pub async fn check_address(
    policy: &RegistrationPolicy,
    host: &str,
    port: u16,
    resolver: &dyn DnsResolver,
) -> Result<(), String> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return check_ip(policy, ip);
    }
    let addrs = resolver
        .resolve(host, port)
        .await
        .map_err(|e| format!("address {host} does not resolve: {e}"))?;
    if addrs.is_empty() {
        return Err(format!("address {host} does not resolve"));
    }
    for addr in addrs {
        check_ip(policy, addr.ip())?;
    }
    Ok(())
}

/// The blocking form of [`check_address`] for a sync caller (a test or a
/// blocking boundary). It runs the lookup on the current thread.
///
/// # Errors
/// As [`check_address`].
pub fn check_address_blocking(
    policy: &RegistrationPolicy,
    host: &str,
    port: u16,
    resolver: &dyn DnsResolver,
) -> Result<(), String> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return check_ip(policy, ip);
    }
    let addrs = resolver
        .resolve_blocking(host, port)
        .map_err(|e| format!("address {host} does not resolve: {e}"))?;
    if addrs.is_empty() {
        return Err(format!("address {host} does not resolve"));
    }
    for addr in addrs {
        check_ip(policy, addr.ip())?;
    }
    Ok(())
}

fn check_ip(policy: &RegistrationPolicy, ip: IpAddr) -> Result<(), String> {
    if policy.allowed_networks.iter().any(|net| net.contains(&ip)) {
        return Ok(());
    }
    Err(format!("address {ip} is outside allowed_networks"))
}

/// Check that a sidecar may create a service for `host`.
///
/// A sidecar may always join a service that exists in the config file.
///
/// # Errors
/// A message that names the host when the policy does not allow a new service
/// for it.
pub fn check_route_host(
    policy: &RegistrationPolicy,
    host: &str,
    is_existing_service: bool,
) -> Result<(), String> {
    if is_existing_service
        || policy
            .allowed_route_hosts
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(host))
    {
        return Ok(());
    }
    Err(format!(
        "route host {host} is not in allowed_route_hosts and no config service has it"
    ))
}

/// Check that `host_id` may register a backend for `service`.
///
/// A host that [`RegistrationPolicy::services_by_host`] does not name may join
/// any service.
///
/// # Errors
/// A message that names the host and the service when the policy forbids it.
pub fn check_service(
    policy: &RegistrationPolicy,
    host_id: &str,
    service: &str,
) -> Result<(), String> {
    let Some(by_host) = &policy.services_by_host else {
        return Ok(());
    };
    let Some(allowed) = by_host.get(host_id) else {
        return Ok(());
    };
    if allowed.iter().any(|name| name == service) {
        return Ok(());
    }
    Err(format!(
        "service {service} is not allowed for host {host_id}"
    ))
}
