// Implementation of the step-driven MTP speculative-decoding shim declared in
// wrapper_mtp.h.
//
// Runs a single-sequence MTP speculative generation loop, modelled on
// examples/speculative-simple.cpp with the MTP-specific setup from tools/server
// (draft context uses ctx_type = LLAMA_CONTEXT_TYPE_MTP and shares the target KV
// cache via ctx_other; target embeddings are enabled when the speculator needs
// them; common_speculative_process replaces the manual draft decode).
//
// The engine (contexts + speculator) is built once in ewe_mtp_init and reused;
// ewe_mtp_begin resets per-generation state (KV cache + sampler); ewe_mtp_step
// advances one draft -> verify -> accept step. Checkpoints are omitted — each
// step always re-evaluates from the committed prefix.
//
// All C++ exceptions are caught at the boundary and reported as NULL / -1.

#include "common.h"       // llama.cpp: common/common.h
#include "sampling.h"     // llama.cpp: common/sampling.h
#include "speculative.h"  // llama.cpp: common/speculative.h
#include "llama.h"

#include "wrapper_mtp.h"

#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>

// Loaded draft (MTP head) model — immutable weights, shared read-only across
// engines. Owns the llama_model and frees it in ewe_mtp_model_free.
struct ewe_mtp_model {
    llama_model * model = nullptr;
};

extern "C" ewe_mtp_model * ewe_mtp_model_load(const char * draft_path) {
    if (draft_path == nullptr) {
        return nullptr;
    }
    try {
        llama_model_params mparams = llama_model_default_params();
        llama_model * model        = llama_model_load_from_file(draft_path, mparams);
        if (model == nullptr) {
            return nullptr;
        }
        auto * h  = new ewe_mtp_model();
        h->model  = model;
        return h;
    } catch (...) {
        return nullptr;
    }
}

extern "C" void ewe_mtp_model_free(ewe_mtp_model * model) {
    if (model == nullptr) {
        return;
    }
    if (model->model != nullptr) {
        llama_model_free(model->model);
    }
    delete model;
}

struct ewe_mtp {
    // Per-generation engine state. The draft MODEL is NOT owned here (it is a
    // shared ewe_mtp_model owned by the caller); only the contexts + speculator
    // + batch belong to this engine.
    const llama_model * model_tgt = nullptr;
    llama_context *     ctx_tgt   = nullptr;
    llama_context *     ctx_dft   = nullptr;
    common_speculative * spec     = nullptr;
    const llama_vocab * vocab     = nullptr;
    llama_batch         batch     = {};
    bool                batch_ok  = false;

    // Kept alive for the lifetime of `spec` (holds ctx pointers / n_max).
    common_params_speculative pspec;

    int32_t n_draft_max = 4;

    // Per-generation state (reset by ewe_mtp_begin).
    common_sampler *         smpl = nullptr;
    std::vector<llama_token> prompt_tgt;
    std::vector<llama_token> draft;
    llama_token              id_last   = 0;
    int                      n_past    = 0;
    int                      n_predict = 0;
    int                      n_prompt_tokens = 0;
    int                      n_generated = 0;
    int                      n_drafted   = 0;
    int                      n_accepted  = 0;
    bool                     done      = true;
};

static void ewe_mtp_reset_generation(ewe_mtp * h) {
    if (h->smpl != nullptr) {
        common_sampler_free(h->smpl);
        h->smpl = nullptr;
    }
    h->prompt_tgt.clear();
    h->draft.clear();
    h->id_last         = 0;
    h->n_past          = 0;
    h->n_predict       = 0;
    h->n_prompt_tokens = 0;
    h->n_generated     = 0;
    h->n_drafted       = 0;
    h->n_accepted      = 0;
    h->done            = true;
}

