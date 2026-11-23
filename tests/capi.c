#include "capcover.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int error_contains(const char *expected) {
    const char *message = capcover_last_error();
    if (!message || !strstr(message, expected)) {
        fprintf(stderr, "expected error '%s', got '%s'\n", expected,
                message ? message : "(null)");
        return 0;
    }
    return 1;
}

int main(int argc, char **argv) {
    if (argc != 3) {
        fprintf(stderr, "usage: capi_test index query\n");
        return 2;
    }
    CapCover *index = capcover_open(argv[1]);
    if (!index) {
        fprintf(stderr, "open: %s\n", capcover_last_error());
        return 1;
    }
    size_t dim = capcover_dim(index);
    size_t n = capcover_n(index);
    size_t k = capcover_k(index);
    if (dim == 0 || n == 0 || k == 0) {
        fprintf(stderr, "bad shape\n");
        capcover_close(index);
        return 1;
    }
    FILE *file = fopen(argv[2], "rb");
    if (!file) {
        perror("query");
        capcover_close(index);
        return 1;
    }
    float *queries = malloc(4 * dim * sizeof(float));
    if (!queries) {
        fclose(file);
        capcover_close(index);
        return 1;
    }
    int status = 1;
    if (fread(queries, sizeof(float), 4 * dim, file) != 4 * dim) {
        fprintf(stderr, "short query file\n");
        goto done;
    }
    fclose(file);
    file = NULL;

    int decision = capcover_may_contain(index, queries, dim, 0.95f);
    if (decision < 0) {
        fprintf(stderr, "may_contain: %s\n", capcover_last_error());
        goto done;
    }
    unsigned char bitmap[4];
    if (capcover_check_bitmap(index, queries, 4, 0.95f, bitmap) != 0) {
        fprintf(stderr, "bitmap: %s\n", capcover_last_error());
        goto done;
    }
    for (size_t i = 0; i < 4; i++) {
        int expected = capcover_may_contain(index, queries + i * dim, dim, 0.95f);
        if (expected < 0) {
            fprintf(stderr, "may_contain[%zu]: %s\n", i, capcover_last_error());
            goto done;
        }
        if (bitmap[i] > 1 || bitmap[i] != (unsigned char)expected) {
            fprintf(stderr, "bitmap[%zu]: %u != %d\n", i, bitmap[i], expected);
            goto done;
        }
    }
    if (decision != (int)bitmap[0]) {
        fprintf(stderr, "decision %d != bitmap[0] %u\n", decision, bitmap[0]);
        goto done;
    }

    if (capcover_may_contain(NULL, queries, dim, 0.95f) != -1 ||
        !error_contains("null index")) {
        goto done;
    }
    if (capcover_check_bitmap(NULL, queries, 4, 0.95f, bitmap) != -1 ||
        !error_contains("null index")) {
        goto done;
    }
    if (capcover_may_contain(index, queries, dim + 1, 0.95f) != -1 ||
        !error_contains("dimension")) {
        goto done;
    }
    if (capcover_may_contain(index, NULL, dim, 0.95f) != -1 ||
        !error_contains("null query")) {
        goto done;
    }
    if (capcover_check_bitmap(index, NULL, 4, 0.95f, bitmap) != -1 ||
        !error_contains("null query")) {
        goto done;
    }
    if (capcover_check_bitmap(index, queries, 4, 0.95f, NULL) != -1 ||
        !error_contains("null query or output buffer")) {
        goto done;
    }

    printf("capi ok dim=%zu n=%zu k=%zu decision=%d bitmap=%u%u%u%u version=%s\n",
           dim, n, k, decision, bitmap[0], bitmap[1], bitmap[2], bitmap[3],
           capcover_version());
    status = 0;

done:
    if (file) {
        fclose(file);
    }
    free(queries);
    capcover_close(index);
    return status;
}
