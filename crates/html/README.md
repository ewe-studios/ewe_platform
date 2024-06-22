# Html

This crate implements the core structure used to describe an html markup document and their related changes that occur, allowing us to express mutations clearly.

My goal for this crate is to create a solid foundation that other crates can use to describe or transcribe a html document into a full incode structure.

It's the underlying structure that is generated when the html_macro is used and the overall goal is that this structure should be expressable both via markup text and in code.

## Parser

A custom html parser with minilistic handling for html documents, fragments is implemented within this package, the performance is prety good.

```bash
Running benches/cwikipedia.rs (target/release/deps/cwikipedia-5a74ab1b46ae91c6)
Benchmarking wikipedia_small: Warming up for 3.0000 s
Warning: Unable to complete 100 samples in 5.0s. You may wish to increase target time to 8.5s, enable flat sampling, or reduce sample count to 50.
wikipedia_small         time:   [1.5939 ms 1.6059 ms 1.6222 ms]
                        change: [-38.582% -22.034% -8.4891%] (p = 0.04 < 0.05)
                        Performance has improved.
Found 9 outliers among 100 measurements (9.00%)
  3 (3.00%) high mild
  6 (6.00%) high severe

wikipedia_big           time:   [15.446 ms 16.096 ms 16.933 ms]
                        change: [-3.5960% +2.7005% +9.2374%] (p = 0.44 > 0.05)
                        No change in performance detected.
Found 10 outliers among 100 measurements (10.00%)
  6 (6.00%) high mild
  4 (4.00%) high severe

html_svg                time:   [31.015 µs 31.115 µs 31.229 µs]
                        change: [-13.012% -5.3290% -0.4313%] (p = 0.18 > 0.05)
                        No change in performance detected.
Found 11 outliers among 100 measurements (11.00%)
  8 (8.00%) high mild
  3 (3.00%) high severe

     Running benches/wikipedia.rs (target/release/deps/wikipedia-e2cdf26d1abd639f)

running 3 tests
test basic_svg_page  ... bench:      31,267.17 ns/iter (+/- 4,619.23)
test wikipedia_big   ... bench:  15,628,116.40 ns/iter (+/- 630,032.94)
test wikipedia_small ... bench:   1,639,379.95 ns/iter (+/- 80,538.45)

test result: ok. 0 passed; 0 failed; 0 ignored; 3 measured; 0 filtered out; finished in 7.05s
```

## Design

In this html structure, there are no string templates, just core html structures that could define specific capability and behaviour without complicated expressions of text replacement with the clarity that there is no virtual DOM but rather a representation of what we want displayed.

This means in general both in browser and on server should work the same with the same set of primitive and singular code.

What we want is simple markup that fully describes the end result that will be displayed on the page and these when on the browser through specific always present field e.g `id` or `data-hash-id` will allow clear identification of which specific node will have operations applied to.

```html
<section id="menu" data-hash-id="#a43nn3">
    <label id="menu-title" data-hash-id="#a43nn3">Gallery Loader</label>
    <remote-island from="/gallery/photos" target="section#main" fade_in="0.1ms" insert="before"/>
</section>
```

These means the underlying rendering system (Browser or Otherwise) bears the needed responsibility to take that repesentation and apply the result to it's representation.

Any non-standard elements are automatically classified as components 

My hope is such structure:

```html
<for-element path="$.children">
     <label><text-for path="$.name" /></label>
     <label><text-for path="$.age" /></label>
</for-element>
```

Where simple json/object paths make it very possible to easily and clearly articulate the definition of what is to be displayed without core simplicity and clarity.

No magic but simple clear description of output that can both be done on the server or client with ease.

### Loading In Web

I forsee a future also where the core elements provided are not enough and people would wish to provide self implemente elements that can provide additional brand focused elements that define what the output should be, a component system that really just derives to core elements or final output elements.

An interesting part is the presence of the `path=` attribute could make a specific new element tag dynamic and thereby be marked as such to help shake down the exact contents that need to be sync'ed down when a change occurs.

So I imagine that on the web, users would be able to load this type of code in two ways:

