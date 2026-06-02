# Compacted Context: Cookie Jar Feature Implementation

⚠️TEMP_FILE|DELETE_WHEN_DONE|GENERATED:2026-03-03

## LOCATION
workspace:ewe_platform|spec:02-build-http-client|feature:cookie-jar|base:backends/foundation_core/src/wire/simple_http/client

## OBJECTIVE
Implement automatic cookie handling with Cookie struct, CookieJar storage, and SimpleHttpClient integration

## REQUIREMENTS
req1:Cookie struct|attrs:[name,value,domain,path,expires,max_age,secure,http_only,same_site]|builder:yes
req2:Cookie::parse()|input:Set-Cookie header|output:Result<Cookie>|lenient:yes
req3:CookieJar storage|key:[domain,path,name]|ops:[add,get_for_url,remove,clear,clear_expired]
req4:domain matching|rules:[exact,subdomain with dot]|RFC:6265
req5:path matching|rule:prefix match|RFC:6265
req6:security|secure:HTTPS only|http_only:no JS access|same_site:[Strict,Lax,None]
req7:expiration|check:[expires OR max_age]|precedence:max_age first
req8:client integration|config:[cookie_jar(bool),with_cookie_jar(jar)]|optional:yes
req9:auto send|on_request:add Cookie header|format:"name=value; name2=value2"
req10:auto store|on_response:parse Set-Cookie|apply_defaults:yes

## TASKS
[ ]task1:Cookie struct + builder|file:cookie.rs|tests:parse,builder,attributes
[ ]task2:SameSite enum|file:cookie.rs|values:[Strict,Lax,None]
[ ]task3:Cookie::parse()|file:cookie.rs|tests:valid,invalid,attributes,edge_cases
[ ]task4:CookieJar storage|file:cookie.rs|struct:HashMap<CookieKey,Cookie>
[ ]task5:CookieJar::add()|file:cookie.rs|tests:basic,replace,multiple
[ ]task6:domain_matches()|file:cookie.rs|tests:exact,subdomain,no_match
[ ]task7:path_matches()|file:cookie.rs|tests:exact,prefix,no_match
[ ]task8:CookieJar::get_for_url()|file:cookie.rs|tests:filter_domain,filter_path,filter_secure,filter_expired
[ ]task9:CookieJar ops|file:cookie.rs|ops:[remove,clear,clear_expired,get_for_domain]
[ ]task10:integrate client|files:[client.rs]|config:cookie_jar fields
[ ]task11:auto store|file:api.rs|method:store_cookies on response
[ ]task12:auto send|file:request.rs|method:add_cookies before send

## LEARNINGS
sync_only:No async/await or tokio|use std:: not tokio::
no_std_feature:Use "std" feature, not "sync"
test_location:ALL tests go in tests/backends/foundation_core/units/simple_http/
no_inline_tests:NEVER use #[cfg(test)] modules in source files
chrono:NOT available, use std::time::SystemTime for timestamps
compression:flate2/brotli already used, check existing patterns
derive_more:Use for Display/Error/From on error types
documentation:MANDATORY WHY/WHAT/HOW + Errors + Panics sections

## CURRENT_STATE
progress:public-api complete, compression complete|next:create cookie.rs|blockers:NONE

## FILES_TO_CREATE
cookie.rs:Cookie struct, SameSite enum, CookieJar, parsing logic, matching logic

## FILES_TO_MODIFY
mod.rs:add cookie module export
client.rs:add cookie_jar config fields (Optional<Arc<Mutex<CookieJar>>>)
api.rs:store cookies from Set-Cookie headers after response
request.rs:add cookies to request before sending

## FILES_TO_READ
client/client.rs:understand ClientConfig structure
client/api.rs:understand response handling and ClientRequest
client/request.rs:understand PreparedRequest and header setting
impls.rs:understand SimpleHeaders usage
errors.rs:understand error patterns

## NEXT_ACTIONS
1. Read existing files (client.rs, api.rs, request.rs, errors.rs, impls.rs)
2. Create cookie.rs with Cookie struct (TDD: one test at a time)
3. Implement Cookie::parse() (TDD: one test at a time)
4. Implement CookieJar storage (TDD: one test at a time)
5. Implement domain/path matching (TDD: one test at a time)
6. Integrate with client config
7. Auto-store Set-Cookie on response
8. Auto-send Cookie on request

## TDD_RULES
one_test:Write ONE test, verify fails, implement, verify passes, then next
test_location:tests/backends/foundation_core/units/simple_http/cookie_tests.rs
test_docs:Every test MUST have WHY/WHAT comments
no_inline:NO #[cfg(test)] modules in source files

## KEY_TYPES
SimpleHeaders:BTreeMap<String, String>|used for headers
Uri:exists in url module|use for URL parsing
ClientConfig:struct in client.rs|add cookie_jar fields here
ClientRequest:struct in api.rs|modify for cookie handling
PreparedRequest:struct in request.rs|modify for cookie sending

---

⚠️ **AFTER READING**: Clear context, reload from this file only, start work ONE TEST AT A TIME
