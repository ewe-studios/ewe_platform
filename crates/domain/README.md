# Domain

A series of crates providing different functionality and binaries for projects.

## Notice
This is a work in progress crate, please do not depend on for now

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

## Binaries

- [watchful](./bin/watchful/): provides a binary that can be called with a config for watching directories/files for changes.

## Crates

- [watchers](./crates/watchers/): provides the underlying core implementation for watching directories and files via configuration.

## ChangeLog

- 7/03/2024: Completed json deserialization for Configuration for dir and file watching

## Philosophy of Engineering

We want to focus the architecture around moving the core side effects (Http, FileSystems, ..etc) to the boundaries of the different parts of the system.

This ensures to keep as much of the core as pure and functional as possible without mixing in concerns from the outside that will make things harder to change, maintain and keep simple.

I believe complexity should be as as much in the boundaries of a system to ensure that those complexity can easily be abstracted and changed as we learn more over time.

This is why we want to employ clean architecture - which is a mix of hexagonal architecture and onion architecture to create a simple where the core domain are as pure and focused on the underlying business logic as much as possible to ensure that since these do not change much we create a core that fully represent the underlying use-cases that the core business needs to meet.

Hexagonal architecture also has a concept of Adapters and Ports where the ports are the means by which
the Adapters interface with the core business domain.

Adapters provide a way for these side effects to provide needed interfaces or meet needed interfaces that the ports can both call and be called by. Because hexagonal architecture seens processes in any system to be divided into "Driven" and "Driving" parts, we can consider what drives these system and what this system drives.

## Communication Semantics

I strongly believe that the underlying core means by which different pieces of the API should communicate must entirely be via channels if possible else should be via channel like structures that make this possible e.g wrapped type that uses a future underneath for some async operations and responds.

```rust

// Receive only structures
trait ReceiveChannel<T>{
    Event = T,

   // we will let ChannelError:Closed indicate when closed
   try_receive(self) -> Result<Event>>
```

In the background, Channels will have a send only version held by the generator of the channel which will be used to deliver events
to the channel.

```rust
// Send only structures
trait SendChannel<T>{
    Event = T,

    close();
    try_send(event: Event);
}
```

Hence like rust mspc crate, we would expect creation of a channel to follow symantic:

```rust
// create sender and receiver handles for a channel of Event type T.
sender, receiver = channels::create<T>()
```

This can come in varying formats but it's deeply important to ensure even APIs that abstract these underlying operations do so via channels so that they can easily keep the leakage as low to near imposssible as much as can be done.

