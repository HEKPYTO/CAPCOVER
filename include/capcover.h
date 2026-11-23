#ifndef CAPCOVER_H
#define CAPCOVER_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct CapCover CapCover;

CapCover *capcover_open(const char *path);
void capcover_close(CapCover *index);
size_t capcover_dim(const CapCover *index);
size_t capcover_n(const CapCover *index);
size_t capcover_k(const CapCover *index);
uint64_t capcover_file_bytes(const CapCover *index);
int capcover_may_contain(const CapCover *index, const float *query, size_t len, float similarity);
int capcover_check_bitmap(const CapCover *index, const float *queries, size_t count, float similarity, uint8_t *out);
const char *capcover_version(void);
const char *capcover_last_error(void);

#ifdef __cplusplus
}
#endif

#endif
