s that clear? And then the open point: do you accept the seven parts of the todo list?

The WebView research is done and pushed: specifications/69-foundation-ui-v2/research/14-webview-control.md. I checked its facts about our repository (the pinned Tauri versions, MainActivity : TauriActivity(), the CI steps); they are correct. I did not check its facts about Tauri and wry myself; the file gives a source for each and lists 14 claims as not confirmed.

The result, in short

Option Result
Tauri as the app shell Does not fit. On mobile, Tauri owns the app’s root view. We want the opposite: our SwiftUI and Compose screens own the root, and the WebView is one node in them.
wry used directly from our Rust core Not practical. wry says that a child WebView is not supported on Android and iOS.
Our own control, learning from Tauri and wry Recommended. A thin native control over the system WebView on each platform, with the patterns of Tauri and wry for loading, custom URL schemes and the message bridge.
foundation_platform and the Tauri examples stay as they are. We reuse their patterns, and maybe two small pieces of code.
The file has six questions for you. I will bring them one at a time after we finish the current list, because the WebView control is later work (D46).

One thing to be careful about: the Tauri and Apple documentation sites were blocked for the agent, so some statements come from source code and search summaries only. The feature for the WebView control must confirm them before any design is final.

Now back to the open item.

Item 6 of 17: A6, how an action reports an error

An action returns a Response. The source document gave it two cases: a view update and a data update. It gave no case for “the action failed”.
You rejected a Result wrapper around Response, so I did not use one.
My choice: a third case on the same type, Response::Error(ActionError). The short form is Response::failed("no stock").
The dispatcher puts it in the control part of a message (D45). The client shows or logs it.

Do you accept Response::Error(ActionError)? Or do you want a different way?

Ya its just a ErrorStack Result<Response> normal rust

Claude’s change makes actions return a plain Result<Response, ErrorTrace<ActionError>>, dropping Response::Error and sending the client only a short message while the full stack stays on the server.

D49 is recorded and pushed. Item 6 is closed. Tell me if a point above is wrong.

Item 7 of 17: A7, do we add #[remote_action]?

In the PR review you proposed a thought: two markers, #[action] and #[remote_action]. A remote action would mark a call that sends the name and the data to the other side.
After that, you added the origin prefix: server:, native: and js:. The prefix is written where the action is used, for example on:tap={server:add_todo}.
So the place is already clear from the prefix:
server:add_todo goes to the backend.
native:filter_todos runs in the local Rust or WASM code.
js:focus_field runs in the page script.
The same function can be registered on the server (it runs there) and declared on the client (the client only knows the name and sends the request). The function does not change.
My choice: no #[remote_action]. One marker, #[action], is enough. The origin prefix says where it runs.
The reason: a second marker would say the same thing as the prefix, in a second place. Two places can disagree.

Your decision: do you accept “no #[remote_action]; the origin prefix says where the action runs”? Or do you want the second marker?

Well we have tauri source code, nothing stops us from taking it and modifying it to fit ours where needed or outright copy it and make it better. So think of it as not a small copy we will later dig in and pull all the good parts that fit our needs

Understood. That decides the question from the research: we build our own, and we take from Tauri freely. I add it to D46.

Added to D46

We build our own WebView control, from the Tauri source. Nothing stops us: we take the Tauri and wry code, change it to fit our design where needed, or copy a part and make it better.
It is not a small copy. Later, we read Tauri and wry in depth and pull out every good part that fits our needs: the WebView setup on each platform, the custom URL scheme handling, the message bridge, the lifecycle handling.
The research file is the first step, not the end. research/14-webview-control.md shows that Tauri as a shell does not fit, because Tauri owns the root view on mobile. The deep study of the source comes with the WebView control feature.

One duty that comes with copying. Tauri and wry have the MIT and Apache-2.0 licences, the same pair as this repository. Those licences allow copying and changing. They ask that we keep the copyright notice for the copied parts. The repository has a NOTICE file for this. The WebView control feature will say: record each copied part and its source there.

D46 is updated and pushed (both places in the text were already correct; the check shows one line because the table row is a single line).

So the WebView question is settled: our own control, built from the Tauri and wry source, with a deep study when that feature starts.

