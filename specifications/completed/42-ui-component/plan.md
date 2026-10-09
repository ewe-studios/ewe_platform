**IMPORTANT**: Not everything needs to be a signal, somethings might just be static set once and that is ok. Dont go signal mad.

Also i was thinking alot about children, components may have different children and forcing the children to be defined, so what if we make IntoHtml accept an optional:

```rust
pub trait HtmlSlot { 
    fn named(&self, named: String) -> Option<Html>;
    fn children(&self) -> Option<Vec<Html>>;
}

pub struct EmptySlot {}

impl HtmlSlot for EmptySlot {
    fn named(&self, named: String) -> Option<Html> {
        None
    }
    fn children(&self) -> Option<Vec<Html>> {
        None
    }
}

pub trait IntoHtml<T = EmptySlot> where T: HtmlSlot { 
    fn into_html(self, slot: Option<T>) -> Html; 
}
```

An optional type that can be supplied which implements Slots, this allows custom components that expect certain slots to be provided and they can force expectation of a giving slot with named(name).expect("i need this slot") or not include that section at all since its not available.

More complex components could have struct fields with required field slots that need to be field and used by the more complex component for internal components it initializes internally.

Since we use pure functions we can just do the following for a new shoeShelf component:

```rust

fn shoeShelf(ctx: Context, receiver: &SharedInstructionReceiver, slot: Option<HtmlSlot>) -> Html {
    let (shoes, setShoes) = ctx.signal();

    let children = slot.children().unwrap_or(vec![]);
    let menu = slot.named("menu");

    return html!{
        <section>
            <h1>{menu}</h1>
            <section>{children}</section>
        </section>
    }
}

```


Secondly, we should review the implementation of /home/darkvoid/Boxxed/@formulas/src.UIFrameworks/src.baseui/base-ui and create a complete specification in here for the different components we will build the foundation_wasm_ui way (not the react way), we learn but design them to fit our own structure and system.


This was  supposed to be part of the spec: specifications/39-foundation-wasm-ui but we moved it out cause it just made sense to be on its own, we did some initial review work of headless ui but i think we can take our time and lean more from base-ui and generate detailed features in the spec around the components (all baseui components) with the underlying change above to the IntoHtml trait and the reminder about not all things needing to be signals, to craft a set of consistent headless components for our needs.
