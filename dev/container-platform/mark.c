/* Biblioteca de ataque da Fase 21b (V3.24): carregada por LD_PRELOAD, grava um
 * arquivo-marca no construtor. Num binário setgid o ld.so em modo seguro (AT_SECURE)
 * ignora o LD_PRELOAD com barra, e a marca não pode aparecer. Só roda no contêiner. */
#include <fcntl.h>
#include <unistd.h>

__attribute__((constructor)) static void marcar(void) {
    int fd = open("/tmp/ward-marca", O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC, 0666);
    if (fd >= 0) {
        if (write(fd, "carregada\n", 10) < 0) {
            /* nada a fazer: a marca é o open */
        }
        close(fd);
    }
}
