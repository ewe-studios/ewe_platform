# Feature 01: Reactive Structure — `<Show>` and `<For>`

Signal-driven PLACEMENT (feature 00 deliberately fixed placement at mount;
this feature makes the SET of mounted fragments reactive). The two
primitives the F5/F6 catalog families and toast lists block on.

## 1. Design principles

- **Built-ins, not macros-within-macros**: `<Show>`/`<For>` join `Fragment`
  in the reserved capitalized-tag namespace (feature 00 §6). The macro arm
  stays THIN — it validates attributes and emits ONE call into runtime
  helpers (`foundation_wasm_ui::reactive::{mount_show, mount_for}`), where
  the real logic lives, testable without the macro.
- **Recipes, not values**: structural content mounts MANY times (every
  toggle, every new list item), so content is a `Render` (feature 00) or a
  render closure — never an `Html` value that would be consumed. Static
  `Html` still works (it implements `Render` by cloning).
- **Scopes own instances**: each mounted instance lives in a fresh
  `ctx.child()` scope; unmounting = `RemoveNode` ops + dropping the scope
  (feature 03's handle-counted Context makes the ownership explicit).
- **Anchored regions**: each primitive owns an empty anchor `<span
  primal-id>` marking its region's END; mounted content is inserted before
  it (`InsertBefore`), so siblings and multiple primitives under one parent
  never interfere, and morphs preserve the anchor like any id-bearing
  element.

## 2. `<Show>`

```rust
html! { ctx, receiver,
    <section>
        <Show when={open.get()}>{ Slot::lazy(|c, r| drawer(c, r)) }</Show>
    </section>
}
```

- `when={expr}` — re-evaluated in ONE effect (signals read inside are the
  dependencies).
- The single `{child}` expression evaluates ONCE to an `impl Render +
  'static`.
- Rising edge: fresh child scope → `render(scope, rcv)` →
  `mount_tracked` before the anchor (ALL top-level ids tracked, so
  multi-root/tagless content unmounts correctly).
- Falling edge: `RemoveNode` per tracked id (unregisters implicitly) +
  drop the scope (disposes the instance's effects).
- Re-show mounts a FRESH instance (recipe semantics — state inside the
  content resets unless it lives in signals owned outside).

## 3. `<For>`

```rust
html! { ctx, receiver,
    <ul>
        <For each={todos.get()} key={|t: &Todo| t.id} render={|c, r, t: &Todo| item(c, r, t)} />
    </ul>
}
```

- `each={expr}` — re-evaluated in ONE effect, must yield `Vec<T>`.
- `key={closure}` (`Fn(&T) -> K`, `K: PartialEq + Clone`) — item identity.
- `render={closure}` (`Fn(&Context, &SharedInstructionReceiver, &T) ->
  Html`) — evaluated once per NEW key, under that item's own child scope.
- Reconciliation per effect run:
  1. removed keys → `RemoveNode` their tracked ids + drop their scopes;
  2. if the KEPT keys' relative order is unchanged (the overwhelmingly
     common append/remove/update case) → NO move ops at all;
  3. otherwise an end→start `InsertBefore` pass restores order (correct,
     occasionally over-moves — an LIS-based minimal-move pass is documented
     future work, NOT v1);
  4. new keys → mount + `InsertBefore` at their position (ref = next
     item's first id, else the anchor).
- Duplicate keys: debug-surfaced via `tracing::error!`, last wins —
  documented, not UB.
- Item CONTENT updates ride the items' own interior signals (the render
  closure receives `&T` at mount; a changed `T` with the SAME key does NOT
  re-render in v1 — keys identify INSTANCES; put changing data in signals.
  Documented loudly; `<For>` v2 may add an update hook).

## 4. Macro surface

- `<Show when={..}>{..}</Show>` — exactly one `when` attr (dynamic), exactly
  one slot child; everything else is a compile error.
- `<For each={..} key={..} render={..} />` — self-closing, exactly those
  three dynamic attrs.
- Both consume no runtime indexes (helpers allocate their own anchor ids).
- Pure form: `<Show>`/`<For>` are COMPILE ERRORS in the pure form (no
  receiver to mount through; a pure tree is a value — conditional/list
  content in pure trees is plain Rust around the macro, as it always was).

## 5. Open risk (verify first)

Mounting creates effects INSIDE a running effect (the watcher) —
`foundation_signals` must support effect creation during stabilize. The
acceptance suite's first test pins this; if it doesn't hold, the helpers
defer mounting to a post-stabilize hook (NotificationManager) instead.

## 6. Testing

Show: mount-on-true ops before the anchor; unmount removes ALL tracked ids
+ disposes scope (interior effect dead after hide); re-show = fresh
instance; static-Html content; nested Show. For: initial render order;
append/remove without move ops (op-stream asserted); reorder restores
order; per-item scope disposal on removal; interior item signals update
after reorder; duplicate-key surfacing; empty→filled→empty round trip.
