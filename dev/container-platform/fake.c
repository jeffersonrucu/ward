/* Servidor falso do V4.2, 2ª parte (Fase 28; D46, D65 e; S5), só para o contêiner do
 * `make test-docker-platform`. Roda como o uid 1000, se marca não-dumpable
 * (`prctl(PR_SET_DUMPABLE, 0)`, o que passaria na conferência extra do `test-mode`) e
 * escuta num socket Unix numa pasta desse uid. Toda conexão aceita e todo byte recebido
 * vão para o registro (uma linha por evento, sem o conteúdo); o teste confere que o
 * registro fica vazio: a CLI de release nunca conecta num falso.
 *
 * Uso: falso <socket> <registro> <segundos>. Imprime "pronto dumpable=<n>" quando
 * escuta; sai 3 se o bind falhar (o caso de /run/ward/<uid>/, que é do root).
 *
 * `falso --ativar <socket> <programa>`: o papel do systemd feito pelo próprio usuário
 * (sem root): cria o socket numa pasta dele e executa <programa> com ele no fd 3 e
 * LISTEN_FDS=1/LISTEN_PID (o `wardd` copiado tenta subir assim).
 *
 * `falso --conectar <socket> x`: controle do teste (conecta e manda um byte), para
 * provar que o registro pega uma conexão de verdade. */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/prctl.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <time.h>
#include <unistd.h>

static void registrar(int fd, const char *evento, long n) {
    char linha[64];
    int k = snprintf(linha, sizeof linha, "%s %ld\n", evento, n);
    if (k > 0 && write(fd, linha, (size_t)k) != k) {
        _exit(4);
    }
}

static int ativar(const char *caminho, const char *programa) {
    int s = socket(AF_UNIX, SOCK_STREAM, 0);
    struct sockaddr_un a;
    memset(&a, 0, sizeof a);
    a.sun_family = AF_UNIX;
    if (s < 0 || strlen(caminho) >= sizeof a.sun_path) {
        return 2;
    }
    strcpy(a.sun_path, caminho);
    unlink(caminho);
    if (bind(s, (struct sockaddr *)&a, sizeof a) != 0 || listen(s, 16) != 0) {
        perror("bind");
        return 3;
    }
    if (s != 3 && (dup2(s, 3) != 3 || close(s) != 0)) {
        return 2;
    }
    char pid[32];
    snprintf(pid, sizeof pid, "%d", (int)getpid());
    setenv("LISTEN_FDS", "1", 1);
    setenv("LISTEN_PID", pid, 1);
    execl(programa, programa, (char *)NULL);
    perror("execl");
    return 127;
}

static int conectar(const char *caminho) {
    int s = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
    struct sockaddr_un a;
    memset(&a, 0, sizeof a);
    a.sun_family = AF_UNIX;
    if (s < 0 || strlen(caminho) >= sizeof a.sun_path) {
        return 2;
    }
    strcpy(a.sun_path, caminho);
    if (connect(s, (struct sockaddr *)&a, sizeof a) != 0 || write(s, "x", 1) != 1) {
        return 3;
    }
    close(s);
    return 0;
}

int main(int argc, char **argv) {
    if (argc == 4 && strcmp(argv[1], "--conectar") == 0) {
        return conectar(argv[2]);
    }
    if (argc == 4 && strcmp(argv[1], "--ativar") == 0) {
        return ativar(argv[2], argv[3]);
    }
    if (argc != 4) {
        fprintf(stderr, "uso: falso <socket> <registro> <segundos>\n");
        return 2;
    }
    if (prctl(PR_SET_DUMPABLE, 0, 0, 0, 0) != 0) {
        perror("prctl");
        return 2;
    }
    int reg = open(argv[2], O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC, 0600);
    if (reg < 0) {
        perror("registro");
        return 2;
    }
    int s = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
    struct sockaddr_un a;
    memset(&a, 0, sizeof a);
    a.sun_family = AF_UNIX;
    if (s < 0 || strlen(argv[1]) >= sizeof a.sun_path) {
        return 2;
    }
    strcpy(a.sun_path, argv[1]);
    unlink(argv[1]);
    if (bind(s, (struct sockaddr *)&a, sizeof a) != 0) {
        printf("bind=%s\n", errno == EACCES ? "EACCES" : strerror(errno));
        return 3;
    }
    if (listen(s, 16) != 0) {
        return 2;
    }
    printf("pronto dumpable=%d\n", prctl(PR_GET_DUMPABLE, 0, 0, 0, 0));
    fflush(stdout);
    time_t fim = time(NULL) + atoi(argv[3]);
    while (time(NULL) < fim) {
        struct pollfd p = {.fd = s, .events = POLLIN};
        if (poll(&p, 1, 100) <= 0) {
            continue;
        }
        int c = accept4(s, NULL, NULL, SOCK_CLOEXEC);
        if (c < 0) {
            continue;
        }
        registrar(reg, "conexao", 0);
        /* Um falso de verdade responderia como o wardd e esperaria a senha. */
        char buf[4096];
        struct pollfd q = {.fd = c, .events = POLLIN};
        while (poll(&q, 1, 2000) > 0) {
            ssize_t n = read(c, buf, sizeof buf);
            if (n <= 0) {
                break;
            }
            registrar(reg, "bytes", (long)n);
        }
        close(c);
    }
    return 0;
}
