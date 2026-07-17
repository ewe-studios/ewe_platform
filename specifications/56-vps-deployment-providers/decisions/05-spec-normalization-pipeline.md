# 05 — Who normalizes provider specs, and where the canonical one lives

**Date:** 2026-07-17
**Status:** **Mostly resolved** (owner, 2026-07-17) — raw specs are kept under
`artefacts/cloud_providers/raw/`, and a registry maps provider → normalizer → raw file.
Two wrinkles remain (§1 where transforms live, §5 output validation).

## The proposal

Owner (2026-07-17):

> *"One of the things I always worry about is the fact we need to have a CLI
> command say in `foundation_codegentools` which will own and normalize different
> provider specs for us — maybe it takes a raw spec and runs the normalization
> steps to create the valid OpenAPI spec and store in our
> `artefacts/cloud_providers` directory?"*

Yes — and it fixes an ownership smell that is already biting.

## Why this is needed, not just tidy

**The generators require a canonical spec, and two of our three providers are not
one.** `foundation_deployment::providers::standard::normalize` states the contract
itself:

> *"The type and client generators expect a canonical OpenAPI 3.x structure:
> schemas in `components/schemas`, a `servers` array with at least one entry, and
> `$ref` pointers instead of inline schemas. Not all provider specs arrive in this
> form."*

Measured 2026-07-17:

| Provider | Shape | Canonical? |
|---|---|---|
| Linode (9.3 MB) | **0 `$ref`s** — every schema inlined per operation | **no** |
| Hetzner (3.4 MB) | **0 `$ref`s**; OpenAPI 3.1.2 nullable arrays; refs only inside discriminator `mapping` | **no** |
| DigitalOcean (3.1 MB) | 7814 `$ref`s | yes |

Our extractor derives a response's type name **from its `ref_path`**
(`ResponseType::Generated`), so a bundled spec produces **no named types at all**.
Linode and Hetzner cannot be generated usefully until something canonicalises
them.

**And the machinery exists but is owned by the wrong crate and called by nobody.**
`normalize.rs` ships `ensure_servers`, `normalize_nullable_types`,
`extract_inline_schemas` (hoists inline objects into `components/schemas` and
replaces them with `$ref`) and `path_to_type_name` — all tested, and referenced
only by their own tests. Its doc says "providers compose them in their `fetch.rs`";
nothing does (the GCP fetch path that would have has since been deleted). They sit
in **`foundation_deployment`**, a *runtime* crate, while the generator lives in
`foundation_codegentools`. Canonicalising a spec is a build-time concern.

## The shape

Two commands, one seam:

```
genapi normalize <provider>    # raw/<provider>.json -> <provider>/openapi.json + _manifest.json
genapi generate  <provider>    # canonical + selection -> src/generated/
```

`normalize` is **file → file** over the committed raw (§2, §4): apply the
provider's transforms, validate the result is canonical, write
`artefacts/cloud_providers/<provider>/openapi.json` **plus its `_manifest.json`**.
No network. `generate` then assumes canonical input and never transforms — so a
generation bug and a normalization bug are never the same bug.

Refreshing a vendor's spec is a separate, occasional act: drop a new file into
`raw/` and re-run `normalize`. That keeps every build deterministic and offline,
and makes a vendor's change show up as a **reviewable diff** in `raw/` rather than
as mystery drift in generated code.

### The manifest earns its keep

The convention already exists:

```json
{ "fetched_at": "...", "provider": "cloudflare",
  "source": "https://github.com/cloudflare/api-schemas",
  "spec_files": ["cloudflare/openapi.json"] }
```

It should also carry what this spec needs:

| Field | Why |
|---|---|
| `spec_version` (`info.version`) | what [feature 00](../features/00-selective-codegen/feature.md) §3 pins and validates — the check reads it from here rather than re-parsing 9.3 MB |
| `openapi` (e.g. `3.0.1`, `3.1.2`) | decides whether `normalize_nullable_types` was needed |
| `transforms` | which transforms ran, in order — otherwise "why does the artefact differ from the vendor's?" is unanswerable |
| `raw_sha256` | detects the vendor moving the spec under us |
| `canonical_sha256` | detects the artefact being hand-edited |

## Wrinkles to resolve

### 1. Where do the transforms live?

They are in `foundation_deployment` (runtime) and used only by its tests.
Candidates:

