/* libbirlik — the BIRLIK engine C ABI.
 *
 * Every platform calls the engine through this header: the Rust agent (FFI), the Android app (JNI through the
 * Rust core) and the iOS app (Swift through the Rust core). The ABI only grows: functions are never removed and
 * struct fields are only appended, so bk_abi_version() tells callers what they can use.
 *
 * Stage B0 defines the interface and validates arguments; stage B1 implements shards on top of llama.cpp. */
#ifndef BIRLIK_BIRLIK_H
#define BIRLIK_BIRLIK_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define BIRLIK_ABI_VERSION 1u

typedef enum bk_status {
    BK_OK = 0,
    BK_ERR_INVALID_ARGUMENT = 1,
    BK_ERR_NOT_IMPLEMENTED = 2,
    BK_ERR_OUT_OF_MEMORY = 3,
    BK_ERR_BACKEND = 4,
    BK_ERR_IO = 5
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
    uint32_t layer_end;     /* last layer, exclusive */
    bk_backend backend;     /* BK_BACKEND_AUTO picks the fastest available backend */
    uint32_t max_tokens;    /* KV cache capacity in tokens */
} bk_shard_params;

/* The ABI version this library implements (BIRLIK_ABI_VERSION at build time). */
uint32_t bk_abi_version(void);

/* Library version, e.g. "0.1.0". The string is static. */
const char *bk_version_string(void);

/* A short English description of a status code. The string is static. */
const char *bk_status_string(bk_status status);

/* Loads layers [layer_begin, layer_end) of a model. On success *out receives a shard that must be released with
 * bk_shard_close(). */
bk_status bk_shard_open(const bk_shard_params *params, bk_shard **out);

/* Runs the shard on token_count tokens starting at position_start. hidden_in and hidden_out hold
 * token_count × hidden_dim f32 values; they may point to the same buffer. */
bk_status bk_shard_forward(bk_shard *shard, uint64_t request_id, uint32_t position_start, uint32_t token_count,
                           const float *hidden_in, float *hidden_out);

/* Releases a shard. Passing NULL is allowed. */
void bk_shard_close(bk_shard *shard);

#ifdef __cplusplus
}
#endif

#endif /* BIRLIK_BIRLIK_H */
