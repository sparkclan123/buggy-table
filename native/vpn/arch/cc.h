#pragma once
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#ifdef _MSC_VER
#define PACK_STRUCT_BEGIN __pragma(pack(push, 1))
#define PACK_STRUCT_END __pragma(pack(pop))
#define PACK_STRUCT_STRUCT
#endif
#ifdef __cplusplus
extern "C" {
#endif
uint32_t vpn_random(void);
#ifdef __cplusplus
}
#endif
#define LWIP_RAND() vpn_random()
#define LWIP_PLATFORM_DIAG(x) do {} while (0)
#define LWIP_PLATFORM_ASSERT(x) do { fprintf(stderr, "lwIP assertion: %s\n", (x)); abort(); } while (0)
