# Html

This crate implements the core structure used to describe an html markup document and their related changes that occur, allowing us to express content clearly, easily with sensible defaults.

The underlying goal overall is never to replace javascript with rust but to entirely agument the experience to hide away as much unecessary context for the developer that allows them focus on what they are trying to build and express. This is nice because it allows us use javascript where it shines (e.g animations, interactivity, ..etc) and web platform primitives (e.g web components) where they shine the most while still enjoying some of the benefits an ismorphic system can bring.

Another goal is also to adapt HTMX simplicity into something more robust and feature full without yet complicating the overall behaviour as currently existing frameworks have made things. We should be able to drop a js file into a page and be able to deliver what we want without complicated build step and unnecessary tooling that moves us away from our actual goal.

## Parser

A custom html UTF-8 parser with minilistic handling for html documents, fragments is implemented within this crate for internal use with the performance at a good overall level for use. It is not as featureful and covering of all HTML specs and is not meant to be such.

For a Wikipedia page of 1MB we have a worstcase performance of 16ms which is still pretty good. See the data in [./benches](./benches/) directory.

The `scraping_course` benchmark was taking from [zenrows rust-html-parser](https://www.zenrows.com/blog/rust-html-parser#benchmark)

```bash
Running benches/cwikipedia.rs (target/release/deps/cwikipedia-5a74ab1b46ae91c6)
Benchmarking wikipedia_small: Warming up for 3.0000 s

Warning: Unable to complete 100 samples in 5.0s. You may wish to increase target time to 8.2s, enable flat sampling, or reduce sample count to 50.

wikipedia_small         time:   [1.6427 ms 1.7432 ms 1.9038 ms]
                        change: [-6.0237% -0.6533% +5.7525%] (p = 0.84 > 0.05)
                        No change in performance detected.
Found 15 outliers among 100 measurements (15.00%)
  7 (7.00%) high mild
  8 (8.00%) high severe

wikipedia_big           time:   [14.746 ms 14.793 ms 14.851 ms]
                        change: [-18.891% -15.756% -12.738%] (p = 0.00 < 0.05)
                        Performance has improved.
Found 8 outliers among 100 measurements (8.00%)
  3 (3.00%) high mild
  5 (5.00%) high severe

Benchmarking scraping_course: Warming up for 3.0000 s
Warning: Unable to complete 100 samples in 5.0s. You may wish to increase target time to 5.4s, enable flat sampling, or reduce sample count to 60.
scraping_course         time:   [1.0653 ms 1.0833 ms 1.1125 ms]
                        change: [-19.062% -13.324% -6.0751%] (p = 0.00 < 0.05)
                        Performance has improved.
Found 11 outliers among 100 measurements (11.00%)
  4 (4.00%) high mild
  7 (7.00%) high severe

html_svg                time:   [30.917 µs 32.195 µs 34.766 µs]
                        change: [-0.6299% +1.5083% +5.4940%] (p = 0.52 > 0.05)
                        No change in performance detected.
Found 11 outliers among 100 measurements (11.00%)
  2 (2.00%) high mild
  9 (9.00%) high severe

     Running benches/wikipedia.rs (target/release/deps/wikipedia-e2cdf26d1abd639f)

running 4 tests
test basic_svg_page       ... bench:      30,655.36 ns/iter (+/- 966.09)
test scraping_course_page ... bench:   1,057,756.70 ns/iter (+/- 4,039.07)
test wikipedia_big        ... bench:  14,556,532.50 ns/iter (+/- 534,594.89)
test wikipedia_small      ... bench:   1,576,857.70 ns/iter (+/- 18,745.62)

test result: ok. 0 passed; 0 failed; 0 ignored; 4 measured; 0 filtered out; finished in 7.41s
```

## Design

In this html structure, there are no string templates, just core html structures that could define specific capability and behaviour without complicated expressions of text replacement with the clarity that there is no virtual DOM but rather a representation of what we want displayed.

This means in general both in browser and on server should work the same with the same set of primitive and singular code.

### Islands and RemoteIslands

In this architecture areas that are meant to be interactive and managed by the framework are always wrapped with an `<Island></Island>` which allows the library either on the server or client to identify specific areas that should be activated for interaction and rendering.

Secondly any area where a `<RemoteIsland/>` occurs as well is seen as a spot where whatever the remote island refers to will be rendered.

Secondly and most importantly, you can think of the concept of an Island and a RemoteIsland as diseappearing tags, i.e when activated they render natural HTML elements that might have super powers but are still no less DOM elements which might be your typical HTML elements or a CustomElement.

They may also include script tags within the generated HTML that might help fullfill the interactivity of the behaviour that should occur client side or mark elements as things the router needs to handle, more on those in below section.

### Goal Visualization

What we want is simple markup that fully describes the end result that will be displayed on the page and these when on the browser through specific always present field e.g `id` or `data-hash-id` will allow clear identification of which specific elements will have operations applied to.

```html
<section id="menu" data-hash-id="#a43nn3">
    <label id="menu-title" data-hash-id="#a43nn3">Gallery Loader</label>
    <remote-island from="/gallery/photos" fade_in="0.1ms" insert="before"/>
</section>
```

These means the underlying rendering system (Browser or Otherwise) bears the needed responsibility to take that repesentation and apply the result to it's representation.

Any non-standard elements are automatically classified as components and online most these components exists as shells on the DOM side.

All components owned/implemented by this project are shells that simply hook into the WASM core which is synchronous and never makes progress unless called with the supplied current state for a given portion (usually we desribe a portion as an `Island`) and receive their next state as pure compiled html with additional metadata (e.g JSON) which the dom can send back for future calls (in some sense allowing the wasm to be stateless in some sense) .

### Loading In Web

I remember simpler days when just adding a script that pointing to a javascript file was all you needed to get things working, wilst module loading has improved alot and I also was one of those chasing a means to load modules back in the day, it seems that simplicity is lost with alot of code splitting, code loading paradigms around.

There was something simple in simply being able to include a small runtime in your broweser and letting the content of the page do the needed work, so we return to that, a simple generated javascript bundle that can contain the WASM runtime fully and instantiate it or stream it from a remote endpoint to reduce initial load size.

```html
<head>
    <script src="https://ewe-platform.io/cdn/v2.0/ewe-platform.js"></script>

</head>
```

This keeps things simple, we can use whatever frameworks, and libraries underneath with whatever packager to generate the final output but it's use should be simple and easy.

### Data Access and Retrieval

As described, even things like getting data in a world where standard interpolation template markers like `{{.name}}` do not exists means we are forced to reduce our expectation to be articulated via the same DOM structures as other htmls and if we take inspiration from [JSONPaths](https://crates.io/crates/jsonpath-rust).

This will be unique only special elements will respect the use of JSONPath, we are not signing up for creating a markup template language, what we want is just a clear way we can express data to be retrieved through special islands that are purpose built for those things. eg `span-for` which generates a span and retreives data from the underlying `Data` object supplied to it.

```html
<for-element path="$.children">
     <label><text-for path="$.name" /></label>
     <label><span-for path="$.age" /></label>
</for-element>
```

### Data Fetching

As WASM supports imports (capability provided by the hosts) - the WASM runtime will simply expect a standard `Request` and `Response` based API which it can use to make the necessary data fetching requests to whatever remote hosts its allowed to do so to fetch external data to generate the needed final dom expression.

This is necessary as we will concept of `RemoteIslands` that could point to some remote endpoint returning standard HTML output (pre-rendered), which might be from the same WASM runtime running on the server or a api following HTMX pattern.

#### Remote Island

Are a way to define remote content that needs to be rendered in a webpage both once or multiple times across different section. This content can be another defined `IslandTemplate` (previously called `DescribedIsland`) in the page or a remote endpoint that returns HTML (this is important because an Island should just render out content into the page with no extra unnecessary jobs - that is do one thing well).

So simple to say, remote island will let you refer to some island of content that could be static or dynamic and need to be placed into a location within the page either where it's defined or to some target of your choosing.

##### RemoteIsland that loads html from a remote endpoint

```html
<remote-island api="/gallery/photos" />
```

##### RemoteIsland targeting a IslandTemplate (local to the page)

```html
<header>
    <remote-island from="title-island" />
</header>


<island-template as="feature-island">
    <section id="feature-intro" class="full-width color-white-0 type-helvetica">
            <h1 class="text-header">
                <span class="no-wrap color-white-0">Updates</span>
        <span class="do-wrap color-white-0">Featuresm, fixes & Improvements.</span>
            </h1>
    </section>
</island-template>
```

Remote islands should be able to scope down the data they indicate is necesary to be used for the request via JSON Path which can be supplied client side if the router and data is client side or indicative to the server side what data to pull from. This might require more fleshing out on exactly what or how that works especially on the server side and where the data comes from since unlike an Island the data is not attached or kept somewhere.

```html
<!-- Where the $.order.details was preloaded somewhere and cached for use -->
<remote-island from="customer-island" with-data="{id: '100'}" />

<island-template as="cutomer-island">
    <with-data from="$">
         <label><span-for path="$.age"/></label>
    </with-data>
</island-template>
```

This means there needs to be a way for the client to describe the data it wants ready for for that page - but even better will be for each individual Islands can also just in it's definition define the data it wants loaded for it's readiness  via `<fetch-data />` and this seems nicer because the definition is now local to the island.

```html
<island as="customer_order_card" >
     <!-- fetch-data defines the relevative points -->
    <fetch-data endpoint="/api/v1/get/customer-data/{$.page.customer_id}" as="$.order" />

    <with-data from="$">
         <label><span-for path="$.order.customer.name" /></label>
    </with-data>
</island-template>
```

Underneath there will be a sender router that takes all these fetches and ensures we do not have a stampeed of network requests or duplicate requests to the same endpoint to minimize resource wastage and manage overall network load better.

And the island is then rendered once it's data is ready and is fully matching whatever specification it expects.

#### Data - JSON Data APIs

No page is ever useful if it can't get data from remote endpoint to be rendered on the page. The `api-endpoint` is the core of these operation allowing you to specify a remote endpoint within the CORS limitation as allowed by the browser to fetch json data you wish to directly render on the page though not as limited on the server since you can make any request you wish.

The niceness of this is this tag would work both client and server side has the means of fetching the data (fetch API in the browser  and a router in the server)
will be supplied to the WASM runtime for it's needs to fetch endpoints (local or remote) to retrieve data for that given island or from it's remote-island directive.

#### Data - Contextaul Data fetchers

These is defined by a `<with-data></with-data>` tag (to be defined) which allows you focus down either pre-fetched data fetched from a remote endpoint via the `<api-endpoint />` which allow retreiving data (JSON data, maybe others formats as well) and specifically drill down via JSON Paths into the data allowing any sub-elements with in the `with-data` node to work from that reduced context.

```html
<api-endpoint typep="application/json" api="/v1/latest-feature-set">
     <!-- with-data lets you drill down into the data even more to render specific details -->
    <with-data from="$.customer">
         <label><span-for path="$.age" /></label>
    </with-data>
    <with-data from="$.order">
         <label>Order: <span-for path="$.number" /></label>
         <label>Date: <span-for path="$.date" /></label>
         <label>Location: <span-for path="$.location" /></label>
    </with-data>
</api-endpoint>
```

### Content Definition

Has already alluded to, every content rendered that is not static or that needs to be reusable and addressable is housed within an `<island></island>` or a `<island-template></island-template>` tag defintion.

Where each has a purpose and behaves to generate the final html output with whatever js code to activate the needed behaviour.

#### Island

`island` defines in markup a location area that is considered dynamic and to be rendered based on either local data or remote data applied to the output definition it holds.

```html
<island on-demand=yes signature="0x34343">
    <api-endpoint api="/v1/latest-feature-set">
        <FeatureBox 
                title={data("title")} 
                description={data("description")} 
                points={data("features", "summaries", "points")} 
        />
    </api-endpoint>
</island>
```

More so, I envision Islands to be able to define local to themselves some underlying data to be fetched for their necessary rendering either via `api-endpoint` or `fetch-data` targeting a remote host endpoint to load data from and combined with `with-data` to localize the specific data to a inner subset.

Naturally such requests would have been pre-identified, called when first seen and cached for re-use.

```html
<island on-demand=yes signature="0x343p43">
    <api-endpoint api="/v1/latest-feature-set">
        <FeatureBox 
            title="$.title"
            description="$.description"
        />
    </api-endpoint>
</island>
```

```html
<island on-demand=yes signature="0x343p43">
    <fetch-data endpoint="/api/v1/latest-feature-set" as="$.feature_set" />

    <with-data from="$.feature_set">
        <FeatureBox 
            title="$.title"
            description="$.description"
        />
    </api-endpoint>
</island>
```

##### Proposal

Taking some inspiration from the idea of `Suspense` in react/solidjs, we can also add that Islands can define `resources` (specific resource items they want made available for a given island to work) and state markers and what should be shown in such states via the `loading` and `failed-rendering|failed-loading|failed-signature` markup local only to island and define what it should show in each of those scenarios.

```html
<island on-demand=yes signature="0x343p43">
    <!-- request specific resouces to only be loaded when island is to be rendered -->
    <resource src="http://greaterworld.components.io/feature_box.js" checksum="0x800..." />
    <resource src="http://myapp.io/assets/static/chat-app.wasm" checksum="0x30..." />
    <resource src="http://myapp.io/assets/static/api.wasm" checksum="0x610..." />


    <!-- state definiton for the island -->
    <loading>Loading.....</loading>
    <failed-loading>Failed to load depencies, please investigate</error-loading>
    <failed-rendering>Failed to render island correctly</error-rendering>
    <failed-signature>Failed to verify signature of island or its content</error-signature>

    <api-endpoint api="/v1/latest-feature-set">
        <FeatureBox 
            title="$.title"
            description="$.description"
        />
    </api-endpoint>
</island>
```

#### Island Templates

When client side, `island-template` are a way to instead define the island in one place (either at the top or bottom of the body element) - they are processed but never rendered and are invisible to the user but exists in the DOM to allow you define reusable blocks of renderable content based on it's `as` attribute which can be refered to from `RemoteIsland`.

The reason his to allow more dry setup by developers on content that can be re-used across different pages (think partials from ruby on rails) that can be included in a page when using full client based rendering or just the content of API response via HTMX pattern of a endpoint returning HTML.

```html
<remote-island from="customer-island" />

<island-template as="cutomer-island" >
     <label><span-for text="20" /></label>
</island-template>
```

##### On Client

When these are declared on the client they simply will be pre-registered by the WASM runtime (compiled, generated an optimized representation it can reuse when called to re-render) and when the page at any point in time calls the `RemoteIsland` with the name of the `IslandTemplate` will generate the needed rendered output which then will be placed in the DOM by the rendering engine.

##### On Server

They are no different from any partials and simply are just called with necessary arguments (data, ..etc) to generate the final output that can be included directly in the full page being requested or as a html partial (e.g from a endpoint returning HTML like HTMX).

### Components

These are any element that powers either some behaviour, conditional rendering or displayed data that is not your standard HTML, they are built both on browser standards like (WebComponent) but also exists to be isomorphic - in that they work either in the browser or server.

Secondly because we do not have templates syntax (i.e `{{...}}`) and everything that defines whats to be rendered are defined within the mark up itself, this means there are core components that come with the runtime (`e.g for-data, if-data, span-for` ...etc) that inately work and perform operations on the data yield their internal representation as the output when rendered.

#### How does it work

In my mind, a component is simply itself an island of html content that behaves in a certain way and outputs to a certain specific representation in the DOM.

This means that things like `for-data`, `if-data`, `span-for` are simply island of content that expand out to their internal children as output with some behaviours attached.

But also it also means there is no `<component/>` tag anywhere to be found but just a registery either in the runtime or through on other WASM or javascript loaded to the page that provides the necessary hook to the WASM runtime about what to call when it sees such a component with a given name (this allows companies to build special functionality tags that meet their needs, potentially branded and keep the underlying defined components in the runtime very small).

In my mind, using a rust trait we could define components as a concept I call an `Island`:

```rust

trait Island {
    fn render(&self, 
        router: Router, 
        request: Request<RequestType>
        data: Option<Data>, 
        content: Option<Fragment>, 
    ) -> Result<Fragment, Error>;
}
```

Where it receives:

##### Data

Data defines some core data loaded by the wasm runtime into a specific context store that it then can use to access either the whole or predefined set of data it needs.

##### Fragment

Fragment would be markup contents that the island would use has children or be definitions of other islands that enhances or add interactive functionality to it's owner.

This allows us supply custom content to an island even if those contents are other islands themselves that get resolved.

#### DOM Container

DOM container would be the supplied entrypoint where the relevant content the island generates will be loaded into. This is always passed by the caller of the `Island` which could be other `Island` and allows us the capability to nest easily which is a first class need when you can have islands that are conditional in their output.

The containers really come in two forms:

##### DocumentFragment DOM

Which is a `DOM` container both server side and client side (think `<document-fragment>`) that can contain output that will be placed however the owner deems fit. I imagine this to be the standard use especially for components that generate output based on operations, think `for-data`, `if` ..etc.

This allows the content in them once received to be placed either in some target on the client by specifying targets as attributes like HTMX which then lets the rendering engine figure out how to place that.

This is interestingly beneficial as the server can simply slot the content of this fragment into place before sending down the final page, or returning the contents as simple HTML partial from a HTTP endpoint (just like HTMX) but even more so is that the client side sees this as basic HTML it can using the attributes (or however we define it's intended target location) slot the element in place regardless.

Which keeps as much unnecessary concerns out of the islands and fully on the rendering layer regardless of where that is.

This means the DOM can just relying entirely on HTMX as is but still work and not care how its done.

##### Page DOM

Which is a `DOM` container for a full page to be rendered and will handle the necessary slotting of the provided content into it's destination within the page.

This is nice because it fits the standard HTML webpage concept and works both server or client side, where on the server you are dealing with a virtual page that collects all necessary outputs from all different islands to generate the final page and all it's necessary scripts and content that meets the intended goal.

And as is our gaol, the page and it's relevant `Islands` will generate whatever javascript or use whatever javascript library's it takes to get a specific behaviour to work when the page is rendered as needed.

Once again, our goal is not to replace the javascript with rust entirely but to allow the rust developer enjoy building web content and letting the browsrer js shine where sensible.

Back to components, this means we can define components as `Islands` that themselves can be called by other Islands to generate the final repesentation which allows us to naturally compose them to create even more complex interactive content.

#### DOM Render Targeting

Which brings the question of how do these content describe where they wish to be rendered for which we do not necessary need to re-invent the wheel as HTMX and even other frameworks like Hotwire simply blend the notion that content can describe through attributes how they wish to be rendered where and in what way. See [HTMX](https://htmx.org/).

```html
<section id="main"></section>

<section id="menu">
    <label>Gallery Loader</label>
    <remote-island from="/gallery/photos" target="section#main" fade_in="0.1ms" insert="before" with_ref="data-id=gallery_handler"/>
    <script type="text/javascript">
       let div_script = document.querySelector("data-id[gallery_handler]");
       console.log("Div Script", div_script);
       console.log("Div Script parent", div_script.parentNode);
    </script>
</section>
```

##### Target specifications

Generally the core main concept for target specification are:

1. Where-ever an island is defined is where it renders itself into, officially disappearing from the markup.

2. If you specifiy a `RemoteIsland` in a location then it renders itself and disappears in that location.

Now this might be tough especially if say some element needs interaction that is to generate further HTML element in the case of say a hidden menu list that is produced JIT (just in time) has the element is interacted with.

We support this by following HTMX principles, the imaginery menu links triggers a request to the server or WASM runtime (in the case of client only rendering) that talks to the main router to trigger the request of the menu options and will be rendered base on whatever `Operational` elements the markup is wrapped with.

This can be rendered `before`, `insert-into`, `after`, `swap-out` and `swap-parent` directives each being self explanatory.

```html
<append target="body">
    <ul>
        <li routable api="/v1/gallery/models">
            Models
        </li>
    </ul>
</append>
```

To simplify these operation components, they can only use either a (`body`, `head`) selector or `tag#id` selector as their target, this seems limiting but it's really great has it simplifies the things we need to support.

At the extreme we will support spaced combination of invocation that specifically indicate the path to walk to reach a giving target node e.g `div#bud div#menu section#items` which will be space separated markers.

###### Supported Operational Elements

- `<before></before>`: Informs the router to place the content before the intended target (defined in it's attribute.

- `<after></after>`: Informs the router to place content after it's target

-`<swap-out></swap-out>`: Informs the router to swap out the target or the trigger of the routers call with the content essentially replacing content.

- `<swap-parent></swap-parent>`: Informs the router to swap the target's parent innerHTML with the content where by replace the target or the point where  the request was triggered if no target with the new content.

The browers own router will typing see these and fire the necessary operation to extract the content in these `Operational` elements and place the content in the correct place.

```html
<menu>
    <remote-island from="/gallery/photos-menu" />
</menu>
```

Imagine if the remote island where to return these content menu shown in a modal:

```html
<append target="body">
    <modal>
        <menu>
            <li routable api="/v1/gallery/models">
                Models
            </li>
            <li routable api="/v1/gallery/locations">
                Locations
            </li>
            <li routable api="/v1/gallery/help">
                Help
            </li>
            <li routable api="/v1/gallery/Blog">
                Blog
            </li>
        </menu>
    </modal>
</append>
```

Aside: The modal can be a CustomElement know to the html page) but can also just be an actual modal with `<div></div>`, but here we are trying to be brief, so we represent it as `<modal></modal>`

As explained, such returned content should come with `Operational` elements that are explicitly indicating where things should be placed.

#### Reactivity and Data

No website is ever useful without data and reaction to data changes. This part is the one that requires the most thoughtful consideration in how it works and if done wrong can remove the simplicity we enjoy from the already defined behaviour we want.

In my mind there are two ways things should work by the nature of our expectation that this system needs to be isomorphic and is able to work on the server or client.

##### Server Data

Server data is not some unique concept, it simply means data that the page receives when rendered on the server, in truth I do not think we need reactivity here, and the primitives we already provie for data access and retrieve (see [Data Access And Retrieval](#data-access-and-retrieval)) would suffice here.

The server will pull whatever data the fragment needs when using `RemoteIslands` that pull from the server as the server fully understands what that Island describes and generates the final output that the island swaps into its location as necessary.

This can mean a remote island never disappears but is always there to be retriggered again for example showing a modal from the server about some content when some other is clicked.

```html
<section menu>
<ul>
    <li>
        <span-for text="About Us">
            <on-click>
                <Modal>
                    <RemoteIsland api="/views/about-us">
                </Modal>
            </on-click>
        </span-for>
    <li>
</ul>
</section>
```

Where the above could be a modal that should be shown when the `About Us` menu item is clicked and within it's click handler which we can and should I think but not mandatory express as markup using the `on-click` will relevantly trigger the `Modal` to do it's work to generate it's modal that nests a `RemoteIsland` that contains the content we need rendered in the modal.

This best explains the concept that islands may not always disappear when their content is swapped in and might still exist in page when they occur and can be interacted with.

This does not necessary mean the `Island` will stay the way it's defined in markup, the developer could well from the island's implementation generate pure html and javascript that does the exact same thing.

The general idea is you are free to define what these tags become like Svelte, they can simply disappear and become normal markup that does exactly what they define for server only rendered content or be a mix of server and client working in sync where the same markup that operates in the server can be shipped to the client and still behave as expected.

I think this is the benefit of the WASM runtime because where-ever it is used and what condition it is used, it should simply be able to generate the final output that is needed.

#### Client Side Data

No truly interactive framework can live without allowing it's users to use data in the client, and I think for this both on the rust side and on the server, the components core definition should be split into two pieces:

- The markup that defines the visual
- The hidden backend that embodies the logic we care about

We can use a counter as a great way to explore this: if we have a dom structure like below to define a counter that can increment and decrement some object value.

```html
<section id=main>
    <island>
         <Counter data={value: 0} />
    </island>
</section>
```

and the content generated from this could be something like from a mental mode perspective:

```html
<div id="counter-app-1" with-data="{data: { value: 0}, hash: ..}">
    <span-for path="$.data.value"/>
    <span text="+" class="plus-button">
        <a href="/Counter/increment/" routable data-at="#counter-app-1"></a>
    </span>
    <span text="+" class="minus-button">
        <a href="/Counter/decrement/" routable data-at="#counter-app-1"></a>
    </span>
    </span>
</div>
```

Note that the use of the </a> here is used to be as explicit as possible, generally there would be a suitable island called `<Link/>` that takes care of these tables for you, but to be clear, we can also just have a plain anchor tag that is mark as `routable` and thereby is owned by the client side router and handles request delivery and response update from the server endpoint.

This brings an interesting question of how the routes came into being, seems like they just appeared out of no where as we never spoke of a router and to be honest it did.

I never envisioned a router directly but when thinking of the problem and regardless there will be a needed router of sorts though this style actually uses the browser API as is and there by takes advantages of the page like nature without requiring a client side SPA like experience, still benefits us in being able to have a central backbone that can do the following:

1. Parrallelize data retrieval (on the client side from remote endpoint and on the server side by talking to the API router)
2. Deduplicate requests for the same data allowing us to efficiently handle say multiple RemoteIslands that render the same content

We can introduce a Router that works both client and server side that can register functionality with say the following rust code as a possible universe:

```rust

struct CounterValue {
    value: usize
}

impl Counter {

    pub fn decrement(state: CounterValue) -> CounterValue {
        let mut clone = state.clone();
        clone.value -= 1;
        clone
    }

    pub fn increment(state: CounterValue) -> CounterValue {
        let mut clone = state.clone();
        clone.value += 1;
        clone
    }
}

enum CounterRequest {
    #[route("/counter/increment")]
    Increment,
    
    #[route("/counter/decrement")] // a possible alternative to the `FromRoute` trait??
    Decrement
}

/// FromRoute is required by the Router to correctly map the
/// request type for a giving route and should be implemented by 
/// request enum.
impl FromRoute CounterRequest {
    fn from<'a>(route: &'a str) -> Result<Self, anyhow::Error> {
        match route {
          "/counter/increment" => Ok(CounterRequest::Increment),
          "/counter/decrement" => Ok(CounterRequest::Increment),
          _ => Err(anyhow::Err("unknown route"))
        }
    }
}

#[island]
#[router(
    ("/counter/increment", Request<CounterRequest>),
    ("/counter/decrement", Request<CounterRequest>),
)]
async fn counter(
    router: Router, 
    request: Request<CounterRequest>
    data: Data<CounterValue>>, 
    content: Option<Fragment>, 
    root: DOM,
) -> Result<Fragment, Error> {
    let instance_id = format!("counter-{}", data.id);
    let counter = match data.state {
        Some(last_state) => Counter::new(last_state),
        None => CounterValue::new(0),
    };
    
    counter = match request.data() {
        CounterRequest::Increment => counter.increment(),
        CounterRequest::Decrement => counter.decrement(),
    };
    
    // I imagine we could use the router this way to get another island
    let clock_fragment = router.get_island("/fragements/clock").await.unwrap();
    
    // but we can also just get data from a json API endpoint
    let user_data = router.get("/v1/users/1").await.unwrap();

    return html!{
        <swap-out target_id={{{instance_id.clone()}}}>
            <div id={{instance_id.clone()}} with_data={{data.to_json(initial)}}>
                <label>user:</label><span_for text={{{user_data.name}}} />
                <label>count:</label><span_for text={{{counter.value}}} />
                <link_for 
                    data_on={{instance_id}}} 
                    route="/counter/increment" 
                    with_content="+"
                />
                <link_for 
                    data_on={{instance_id}} 
                    route="/counter/decrement" 
                    with_content="-"
                />
                <children_for with={{{clock_fragment}}} />
                <children_for with={{{content}}} />
            </div>
        </swap-out>
    }
}

```

Another interesting idea that I like is to keep the same markup we would do with `island-template` for the server side as well, because we can generate the contents either at build time of the rust binary or generate JIT when the endpoint is triggered.

### Data Security

Similar to Livewire, any data sent down and the relevant state of an interactive island will be hashed and have attached a signature which allows us perform tamper checks on incoming state from the frontend if such was tampered with.

This secret would generally be housed outside the WASM runtime and any code in the frontend but be exclusively in the server.

Where applicable, this can also be turned off when not needed for a whole app.
