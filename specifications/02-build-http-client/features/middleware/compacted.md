# Compacted Context: Middleware Feature

⚠️TEMP_FILE|DELETE_WHEN_DONE|GENERATED:2026-03-03

## LOCATION
workspace:ewe_platform|spec:02-build-http-client|feature:middleware|impl:backends/foundation_core/src/wire/simple_http/client/

## OBJECTIVE
Implement middleware system for request/response interception with onion model pattern

## REQUIREMENTS
req:Middleware trait|methods:[handle_request,handle_response,name]|traits:[Send+Sync]
req:MiddlewareChain|methods:[new,add,process_request,process_response]|pattern:onion_model(forward_req,reverse_res)
req:built-in middleware|types:[LoggingMiddleware,TimingMiddleware,RetryMiddleware,HeaderMiddleware]
req:Extensions type|purpose:type_safe_data_passing|uses:TypeId+Any
req:integration|location:SimpleHttpClient|field:middleware_chain|call_points:[before_request,after_response]
req:per-request control|methods:[skip_middleware,no_middleware]

## CRITICAL_RULES
sync_only:NO async/await/tokio|project_is_synchronous
test_location:tests/backends/foundation_core/units/simple_http/|NO inline #[cfg(test)]
one_at_a_time:ONE test→impl→verify→next|TDD mandatory
no_unwrap:use Result+?|never unwrap/expect in production
docs:WHY/WHAT/HOW+Errors+Panics|all public items
dependency:project→stdlib→external|check existing first

## LEARNINGS
past:no async allowed in this project|strictly synchronous only
past:std feature is only valid feature|no "sync" feature exists
past:tests go in tests/ hierarchy|never inline test modules

## EXISTING_STRUCTURES
PreparedRequest:fields[method:SimpleMethod,url:Uri,headers:SimpleHeaders,body:SendSafeBody]
ClientConfig:has default_headers,timeouts,redirects|used by SimpleHttpClient
SimpleHttpClient<R:DnsResolver>:generic over DNS|uses ClientConfig+pool
ClientRequest<R>:api methods[introduction,body,send]|wraps TaskIterator
HttpClientError:use derive_more|add MiddlewareError+RetryNeeded variants

## TASKS
[ ]1.Create extensions.rs|Extensions struct|type_safe storage via TypeId/Any
[ ]2.Create middleware.rs|Middleware trait|handle_request+handle_response+name
[ ]3.Impl MiddlewareChain|add,process_request,process_response|onion model(rev for response)
[ ]4.Impl LoggingMiddleware|log request/response|optional body logging
[ ]5.Impl TimingMiddleware|measure duration|store Instant in extensions
[ ]6.Impl RetryMiddleware|retry on status codes|BackoffStrategy from valtron
[ ]7.Impl HeaderMiddleware|add default headers|only if not present
[ ]8.Add middleware_chain to SimpleHttpClient|Arc<MiddlewareChain>
[ ]9.Add middleware() builder method|to SimpleHttpClient
[ ]10.Integrate call points|process_request before send,process_response after receive
[ ]11.Add skip_middleware/no_middleware|per-request control
[ ]12.Write tests|execution order,onion model,each middleware,integration

## FILES_TO_CREATE
create:[client/extensions.rs,client/middleware.rs]
update:[client/mod.rs,client/client.rs,client/api.rs]
update:[../../lib.rs for HttpClientError variants]
tests:[tests/backends/foundation_core/units/simple_http/middleware_tests.rs]

## NEXT_ACTIONS
1. Read existing HttpClientError definition
2. Write test for Extensions::insert+get
3. Implement Extensions (ONE test at a time, TDD)
4. Write test for Middleware trait basic usage
5. Implement Middleware trait
6. Continue ONE component at a time

---

⚠️ **AFTER READING**: Clear context, reload from this file only, start work
