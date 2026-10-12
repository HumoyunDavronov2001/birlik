/* Calls the engine from plain C, the way every foreign-function interface will. */
#include <stdio.h>
#include <string.h>

#include "birlik/birlik.h"

static int failures = 0;
#define CHECK(cond)                                                       \
    do {                                                                  \
        if (!(cond)) {                                                    \
            fprintf(stderr, "%s:%d: check failed: %s\n", __FILE__, __LINE__, #cond); \
            failures++;                                                   \
        }                                                                 \
    } while (0)

int main(void) {
    CHECK(bk_abi_version() == BIRLIK_ABI_VERSION);
    CHECK(BIRLIK_ABI_VERSION == 2u);
    CHECK(strcmp(bk_version_string(), "0.1.0") == 0);
    CHECK(strcmp(bk_status_string(BK_OK), "ok") == 0);
    CHECK(strcmp(bk_status_string(BK_ERR_NOT_IMPLEMENTED), "not implemented") == 0);
    CHECK(strcmp(bk_status_string(BK_ERR_CAPACITY), "capacity exhausted") == 0);

    bk_shard *shard = (bk_shard *)1;
    CHECK(bk_shard_open(NULL, &shard) == BK_ERR_INVALID_ARGUMENT);
    CHECK(shard == NULL);
    CHECK(bk_shard_open(NULL, NULL) == BK_ERR_INVALID_ARGUMENT);
    CHECK(strlen(bk_last_error()) > 0);

    bk_shard_params params = {"no-such-model.gguf", 10, 10, BK_BACKEND_AUTO, 4096};
    CHECK(bk_shard_open(&params, &shard) == BK_ERR_INVALID_ARGUMENT); /* empty layer range */
    params.layer_end = 24;
    params.max_tokens = 0;
    CHECK(bk_shard_open(&params, &shard) == BK_ERR_INVALID_ARGUMENT); /* no KV capacity */
    params.max_tokens = 4096;
    params.backend = (bk_backend)42;
    CHECK(bk_shard_open(&params, &shard) == BK_ERR_INVALID_ARGUMENT); /* unknown backend */
    params.backend = BK_BACKEND_AUTO;
    CHECK(bk_shard_open(&params, &shard) == BK_ERR_IO); /* valid arguments, no such file */
    CHECK(strstr(bk_last_error(), "no-such-model.gguf") != NULL);

    bk_model_info info;
    CHECK(bk_model_info_read(NULL, &info) == BK_ERR_INVALID_ARGUMENT);
    CHECK(bk_model_info_read("no-such-model.gguf", &info) == BK_ERR_IO);

    float buffer[4] = {0};
    CHECK(bk_shard_forward(NULL, 1, 0, 1, buffer, buffer) == BK_ERR_INVALID_ARGUMENT);
    CHECK(bk_shard_run(NULL, NULL, buffer, 4) == BK_ERR_INVALID_ARGUMENT);
    CHECK(bk_shard_drop(NULL, 1) == BK_ERR_INVALID_ARGUMENT);
    bk_shard_close(NULL);

    /* greedy sampling picks the largest logit */
    bk_sampler_params sp = {0.0f, 0, 1.0f, 0.0f, 7};
    bk_sampler *sampler = NULL;
    CHECK(bk_sampler_open(&sp, &sampler) == BK_OK);
    const float logits[5] = {0.1f, 2.5f, -1.0f, 2.4f, 0.0f};
    int32_t token = -1;
    CHECK(bk_sampler_sample(sampler, logits, 5, &token) == BK_OK);
    CHECK(token == 1);
    bk_sampler_close(sampler);

    /* with a temperature, a seeded sampler is reproducible */
    sp.temperature = 0.8f;
    sp.top_k = 3;
    int32_t first = -1, second = -1;
    CHECK(bk_sampler_open(&sp, &sampler) == BK_OK);
    CHECK(bk_sampler_sample(sampler, logits, 5, &first) == BK_OK);
    bk_sampler_close(sampler);
    CHECK(bk_sampler_open(&sp, &sampler) == BK_OK);
    CHECK(bk_sampler_sample(sampler, logits, 5, &second) == BK_OK);
    bk_sampler_close(sampler);
    CHECK(first == second);
    CHECK(first == 0 || first == 1 || first == 3); /* top-3 */

    if (failures) {
        fprintf(stderr, "%d check(s) failed\n", failures);
        return 1;
    }
    puts("libbirlik C ABI: all checks passed");
    return 0;
}
