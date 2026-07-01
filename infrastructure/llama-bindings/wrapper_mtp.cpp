// Implementation of the MTP speculative-decoding shim declared in wrapper_mtp.h.
//
// Runs a self-contained single-sequence MTP speculative generation loop,
// modelled on examples/speculative-simple.cpp with the MTP-specific setup from
// tools/server (draft context uses ctx_type = LLAMA_CONTEXT_TYPE_MTP and shares
// the target KV cache via ctx_other; target embeddings are enabled when the
// speculator needs them; common_speculative_process replaces the manual draft
// decode). Checkpoints are intentionally omitted — each call is a fresh single
// sequence; if the context requires checkpoint-based rollback the loop would
// still function via the always-re-evaluate path.
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

struct ewe_mtp {
    const llama_model * model_tgt = nullptr;
    llama_model *       model_dft = nullptr;
    llama_context *     ctx_tgt   = nullptr;
    llama_context *     ctx_dft   = nullptr;
    common_speculative * spec     = nullptr;

    // Kept alive for the lifetime of `spec` (holds ctx pointers / n_max).
    common_params_speculative pspec;
    common_params_sampling    sparams;

    int32_t n_draft_max = 4;
};

extern "C" ewe_mtp * ewe_mtp_init(const llama_model * target_model,
                                  const char *        draft_path,
                                  uint32_t            n_ctx,
                                  uint32_t            n_batch,
                                  int32_t             n_threads,
                                  int32_t             n_draft_max,
                                  ewe_mtp_sampling    sampling) {
    if (target_model == nullptr || draft_path == nullptr) {
        return nullptr;
    }
    try {
        // Load the draft (MTP head) model.
        llama_model_params mparams = llama_model_default_params();
        llama_model * model_dft    = llama_model_load_from_file(draft_path, mparams);
        if (model_dft == nullptr) {
            return nullptr;
        }

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
            llama_model_free(model_dft);
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
            llama_model_free(model_dft);
            return nullptr;
        }

        auto * h      = new ewe_mtp();
        h->model_tgt  = target_model;
        h->model_dft  = model_dft;
        h->ctx_tgt    = ctx_tgt;
        h->ctx_dft    = ctx_dft;
        h->n_draft_max = n_draft_max;

        h->sparams                = common_params_sampling();
        h->sparams.temp           = sampling.temperature;
        h->sparams.top_k          = sampling.top_k;
        h->sparams.top_p          = sampling.top_p;
        h->sparams.penalty_repeat = sampling.repeat_penalty;
        h->sparams.seed           = sampling.seed;

        h->pspec            = common_params_speculative();
        h->pspec.types      = { COMMON_SPECULATIVE_TYPE_DRAFT_MTP };
        h->pspec.draft.n_max   = n_draft_max;
        h->pspec.draft.ctx_tgt = ctx_tgt;
        h->pspec.draft.ctx_dft = ctx_dft;

        h->spec = common_speculative_init(h->pspec, 1);
        if (h->spec == nullptr) {
            llama_free(ctx_dft);
            llama_free(ctx_tgt);
            llama_model_free(model_dft);
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
    if (h->spec != nullptr) {
        common_speculative_free(h->spec);
    }
    if (h->ctx_dft != nullptr) {
        llama_free(h->ctx_dft);
    }
    if (h->ctx_tgt != nullptr) {
        llama_free(h->ctx_tgt);
    }
    if (h->model_dft != nullptr) {
        llama_model_free(h->model_dft);
    }
    delete h;
}

extern "C" char * ewe_mtp_generate(ewe_mtp *    h,
                                   const char * prompt,
                                   int32_t      n_predict,
                                   int32_t *    n_prompt_tokens,
                                   int32_t *    n_generated,
                                   int32_t *    n_drafted,
                                   int32_t *    n_accepted) {
    if (h == nullptr || prompt == nullptr) {
        return nullptr;
    }
    common_sampler * smpl = nullptr;
    llama_batch      batch = {};
    bool             batch_inited = false;
    try {
        const llama_vocab * vocab = llama_model_get_vocab(h->model_tgt);

        std::vector<llama_token> inp = common_tokenize(h->ctx_tgt, std::string(prompt), true, true);
        if (inp.empty() || (uint32_t) inp.size() >= llama_n_ctx(h->ctx_tgt)) {
            return nullptr;
        }
        if (n_prompt_tokens != nullptr) {
            *n_prompt_tokens = (int32_t) inp.size();
        }

        smpl = common_sampler_init(h->model_tgt, h->sparams);
        if (smpl == nullptr) {
            return nullptr;
        }

        // Enable target embeddings if the MTP speculator needs them.
        llama_set_embeddings(h->ctx_tgt, common_speculative_need_embd(h->spec));

        // Evaluate the prompt (all but the last token) on the target.
        if (llama_decode(h->ctx_tgt, llama_batch_get_one(inp.data(), (int32_t) inp.size() - 1)) != 0) {
            common_sampler_free(smpl);
            return nullptr;
        }

        llama_token              id_last = inp.back();
        std::vector<llama_token> prompt_tgt(inp.begin(), inp.end() - 1);
        prompt_tgt.reserve(llama_n_ctx(h->ctx_tgt));
        int n_past = (int) inp.size() - 1;

        common_speculative_begin(h->spec, 0, prompt_tgt);

        batch        = llama_batch_init(llama_n_batch(h->ctx_tgt), 0, 1);
        batch_inited = true;

        std::string out_text;
        int         generated = 0;
        int         drafted   = 0;
        int         accepted  = 0;
        bool        eos       = false;

        std::vector<llama_token> draft;

        while (true) {
            // Generate the draft for the current position.
            draft.clear();
            common_speculative_get_draft_params(h->spec, 0) = {
                /* .drafting = */ true,
                /* .n_max    = */ h->n_draft_max,
                /* .n_past   = */ n_past,
                /* .id_last  = */ id_last,
                /* .prompt   = */ &prompt_tgt,
                /* .result   = */ &draft,
            };
            common_speculative_draft(h->spec);
            const int n_draft = (int) draft.size();

            // Target batch: [id_last, draft0, draft1, ...].
            common_batch_clear(batch);
            common_batch_add(batch, id_last, n_past++, { 0 }, true);
            for (int i = 0; i < n_draft; ++i) {
                common_batch_add(batch, draft[i], n_past + i, { 0 }, true);
            }

            if (llama_decode(h->ctx_tgt, batch) != 0) {
                break;
            }

            // MTP processing (draft-side) — replaces the manual draft decode.
            if (!common_speculative_process(h->spec, batch)) {
                break;
            }

            // Verify: sample from the target logits and accept the matching
            // draft prefix. `ids` = accepted draft tokens + one bonus token.
            std::vector<llama_token> ids =
                common_sampler_sample_and_accept_n(smpl, h->ctx_tgt, draft);
            if (ids.empty()) {
                break;
            }

            common_speculative_accept(h->spec, 0, (int) ids.size() - 1);

            n_past += (int) ids.size() - 1;
            drafted += n_draft;
            accepted += (int) ids.size() - 1;

            for (size_t i = 0; i < ids.size(); ++i) {
                prompt_tgt.push_back(id_last);
                id_last = ids[i];

                if (llama_vocab_is_eog(vocab, id_last)) {
                    eos = true;
                    break;
                }
                out_text += common_token_to_piece(h->ctx_tgt, id_last);
                generated++;
                if (generated >= n_predict) {
                    break;
                }
            }

            // Drop KV entries for any rejected draft tokens.
            llama_memory_seq_rm(llama_get_memory(h->ctx_tgt), 0, n_past, -1);
            llama_memory_seq_rm(llama_get_memory(h->ctx_dft), 0, n_past, -1);

            if (eos || generated >= n_predict) {
                break;
            }
        }

        if (n_generated != nullptr) {
            *n_generated = generated;
        }
        if (n_drafted != nullptr) {
            *n_drafted = drafted;
        }
        if (n_accepted != nullptr) {
            *n_accepted = accepted;
        }

        llama_batch_free(batch);
        common_sampler_free(smpl);

        char * out = static_cast<char *>(std::malloc(out_text.size() + 1));
        if (out == nullptr) {
            return nullptr;
        }
        std::memcpy(out, out_text.c_str(), out_text.size() + 1);
        return out;
    } catch (...) {
        if (batch_inited) {
            llama_batch_free(batch);
        }
        if (smpl != nullptr) {
            common_sampler_free(smpl);
        }
        return nullptr;
    }
}
