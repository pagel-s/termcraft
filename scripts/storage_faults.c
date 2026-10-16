// Linux-only test interposer. It can affect only checkpoint files below the
// disposable root explicitly supplied by test_storage_recovery.py.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <signal.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

static ssize_t (*next_write)(int, const void *, size_t);
static int (*next_fsync)(int);
static int (*next_rename)(const char *, const char *);
static pthread_once_t initialized = PTHREAD_ONCE_INIT;
static atomic_int partial_writes;
static atomic_int renames;
static void initialize(void) {
    next_write = dlsym(RTLD_NEXT, "write");
    next_fsync = dlsym(RTLD_NEXT, "fsync");
    next_rename = dlsym(RTLD_NEXT, "rename");
}
static int target(const char *path) {
    const char *root = getenv("TERMCRAFT_TEST_FAULT_ROOT");
    if (!root || !*root || strncmp(root, path, strlen(root)) || path[strlen(root)] != '/') return 0;
    const char *base = strrchr(path, '/');
    if (!base) return 0;
    const char *selector = getenv("TERMCRAFT_TEST_FAULT_FILE");
    if (!selector) selector = "player_progression.bin.tmp";
    const char *relative = path + strlen(root) + 1;
    if (!strcmp(selector, "all")) return 1;
    if (!strcmp(selector, "commit")) return !strcmp(relative, ".world-checkpoint/complete.tmp");
    if (!strcmp(selector, "restore")) return strstr(base + 1, ".restore-") != NULL;
    if (!strcmp(selector, "chunks")) return !strchr(relative, '/') && strstr(relative, "_chunk_") && strstr(relative, ".bin.tmp");
    if (!strcmp(selector, "server")) return !strchr(relative, '/') && !strncmp(relative, ".server-state-", 14);
    return !strcmp(relative, selector);
}
static int target_fd(int fd) {
    char proc[64], path[4096];
    snprintf(proc, sizeof(proc), "/proc/self/fd/%d", fd);
    ssize_t size = readlink(proc, path, sizeof(path) - 1);
    if (size < 0) return 0;
    path[size] = 0;
    return target(path);
}
static void mode(char *buffer, size_t size) {
    buffer[0] = 0;
    const char *control = getenv("TERMCRAFT_TEST_FAULT_CONTROL");
    if (!control) return;
    int fd = open(control, O_RDONLY | O_CLOEXEC);
    if (fd < 0) return;
    ssize_t n = read(fd, buffer, size - 1);
    close(fd);
    if (n > 0) buffer[n] = 0;
}
static void mark(const char *text) {
    const char *path = getenv("TERMCRAFT_TEST_FAULT_MARKER");
    if (!path) return;
    int fd = open(path, O_WRONLY | O_CREAT | O_APPEND | O_CLOEXEC, 0600);
    if (fd >= 0) { (void)next_write(fd, text, strlen(text)); close(fd); }
}
ssize_t write(int fd, const void *bytes, size_t size) {
    pthread_once(&initialized, initialize);
    if (target_fd(fd)) {
        char fault[64]; mode(fault, sizeof(fault));
        if (!strcmp(fault, "full")) { mark("write-enospc\n"); errno = ENOSPC; return -1; }
        if (!strcmp(fault, "partial")) {
            if (atomic_fetch_add(&partial_writes, 1) == 0 && size > 1) {
                mark("partial-write\n"); return next_write(fd, bytes, size / 2);
            }
            mark("partial-enospc\n"); errno = ENOSPC; return -1;
        }
        if (!strcmp(fault, "freeze-write") && size > 1) {
            ssize_t result = next_write(fd, bytes, size / 2);
            mark("freeze-partial-write\n"); raise(SIGSTOP);
            return result;
        }
        if (!strcmp(fault, "slow")) {
            mark("slow-write-enter\n");
            struct timespec delay = { .tv_sec = 0, .tv_nsec = 300000000 };
            while (nanosleep(&delay, &delay) && errno == EINTR) {}
        }
    }
    return next_write(fd, bytes, size);
}
int fsync(int fd) {
    pthread_once(&initialized, initialize);
    if (target_fd(fd)) {
        char fault[64]; mode(fault, sizeof(fault));
        if (!strcmp(fault, "sync")) { mark("sync-eio\n"); errno = EIO; return -1; }
    }
    return next_fsync(fd);
}
int rename(const char *source, const char *destination) {
    pthread_once(&initialized, initialize);
    if (target(source)) {
        char fault[64]; mode(fault, sizeof(fault));
        if (!strncmp(fault, "freeze-", 7) || !strcmp(fault, "rename")) {
            const char *after = getenv("TERMCRAFT_TEST_FAULT_AFTER");
            int nth = after ? atoi(after) : 1;
            if (atomic_fetch_add(&renames, 1) + 1 == nth) {
                if (!strcmp(fault, "rename")) { mark("rename-eio\n"); errno = EIO; return -1; }
                if (!strcmp(fault, "freeze-before")) { mark("freeze-before-rename\n"); raise(SIGSTOP); }
                if (!strcmp(fault, "freeze-after")) {
                    int result = next_rename(source, destination);
                    if (!result) { mark("freeze-after-rename\n"); raise(SIGSTOP); }
                    return result;
                }
            }
        }
        if (!strcmp(fault, "crash-before")) { mark("crash-before-rename\n"); _exit(97); }
        if (!strcmp(fault, "crash-after")) {
            int result = next_rename(source, destination);
            if (!result) { mark("crash-after-rename\n"); _exit(98); }
            return result;
        }
    }
    return next_rename(source, destination);
}
