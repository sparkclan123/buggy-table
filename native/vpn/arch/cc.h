#pragma once
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#ifdef _MSC_VER
#include <BaseTsd.h>
#if !defined(_SSIZE_T_) && !defined(_SSIZE_T_DEFINED)
typedef SSIZE_T ssize_t;
#define _SSIZE_T_
#define _SSIZE_T_DEFINED
#endif
#ifndef SSIZE_MAX
#define SSIZE_MAX INTPTR_MAX
#endif
#define LWIP_NO_UNISTD_H 1
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
