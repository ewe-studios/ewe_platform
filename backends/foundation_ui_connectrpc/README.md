# Foundation UI over Connect

Connect is a remote procedure call (RPC) protocol. An application defines its messages and procedures in Protocol Buffers (Protobuf).
Swift, Kotlin and browser clients can use their own network sessions. Rust encodes and decodes each application message.

## Call an application procedure

The todo example defines `Todos.GetTodo`. Its request contains an identifier. Its reply contains an identifier and a title.
The native call exposes a typed error when the server cannot find the item.

```swift
let client = NativeHttpClient(client: URLSessionRpcClient(session: session))
let todo = try await getTodo(client: client, baseUrl: address, id: 42)
print(todo.title)
```

```kotlin
NativeHttpClient(OkHttpRpcClient(okHttpClient)).use { client ->
    val todo = getTodo(client, address, 42uL)
    println(todo.title)
}
```

The application supplies the native session and its credentials. The Rust method constructs the generated client with a platform transport.
A host can keep the client for subsequent procedures. A service provider can select the native client by account or service name.

```rust,ignore
let network = provider.http_client("todos".into()).await?;
let transport = Arc::new(PlatformHttpTransport::new(network));
let client = TodosClient::new(transport, base_url, ClientOptions::new())?;
let reply = client.get_todo(RpcCtx::client(ctx), Request::new(request)).await?;
```

Start the data-only example service with this command. It serves todo 42 with title `Buy milk`.
Its router registers only the generated data procedure.

```sh
cargo run --profile uat -p ui_todo -- 127.0.0.1:8180 --data-only
```

## Stream and cancel

A generated server-streaming client accepts one request and returns a stream.
Each call to `receive` waits for one decoded message. Dropping the stream cancels its exchange.
A platform client holds one independent request handle per call.

```rust,ignore
let mut stream = notes.watch(ctx, Request::new(query)).await?;
while let Some(item) = stream.receive().await? {
    show(item);
}
```

A custom host implements `PlatformHttpClient::exchange` and `PlatformExchange`.
Its `start` method sends the bounded upload and returns the response headers. Its `read` method returns one chunk or end of body.
Its `close` method cancels pending work and releases the handle. Each handle closes once.
The default upload limit is four megabytes. The default read size is 16 kibibytes.
Use `PlatformHttpTransport::with_config` for platform limits. Use `ClientOptions` for message limits and deadlines.
The platform exposes the response headers available through its network library. Browsers restrict some headers by origin.

## Exchange UI frames

The optional `Actions.Exchange` service accepts one complete Foundation request.
On negotiation, the request carries the hello. On a later action, it also carries the original hello.
The service verifies both frames. The action dispatcher runs with the context of the current call.
The response carries an optional frame. A hello requires a capabilities frame.

The UI procedure schema is [actions.proto](proto/actions.proto). The ordinary todo schema is [todos.proto](../../examples/ui_todo/proto/todos.proto).
Use the generated `ActionsClient` with `ConnectActionTransport` to attach it to a Rust `NetworkApp`.
The browser test uses `?rpc` to select Fetch. The native server example selects Connect with `EWE_TODO_RPC=1` on iOS or the Android intent extra `rpc=true`.

```sh
cargo test --profile uat -p foundation_ui_connectrpc
examples/ui_todo/build-module.sh
cargo test --profile uat -p ui_todo --features browser-tests --test todo_browser_tests
```