extern "C" ewe_mtp * ewe_mtp_init(const llama_model *   target_model,
                                  const ewe_mtp_model * draft_model,
                                  uint32_t              n_ctx,
                                  uint32_t              n_batch,
                                  int32_t               n_threads,
                                  int32_t               n_draft_max) {
    if (target_model == nullptr || draft_model == nullptr || draft_model->model == nullptr) {
        return nullptr;
    }
    try {
        // The draft model is shared + owned by the caller — reference it, never
        // load or free it here.
        llama_model * model_dft = draft_model->model;

        // Target context. MTP needs recurrent-state snapshots on the target for
        // draft rollback (n_rs_seq = n_max).
        llama_context_params cparams_tgt = llama_context_default_params();
        cparams_tgt.n_ctx           = n_ctx;
        cparams_tgt.n_batch         = n_batch;
        cparams_tgt.n_threads       = n_threads;
        cparams_tgt.n_threads_batch = n_threads;
        cparams_tgt.n_rs_seq        = n_draft_max > 0 ? (uint32_t) n_draft_max : 0u;

        llama_context * ctx_tgt =
            llama_init_from_model(const_cast<llama_model *>(target_model), cparams_tgt);
        if (ctx_tgt == nullptr) {
            return nullptr;
        }

        // Draft (MTP) context: shares the target KV cache.
        llama_context_params cparams_dft = llama_context_default_params();
        cparams_dft.n_ctx           = n_ctx;
        cparams_dft.n_batch         = n_batch;
        cparams_dft.n_threads       = n_threads;
        cparams_dft.n_threads_batch = n_threads;
        cparams_dft.ctx_type        = LLAMA_CONTEXT_TYPE_MTP;
        cparams_dft.n_rs_seq        = 0;
        cparams_dft.ctx_other       = ctx_tgt;

        llama_context * ctx_dft = llama_init_from_model(model_dft, cparams_dft);
        if (ctx_dft == nullptr) {
            llama_free(ctx_tgt);
            return nullptr;
        }

        auto * h       = new ewe_mtp();
        h->model_tgt   = target_model;
        h->ctx_tgt     = ctx_tgt;
        h->ctx_dft     = ctx_dft;
        h->vocab       = llama_model_get_vocab(target_model);
        h->n_draft_max = n_draft_max;
        h->batch       = llama_batch_init(llama_n_batch(ctx_tgt), 0, 1);
        h->batch_ok    = true;

        h->pspec               = common_params_speculative();
        h->pspec.types         = { COMMON_SPECULATIVE_TYPE_DRAFT_MTP };
        h->pspec.draft.n_max   = n_draft_max;
        h->pspec.draft.ctx_tgt = ctx_tgt;
        h->pspec.draft.ctx_dft = ctx_dft;

        h->spec = common_speculative_init(h->pspec, 1);
        if (h->spec == nullptr) {
            llama_batch_free(h->batch);
            llama_free(ctx_dft);
            llama_free(ctx_tgt);
            delete h;
            return nullptr;
        }

        return h;
    } catch (...) {
        return nullptr;
    }
}

extern "C" void ewe_mtp_free(ewe_mtp * h) {
    if (h == nullptr) {
        return;
    }
    if (h->smpl != nullptr) {
        common_sampler_free(h->smpl);
    }
    if (h->batch_ok) {
        llama_batch_free(h->batch);
    }
    if (h->spec != nullptr) {
        common_speculative_free(h->spec);
    }
    if (h->ctx_dft != nullptr) {
        llama_free(h->ctx_dft);
    }
    if (h->ctx_tgt != nullptr) {
        llama_free(h->ctx_tgt);
    }
    // Note: the draft model is NOT owned by the engine — it is a shared
    // ewe_mtp_model freed separately by ewe_mtp_model_free.
    delete h;
}

extern "C" int32_t ewe_mtp_begin(ewe_mtp *        h,
                                 const char *     prompt,
                                 int32_t          n_predict,
                                 ewe_mtp_sampling sampling) {
    if (h == nullptr || prompt == nullptr) {
        return -1;
    }
    try {
        ewe_mtp_reset_generation(h);

        // Reset the shared KV cache so a reused engine starts clean.
        llama_memory_clear(llama_get_memory(h->ctx_tgt), true);
        llama_memory_clear(llama_get_memory(h->ctx_dft), true);

        common_params_sampling sparams;
        sparams.temp           = sampling.temperature;
        sparams.top_k          = sampling.top_k;
        sparams.top_p          = sampling.top_p;
        sparams.penalty_repeat = sampling.repeat_penalty;
        sparams.seed           = sampling.seed;

        h->smpl = common_sampler_init(h->model_tgt, sparams);
        if (h->smpl == nullptr) {
            return -1;
        }

        std::vector<llama_token> inp =
            common_tokenize(h->ctx_tgt, std::string(prompt), true, true);
        if (inp.empty() || (uint32_t) inp.size() >= llama_n_ctx(h->ctx_tgt)) {
            return -1;
        }
        h->n_prompt_tokens = (int) inp.size();

        // Enable target embeddings if the MTP speculator needs them.
        llama_set_embeddings(h->ctx_tgt, common_speculative_need_embd(h->spec));

        // Evaluate the prompt (all but the last token) on the target.
        if (llama_decode(h->ctx_tgt, llama_batch_get_one(inp.data(), (int32_t) inp.size() - 1)) != 0) {
            return -1;
        }

        h->id_last = inp.back();
        h->prompt_tgt.assign(inp.begin(), inp.end() - 1);
        h->prompt_tgt.reserve(llama_n_ctx(h->ctx_tgt));
        h->n_past    = (int) inp.size() - 1;
        h->n_predict = n_predict;
        h->done      = false;

        common_speculative_begin(h->spec, 0, h->prompt_tgt);

        return h->n_prompt_tokens;
    } catch (...) {
        h->done = true;
        return -1;
    }
}

