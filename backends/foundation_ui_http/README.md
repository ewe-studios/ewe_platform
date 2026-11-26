# Foundation view routes

This crate serves a Foundation view over the Hypertext Transfer Protocol (HTTP).
The handler chooses the response from the request and its application services.

~~~rust
use foundation_ui_http::{get, Response, ViewError, ViewRequest};
use foundation_ui_runtime::html;

struct Settings { title: String }

#[get("/initial")]
fn initial(request: ViewRequest) -> Result<Response, ViewError> {
    let settings = request.require::<Settings>()?;
    Ok(Response::frame(html! { <Text>{settings.title.as_str()}</Text> }))
}
~~~

Register the handler with `router.mount(initial)?`.
The client receives one raw frame and replaces its own view.
A targeted handler can queue operations on `request.render_context()` and pass that context to `Response::frame`.

Use `Response::html(tree)` for Hypertext Markup Language (HTML).
Use `Response::json(value)` for JavaScript Object Notation (JSON).
A handler can select different formats for different requests.

For a test, construct a `ViewRequest` and keep its `session()`.
Call `initial.respond(request).await?` to inspect the selected `Response`.
Read the media type with `response.media_type()`.
Use `session.body(response).write_body(&mut writer)?` to render to any byte writer.

Install the signer and any shared node allocator in the host context before request middleware runs.
Middleware can install verified client capabilities.
Otherwise, the adapter reads the initial request's `x-ewe-hello` header.
A request without either uses the current web vocabulary.

### Serve a signed view

Use `Response::frame((tree, state))` when a control in the tree contains a state marker.
The writer signs the state with the request signer.
The following template returns the tree and its state together.

~~~rust
use foundation_ui_http::Response;
use foundation_ui_runtime::html;

fn counter(value: u32) -> Response {
    Response::frame(html! { state = value, <Text primal::state>{value}</Text> })
}
~~~

Use `request.signals()` to create reactive values for one request.
The writer keeps the scope alive. It stabilizes the scope before writing the body.

### Mount an action registry

~~~rust
use foundation_ui_http::{ActionRegistryRouteHandler, http::{shared::app::HttpApp, Serve}};
use foundation_ui_runtime::action::ActionRegistry;
use std::sync::Arc;

fn app(registry: ActionRegistry) -> Result<HttpApp<Arc<dyn Serve>>, Box<dyn std::error::Error>> {
    let mut app = HttpApp::new_serve();
    app.router.mount(ActionRegistryRouteHandler::new(registry))?;
    Ok(app)
}
~~~

The handler accepts POST requests at `/action` and `/action/*`.
A named path must agree with the action name inside the frame.
The handler accepts one raw request and returns a length-prefixed reply.
Install an `Arc<dyn ActionRequestPolicy>` in the host or middleware context to check requests before dispatch.

### Share live delivery

Mount `ActionSocketRoute::new(dispatcher.clone())` to serve binary WebSocket actions at `/socket`.
Each screen negotiates its own session. An action reply returns to that screen.
Install `SubscriptionReplay` in the request context to select the streams that the caller can read.

Register the same `EventDelivery` instance for GET and POST on the feed path.
The GET opens the downstream connection. Its hello header negotiates a session.
Use the Accept header `text/event-stream` for server-sent events or `application/primal-frame` for length-prefixed frames.
An authenticated POST can resume one subscription on that connection.
Set `DeliveryPrincipal` from trusted authentication middleware on both requests.
