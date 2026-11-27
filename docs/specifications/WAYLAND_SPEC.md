# Protocolos y compatibilidad Wayland

Prioridad inicial (no implica soporte actual de Smithay).
- P0: xdg-shell, wl_shm, linux-dmabuf, xdg-decoration, wlr-layer-shell, xdg-activation, viewporter, fractional-scale, ext-session-lock, linux-drm-syncobj.
- P1: presentation-time, ext-foreign-toplevel-list, ext-workspace, pointer-constraints, relative-pointer, text-input-v3, input-method-v2, idle-inhibit, ext-image-copy-capture.
- P2: color-management, integración KDE AppMenu, protocolos adicionales según apps.

Separar permisos de protocolos privilegiados del cliente estándar. No permitir lectura de otras ventanas ni entrada emulada arbitraria. XWayland como compatibilidad; documentar límites de aislamiento X11. Validar clientes GTK, Qt, Electron y juegos; WLCS donde esté soportado. Lock: si cliente de bloqueo muere, compositor permanece bloqueado. Gestos y activación respetan foco, seat y seriales.

Fuente: [Wayland protocols](https://wayland.app/protocols/), [Smithay](https://github.com/Smithay/smithay), [WLCS](https://github.com/canonical/wlcs).

## Revisión upstream 2026-11-24
Smithay 0.7.0 documenta backend winit, DRM, GBM y módulos drm_syncobj, fractional_scale, session_lock y foreign_toplevel_list; presencia de API no es integración probada. ext-foreign-toplevel-list es intencionalmente mínimo y no cubre toda la gestión de ventanas: diseñar contrato autenticado del compositor para el shell. xdg-decoration negocia decoraciones; no asumir SSD universal. Diferenciar composición within-window (UI) y behind-window (compositor). No exponer textura del fondo a clientes arbitrarios. Fuentes: [Smithay backend](https://docs.rs/smithay/0.7.0/smithay/backend/), [Wayland](https://docs.rs/smithay/0.7.0/smithay/wayland/), [toplevel list](https://wayland.app/protocols/ext-foreign-toplevel-list-v1).

## Ampliación de protocolos candidatos
P0/P1: ext-background-effect-v1 (blur por región; soporte upstream a verificar para revisión Smithay elegida), cursor-shape, xdg-toplevel-icon, idle-notify, primary-selection, data-device (clipboard/DnD), tablet, keyboard-shortcuts-inhibit según casos. P1/P2: security-context (identidad de cliente sandbox proporcionada por launcher confiable, no reemplaza sandbox), xdg-session-management (restauración cooperativa, no recuperación transparente), DRM lease, color-management, explicit sync, tearing-control, fifo/commit-timing según soporte. [Análisis](../research/GRAPHICS_AND_PROTOCOLS.md). No exponer protocolos privilegiados a clientes arbitrarios. Adaptador KDE blur opcional; priorizar estándar. El protocolo de background-effect no transmite textura del fondo al cliente ni expresa toda la óptica SupraGlass.

## Backend-independent window model (experimental)
[SupraWindow](../../crates/supra-window/src/lib.rs) models toplevel lifecycle before selecting the external compositor backend. Internal tokens are not Wayland serials; the experimental model retains outstanding requests in order. Acknowledgement consumes the matching request and older requests, and the latest acknowledgement before commit supersedes earlier acknowledgements. Actual buffer dimensions may differ from a suggestion. Trusted activation is separate from client requests; no authorization or wire implementation is provided. Close is advisory and preserves mapping, pending configurations and focus; unmap resets configure state and clears focus if applicable. Window identity, seat/client isolation, buffer release, wire serial mapping and queue backpressure, input serial validation and protocol conformance belong to the future backend. Tests cover invalid/stale state, focus, close/remap and destruction; CI pending. No external dependency adopted.

## Real protocol baseline (external reference)
[Wayland lab](../../tools/wayland-lab/README.md): independent Weston headless CI, private socket, bounded SHM client, per-object trace checks and artifacts. Ubuntu provides the external reference compositor; no code imported into SupraDesktop. Reuse the same client and trace contract when the real compositor backend is authorized. Focus, input, resize, screenshots, buffer release and own compositor remain NOT_RUN.

## Pinned nested Smithay feasibility test
[Candidate lab](../../tools/smithay-lab/README.md) runs unmodified upstream Smallvil on Weston in separate ephemeral CI, with Mesa software rendering requested, private socket and the existing real-client evidence checks. No host session, no Smithay code imported into SupraDesktop. This is an external feasibility experiment, not M02 completion. CI pending.

## Concurrent clients (external reference)
[Multi-client harness](../../tools/wayland-lab/multiclient.py) runs two distinct SHM clients with separate traces against the isolated Weston or Smallvil socket. Requires configure/ack/commit/frame for both, then actual survivor callback progress after peer termination. It does not verify focus, resize, input, security isolation, frame presentation timing or our own compositor. CI pending.

## Configure ordering and advisory close correction
Source: [xdg-shell protocol](https://wayland.app/protocols/xdg-shell), xdg_surface.ack_configure and xdg_toplevel.configure/close. A client may acknowledge an earlier outstanding configure even when a newer one exists, acknowledge multiple before committing, choose dimensions when a hint is zero and continue drawing after an advisory close. The original model incorrectly rejected these cases. The backend-independent queue now retains sent tokens, consumes through an acknowledged token and rejects stale/foreign tokens. Maximum 256 outstanding requests per window is an experimental backpressure policy; the future backend must reserve capacity before emitting a configure, never silently discard a sent serial. Zero suggested dimensions are accepted while committed size remains positive within the existing lab ceiling. Close does not revoke focus or invalidate pending state. No wire implementation, actual buffer lifecycle or security conformance claimed; CI pending. No external source/dependency incorporated.
