# Valtron Based Http Framework

I have been having this idea for a while, we've invested sometime in implementing a routing library here: /home/darkvoid/Boxxed/@dev/ewe_platform/crates/routing/ which we can definitely bring in, into a new crate called foundation_http which builds off the http parser we've implemented in foundation_core.

This can power the creation of http servers (based applications) with our already implemented support for websocket, SSE (server sent events) and http parsing and clients.

What i imagine is a world where i can create a http application like:

```rust
#[derive(Default)]
pub struct Hello;

// Serve will require its implementer to als implement default()
// this will allow them to simple be either unit structs that implement
// the trait and perform some actions of sort.
// they can take the connection, spawn a valtron task or own it
// the core idea is they own and control the connection and request.
// requests are never multiplexed. Process request by the handler once
// done, a handler could end up owning the connection stream forever e.g websocket,
// user error which is ok.
impl Serve for Hello {
    fn serve(bag: Arc<ContextBag>, req: SimpleIncomingRequest, conn: SharedByteBufferStream<RawStream>) {
        // then you can pull the context like, returns a Arc<PostgreSQL> to allow easy sharing
        let Some(db) = bag.get<PostgresSQL>::() else {
            // write failure to conn
        };

        // users can decide they want to spawn this into a valtron task or handle it within the
        // giving socket thread
    }
}


// app is the main router and handler for all things
let mut app = foundation_http::app::create();

// app has a context bag every task gets and you can store things there.
app.context().store(PostgresSQL::new());

// app owns the routes and people can register their handlers
// by simply calling app.route with the handler and provide information about the method and route
// they wish to match.
app.route::<Hello>(SimpleHttpMethod::GET, "/apps/centers");
app.route::<Hello>(SimpleHttpMethod::POST, "/apps/centers");
app.route::<Hello>(SimpleHttpMethod::DELETE, "/apps/centers");


// internally the server
// server will use the foundation_core::BackgroundJobRunner to spawn multiple worker threads
// then deliver every incoming socket to them for handling and processing,
// blocking the connection pool until one is free to take on another incoming connection and
// provide a naturally throughput handling and blocking.
let server = foundation_http::server::new_server(app);


let shutdown_signal = Arc::new(OnSignal::new());

// signals will turn on the OnSignal to indicate shutdown.
signals::listen_for_sigint(shutdown_signal.clone());

// serve normal http which will block
// serve will stop once signal indicates its should die.
server.serve(shutdown_signal.clone());

// serve tls
// serve will stop once signal indicates its should die.
server.serve_tls(RustTlsAcceptor::from_pem(cert, key), shutdown_signal.clone());

```