- **`foundation_openapi`** — its own doc already claims the job ("parse OpenAPI
  specs … and produce normalized JSON representations"), and it is where selection
  and the closure now live. The CLI in `foundation_codegentools` then just drives
  it. **Recommended.**
- `foundation_codegentools` directly — fine, but then `foundation_openapi` (the
  spec-processing library) cannot canonicalise, which is odd.

Either way `foundation_deployment` loses them. Its tests come along; nothing else
references them.

### 2. Do we keep the raw spec? — **resolved: yes, under `artefacts/cloud_providers/raw/`** (owner, 2026-07-17)

> *"We could add a `artefacts/cloud_providers/raw` for the raw ones."*

```
artefacts/cloud_providers/raw/<provider>.json        # exactly what the vendor published
artefacts/cloud_providers/<provider>/openapi.json    # canonical — what genapi generates from
artefacts/cloud_providers/<provider>/_manifest.json  # provenance + the pins
```

This is what makes normalization **reproducible and offline**: the input is
committed, so re-running a transform is deterministic, a transform bug is fixable
without re-fetching (a re-fetch may not return the same document), and "what did
the vendor actually say?" stays answerable. It also makes the transform reviewable
— the diff between `raw/` and the canonical artefact *is* what we did to their
spec.

The cost is committed bytes, and it is already the house's practice: cloudflare's
artefact is **19 MB committed today**. Linode's raw (9.3 MB) plus canonical is the
same order.

It also settles Linode's odd source: its raw spec is an owner-supplied **local git
clone**, not a URL. Vendoring the raw file into `raw/` makes the source a
provenance note in the manifest rather than a build dependency on a path outside
the repo.

### 3. Is `normalize` also `fetch`? — **resolved: no** (follows from §2/§4)

`normalize` reads the committed `raw/<provider>.json`. Fetching is a separate,
occasional act — drop a new raw file in, re-run `normalize`, review the diff.

`fetch_standard_spec(provider, url, out_dir)` already exists in
`foundation_deployment` (used by nothing but its own tests). It could become
`genapi fetch <provider>` writing into `raw/`, which would be a convenience, not a
build step. Worth doing only once a vendor URL needs refreshing often; Linode's
"source" is a local clone anyway.

### 4. Where do per-provider transform recipes live? — **resolved: a registry, pointing at the raw file** (owner, 2026-07-17)

> *"Then we can just always go update it and add a new provider and normalizer
> pointing at a raw json file which it transforms into the correct openapi json
> file."*

Adding a provider is one registry entry — name, normalizer, raw file — alongside
the existing `SPLIT_OUT_PROVIDERS` table:

```rust
// foundation_codegentools
const PROVIDER_SPECS: &[ProviderSpec] = &[
    ProviderSpec { name: "hetzner",
                   raw:  "artefacts/cloud_providers/raw/hetzner.json",
                   normalizer: Normalizer::Hetzner },   // hoist inline + 3.1 nullable
    ProviderSpec { name: "linode",
                   raw:  "artefacts/cloud_providers/raw/linode.json",
                   normalizer: Normalizer::Linode },    // hoist inline + pin apiVersion
    ProviderSpec { name: "digitalocean",
                   raw:  "artefacts/cloud_providers/raw/digitalocean.json",
                   normalizer: Normalizer::Passthrough }, // already canonical
];
```

So `genapi normalize <provider>` is file → file: read the committed raw, apply
that provider's normalizer, write the canonical artefact + manifest. No network in
the loop, and a new provider is an entry plus (if its quirks are new) a normalizer.

### 4b. A note on the existing artefacts

Today `artefacts/cloud_providers/<provider>/openapi.json` holds the **raw** vendor
spec — nothing has ever normalized one (the helpers are unused). It works for
cloudflare because that spec happens to arrive canonical already (18,727 `$ref`s,
`servers` present, OpenAPI 3.0.3).

So this is a rename in meaning: `<provider>/openapi.json` becomes *the canonical
output*, and the raw input moves to `raw/`. For providers that arrive canonical
(cloudflare, DigitalOcean) the normalizer is a pass-through and the two files are
byte-identical — worth keeping anyway, so every provider goes through one pipeline
rather than two.

### 5. Does `normalize` validate its own output?

It should — that is the cheap version of "did the transform work". Assert the
result is canonical (schemas in `components/schemas`, `servers` non-empty, no
inline object schemas left in operations) and fail loudly otherwise. Without it a
half-applied transform produces a spec that generates quietly-wrong code, which is
the exact failure mode spec-53's audit kept finding.

## To resolve

1. **Open:** transforms move to `foundation_openapi` (recommended — it is the
   spec-processing library, and where selection/closure now live) or straight into
   `foundation_codegentools`?
2. ~~Keep the raw spec?~~ — **resolved**: yes, committed under
   `artefacts/cloud_providers/raw/`.
3. ~~One `normalize` (fetch + transform) or separate?~~ — **resolved by §4**:
   `normalize` is **file → file** over the committed raw. Fetching is a separate,
   occasional act (drop a new raw file in), which keeps generation offline and
   deterministic.
4. ~~Where do recipes live?~~ — **resolved**: a registry entry per provider (name,
   normalizer, raw path).
5. **Open (confirm):** `normalize` validates its own output is canonical — schemas
   in `components/schemas`, `servers` non-empty, no inline object schemas left in
   operations — and fails loudly. Without it a half-applied transform generates
   quietly-wrong code, the exact failure mode spec-53's audit kept finding.