Item 7 of 17: A7, do we add #[remote_action]?

In the PR review you proposed a thought: two markers, #[action] and #[remote_action].
After that, you added the origin prefix: server:, native: and js:. It is written where the action is used, for example on:tap={server:add_todo}.
So the prefix already says where the action runs:
server:add_todo goes to the backend.
native:filter_todos runs in the local Rust or WASM code.
js:focus_field runs in the page script.
My choice: no #[remote_action]. One marker, #[action], is enough.
The reason: a second marker would say the same thing as the prefix, in a second place. Two places can disagree.

Your decision: do you accept “no #[remote_action]; the origin prefix says where the action runs”? Or do you want the second marker?

Ya one action marker makes sense, the marker gives the manager on the platform indication of where to go

A7 is confirmed: one marker, #[action]. The origin prefix (server:, native:, js:) tells the action manager on each platform where to send the call. Item 7 is closed.

Item 8 of 17: A8, how the developer writes the scope and the origin

An action has two markers. You decided both in the PR review (D37). This item is only about how they look in code.

Marker 1: the scope. “On which platforms does this action exist?”

It is written on the function, one time:

#[action] // exists on every platform
async fn add_todo(…) -> Result<Response, …> { … }

#[action(web)] // exists on the web only
async fn pick_file(…) -> Result<Response, …> { … }

#[action(mobile)] // exists on iOS and Android only
async fn pick_file(…) -> Result<Response, …> { … }
Most actions have no scope word. They are written one time and run everywhere.
A scope word is only for an action that must be different on a platform, for example a file picker. The web form and the mobile form have the same name and live in two modules.

Marker 2: the origin. “Where does this action run?”

It is written in two places:

// a) when the app builds its registry: the registry is told where the action lives
registry.register(Origin::Server, add_todo);
registry.register(Origin::Native, filter_todos);

// b) where a template uses the action: the prefix says where to send the call
html! { <Button on:tap={server:add_todo}>"Add"</Button> }
html! { <Button on:tap={native:filter_todos}>"Open"</Button> }
html! { <button on:tap="js:focus_field">"Focus"</button> } // a page script function, web only
For server: and native:, the template names the Rust function. A wrong name does not compile.
For js:, the template has the name as text, because a page script function is not a Rust item.

Your decision:

Do you accept #[action], #[action(web)] and #[action(mobile)] for the scope?
Do you accept registry.register(Origin::Server, add_todo) to tell the registry the origin?
Do you accept on:tap={server:add_todo} and on:tap="js:focus_field" in a template?

But we can have marker 1 and it generates a struct with same function name exactly with a function that returns the origin, then register.register(add_todo) works

Yes, that is simpler. The origin is a fact about the action, so the action carries it. The registry and the template read it from the struct.

What it becomes

The marker says the origin (and the scope, when one is needed):

#[action(server)] // runs on the server; exists on every platform
async fn add_todo(…) -> Result<Response, …> { … }

#[action(native)] // runs in the local Rust or WASM code
async fn filter_todos(…) -> Result<Response, …> { … }

#[action(server, web)] // runs on the server; exists on the web only
async fn pick_file(…) -> Result<Response, …> { … }

The macro generates a struct with exactly the function’s name, with the name, the origin and the scope:

pub struct add_todo;
impl Action for add_todo {
const NAME: &'static str = "add_todo";
fn origin(&self) -> Origin { Origin::Server }
fn scope(&self) -> Scope { Scope::All }
// … the call glue
}

Then:

registry.register(add_todo); // the registry asks the struct for its origin
html! { <Button on:tap={add_todo}>"Add"</Button> } // the macro writes "server:add_todo" for you

What this gives

The origin is written one time, on the function. Not at the register call, and not in each template.
The registry cannot be told a wrong origin, and a template cannot have a wrong prefix.
The reference on the wire is the same as before: server:add_todo. The action manager of each platform reads the prefix and knows where to go.
The same struct is registered on the server and on the client. On the server, the registry runs it. On the client, the registry sees Origin::Server and sends the call to the server.
A page script function has no Rust struct. So it stays as text in a template: on:tap="js:focus_field".

