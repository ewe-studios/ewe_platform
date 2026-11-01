/* vfs_shim.c — variadic libc entry points forwarded to the Rust shim.
 *
 * libc's `open` and `openat` are variadic: the mode argument exists only when
 * `flags` carries `O_CREAT` or `O_TMPFILE`. Stable Rust cannot define a
 * variadic `extern "C"` function, so a fixed-argument Rust definition gets the
 * mode on the wrong register on some architectures. These C entry points read
 * the optional mode with `va_arg` and forward to the Rust handlers, which
 * decide whether the path is virtual or must reach the real libc call through
 * `RTLD_NEXT`.
 */

#define _GNU_SOURCE 1

#include <fcntl.h>
#include <stdarg.h>

/* Rust handlers defined in `src/native/vfs/shim/mod.rs`. */
int foundation_vfs_open(const char *pathname, int flags, unsigned int mode);
int foundation_vfs_open64(const char *pathname, int flags, unsigned int mode);
int foundation_vfs_openat(int dirfd, const char *pathname, int flags,
                          unsigned int mode);

/* Read the mode only when the flags promise one; otherwise there is no mode
 * argument to read, and reading it would consume the caller's register. */
static unsigned int shim_open_mode(int flags, va_list ap) {
    if (flags & (O_CREAT | O_TMPFILE)) {
        return va_arg(ap, unsigned int);
    }
    return 0;
}

int open(const char *pathname, int flags, ...) {
    va_list ap;
    va_start(ap, flags);
    unsigned int mode = shim_open_mode(flags, ap);
    va_end(ap);
    return foundation_vfs_open(pathname, flags, mode);
}

int open64(const char *pathname, int flags, ...) {
    va_list ap;
    va_start(ap, flags);
    unsigned int mode = shim_open_mode(flags, ap);
    va_end(ap);
    return foundation_vfs_open64(pathname, flags, mode);
}

int openat(int dirfd, const char *pathname, int flags, ...) {
    va_list ap;
    va_start(ap, flags);
    unsigned int mode = shim_open_mode(flags, ap);
    va_end(ap);
    return foundation_vfs_openat(dirfd, pathname, flags, mode);
}

int __openat64_time64(int dirfd, const char *pathname, int flags, ...) {
    va_list ap;
    va_start(ap, flags);
    unsigned int mode = shim_open_mode(flags, ap);
    va_end(ap);
    return foundation_vfs_openat(dirfd, pathname, flags, mode);
}