1. With the engine shell that can load wasm modules that contain relevant component implementations and definition. This allows brands to create brand specific components they can use with lightweight modules. And they can use newer versions of the core components as times is necessary and as they are updated and expanded.

```html
<head>
    <script src="https://ewe-platform.io/cdn/v2.0/shell.js"></script>
    <script type="text/javascript">
     // I am thinking to let others just load the core wasm
        // components not with the core but as an additional resource
        // to possibly allow them upgrade the core components by just
        // switching versions.
        engine.loadup("https://ewe-platform.io/cdn/v2.1/core.wasm");
        
     // Then brands can just also included, then marked with eweup=true to active and get registered by the WASM system for genereation.
        // The idea is these two will just link up via WASM capability imports
        engine.loadup("https://brand.io/cdn/v2.1/brand.wasm");
    </script>

</head>
```

2. With a single bundled javascript file that contains all the both the core module and all necesary core components and methods needed to work.  Which will internally instantiated the wasm embedded within itself usually base64.

```html
<head>
    <script src="https://ewe-platform.io/cdn/v2.0/engine.js"></script>
</head>
```

### Core Operations

Taking inspiration from HTMx and Basecamp Hotwire, there are core operations that the shell or core must support out of the box to allow consistent behaviours on both side.

1. Diffing updates - this will look at incoming changed wrapped in a <DiffUpdate/> that the wasm engines knows what it means and where its going.

2. Html Updates - this supports the HTMx update structure that sends down html as is which the underlying engine looking at the incoming html and who triggered can decide where to place the html. For this I am considering moving from just attribute definitions to actual core markup that defines what operation the markup should be but then this is limiting as it means the returned html cant be dynamically placed in a target but the target must be known before hand unless that is done at the point of definition.

HTMx attribute based approach:

```html
<section id="main"></section>

<section id="menu">
    <label>Gallery Loader</label>
    <remote-island from="/gallery/photos" target="section#main" fade_in="0.1ms" insert="before"/>
</section>
```

Html tag based approach:

```html
<section id="main"></section>

<section id="menu">
    <label>Gallery Loader</label>
    <remote-island from="/gallery/photos">
        <insert-after>
            <target query="section#main" />
        </insert-after>
        <before>
            <animation>
                <preset>
                    <height as="0px" />
                    <width as="0px" />
                    <visibility as="false" />
                </preset>
            <animation>
        </before>
        <after>
            <animation tween="in-out-elliptic">
                <sequence>
                    <fade_in duration="0.1ms" />
                    <height to="200px" />
                    <width to="400px" />
                 </sequence>
                 <after>
                    <append>
                        <script id="gallery_handler">
                           let div_script = document.querySelector("script#gallery_handler");
                           console.log("Div Script", div_script);
                           console.log("Div Script parent", div_script.parentNode);
                        </script>
                    </append>
                 </after>
            <animation>
        </after>
    </remote-island>
</section>
```

Tags have the concept of Applicators that are expressive, and can be defined serially and understood serially by looking at where they are defined and what operations they will perform.

This allows people to build a combination of applicators that when applied to the target element generates the final outcome.

This means that if we explain above html we can say:

1. <remote-island> loads up a island of content which can either be defined in the page or some http API returning html (standard htmx) that allows us to reuse the islands across the page.

3. The remote island applies specific operations to the returned content defined in <before/> and <after/> tags that can contain other Applicators that will be applied to the target element before its mounted and after it is mounted. In this case we apply an animation that applies a preset to different styles of the element setting those (height, weight and display) to a set of values that basically hides the element though mounted and then in it's <after/> animates those using a custom tween into fade in and with height and width to specific values that shows the element.

4. Secondly these means Applicators provide a before and after declarative hook that allows us hook simply apply another set of Applicators to the element.

5. This also means that each applicator allows only a certain set of applicators described by markup that are allowed within itself, this allows us syntax check and return understandable errors about what is going on and why the result is not as expected.

The nice thing about this is while the overall markup is now more verbose, it is also more clear, nothing is ihidden behind attributes to be remembered and its clear where and what the overall execution is.

