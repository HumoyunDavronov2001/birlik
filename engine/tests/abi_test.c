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
    CHECK(strcmp(bk_version_string(), "0.1.0") == 0);
    CHECK(strcmp(bk_status_string(BK_OK), "ok") == 0);
    CHECK(strcmp(bk_status_string(BK_ERR_NOT_IMPLEMENTED), "not implemented") == 0);

    bk_shard *shard = (bk_shard *)1;
    CHECK(bk_shard_open(NULL, &shard) == BK_ERR_INVALID_ARGUMENT);
    CHECK(shard == NULL);
    CHECK(bk_shard_open(NULL, NULL) == BK_ERR_INVALID_ARGUMENT);

    bk_shard_params params = {"model.gguf", 10, 10, BK_BACKEND_AUTO, 4096};
    CHECK(bk_shard_open(&params, &shard) == BK_ERR_INVALID_ARGUMENT); /* empty layer range */
    params.layer_end = 24;
    params.max_tokens = 0;
    CHECK(bk_shard_open(&params, &shard) == BK_ERR_INVALID_ARGUMENT); /* no KV capacity */
    params.max_tokens = 4096;
    CHECK(bk_shard_open(&params, &shard) == BK_ERR_NOT_IMPLEMENTED); /* valid, implemented in B1 */

    float buffer[4] = {0};
    CHECK(bk_shard_forward(NULL, 1, 0, 1, buffer, buffer) == BK_ERR_INVALID_ARGUMENT);
    bk_shard_close(NULL);

    if (failures) {
        fprintf(stderr, "%d check(s) failed\n", failures);
        return 1;
    }
    puts("libbirlik C ABI: all checks passed");
    return 0;
}
