/* Attack library of Phase 21b (V3.24): loaded by LD_PRELOAD, it writes a mark file in
 * the constructor. In a setgid binary the ld.so in secure mode (AT_SECURE) ignores the
 * LD_PRELOAD with a slash, and the mark cannot appear. It only runs in the container. */
#include <fcntl.h>
#include <unistd.h>

__attribute__((constructor)) static void mark(void) {
    int fd = open("/tmp/ward-marca", O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC, 0666);
    if (fd >= 0) {
        if (write(fd, "carregada\n", 10) < 0) {
            /* nothing to do: the mark is the open */
        }
        close(fd);
    }
}
