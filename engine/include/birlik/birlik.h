/* libbirlik — the BIRLIK engine C ABI.
 *
 * Every platform calls the engine through this header: the Rust agent (FFI), the Android app (JNI through the
 * Rust core) and the iOS app (Swift through the Rust core). The ABI only grows: functions are never removed and
 * struct fields are only appended, so bk_abi_version() tells callers what they can use.
 *
 * A model is split into windows of consecutive layers. The repeating layers are numbered 0 .. layer_count - 1 and
 * the output head (output norm and the matrix that gives logits) is layer layer_count, as in llama.cpp's
 * n_gpu_layers; the whole model is the window [0, layer_count + 1). A shard is one window loaded on one device; the
 * windows of a model form a pipeline (the ring). The shard that starts at layer 0 takes token ids, the others take
 * the hidden state of the previous layer; the shard with the output head gives logits, the others hidden states.
 * Shards are built on llama.cpp with the patches in engine/patches. A shard is not thread-safe: call it from one
 * thread at a time. */
#ifndef BIRLIK_BIRLIK_H
#define BIRLIK_BIRLIK_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* 1: stage B0 interface. 2: shards on llama.cpp — model info, bk_shard_run, KV management, prefetching,
 * tokenizer and sampler. */
#define BIRLIK_ABI_VERSION 2u

typedef enum bk_status {
    BK_OK = 0,
    BK_ERR_INVALID_ARGUMENT = 1,
    BK_ERR_NOT_IMPLEMENTED = 2,
    BK_ERR_OUT_OF_MEMORY = 3,
    BK_ERR_BACKEND = 4,
    BK_ERR_IO = 5,
    BK_ERR_CAPACITY = 6,   /* the KV cache or the request slots of a shard are full */
    BK_ERR_UNSUPPORTED = 7 /* the model or the requested backend cannot be used this way */
} bk_status;

/* Values match birlik.v1.Backend in protocol/birlik/v1/device.proto. */
typedef enum bk_backend {
    BK_BACKEND_AUTO = 0,
    BK_BACKEND_CPU = 1,
    BK_BACKEND_CUDA = 2,
    BK_BACKEND_HIP = 3,
    BK_BACKEND_VULKAN = 4,
    BK_BACKEND_METAL = 5,
    BK_BACKEND_OPENCL = 6,
    BK_BACKEND_HEXAGON = 7,
    BK_BACKEND_SYCL = 8,
    BK_BACKEND_CANN = 9,
    BK_BACKEND_WEBGPU = 10
} bk_backend;

/* A loaded, contiguous range of model layers on one device. Opaque. */
typedef struct bk_shard bk_shard;

typedef struct bk_shard_params {
    const char *model_path; /* GGUF file */
    uint32_t layer_begin;   /* first layer, inclusive */
    uint32_t layer_end;     /* last layer, exclusive; layer_count + 1 includes the output head */
    bk_backend backend;     /* BK_BACKEND_AUTO picks the fastest available backend */
    uint32_t max_tokens;    /* KV cache capacity in tokens, shared by all requests of the shard */
} bk_shard_params;

/* The ABI version this library implements (BIRLIK_ABI_VERSION at build time). */
uint32_t bk_abi_version(void);

/* Library version, e.g. "0.1.0". The string is static. */
const char *bk_version_string(void);

/* A short English description of a status code. The string is static. */
const char *bk_status_string(bk_status status);

/* Loads layers [layer_begin, layer_end) of a model. Only the weights of those layers are read; the shard that
 * starts at layer 0 also loads the token embeddings. [layer_count, layer_count + 1) is the output head alone: it
 * turns the last hidden state into logits, so the device that holds the first layers can keep the head too, as
 * prima.cpp's ring does. On success *out receives a shard that must be released with bk_shard_close(). */
bk_status bk_shard_open(const bk_shard_params *params, bk_shard **out);

/* Runs a middle shard (neither layer 0 nor the output head) on token_count tokens starting at position_start.
 * hidden_in and hidden_out hold token_count × hidden_dim f32 values; they may point to the same buffer.
 * The same as bk_shard_run() with hidden input; other shards return BK_ERR_INVALID_ARGUMENT. */
bk_status bk_shard_forward(bk_shard *shard, uint64_t request_id, uint32_t position_start, uint32_t token_count,
                           const float *hidden_in, float *hidden_out);

/* Releases a shard. Passing NULL is allowed. */
void bk_shard_close(bk_shard *shard);

/* --- ABI version 2 --------------------------------------------------------------------------------------- */

