# Html

This crate implements the core structure used to describe an html markup document and their related changes that occur, allowing us to express mutations clearly.

My goal for this crate is to create a solid foundation that other crates can use to describe or transcribe a html document into a full incode structure.

It's the underlying structure that is generated when the html_macro is used and the overall goal is that this structure should be expressable both via markup text and in code.

## Design

In this html structure, there are no string templates, just core html structures that could define specific capability and behaviour without complicated expressions of text replacement without a loss in simplicity and completedness.

My hope is such structure:

```html
<for-element path="$.children">
     <label><text-for path="$.name" /></label>
     <label><text-for path="$.age" /></label>
</for-element>
```

Where simple json/object paths make it very possible to easily and clearly articulate the definition of what is to be displayed without core simplicity and clarity.

No magic but simple clear description of output that can both be done on the server or client with ease.

I forsee a future also where the core elements provided are not enough and people would wish to provide self implemente elements that can provide additional brand focused elements that define what the output should be, a component system that really just derives to core elements or final output elements.

So I imagine that on the web, users would be able to load this type of code in two ways:

1. With the engine shell that can load wasm modules that contain relevant component implementations and definition. This allows brands to create brand specific components they can use with lightweight modules. And they can use newer versions of the core components as times is necessary and as they are updated and expanded.

```html

<head>
    <script src="https://ewe-platform.io/cdn/v2.0/engine_shell.js"></script>
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

1. With a single bundled javascript file that contains all the both the core module and all necesary core components and methods needed to work.  Which will internally instantiated the wasm embedded within itself usually base64.

```html

<head>
 <!-- 
        the engine.js  can be initialized in two ways:
        
        1. With the engine shell that can load wasm modules that contain
        	relevant component implementations and definition. 
            This allows brands to create brand specific components they can use with lightweight modules. And they can use newer versions of the core components as times is necessary
            and as they are updated and expanded.
        
        2. With a single bundled javascript file that contains all the necessary 
    -->
    <script src="https://ewe-platform.io/cdn/v2.0/engine_shell.js"></script>
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

 <!-- 
        2. With a single bundled javascript file that contains all the both the core module and all necesary core components and methods needed to work.  Which will internally instantiated
        the wasm embedded within itself usually base64.
    -->
    <script src="https://ewe-platform.io/cdn/v2.0/engine.js"></script>

</head>

```
