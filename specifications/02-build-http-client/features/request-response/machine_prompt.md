# Machine-Optimized Prompt: Request/Response Feature

⚠️GENERATED|DO_NOT_EDIT|REGENERATE_FROM:feature.md|GENERATED:2026-02-01

## META
feature:request-response|status:pending|priority:high|effort:small|depends:[foundation,connection]
tasks:0/10|completion:0%|created:2026-01-18|updated:2026-01-24

## OVERVIEW
Request builder API (ClientRequestBuilder)|response types (ResponseIntro)|prepared request structure (PreparedRequest)
fluent API|reuse simple_http types|no duplication

## DEPENDENCIES
requires:foundation ✅,connection ✅|used_by:task-iterator,public-api
types_from:simple_http/impls.rs|errors:HttpClientError|url:ParsedUrl

## REUSING_TYPES
SimpleResponse<T>:final response|IncomingResponseParts:iterator yields|Status:HTTP codes
Proto:HTTP version|SimpleHeaders:headers|SimpleBody:body variants
SimpleMethod:HTTP methods|SimpleHeader:header keys|Http11RequestIterator:request renderer

## TYPES_TO_CREATE
ResponseIntro:wrapper for (Status,Proto,Option<String>)|public user-facing
PreparedRequest:internal ready-to-send request|pub(crate)|method,url,headers,body
ClientRequestBuilder:fluent API|public|method,url,headers,body optional

## RESPONSEINTRO_SPEC
fields:[status:Status,proto:Proto,reason:Option<String>]
impl:From<(Status,Proto,Option<String>)>
visibility:public|usage:user-facing wrapper

## PREPAREDREQUEST_SPEC
fields:[method:SimpleMethod,url:ParsedUrl,headers:SimpleHeaders,body:SimpleBody]
method:into_request_iterator()->Http11RequestIterator
visibility:pub(crate)|usage:internal only

## CLIENTREQUESTBUILDER_SPEC
fields:[method:SimpleMethod,url:ParsedUrl,headers:SimpleHeaders,body:Option<SimpleBody>]
new:(method,url)->Result<Self,HttpClientError>|parses URL via ParsedUrl
headers:[header(key,value),headers(SimpleHeaders)]|fluent returns Self
body:[body_text(text),body_bytes(bytes),body_json<T>(value),body_form(params)]
convenience:[get(url),post(url),put(url),delete(url),patch(url),head(url),options(url)]
build:build()->PreparedRequest|consumes builder

## FILE_STRUCTURE
client/request.rs:ClientRequestBuilder,PreparedRequest|NEW
client/intro.rs:ResponseIntro|NEW
client/mod.rs:re-exports|UPDATE

## IMPLEMENTATION_NOTES
reuse:simple_http/impls.rs types|DO NOT duplicate
pattern:fluent builder|chained methods return Self
visibility:PreparedRequest pub(crate)|ResponseIntro public
error:HttpClientError for URL parse failures

## TASKS
[ ]task1:create intro.rs|ResponseIntro struct|(status,proto,reason)
[ ]task2:impl From<tuple> for ResponseIntro|conversion
[ ]task3:create request.rs|PreparedRequest,ClientRequestBuilder
[ ]task4:impl ClientRequestBuilder::new|URL parsing via ParsedUrl
[ ]task5:impl header methods|header(),headers()
[ ]task6:impl body methods|body_text,body_bytes,body_json,body_form
[ ]task7:impl convenience methods|get,post,put,delete,patch,head,options
[ ]task8:impl build()|PreparedRequest creation
[ ]task9:impl PreparedRequest::into_request_iterator|Http11RequestIterator
[ ]task10:write tests|comprehensive unit tests

## VERIFICATION
cmds:[cargo fmt --check,cargo clippy -D warnings,cargo test --package foundation_core]
tests:request + intro tests|unit coverage
standards:.agents/stacks/rust.md

## SUCCESS_CRITERIA
ResponseIntro wraps correctly|From impl works
PreparedRequest holds data|into_request_iterator works
ClientRequestBuilder fluent API|all methods chain
convenience methods work|get,post,etc
body methods work|text,bytes,json,form
header methods work|header,headers
tests pass|fmt pass|clippy pass

## RETRIEVAL_REQUIRED
search:[simple_http/impls.rs patterns]|read:[Http11RequestIterator impl]
check:[existing builder patterns]|verify:[SimpleHeaders,SimpleBody usage]
errors:[HttpClientError handling]|tests:[existing test patterns]

## DOCS_TO_READ
../requirements.md|./feature.md|simple_http/impls.rs|connection/mod.rs|errors.rs

---
Token reduction: feature.md (~6KB) → machine_prompt.md (~2KB) = 67% savings
