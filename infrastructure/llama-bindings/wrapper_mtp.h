// C shim over llama.cpp's C++ speculative-decoding API (common/speculative.h)
// for Multi-Token Prediction (MTP).
//
// WHY: `common_speculative_*` is a stateful C++ API tightly coupled to the
// target/draft contexts and the common sampler; bindgen cannot bind it. This
// shim runs a single-sequence MTP speculative generation loop (modelled on
// examples/speculative-simple + the server's MTP path) behind a small
// `extern "C"` surface. The implementation lives in wrapper_mtp.cpp.
//
// The generator is STEP-DRIVEN: `ewe_mtp_init` builds the reusable engine (the
// two contexts + speculator — the expensive part) ONCE; `ewe_mtp_begin` starts
// a generation (resetting KV cache + sampler); `ewe_mtp_step` advances one
// speculative step and yields the committed text. This lets a caller reuse one
// engine across many generations (no per-call draft reload) and stream token
// pieces as they are produced.
//
// The MTP head is a SEPARATE draft GGUF (e.g. `mtp-gemma-4-E2B-it.gguf`, arch
// Gemma4Assistant) that shares the target model's KV cache.

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

struct llama_model;

// Opaque MTP speculative generator: owns a target context, an MTP draft
// context, and the speculator (all reusable across generations).
typedef struct ewe_mtp ewe_mtp;

// Sampling parameters for the target model (kept minimal + stable across FFI).
typedef struct ewe_mtp_sampling {
    float    temperature;
    int32_t  top_k;
    float    top_p;
    float    repeat_penalty;
    uint32_t seed;
} ewe_mtp_sampling;

// Create an MTP speculative engine over an already-loaded target model plus a
// draft (MTP head) GGUF at `draft_path`. `n_draft_max` is the max draft tokens
// proposed per step. Reusable across many `ewe_mtp_begin`/`ewe_mtp_step`
// generations.
//
// Returns NULL on failure (draft load failed, context creation failed, or MTP
// init failed).
ewe_mtp * ewe_mtp_init(const struct llama_model * target_model,
                       const char *               draft_path,
                       uint32_t                   n_ctx,
                       uint32_t                   n_batch,
                       int32_t                    n_threads,
                       int32_t                    n_draft_max);

// Free an engine from `ewe_mtp_init`. NULL-safe.
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
