#include "capcover.h"
#include <stdio.h>
#include <stdlib.h>

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
    if (fread(queries, sizeof(float), 4 * dim, file) != 4 * dim) {
        fprintf(stderr, "short query file\n");
        free(queries);
        fclose(file);
        capcover_close(index);
        return 1;
    }
    fclose(file);
    int decision = capcover_may_contain(index, queries, dim, 0.95f);
    if (decision < 0) {
        fprintf(stderr, "may_contain: %s\n", capcover_last_error());
        free(queries);
        capcover_close(index);
        return 1;
    }
    unsigned char bitmap[4];
    if (capcover_check_bitmap(index, queries, 4, 0.95f, bitmap) != 0) {
        fprintf(stderr, "bitmap: %s\n", capcover_last_error());
        free(queries);
        capcover_close(index);
        return 1;
    }
    for (size_t i = 0; i < 4; i++) {
        if (bitmap[i] > 1) {
            fprintf(stderr, "bad bitmap byte\n");
            free(queries);
            capcover_close(index);
            return 1;
        }
    }
    printf("capi ok dim=%zu n=%zu k=%zu decision=%d bitmap=%u%u%u%u version=%s\n",
           dim, n, k, decision, bitmap[0], bitmap[1], bitmap[2], bitmap[3],
           capcover_version());
    free(queries);
    capcover_close(index);
    return 0;
}
