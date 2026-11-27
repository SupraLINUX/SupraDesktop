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
[SupraWindow](../../crates/supra-window/src/lib.rs) models toplevel lifecycle before selecting the external compositor backend. Internal tokens are not Wayland serials; latest-unacknowledged coalescing is a lab policy, not a protocol rule. Actual buffer dimensions may differ from a suggestion. Trusted activation is separate from client requests; no authorization or wire implementation is provided. Close is advisory and unmap resets configure state; explicit focus clearing is conservative, not a final focus policy. Window identity, seat/client isolation, buffer release, real configure/ack queue, input serial validation and protocol conformance belong to the future backend. Tests cover invalid/stale state, focus, close/remap and destruction; CI pending. No external dependency adopted.

## Real protocol baseline (external reference)
[Wayland lab](../../tools/wayland-lab/README.md): independent Weston headless CI, private socket, bounded SHM client, per-object trace checks and artifacts. Ubuntu provides the external reference compositor; no code imported into SupraDesktop. Reuse the same client and trace contract when the real compositor backend is authorized. Focus, input, resize, screenshots, buffer release and own compositor remain NOT_RUN.