// Copy a std::string into a fresh malloc'd C string (caller frees). Returns
// nullptr only on allocation failure.
static char * ewe_mtp_dup(const std::string & s) {
    char * out = static_cast<char *>(std::malloc(s.size() + 1));
    if (out != nullptr) {
        std::memcpy(out, s.c_str(), s.size() + 1);
    }
    return out;
}

extern "C" int32_t ewe_mtp_step(ewe_mtp * h, char ** out_piece) {
    if (out_piece != nullptr) {
        *out_piece = nullptr;
    }
    if (h == nullptr || h->smpl == nullptr) {
        return -1;
    }
    if (h->done) {
        return 0;
    }
    try {
        // Generate the draft for the current position.
        h->draft.clear();
        common_speculative_get_draft_params(h->spec, 0) = {
            /* .drafting = */ true,
            /* .n_max    = */ h->n_draft_max,
            /* .n_past   = */ h->n_past,
            /* .id_last  = */ h->id_last,
            /* .prompt   = */ &h->prompt_tgt,
            /* .result   = */ &h->draft,
        };
        common_speculative_draft(h->spec);
        const int n_draft = (int) h->draft.size();

        // Target batch: [id_last, draft0, draft1, ...].
        common_batch_clear(h->batch);
        common_batch_add(h->batch, h->id_last, h->n_past++, { 0 }, true);
        for (int i = 0; i < n_draft; ++i) {
            common_batch_add(h->batch, h->draft[i], h->n_past + i, { 0 }, true);
        }

        if (llama_decode(h->ctx_tgt, h->batch) != 0) {
            h->done = true;
            return -1;
        }
        if (!common_speculative_process(h->spec, h->batch)) {
            h->done = true;
            return -1;
        }

        std::vector<llama_token> ids =
            common_sampler_sample_and_accept_n(h->smpl, h->ctx_tgt, h->draft);
        if (ids.empty()) {
            h->done = true;
            return -1;
        }

        common_speculative_accept(h->spec, 0, (int) ids.size() - 1);

        h->n_past     += (int) ids.size() - 1;
        h->n_drafted  += n_draft;
        h->n_accepted += (int) ids.size() - 1;

        std::string piece;
        for (size_t i = 0; i < ids.size(); ++i) {
            h->prompt_tgt.push_back(h->id_last);
            h->id_last = ids[i];

            if (llama_vocab_is_eog(h->vocab, h->id_last)) {
                h->done = true;
                break;
            }
            piece += common_token_to_piece(h->ctx_tgt, h->id_last);
            h->n_generated++;
            if (h->n_generated >= h->n_predict) {
                h->done = true;
                break;
            }
        }

        // Drop KV entries for any rejected draft tokens.
        llama_memory_seq_rm(llama_get_memory(h->ctx_tgt), 0, h->n_past, -1);
        llama_memory_seq_rm(llama_get_memory(h->ctx_dft), 0, h->n_past, -1);

        if (out_piece != nullptr) {
            *out_piece = ewe_mtp_dup(piece);
            if (*out_piece == nullptr) {
                h->done = true;
                return -1;
            }
        }

        return h->done ? 0 : 1;
    } catch (...) {
        h->done = true;
        return -1;
    }
}

extern "C" void ewe_mtp_stats(const ewe_mtp * h,
                              int32_t *       n_prompt_tokens,
                              int32_t *       n_generated,
                              int32_t *       n_drafted,
                              int32_t *       n_accepted) {
    if (h == nullptr) {
        return;
    }
    if (n_prompt_tokens != nullptr) {
        *n_prompt_tokens = h->n_prompt_tokens;
    }
    if (n_generated != nullptr) {
        *n_generated = h->n_generated;
    }
    if (n_drafted != nullptr) {
        *n_drafted = h->n_drafted;
    }
    if (n_accepted != nullptr) {
        *n_accepted = h->n_accepted;
    }
}