We can use [crossbeam](https://github.com/crossbeam-rs/crossbeam) which implements a nicer channel API on top of rust internal [mspc](https://doc.rust-lang.org/std/sync/mpsc/) with more custom features.

I believe if we force the different exit points of the system to only be able to accept or return channels, this forces us to be thoughtful about what goes in and out and how they operate, secondly, this will make us more thoughtful about what exactly we expose and move us towards the concept of "tell them what to do" not "how to do it".

But the more I think about this the more I find this very limited, because even if channels are the underlying structure used for communication across different part of a system (local boundaries) they should still need to be wrapped around some functional API that fits the necessary purposes either in the driving or driven side of the architecture.

```rust
trait ConfigurationService {
    GetFileConfigFromService(Address, BusinessDomain)
}
```

This is where a rich language like rust with rich enums that can both express state and intent in a rich syntax become deeply useful. We can create type alias or thin wrappers around channels that send a single definition of an event or request represented by a rich enum that clearly communicate the intent or state we are trying to transfer into and out of the business logic domain which ensures our exit points or boundary edges communicate purely through data objects and not concrete function calls that make serialization hard. With such boundaries being simply DTO objects if you will you can imagine the flexibility such boundaries provide overall in the environment they can run in.

So even if our facade is wrapping a channel and representing a API that meets a driven or driving trait, that underlying structure need not leak to the port and they simple express intent to either side with a data object and not direct functional call.

The Adapters naturally would call their internal function to perform action but this detail is:

- Hidden from the business domain and port.
- Internally repersentable in any way as the Adapter sees fit.

The port will be a specific trait that the business logic uses and the adapter connects with the port to provide that functionality via its reception of domain defined enums representing intentional action to be performed on it's behalf or its change of state for which the adapter might be of interest to react to (in the case of driven APIs).

We can think of the port has defining specific capability and the Adapters (maybe a wrapped channel) meeting that capability.

This means the business domain must express out boundaries with two concepts:

- Events: These represent state changes that are required to be communicated clearly to related Adapters

This way it moves from an imperative API to more of a reactive one that based on the list of events it receives it reacts to those and generates relevant calls (commands) to necessary ports (calling connected Adapters) to perform specific actions for it.

The nice piece of this is driving Adapters who call the relevant Ports the business domain exposes will simple turn those calls into relevant events as appropriate for the system to react to.

This will mimick the architecture [Crux](https://github.com/redbadger/crux) sells but in a easier, more thoughtful way without all the code generation (which is not bad) and lets us better understand this architecture far better before creating code gen abstractions that make it hard to reason about fundamentally.

This means states and events roughly should be communicated with a type of enum like:

```rust
enum AppEvents {
    ConfigUpdated,
    DirConfigUpdated,
    FileConfigUpdated,
}

enum AppRequests {
    Http(HttpRequest), // == this as an example troubles me, we do not want such leakage into the domain, yet we need to be able to perform such operations.
    WatchFile(Id, FileWatcher),
    DirWatcher(Id, DirWatcher),
}

enum AppData {
    Events(AppEvents)
    Requests(AppRequests)
}

// [DEPRECATED]: Go lower for a more refined trait
//
// Random name just indicative this is our business domain
trait BusinessDomain {
    In() -> ReceiveChannel<AppEvents>
    Out() -> ReceiveChannel<OutEvents>
    Requests() -> ReceiveChannel<AppRequests>
}
```

Looking at the code above, everything pleases me except the AppRequests as an example, we see the problem where no matter how when Requests are represented as pure enums with states, e.g the `Http(HttpRequest)` itself is a leak.

It is leaking cross boundaries details `Http` into the domain business and the question is do we really need this? Does the domain need to be aware of a concept of an `HttpRequest`, should we not instead represent this as an underlying hidden detail that the underlying events do not care about.

We could make a specific domain request type that indicates it might need to use http internally by some use-case operation but it does not necessary need to actually be the `HttpRequest` itself.

That is: we should describe an operation native to the domain and intrinsicially connected to it's use-cases which might use some http underlying platform tooling but never is something someone needs to know from a top-down perspective.

Why you might ask ?

Well if say the definition of `HttpRequest` ever changes, then we will be forced to change business logic as well.

Hence the domain request should be specific and be within actual bounded contexts and contextual wordings to the business of the domain not platform operations.

This allows us indicate what we want the domain to do and not how to do it, we end up building systems around the actual goals of the domain and not end up creating systems that describe how its going to do it by leaking such platform details into the definition.

Hence things need to switch abit to match the following below instead:

```rust

// Custom definition of what a file is, i.e a file path string.
type File = String;

// Platform implementations then wrap these definition to provide platform
// features that a use-case may use
trait FileWatcher {
    async WatchFile(File, BusinessDomain)
}

// A unique use-case that defines a specific type of request
// making it definitive about what's going on and what exactly
// does it mean.
type BatchDelegationRequest = Vec<AppRequest>;
type BatchDelegationResponse = Vec<AppEvents>;
trait GenerateBatchDelegationService {
    GenerateBatchDelegation(BatchDelegationRequest, BusinessDomain)
}

// Indicative of a request set that is uniquely identified by an Id.
type NamedRequest struct(RequestId, Vec<AppRequests>)

// Custom definition of what a Dir is, i.e a dir path string.
type Dir = String;

// The different types of unique requests that can be generated
// and handled later as returned events.
struct AppRequests {
    WatchFile(File),
    WatchDir(Dir),
    ViewModel(),

    // Request become contextual using context wordings and never
    // ambiguous or platform specific.
    BatchDelegation(BatchDelegationRequest),
}

// Indicative of a event set that is uniquely identified by an Id,
// usually connected to a NamedRequest via the same RequestId.
type NamedEvent struct(RequestId, Vec<AppEvents)

// these are defined by the domain and hence are a port
// and adapter that ensure handle needed conversion from the underlying
// implementation that handles such.
// Generally, these are left to implement their internal logic as they sit fit.
enum AppEvents {
    View(ViewModel)
    WatchingFile(path),
    FailedFileConfigurationRequest(Error),
    SucceededFileConfiguratonRequest(Response),
    GeneratedBatchDelegation(BatchDelegationRequest, BatchDelegationResponse),
}

// Random name just indicative this is our business domain
trait BusinessDomain {

    // Outgoing Requests (The Driving Adapters): This returns a output
    // channel (sending only) that provides
    // relevant use-cases, boundaries adapters a means
    // to receive requests from the business domain
    // that should be performed, these contain side effects
    // that should never leak into the domain but also allow
    // the domain to interact with the outside boundaries/world
    // to peform operations on it's behalf.
    //
    // Think of use-cases, business operations that are adjacent and
    // not intrinsic to the business domain e.g saving to a persistent db,
    // requesting FileSystem Files, etc. Layers that are relevant but
    // should be in the boundaries and not internals of the domain.
    Requests() ReceiveChannel<NamedRequest>

    // Respond (The Driving Ports) provides a clear indication that these events
    // are domain based events that are owned by the BusinessDomain
    // and never are the leakage of outside events into the domain but
    // provide responses to the domain on outgoing requests from BusinesDomain.Requests()
    // sent out, these are the responses to NamedRequests.
    Respond(NamedEvent)

    // InComing Requests (The Driven Adapter & Port): These support the case where other
    // boundary services want the business domain perform specific operation
    // that are owned, controlled and maintain by it, usually things that drive
    // internal change of state, generation of internal calculations that produes
    // these state changes or confirmation of some underlying facts e.g requesting
    // the business domain ViewModel repreenting its current view of its own state
    // that others will find useful for their operations.
    //
    // These meets the Hexagonal architecture on the driven side: allowing the outside
    // world to interact with the business domain by requesting its do something without
    // encroaching on those internal logic or leaking across boundaries.
    //
    // It returns a ReceiveChannel that is tied to the specific request
    // received and will used that to respond to the requests.
    // This means internally it captures and relevantly uses the
    // This channel to respond accordingly.
    //
    // Gernerally it creates a NamedRequest underneath.
    Do(AppRequest) ReceiveChannel<NamedEvents>

    // Events indicative of relevant changes within the system
    // that it both listens to and the outside would could listen
    // to for historical storage of the total changes the underlying
    // system has undergone.
    Events() -> ReceiveChannel<AppEvents>
}

```

The overall trait describes everything important for such an architecture that meets our goals to be possible:

1. Clear boundaries separation with Driving Ports/Adapters - This is the fact that business domain logic and the relevant use-cases (driving operations, Driving Side of Hexagonal architecture) that might contain side effects are heavily separately from the internal boundaries of the doamin and do not leak into the business domain itself. e.g reading from file system, written to database, etc. This means updates regarding these different operations always returns as NamedEvents indicating of these being operations that are clearly a response to requests.

2. Clear boundaring separation with Driven Ports/Adapters - This are outside operations (e.g from the client or console) that trigger different requests to the business domain to perform specific logic actions on their behalf which will always results in NamedRequests being created with returned NamedEvents represented by the `ReceiveChannel<Id, AppEvent>` through which the domain responds with.

3. Ability to listen into actual events the domain is generating for those wishing to get the full waterhose of underlying changes the business domain has undergone. This will be useful for when you wish to pipe these state changes into a persistent layer that allows resurrection of the domain to the state it was last before some operation or death.

4. Focus on all core methods outputing `ReceiveChannel`s that ensuring only the functional calls provide a means of delivery requests that respond with actual response `ReceiveChannel`s keeping the `SendChannel`s internal to the domain and is suited to the specific request. This allows implementations to be focused on the job at hand within their local context, keeping readability consistent without the developers jumping between context points of where request gets sent and where its routed and then handled. The code should be clear readily that it sends a requests, it get back a channel and answers that specific request within that context. The reader can just assume routing worked and execution was handled correctly without jumping and loosing local context.

## Platform Abstraction

To make such an architecture viable across different systems and platforms, we also proprose a concept called `PlatformContext` that will be used by the BusinessDomain and relevant use-cases to access core features like http, date, datetime, etc that abstracts these platform around a set of core traits which allows these to be fed in via dependency injection regardless of the underlying running runtime e.g native Rust, WASM, android/ios linking libraries.

This allows us abstract the core platform provision into systems that are feed into the both the BusinessDomain and other other use-cases, whilst still have the capability to abstract as much underlying operations of these platform context in ways that remove all side-effects from the BusinessDomain.

```rust
// in some Platform crate
use serde::Serialize;

trait Http<T, M implements Serialize> {
    type Event = T;

    get(url) ReceiveChannel<Event>
    post(url, data: M) ReceiveChannel<Event>
}

trait Date {
    now() ReceiveChannel<DateTime>
    parse_str(date_str: &str) ReceiveChannel<DateTime>
}

```

```rust
use platform::{Http, Date};

struct PlatformContext {
    http: Http,
    date: Date,
}
```

An example of this is the `http()` context provided by the platform that helps to abstract out the core logic of delivery an http requests form the domain, this can be abstract in a way that the actual delivery of the request is done in some thread or by the native platform in a different process so that the side-effects really are as outside of the business or use-case boundaries as much as is possible, making the business domain and use-case side-effect free and easily testable.

One interesting question is if things like `date()` should be async via the `ReceiveChannel` that are used to across most async operations
to scaffold these operations and provide a clear way to work with them, while this generally can be sync, as most libraries are, there seems to be some benefit in (though not always) making all Plaform systems async even if not needed to both:

1. Create a consistent API surface
2. Easily fit into environments where such information is actually async e.g WASM

## Example BusinessDomain and UseCase

```rust

// These traits should be specifc, they should not be generic
// when a domain logic needs some side effect from some external layer
// even if its really just http they should still be specific to the job done.
// this allows us to be more specialized to the use-case and never become too
// deeply tied to the actual platform details.
trait ConfigurationService {
    GetFileConfigFromService(NamedRequest<Address>, BusinessDomain)
}

pub type ServiceURL = String;

#[derive(UseCase)]
pub struct ConfigServiceManager{
    url: ServiceURL,
    context: PlatformContext,
}

impl ConfigurationService on ConfigServiceManager {
    pub fn GetFileConfigFromService(&self, data: NamedRequest, shell: DomainShell) {
        // use spawn_local
        spawn_local({
            async move {
                response = self.context.http().get(self.url, data).await;
                if !response.ok {
                    let mut reason = None;
                    switch response.statusCode {
                        ...
                        () => reason = Some("unexpected server failure");
                    }

                    shell.respond(data.to(AppEvent::FailedFileConfigurationRequest(reason)));
                    return;
                }

                shell.respond(data.to(AppEvents::SucceededFileConfiguratonRequest(response.text)))
            }
        })
    }
}


// domain operation use case


#[derive(DomainLayer)]
let app = BusinessDoman()

// get systems underlying platform - could be from WASM imports exposed to a WASM module.
let platform = PlatformContext::get()

// connect does all the plumbing to connect both together, basically all it's
ConfigServiceManager(ServiceURL("http://yayaya.co/api/v1/config/server"), platform).service(app);

// some business operation
impl App {
    pub fn updateConfigurationService(&self) {
        var requestChannel = self.requestChannel()
        let updateConfigRequest =
    }
}

```

## Usecase & Platforms: Events & Implementation Details

One interesting question arise, if we as we plan split the intent of an operation from it's actual implementation and execution by using requests and events as the separation barrier, how exactly should this all work in an wholesome complete manner?

We want to reduce as much of the moving pieces people need to inherently store in their heads without loosing themselves in too much details being paralized by exactly how something should work!

In all honesty, these are abstractions, but these abstractions should be as simple as possible, and work regardless of underlying platform or native details.

In my mind, Platforms are different in that they do not own events of their own but only requests, these means platform exists in the plain of not being within a boundary but are side-effects that are necessary but are not intricately within the functionality of the domain.

```rust

// PlatformErrors are very specifc to each platform else they
// always provide specific details that are unique to them.
enum PlatformError<T> {
    BadRequest(T),
    InternalErrors(T)
    NotImplemented(T),
}



trait Platform<Request> {
    handle(r: Request, channel: SendChannel) PlatformResult<()>
}

trait PlatformCoordinator {
    register(k)
}



```
