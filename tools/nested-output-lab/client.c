/* Disposable Wayland output transport. Generated protocol bindings come
 * from the CI system package; no production code or dependencies imported. */
#define _GNU_SOURCE
#include <wayland-client.h>
#include "xdg-shell-client-protocol.h"
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <unistd.h>

enum { WIDTH = 160, HEIGHT = 96, BYTES = WIDTH * HEIGHT * 4 };
struct app {
    struct wl_display *display;
    struct wl_compositor *compositor;
    struct wl_shm *shm;
    struct xdg_wm_base *wm;
    bool configured, closed, frame;
};
struct image {
    struct wl_buffer *buffer;
    uint32_t *pixels;
    bool released;
};
static void check(bool ok, const char *message) {
    if (!ok) { fprintf(stderr, "%s\n", message); exit(EXIT_FAILURE); }
}
static void ping(void *data, struct xdg_wm_base *wm, uint32_t serial) {
    (void)data; xdg_wm_base_pong(wm, serial);
}
static const struct xdg_wm_base_listener wm_listener = {.ping = ping};
static void global(void *data, struct wl_registry *registry, uint32_t name, const char *interface, uint32_t version) {
    struct app *a = data;
    (void)version;
    if (!strcmp(interface, "wl_compositor") && !a->compositor)
        a->compositor = wl_registry_bind(registry, name, &wl_compositor_interface, 1);
    else if (!strcmp(interface, "wl_shm") && !a->shm)
        a->shm = wl_registry_bind(registry, name, &wl_shm_interface, 1);
    else if (!strcmp(interface, "xdg_wm_base") && !a->wm) {
        a->wm = wl_registry_bind(registry, name, &xdg_wm_base_interface, 1);
        xdg_wm_base_add_listener(a->wm, &wm_listener, a);
    }
}
static void removed(void *data, struct wl_registry *registry, uint32_t name) {
    (void)data; (void)registry; (void)name;
}
static const struct wl_registry_listener registry_listener = {.global = global, .global_remove = removed};
static void configured(void *data, struct xdg_surface *surface, uint32_t serial) {
    struct app *a = data;
    xdg_surface_ack_configure(surface, serial);
    a->configured = true;
}
static const struct xdg_surface_listener surface_listener = {.configure = configured};
static void toplevel_configure(void *data, struct xdg_toplevel *top, int32_t w, int32_t h, struct wl_array *states) {
    (void)data; (void)top; (void)w; (void)h; (void)states;
}
static void closed(void *data, struct xdg_toplevel *top) {
    (void)top; ((struct app *)data)->closed = true;
}
static const struct xdg_toplevel_listener top_listener = {.configure = toplevel_configure, .close = closed};
static void done(void *data, struct wl_callback *cb, uint32_t time) {
    (void)time; ((struct app *)data)->frame = true; wl_callback_destroy(cb);
}
static const struct wl_callback_listener frame_listener = {.done = done};
static void released(void *data, struct wl_buffer *buffer) {
    (void)buffer; ((struct image *)data)->released = true;
}
static const struct wl_buffer_listener buffer_listener = {.release = released};
static void pump(struct app *a) {
    check(!a->closed && wl_display_dispatch(a->display) >= 0, "Wayland disconnected or closed");
}
static void prepare(struct app *a, struct image *image) {
    uint8_t rgba[BYTES];
    check(fread(rgba, 1, sizeof rgba, stdin) == sizeof rgba, "Incomplete renderer frame");
    int fd = memfd_create("supra-nested-output", MFD_CLOEXEC);
    check(fd >= 0 && ftruncate(fd, BYTES) == 0, "Cannot allocate shared frame");
    image->pixels = mmap(NULL, BYTES, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    check(image->pixels != MAP_FAILED, "Cannot map frame");
    for (size_t n = 0; n < WIDTH * HEIGHT; ++n) {
        const uint8_t *p = &rgba[n*4];
        check(p[3] == 255, "Nonopaque composed output");
        image->pixels[n] = 0xff000000u | ((uint32_t)p[0] << 16) | ((uint32_t)p[1] << 8) | p[2];
    }
    struct wl_shm_pool *pool = wl_shm_create_pool(a->shm, fd, BYTES);
    close(fd);
    check(pool != NULL, "Cannot create pool");
    image->buffer = wl_shm_pool_create_buffer(pool, 0, WIDTH, HEIGHT, WIDTH*4, WL_SHM_FORMAT_XRGB8888);
    wl_shm_pool_destroy(pool);
    check(image->buffer != NULL, "Cannot create buffer");
    wl_buffer_add_listener(image->buffer, &buffer_listener, image);
}
int main(void) {
    struct app a = {0};
    struct image images[2] = {0};
    a.display = wl_display_connect(NULL);
    check(a.display != NULL, "Cannot connect to private compositor");
    struct wl_registry *registry = wl_display_get_registry(a.display);
    wl_registry_add_listener(registry, &registry_listener, &a);
    check(wl_display_roundtrip(a.display) >= 0 && a.compositor && a.shm && a.wm, "Missing globals");
    struct wl_surface *surface = wl_compositor_create_surface(a.compositor);
    struct xdg_surface *xdg = xdg_wm_base_get_xdg_surface(a.wm, surface);
    struct xdg_toplevel *top = xdg_surface_get_toplevel(xdg);
    xdg_surface_add_listener(xdg, &surface_listener, &a);
    xdg_toplevel_add_listener(top, &top_listener, &a);
    xdg_toplevel_set_title(top, "SupraDesktop optical output lab");
    wl_surface_commit(surface);
    while (!a.configured) pump(&a);
    for (unsigned n = 0; n < 2; ++n) {
        prepare(&a, &images[n]);
        a.frame = false;
        struct wl_callback *callback = wl_surface_frame(surface);
        wl_callback_add_listener(callback, &frame_listener, &a);
        wl_surface_attach(surface, images[n].buffer, 0, 0);
        wl_surface_damage(surface, 0, 0, WIDTH, HEIGHT);
        wl_surface_commit(surface);
        while (!a.frame) pump(&a);
        printf("FRAME %u\n", n+1); fflush(stdout);
        /* Hold the mapped frame until parent acknowledges; in the readback
         * job the independent host capture must finish before this byte. */
        check(fgetc(stdin) == 'A', "Missing renderer frame acknowledgment");
    }
    // Storage lifetime is independent of the drawing callback. Two distinct
    // allocations; retain both until the actual release events.
    wl_surface_attach(surface, NULL, 0, 0);
    wl_surface_commit(surface);
    while (!images[0].released || !images[1].released) pump(&a);
    puts("RELEASE 2"); fflush(stdout);
    for (unsigned n = 0; n < 2; ++n) {
        wl_buffer_destroy(images[n].buffer);
        munmap(images[n].pixels, BYTES);
    }
    xdg_toplevel_destroy(top);
    xdg_surface_destroy(xdg);
    wl_surface_destroy(surface);
    xdg_wm_base_destroy(a.wm);
    wl_shm_destroy(a.shm);
    wl_compositor_destroy(a.compositor);
    wl_registry_destroy(registry);
    check(wl_display_roundtrip(a.display) >= 0, "Final protocol roundtrip failed");
    wl_display_disconnect(a.display);
    return 0;
}
