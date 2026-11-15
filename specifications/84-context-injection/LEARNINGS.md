# LEARNINGS — Context Injection

## Why the effects belong to `Ctx` and not to a global (2026-11-15)

The old design put `Env` in `foundation_core::env` and called it "the effect environment". A
host built it once and a task read it from the root context. That design worked for one
process with one configuration. It failed four other cases:

1. **Two tests in one process.** A `static` or a `OnceLock` holds one `Env`. The second test
   sees the effects of the first.
2. **Two requests in one Worker isolate.** Cloudflare reuses an isolate for many requests. A
   per-isolate `Env` is right for the time and the entropy, but a test that wants a different
   `Env` for one request cannot have it.
3. **A library inside another program.** The library chooses the effects, and the host cannot.
4. **A missing `Env`.** A global gives a panic at start-up or a fallback to the real clock.
   `Ctx` gives `EnvError::Missing` at the use site.

`Ctx` already carries the clock, the config and the RNG (spec 76). The remaining effects are
the timer, the entropy source and the ID source. They are three `Arc` fields in one struct,
stored as one typed entry. One `get` and one error, not three.

## Why `Env` does not hold the clock (2026-11-15)

The first design had `Env { clock, entropy, ids, timer }`. `Ctx` also has a clock. The DST law
is "one clock per run": the harness advances one clock and every deadline follows. Two clocks
in two places break that law, and the second one is easy to forget.

So the clock stays an engine-minimum element of `Ctx`. `Env` holds only the effects that `Ctx`
does not. Any consumer that needs the time reads `ctx.clock()`.

## Why `Timer` takes the `Ctx` as an argument (2026-11-15)

The natural shape was `ValtronTimer { ctx: Ctx }`, built once and stored in the `Env`. That is
an `Arc` cycle: `Ctx` → map → `Env` → `ValtronTimer` → `Ctx`. The map is never freed, and the
leak is invisible in a test.

So `Timer::sleep_until(&self, ctx: Ctx, deadline)` takes a clone of the caller's `Ctx`. The
`Arc` cycle is impossible, and the timer still uses the clock of the caller, which is what the
DST harness needs.

## Why one bundle and not three typed entries (2026-11-15)

`Ctx::get::<T>()` keys on `TypeId::of::<T>()`. A trait object (`Arc<dyn EntropySource>`) is
unsized, so it cannot be an entry on its own. The choices were a wrapper type per effect, or
one `Env`. One `Env` gives one `get`, one error and one place to replace a member in a test.
The cost is that a caller that needs one effect reads the whole bundle. That cost is small: it
is one `Arc` clone of a struct with three fields.

## Why the owner moved this out of spec 80 (2026-11-15)

Spec 80 is "platform prerequisites": seams that the agent RPC server and the proxy need. The
effects are not a prerequisite of one consumer. Every crate uses them. The owner direction:
"Ctx should be the main means of all of such injection... lets create a small spec (featureless)
that adds the bits into foundation_context then others get it by default."

Spec 80 now holds the two seams that are genuinely per-consumer: the transport facts and the
migration groups. It depends on this specification for the clock and the timer.

## The `IdSource` clash, and the rule it shows (2026-11-15)

Two different ideas had the name `IdSource`:

- the effect: a trait with `next_id()` → a new sortable ID;
- the sandbox policy: `enum IdSource { Random, Fixed(SandboxId) }`.

The fix keeps the name on the effect and renames the policy to `SandboxIdPolicy`. The general
rule: a name that describes an effect belongs to the effect, and a policy enum gets a name that
says what it governs.

## Dead end: a separate crate for the effects (2026-11-15)

A new crate `foundation_effects` would keep `foundation_context` small. It fails the dependency
test: `foundation_core` depends on `foundation_context`, and the `ValtronTimer` needs valtron.
The timer trait must be in a crate below `foundation_core`. `foundation_context` already sits
there, already depends on `foundation_compact`, and already holds the clock. One crate is
simpler than two.
