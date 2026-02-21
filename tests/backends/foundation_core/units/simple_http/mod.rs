// Consolidated test module for `tests/backends/foundation_core/units/simple_http`.
//
// This file re-exports all per-module test files so the integration-test binary
// for this directory includes every unit test in a single crate. The list
// mirrors the files created during the non-destructive migration.
//
// If you add new unit test files into this directory, add a corresponding
// `mod` entry here so the test runner picks them up.

#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(unused_mut)]

mod actions_tests;
mod client_tests;
mod connection_tests;
mod dns_tests;
mod errors_tests;
mod impls_chunk_parser_tests;
mod impls_line_feed_tests;
mod impls_service_action_tests;
mod impls_simple_incoming_tests;
mod impls_simple_url_tests;
mod intro_tests;
mod pool_tests;
mod request_tests;
mod simple_http_client_tests;
mod tls_task_tests;
mod tls_tests;
mod url_authority_tests;
mod url_mod_tests;
mod url_path_tests;
mod url_query_tests;
mod url_scheme_tests;
mod url_tests;
