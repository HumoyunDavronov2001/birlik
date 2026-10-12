// libbirlik — stage B0: the C ABI with argument validation. Shards are implemented in stage B1 (llama.cpp).
#include "birlik/birlik.h"

namespace {
constexpr const char *kVersion = "0.1.0";
}  // namespace

extern "C" {

uint32_t bk_abi_version(void) { return BIRLIK_ABI_VERSION; }

const char *bk_version_string(void) { return kVersion; }

const char *bk_status_string(bk_status status) {
    switch (status) {
        case BK_OK: return "ok";
        case BK_ERR_INVALID_ARGUMENT: return "invalid argument";
        case BK_ERR_NOT_IMPLEMENTED: return "not implemented";
        case BK_ERR_OUT_OF_MEMORY: return "out of memory";
        case BK_ERR_BACKEND: return "backend error";
        case BK_ERR_IO: return "i/o error";
    }
    return "unknown status";
}

bk_status bk_shard_open(const bk_shard_params *params, bk_shard **out) {
    if (out == nullptr) return BK_ERR_INVALID_ARGUMENT;
    *out = nullptr;
    if (params == nullptr || params->model_path == nullptr || params->model_path[0] == '\0') {
        return BK_ERR_INVALID_ARGUMENT;
    }
    if (params->layer_end <= params->layer_begin || params->max_tokens == 0) return BK_ERR_INVALID_ARGUMENT;
    const int backend = static_cast<int>(params->backend);  // enums may be unsigned: compare as int
    if (backend < BK_BACKEND_AUTO || backend > BK_BACKEND_WEBGPU) return BK_ERR_INVALID_ARGUMENT;
    return BK_ERR_NOT_IMPLEMENTED;
}

bk_status bk_shard_forward(bk_shard *shard, uint64_t request_id, uint32_t position_start, uint32_t token_count,
                           const float *hidden_in, float *hidden_out) {
    (void)request_id;
    (void)position_start;
    if (shard == nullptr || token_count == 0 || hidden_in == nullptr || hidden_out == nullptr) {
        return BK_ERR_INVALID_ARGUMENT;
    }
    return BK_ERR_NOT_IMPLEMENTED;
}

void bk_shard_close(bk_shard *shard) { (void)shard; }

}  // extern "C"