Three points to confirm

The origin word in the marker: #[action(server)] and #[action(native)]. Correct?
When the marker has no origin word (#[action]), is that a compile error (“say server or native”), or does it mean server?
In a Rust template, on:tap={add_todo} with no prefix, because the struct knows its origin. Correct?

Sure, though in my mind #[action] by default just means it runs on all, no need to mark

Good. Points 1 and 3 are yes: #[action(server)] / #[action(native)] exist, and a Rust template writes on:tap={add_todo} with no prefix.

For point 2, I must be sure that I read “runs on all” correctly, because there are two things it can mean:

Reading (a): “all” is about platforms

#[action] means: this action exists on the web, on iOS and on Android. No scope word is needed.
The origin still needs a default. With this reading I would make it server.
So #[action] equals #[action(server)], on every platform.

Reading (b): “all” is also about where it runs

#[action] means: this action has no fixed place. It can run on the server, and it can run in the local Rust or WASM code.
The place comes from where the app registers it:
The app registers it in the local registry: the action manager runs it locally, with no network.
The app does not have it locally: the action manager sends the call to the server.
#[action(server)] and #[action(native)] are then only for an action that must run in one place. For example, an action that reads the database must be server.

Reading (b) fits your earlier words from the PR review: “the action might be remote and we don’t care”. But it gives the action manager one more rule: “local first, then server”.

Which reading is yours, (a) or (b)?

What i mean is: #[action] itself indicates this action can run across both:

A server: means the action is a http/rpc/ call to the server, js: means its calling js, native: just means its calling native, but the truth: js: and native: mean the same thing, can your host to execute this, so maybe host: instead.

The Scope you added is nice, cause we can just not include those functions in the client bundle, but it feels like we are using Origin the same already right ?

Yes to host:. And to your question: no, origin and scope are not the same thing, but your point removes one origin. Let me show it.

1. host: replaces js: and native:

You are right: js: and native: mean the same thing: “ask the host that I run in to execute this”. The caller does not care if the host does it in JavaScript, in WASM or in compiled Rust. So there are two origins, not three:

Origin Meaning
server: A call to the server (HTTP, RPC, or another transport)
host: A call to the local host: the page (JavaScript or WASM) on the web, the app (Rust core) on a phone

On the web, the host’s registry holds both kinds under one name space: a page script function and a WASM action. The action manager looks up the name. It does not care which kind it finds.

2. Origin and scope answer two different questions

   Question Values Used for
   Origin Where does the call go? server, host The action manager routes the call at run time
   Scope Which platform has this action? all, web, mobile The build includes it in a client bundle or leaves it out

They are not the same, because each combination is real:

Action Origin Scope
Add a todo in the database server all
Filter the list with no network host all
Focus the text field with a page script host web
Open the phone’s native file picker host mobile
A web-only upload endpoint server web

So js: was never a third origin. It is host with scope web. That is the overlap you felt, and it is now gone.

3. #[action] with no word

It means: no limit. The action can run on the server and on the host, on every platform. A word is only a limit:

#[action] // can run on both sides, every platform
#[action(server)] // only on the server
#[action(host)] // only on the host
#[action(host, web)] // only on the host, only in the web bundle

One point to confirm. For a plain #[action] that can run on both sides, the action manager needs one rule to pick. My proposal: the host first, when the host has the action in its registry; if not, the server. So an app that ships the action in its client bundle runs it locally with no network, and an app that does not ship it calls the server. Is that the rule you want?

Ah I see, cool

Good. I record it as D50, and I take “cool” as yes to the rule “the host first, then the server”. Tell me if that part is not what you meant.

D50: the action markers (replaces A8 and part of D37)

Two origins: server (a call to the server) and host (a call to the local host: the page on the web, the app on a phone). js: and native: are gone; both are host:.
Scope is a different question: which platform has the action (all, web, mobile). The build uses it to put the action in a client bundle or leave it out.
One marker carries both: #[action] has no limit. #[action(server)], #[action(host)], #[action(host, web)] add a limit.
The macro generates a struct with exactly the function’s name. It gives the name, the origin and the scope.
registry.register(add_todo): the registry reads the origin from the struct.
on:tap={add_todo} in a Rust template: the macro writes the reference. A page script function is text: on:tap="host:focus_field".
A plain #[action] runs on the host when the host’s registry has it; if not, the call goes to the server.

D50 is recorded and pushed. Item 8 is closed. The eight open questions (A1 to A8) are now all decided.

Now the design choices that the research did not make.

Item 9 of 17: how the state travels on the wire

A component has a state, for example “the list filter is open” or “this item’s row id is 17”.
The server signs the state and sends it to the client. The client keeps it and sends it back with the next action. The client never reads or changes it.
The research had two ways to send the state to the client:
Way 1: a special message that holds only the state.
Way 2: the state together with the view, in one frame.
My choice: one way only. The state is a shared UI operation with the name AttachState. It says: “put these state bytes on the root node of this component”.
table AttachState { // a member of UiOperation; every platform applies it
root_node: uint; // the component's root node
state: [ubyte]; // the signed state, as bytes
}
Why one way: your rule that a change has one form (D36), and D43.
Why it is an operation in the list: the view change and its new state are then in the same frame, in order. They cannot arrive at different times. If the frame arrives, both arrive.
Why the state is plain bytes: the client must send back exactly the bytes that it received. With bytes, the client only copies them. On the web, the bytes sit on the root element as base64url text (your decision D12). On a phone, they sit in the node record.
A server that wants to change only the state, with no view change, sends a frame with one AttachState operation.

Your decision: do you accept AttachState as the one way that the state travels, as a shared UI operation with the state as bytes?

Yes love it

Confirmed. AttachState is the one way that the state travels. Item 9 is closed.

Item 10 of 17: the syntax of a view with state in html!

The problem

The source document writes a view with state like this: html! { ctx, Some(&state), <template> }.
The current code already has a form with the same shape: html! { ctx, receiver, <template> }. It is the reactive form of spec 39: it builds the page and makes one effect for each signal.
Both are “two expressions, then a template”. The macro cannot tell which one the developer means.
The reactive form must keep working (D40: do not redesign what works).

My choice

A leading word, view, makes the difference clear:

#[action]
async fn add_todo(ctx: &Context, event: Interaction<TodoList>)
-> Result<Response, ErrorTrace<ActionError>>
{
let next = event.state.with_item(&event.input_value);

    Ok(html! { view(ctx, Some(&next)),
        <List>
            { next.items.iter().map(todo_row) }
        </List>
    }?)

}
view(ctx, Some(&next)) says: “this is the new view of the component, and next is its new state”.
The macro returns the operations for the component, with one AttachState operation for next. The template does not sign anything; the dispatcher signs the state later.
With D49, the result is a Result, so the normal ? works. A template problem at run time (for example an unknown colour token) becomes an error in the error stack.
html! { <template> } and html! { ctx, receiver, <template> } stay exactly as they are today.

Other ways I considered

Way Why I did not choose it
Keep the document’s form and change the reactive form It breaks code that works
A new macro name, for example view! { ctx, Some(&next), … } The research says the template macro is html!; a second macro name is one more thing to learn. But it is the cleanest to read

Your decision: do you accept html! { view(ctx, Some(&next)), … }? Or do you prefer a separate macro view! { ctx, Some(&next), … }, or another form?

The source document writes a view with state like this: html! { ctx, Some(&state), <template> }.
The current code already has a form with the same shape: html! { ctx, receiver, <template> }.

Why does it feel like you are fighting how to distinguish receiver from state.

But what if you just expand the macro to take a third field that can be Some or None.

Now state is clear and differentiated

You are right. I was fighting it. The simple answer is one more field.

D51: html! takes an optional third field for the state

html! { <template> } // pure form, as today
html! { ctx, receiver, <template> } // reactive form, as today
html! { ctx, receiver, Some(&state), <template> } // new: the same form, with state
html! { ctx, receiver, None, <template> } // the same as the two-field form
The macro counts the fields before the template: none, two or three. No keyword, no second macro.
The receiver stays what it is: the thing that takes the operations.
The state is the third field, Some(&state) or None. It is clear and it cannot be mixed up with the receiver.
When the state is Some, the macro also gives the receiver one AttachState operation for the root of the template. The template does not sign it; the dispatcher signs it later.

In an action it looks like this:

#[action]
async fn add_todo(ctx: &Context, event: Interaction<TodoList>)
-> Result<Response, ErrorTrace<ActionError>>
{
let next = event.state.with_item(&event.input_value);
let mut ops = ctx.receiver(); // collects the operations of this response

    html! { ctx, ops, Some(&next),
        <List>{ next.items.iter().map(todo_row) }</List>
    };

    Ok(Response::UIUpdate(ops.into_operations()))

}

One detail that I will design, not you: in an action, the receiver is a simple collector of operations for the response. In the current reactive code, the receiver queues operations for the live page. Both take operations, so the macro treats them the same. I will write the exact trait in feature 08 so both fit.

I record D51. It replaces my view(...) form (R9).

D51 is recorded and pushed. Item 10 is closed. Tell me if the example above is not what you had in mind.

Item 11 of 17: the name of the new shared crate

The research said that the shared parts go into “the shared runtime crate”, but it gave no name.
The shared parts are: the state signer, the action types and the registry, the action manager, the dispatcher.
They are written one time in Rust. The server, the WASM module and the phone’s Rust core all use them.
They cannot go in foundation_ui_traits, because that crate must stay small with no dependencies.
My choice: a new crate with the name foundation_ui_runtime.

So the new crates of this spec are:

Crate What it holds
foundation_ui_wire The FlatBuffers schema and the generated code (your decision D21)
foundation_ui_runtime The signer, the actions, the registry, the manager, the dispatcher
foundation_ui_native The Rust core for a phone app, with the bindings to Swift and Kotlin

Your decision: do you accept the name foundation_ui_runtime? Or do you want a different name? The same question for foundation_ui_native, which I also named.

Which is now an interesting thing.

I forgot the receiver takes the operation so do we even need Response ?

Seems we should just return a Result<()> instead and give the receiver() a method for state update too?

Yes. You are right, and it makes the action simpler. If the receiver takes the operations, the action has nothing left to return except “it worked” or “it failed”.

What it becomes

#[action]
async fn add_todo(ctx: &Context, event: Interaction<TodoList>)
-> Result<(), ErrorTrace<ActionError>>
{
let next = event.state.with_item(&event.input_value);
let out = ctx.receiver();

    html! { ctx, out, Some(&next),
        <List>{ next.items.iter().map(todo_row) }</List>
    };
    Ok(())

}

The receiver gets three kinds of method, one for each kind of message (D45):

Method What it takes What it becomes in the frame
The html! macro writes to it UI operations operation messages
out.state(root_node, &state) A new state for a component, with no view change An AttachState operation
out.data(target_node, data) Data for the app’s code (Arrow, a data table, MessagePack) data messages

What goes away

The Response type, with UIUpdate and DataUpdate. The receiver’s methods replace both cases.
The step “take the response and unpack it” in the dispatcher.

What stays true

The action still does not pick an encoding (D16). The receiver collects logical items. The delivery handler turns them into the frame.
The order is kept: the receiver keeps the items in the order that the action wrote them, and that is the order of the list in the frame (D45).
An error is the Err of the Result (D49). When the action returns Err, the dispatcher drops what the receiver collected and sends only the error. So a failed action leaves no half update on the client.

One more gain: the same action code works in both modes. In live mode, the receiver is the live page’s receiver. In stateless mode, it is the collector for one response. The action does not know the difference.

One point to confirm: where does the action get the receiver?

(a) From the context: ctx.receiver(). The signature stays as the source document has it: two parameters.
(b) As a third parameter: async fn add_todo(ctx: &Context, event: Interaction<TodoList>, out: &mut Receiver).

I recommend (a). Which do you want?

It already has all the operations then the caller of the action drains all the relieved updates including the state and sends it off to client

Yes, exactly. That is the flow:

The caller of the action (the dispatcher) makes a receiver and gives it to the action through the context.
The action runs. Everything it produces goes into the receiver: the UI operations from html!, the state, and data.
The action returns Ok(()) or Err(...).
On Ok, the caller drains the receiver. It takes all the updates, the state included, seals the state, builds the frame and sends it to the client.
On Err, the caller drops what the receiver holds and sends only the error.

I record it as D52. For the open point, I take (a): the action gets the receiver from the context, ctx.receiver(), so the signature keeps the two parameters of the source document. Tell me if you want it as a third parameter.

D52 is recorded and pushed.

Item 11 of 17: the names of the new crates

The research said that the shared parts go into “the shared runtime crate”, but it gave no name.
The shared parts are: the state signer, the action types and the registry, the action manager, the dispatcher, and now the receiver.
They cannot go in foundation_ui_traits, because that crate must stay small with no dependencies.
Crate What it holds Who named it
foundation_ui_wire The FlatBuffers schema and the generated code You (D21)
foundation_ui_runtime The signer, the actions, the registry, the manager, the dispatcher, the receiver Me
foundation_ui_native The Rust core for a phone app, with the bindings to Swift and Kotlin Me

Your decision: do you accept the names foundation_ui_runtime and foundation_ui_native? Or do you want different names?

One point to confirm: where does the action get the receiver?

(a) From the context: ctx.receiver(). The signature stays as the source document has it: two parameters.
(b) As a third parameter: async fn add_todo(ctx: &Context, event: Interaction<TodoList>, out: &mut Receiver).

A always

Confirmed: (a), always. The action gets the receiver from the context with ctx.receiver(), and the signature keeps two parameters. D52 already says this, so no text changes.

Item 11 of 17: the names of the new crates

Crate What it holds Who named it
foundation_ui_wire The FlatBuffers schema and the generated code You (D21)
foundation_ui_runtime The signer, the actions, the registry, the manager, the dispatcher, the receiver Me
foundation_ui_native The Rust core for a phone app, with the bindings to Swift and Kotlin Me

Do you accept the names foundation_ui_runtime and foundation_ui_native? Or do you want different names?

Sounds good

Confirmed: foundation_ui_runtime and foundation_ui_native. Item 11 is closed.

Item 12 of 17: where the 19 current web operations go

The background

The current web code has 19 operations in one Rust type, DomOp. They are HTML-shaped: create an element, set an attribute, add a class, set a style, and so on.
You decided that DomOp does not stay as a second vocabulary (D10), and that the old code retires only when the new code does the same work on the web (D27).
So each of the 19 must have a place in the new schema. If one has no place, a page that uses it stops working.

My choice, with your grouping (D42, D44)

Four of the 19 are things that every platform can do. They use the shared type, UiOperation:

Current operation Shared operation
Set text SetText
Append child AppendChild
Insert before InsertBefore
Remove child, remove node Remove

The other fifteen are things that only a browser can do. They go in WebOperation, as eleven types:

Current operation Web operation
Create element (with an HTML tag) WebElement
Create text node WebText
Set attribute, remove attribute WebAttr
Set property WebProperty
Set style WebStyle
Add class, remove class WebClass
Add event listener, remove event listener WebListener
Register node, unregister node WebRegistry
Replace node WebReplaceNode
Morph node (the HTML morph, D8) HtmlMorph
(new) a scoped script ScriptNode
A mobile app never applies these. They are in the web field, and mobile ignores that field.
The known HTML tag and attribute names keep their numbers from the current tables. A custom name travels as text.

One thing against your rule D43 that I see now. Five of my web types hold a flag inside to say “add or remove”: WebAttr (set or remove), WebClass (add or remove), WebListener (add or remove), WebRegistry (register or unregister). The reader must look at the flag. With D43, each should be its own type: WebSetAttr and WebRemoveAttr, WebAddClass and WebRemoveClass, and so on. That gives fifteen web types, one for each current operation, and no flag.

Your decision:

Do you accept this mapping: four go to the shared type, the rest are web-only?
For the pairs: one type for each operation with no flag (fifteen web types, by D43)? Or the pairs with a flag (eleven)?

Sure works

Monthly spend limit reached
The request was refused (×5)

Hi

The request was refused (×2)

I hit my usage limit while you were working, but it has reset now. Please continue from where you left off.

The request was refused
The request couldn’t be completed.
