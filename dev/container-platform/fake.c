/* Fake server of V4.2, 2nd part (Phase 28; D46, D65 e; S5), only for the container of
 * `make test-docker-platform` (and of `make test-install`). Runs as uid 1000, marks itself
 * non-dumpable (`prctl(PR_SET_DUMPABLE, 0)`, which would pass the extra check of
 * `test-mode`) and listens on a Unix socket in a folder of that uid. Every accepted
 * connection and every byte received go to the record (one line per event, without the
 * content); the test verifies that the record stays empty: the release CLI never
 * connects to a fake.
 *
 * Usage: <binary> <socket> <record> <seconds> (the test scripts install it under a short
 * name inside the container). Prints the ready marker with "dumpable=<n>" when it listens;
 * exits 3 if the bind fails (the case of /run/ward/<uid>/, which belongs to root).
 *
 * `<binary> --activate <socket> <program>`: the role of systemd done by the user itself
 * (without root): creates the socket in a folder of the user and executes <program> with
 * it on fd 3 and LISTEN_FDS=1/LISTEN_PID (the copied `wardd` tries to start like this).
 *
 * `<binary> --connect <socket> x`: control of the test (connects and sends one byte), to
 * prove that the record catches a real connection. */
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

static void record(int fd, const char *event, long n) {
    char line[64];
    int k = snprintf(line, sizeof line, "%s %ld\n", event, n);
    if (k > 0 && write(fd, line, (size_t)k) != k) {
        _exit(4);
    }
}

static int activate(const char *path, const char *program) {
    int s = socket(AF_UNIX, SOCK_STREAM, 0);
    struct sockaddr_un a;
    memset(&a, 0, sizeof a);
    a.sun_family = AF_UNIX;
    if (s < 0 || strlen(path) >= sizeof a.sun_path) {
        return 2;
    }
    strcpy(a.sun_path, path);
    unlink(path);
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
    execl(program, program, (char *)NULL);
    perror("execl");
    return 127;
}

static int connect_once(const char *path) {
    int s = socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC, 0);
    struct sockaddr_un a;
    memset(&a, 0, sizeof a);
    a.sun_family = AF_UNIX;
    if (s < 0 || strlen(path) >= sizeof a.sun_path) {
        return 2;
    }
    strcpy(a.sun_path, path);
    if (connect(s, (struct sockaddr *)&a, sizeof a) != 0 || write(s, "x", 1) != 1) {
        return 3;
    }
    close(s);
    return 0;
}

int main(int argc, char **argv) {
    if (argc == 4 && strcmp(argv[1], "--connect") == 0) {
        return connect_once(argv[2]);
    }
    if (argc == 4 && strcmp(argv[1], "--activate") == 0) {
        return activate(argv[2], argv[3]);
    }
    if (argc != 4) {
        fprintf(stderr, "uso: falso <socket> <registro> <segundos>\n");
        return 2;
    }
    if (prctl(PR_SET_DUMPABLE, 0, 0, 0, 0) != 0) {
        perror("prctl");
        return 2;
    }
    int rec = open(argv[2], O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC, 0600);
    if (rec < 0) {
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
    time_t end = time(NULL) + atoi(argv[3]);
    while (time(NULL) < end) {
        struct pollfd p = {.fd = s, .events = POLLIN};
        if (poll(&p, 1, 100) <= 0) {
            continue;
        }
        int c = accept4(s, NULL, NULL, SOCK_CLOEXEC);
        if (c < 0) {
            continue;
        }
        record(rec, "conexao", 0);
        /* A real fake would answer like the wardd and wait for the password. */
        char buf[4096];
        struct pollfd q = {.fd = c, .events = POLLIN};
        while (poll(&q, 1, 2000) > 0) {
            ssize_t n = read(c, buf, sizeof buf);
            if (n <= 0) {
                break;
            }
            record(rec, "bytes", (long)n);
        }
        close(c);
    }
    return 0;
}