### Core Parts

There are two parts to this structure:

1. The Content writers that generate the actual markup that is produced and put into the DOM to generate the expected output. This needs to soely focus on actual final form of what the DOM should look like and will be the passed to the applicators to generate the output that is written to the streams.

The markup will internally generate the final form that generates the final markup as `byte` stream.

The markup would support two forms of rendering:

### Partials

Which indicates to the Applicators you are dealing with a partial (remember the rails days) where a partial template is to be rendered.

This allows the Applicators to generate appropriate output for a partial markup rendering

```rust
Applicator.generate(Markup::partial(), encoding);
```

#### HTMX Partials

An interesting benefit of this approach is we can actually replicate the HTMx spec for partials in how we output them using htmx attributes that allows even those who prefere htmx to be able to use the html generation system

```rust
Applicator.generate(Markup::htmx(), encoding);
```

### Page

A page is a full html document with all relevant elements within that page, when this mode is used for generating markup then the Applicators know to seek the relevant content they need to use or output to will be within the source markup provided.

This allows you build a full page for the first initial render or for SEO related efforts.

```rust
Applicator.generate(Markup::page(), encoding);
```

Which means the markup must support CSS selectors in some level to make it suitable and working.

1. The actual Applicators - the structure describing the markup to be generated and what otherlying applicators to be applied to the generated markup at different levels. Each powering or providing a somewhat compiler of sorts that will generate the final result with all desired interactions and capability we expect.

```rust

Applicators::html("section").children(
    Applicators::html("label").children(
        Applicators::text("Gallery Loader"),
    ),
    Applicators::RemoteIsland("/gallery/photos").children(
        Applicators::insert_after(
            Applicators::target("section#main"),
        ),
        Applicators::before().children(
            Applicators::animation().children(
                Applicators::animations::preset().children(
                    Applicators::Styles::Visibility(false),
                    Applicators::Styles::Width("0px"),
                    Applicators::Styles::height("0px"),
                )
            )
        ),
        Applicators::after().children(
            Applicators::animation()
            .attrs(Applicator::Attrs::Tween("in-out-elliptic"))
            .children(
                Applicators::animations::sequence().children(
                    Applicators::Styles::FadeIn("0.1ms"),
                    Applicators::Styles::Width("200px"),
                    Applicators::Styles::height("200px"),
                ),
                Applicators::after().children(
                    Applicators::append_child().children(
                        Applicators::html("script")
                        .attr("id", "gallery_handler")
                        .children(
                            Applicators::text(r#"
                              let div_script = document.querySelector("script#gallery_handler");
                                 console.log("Div Script", div_script);
                                 console.log("Div Script parent", div_script.parentNode);
                            "#)
                        )
                    )
                ),
            )
        ),
        Applicators::after().children(),
    ),
)
```

The key idea here is the `Applicators` generate a result internally possibly a `Markup` which then is parsed to different Applicators to apply specific operation on, which could be an operation that relies on a root `Markup` as in the instance of the `Markup::page()` which provides a working full page DOM for interactions with.

This allows Applicators to tailor their underlying behaviour and implementation to a specific output e.g `Applicator::insert-after` will in the case of a `Markup::page()` look for the parent within the page to apply the insert after operation but in the case of a `Markup::partial()` wrap the output in a `<insert-after query="...">` root markup that the receiver understands what operation to perform on the client side for the result (same also for `Markup::htmx()` which generates html compartible markup).

In my thought process, `Applicators` should have only 1 `after` and `before` Applicators to simplify things, the user should ensure to apply whatever operations they want in those and not litter the place with multiple befores and afters.

To make this more generic and easily extendable, I feel Applicators should match a trait object that will allow extension by other users, something akin to a markup `Blueprint` that focus on 3 things.

1. The data it might use in generate
2. The markup it generates
3. The parent markup it applies its result to
4. The encoding the markup uses to serialize it's result.

```rust
trait Applicator {
    fn generate(&self, data: Option<Data>, root: Markup, encoding: Encoding);
}
```