/* Model metadata, read from the GGUF header without loading weights. */
typedef struct bk_model_info {
    char architecture[32];   /* e.g. "llama", "qwen3" */
    uint32_t layer_count;    /* repeating layers; the output head is layer layer_count */
    uint32_t hidden_dim;     /* width of the hidden state passed between shards */
    uint32_t vocab_size;     /* logits per output token */
    uint32_t context_length; /* trained context length */
    uint64_t file_size;      /* bytes */
    uint32_t splittable;     /* 1 when the architecture can be split into windows */
} bk_model_info;

bk_status bk_model_info_read(const char *model_path, bk_model_info *out);

typedef struct bk_shard_info {
    bk_model_info model;
    uint32_t layer_begin;
    uint32_t layer_end;
    bk_backend backend; /* the backend the shard's layers run on */
} bk_shard_info;

bk_status bk_shard_get_info(const bk_shard *shard, bk_shard_info *out);

/* The last error message of the calling thread, from the engine or from llama.cpp; "" if there was none.
 * The string stays valid until the next engine call on the same thread. */
const char *bk_last_error(void);

/* One step of one request through a shard. */
typedef struct bk_batch {
    uint64_t request_id;     /* a request keeps its KV cache in the shard until bk_shard_drop() */
    uint32_t position_start; /* position of the first token */
    uint32_t token_count;
    const int32_t *tokens;   /* a shard that starts at layer 0: token_count token ids; NULL otherwise */
    const float *hidden;     /* other shards: token_count × hidden_dim f32 values; NULL otherwise */
    uint32_t all_logits;     /* a shard with the output head: 0 = logits of the last token only, 1 = of every token */
} bk_batch;

/* Runs a shard. A shard without the output head writes token_count × hidden_dim hidden-state values to out; a
 * shard with it writes vocab_size logits per output token. out_count is the capacity of out in floats.
 * Returns BK_ERR_CAPACITY when the KV cache or the request slots are full. */
bk_status bk_shard_run(bk_shard *shard, const bk_batch *batch, float *out, size_t out_count);

/* Forgets a request and frees its KV cache. Unknown requests are ignored. */
bk_status bk_shard_drop(bk_shard *shard, uint64_t request_id);

/* Removes a request's KV cache from position onwards, e.g. after rejected speculative tokens. */
bk_status bk_shard_truncate(bk_shard *shard, uint64_t request_id, uint32_t position);

/* Starts reading the shard's weights from disk in the background and returns at once (prima.cpp's prefetching):
 * a shard whose weights do not stay in memory is read while the other shards of the ring compute, so it is ready
 * when its turn comes. It does nothing when the weights need no prefetching: a GPU holds them, or they fit in free
 * memory when the shard opens, so the operating system keeps them after the first pass. */
bk_status bk_shard_prefetch(bk_shard *shard);

/* Threads for computing on the CPU; 0 = one per physical core (the default). */
bk_status bk_shard_set_threads(bk_shard *shard, uint32_t threads);

/* Text to token ids with the model's tokenizer. *count receives the number of tokens; if it exceeds capacity,
 * nothing is written and BK_ERR_CAPACITY is returned. add_special adds the model's BOS token, if any. */
bk_status bk_shard_tokenize(const bk_shard *shard, const char *text, size_t text_len, uint32_t add_special,
                            int32_t *tokens, size_t capacity, size_t *count);

/* The text of one token, written to out without a terminating zero; *len receives its length in bytes. */
bk_status bk_shard_token_text(const bk_shard *shard, int32_t token, char *out, size_t capacity, size_t *len);

/* 1 if the token ends generation (end of sequence, end of turn), else 0. */
uint32_t bk_shard_token_is_end(const bk_shard *shard, int32_t token);

/* Picks the next token from logits. Opaque; one per request. */
typedef struct bk_sampler bk_sampler;

typedef struct bk_sampler_params {
    float temperature; /* 0 = greedy: always the most likely token */
    uint32_t top_k;    /* 0 = off */
    float top_p;       /* 1 = off */
    float min_p;       /* 0 = off */
    uint64_t seed;
} bk_sampler_params;

bk_status bk_sampler_open(const bk_sampler_params *params, bk_sampler **out);

/* Samples from vocab_size logits and returns the token id in *token. */
bk_status bk_sampler_sample(bk_sampler *sampler, const float *logits, uint32_t vocab_size, int32_t *token);

void bk_sampler_close(bk_sampler *sampler);

#ifdef __cplusplus
}
#endif

#endif /* BIRLIK_BIRLIK_H */
