// C shim over llama.cpp's C++ speculative-decoding API (common/speculative.h)
// for Multi-Token Prediction (MTP).
//
// WHY: `common_speculative_*` is a stateful C++ API tightly coupled to the
// target/draft contexts and the common sampler; bindgen cannot bind it. This
// shim runs a self-contained single-sequence MTP speculative generation loop
// (mirroring examples/speculative-simple + the server's MTP path) behind a
// small `extern "C"` surface. The implementation lives in wrapper_mtp.cpp.
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
// context, the speculator, and a sampler.
typedef struct ewe_mtp ewe_mtp;

// Sampling parameters for the target model (kept minimal + stable across FFI).
typedef struct ewe_mtp_sampling {
    float   temperature;
    int32_t top_k;
    float   top_p;
    float   repeat_penalty;
    uint32_t seed;
} ewe_mtp_sampling;

// Create an MTP speculative generator over an already-loaded target model plus
// a draft (MTP head) GGUF at `draft_path`. `n_draft_max` is the max draft
// tokens proposed per step.
//
// Returns NULL on failure (draft load failed, context creation failed, the
// context does not support the required KV operations, or MTP init failed).
ewe_mtp * ewe_mtp_init(const struct llama_model * target_model,
                       const char *               draft_path,
                       uint32_t                   n_ctx,
                       uint32_t                   n_batch,
                       int32_t                    n_threads,
                       int32_t                    n_draft_max,
                       ewe_mtp_sampling           sampling);

// Free a generator from `ewe_mtp_init`. NULL-safe.
void ewe_mtp_free(ewe_mtp * h);

// Run MTP speculative generation for a single prompt.
//
// `prompt` is a NUL-terminated UTF-8 string (already chat-templated). Generates
// up to `n_predict` tokens. Returns a newly heap-allocated NUL-terminated
// string with the generated text (free with `ewe_chat_string_free`), or NULL on
// error. Output stats are written to the out-params when non-NULL:
//   *n_prompt_tokens — prompt token count
//   *n_generated     — generated token count
//   *n_drafted       — total draft tokens proposed
//   *n_accepted      — total draft tokens accepted
char * ewe_mtp_generate(ewe_mtp *    h,
                        const char * prompt,
                        int32_t      n_predict,
                        int32_t *    n_prompt_tokens,
                        int32_t *    n_generated,
                        int32_t *    n_drafted,
                        int32_t *    n_accepted);

#ifdef __cplusplus
}
#endif
