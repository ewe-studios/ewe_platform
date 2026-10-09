// C shim over llama.cpp's C++ speculative-decoding API (common/speculative.h)
// for Multi-Token Prediction (MTP).
//
// WHY: `common_speculative_*` is a stateful C++ API tightly coupled to the
// target/draft contexts and the common sampler; bindgen cannot bind it. This
// shim runs a single-sequence MTP speculative generation loop (modelled on
// examples/speculative-simple + the server's MTP path) behind a small
// `extern "C"` surface. The implementation lives in wrapper_mtp.cpp.
//
// Two-level design, split for concurrency:
//
//   * `ewe_mtp_model` — the loaded draft (MTP head) GGUF weights. IMMUTABLE and
//     expensive (~100 MB); load it ONCE with `ewe_mtp_model_load` and share it
//     read-only across every engine. Free with `ewe_mtp_model_free`.
//
//   * `ewe_mtp` — a per-generation ENGINE: a target context, an MTP draft
//     context, the speculator, and a sampler. Cheap to build; create a FRESH
//     one per `generate()`/`stream()` from a shared target model + a shared
//     `ewe_mtp_model`. Two concurrent generations use two independent engines
//     (they must — each holds mutable KV state).
//
// The engine is STEP-DRIVEN: `ewe_mtp_init` builds it; `ewe_mtp_begin` starts a
// generation (resetting KV cache + sampler); `ewe_mtp_step` advances one
// speculative step and yields the committed text.
//
// The MTP head is a SEPARATE draft GGUF (e.g. `mtp-gemma-4-E2B-it.gguf`, arch
// Gemma4Assistant) that shares the target model's KV cache. The `ewe_mtp_model`
// must outlive every engine created from it.

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

struct llama_model;

// Opaque loaded draft (MTP head) model — immutable, shareable across engines.
typedef struct ewe_mtp_model ewe_mtp_model;

// Opaque MTP speculative ENGINE: a target context, an MTP draft context, and
// the speculator. Built fresh per generation; NOT shareable across concurrent
// generations.
typedef struct ewe_mtp ewe_mtp;

// Sampling parameters for the target model (kept minimal + stable across FFI).
typedef struct ewe_mtp_sampling {
    float    temperature;
    int32_t  top_k;
    float    top_p;
    float    repeat_penalty;
    uint32_t seed;
} ewe_mtp_sampling;

// Load the draft (MTP head) GGUF at `draft_path`. Returns NULL on failure. The
// result is immutable and may be shared across many `ewe_mtp_init` engines
// concurrently; it must outlive them all.
ewe_mtp_model * ewe_mtp_model_load(const char * draft_path);

// Free a draft model from `ewe_mtp_model_load`. NULL-safe. Must not be called
// while any engine created from it is still alive.
void ewe_mtp_model_free(ewe_mtp_model * model);

// Create a per-generation MTP engine over an already-loaded target model plus a
// shared, already-loaded draft model. `n_draft_max` is the max draft tokens
// proposed per step. Build a fresh engine per generation.
//
// Returns NULL on failure (context creation or MTP init failed). Does NOT take
// ownership of either model.
ewe_mtp * ewe_mtp_init(const struct llama_model * target_model,
                       const ewe_mtp_model *      draft_model,
                       uint32_t                   n_ctx,
                       uint32_t                   n_batch,
                       int32_t                    n_threads,
                       int32_t                    n_draft_max);

// Free an engine from `ewe_mtp_init`. NULL-safe. Does not free the draft model.
void ewe_mtp_free(ewe_mtp * h);

// Begin a new generation for `prompt` (a NUL-terminated, already chat-templated
// UTF-8 string), generating up to `n_predict` tokens. Resets the KV cache and
// (re)builds the sampler from `sampling`. Returns the prompt token count, or -1
// on error.
int32_t ewe_mtp_begin(ewe_mtp *        h,
                      const char *     prompt,
                      int32_t          n_predict,
                      ewe_mtp_sampling sampling);

// Advance one speculative step. Writes a newly heap-allocated NUL-terminated
// string with the text committed this step to `*out_piece` (free with
// `ewe_chat_string_free`; may be an empty string). The piece is valid whatever
// the return value.
//
// Returns: 1 = produced this step, more to come; 0 = generation complete (the
// final piece, if any, is in *out_piece); -1 = error.
int32_t ewe_mtp_step(ewe_mtp * h, char ** out_piece);

// Read cumulative stats for the current/last generation.
void ewe_mtp_stats(const ewe_mtp * h,
                   int32_t *       n_prompt_tokens,
                   int32_t *       n_generated,
                   int32_t *       n_drafted,
                   int32_t *       n_accepted);

#ifdef __cplusplus
}
#endif
