/*
 * Minimal freestanding string helpers for the hello user payload.
 *
 * The lab-9 reference tree lists string.c in service/hello/Makefile but does
 * not ship the file; hello.c needs compiler-emitted memset/memcpy (struct
 * copies and array initialisation under -ffreestanding). This file is the
 * student's minimal reconstruction to make the userland payload link, kept
 * deliberately small and dependency-free.
 */

typedef unsigned long size_t;

void *memset(void *dst, int c, size_t n)
{
    unsigned char *d = (unsigned char *)dst;
    while (n-- > 0)
        *d++ = (unsigned char)c;
    return dst;
}

void *memcpy(void *dst, const void *src, size_t n)
{
    unsigned char *d = (unsigned char *)dst;
    const unsigned char *s = (const unsigned char *)src;
    while (n-- > 0)
        *d++ = *s++;
    return dst;
}

void *memmove(void *dst, const void *src, size_t n)
{
    unsigned char *d = (unsigned char *)dst;
    const unsigned char *s = (const unsigned char *)src;
    if (d < s) {
        while (n-- > 0)
            *d++ = *s++;
    } else {
        d += n;
        s += n;
        while (n-- > 0)
            *--d = *--s;
    }
    return dst;
}
