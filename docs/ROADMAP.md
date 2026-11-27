# Hitos y puertas de aceptación

Todos pendientes salvo creación de documentación fundacional (M00 parcial). No son promesas de fechas. Una puerta exige evidencia; si falla, mantener estado experimental.

| Hito | Entregable / puerta | Dependencias |
|---|---|---|
| M00 | Inventario, principios, ADR, fuentes, licencias, amenazas y criterios revisados | — |
| M01 | SupraLab con patrones deterministas, capturas, métricas y material de referencia | M00 |
| M02 | Compositor anidado, cliente Wayland, foco, entrada, resize y cierre | M00 |
| M03 | SupraGlass con blur, composición, sombras y refracción calibrables; fallback | M01, M02 |
| M04 | SupraUI con controles accesibles, tokens y acciones semánticas | M01, M02 |
| M05 | Desktop, Dock, barra superior, lanzamiento y registro de apps | M02–M04 |
| M06 | Spaces, Mission Control, Exposé, tiling, fullscreen, Stage Manager | M05 |
| M07 | Sesión nativa DRM/KMS, XWayland, escalado y bloqueo fail-closed | M02, M05 |
| M08 | Servicios, portales, búsqueda, notificaciones, control | M04, M05 |
| M09 | Files, Settings, Preview, Terminal, Monitor, Archive | M04, M08 |
| M10 | Matriz Qt/GTK/Electron/Flatpak/Steam/Wine | M07–M09 |
| M11 | Hardening, accesibilidad, rendimiento, recuperación, pruebas de permisos | M07–M10 |
| M12 | PKGBUILD, repositorio Arch congelado, imagen atómica y sesión/ISO experimental | M11 |
| M13 | Auditoría y 1.0 únicamente con aprobación y puertas satisfechas | M12 |

AI Ready transversal desde M00: catálogo y permisos en M01–M04; acciones nativas y supervisión en M05–M08; MCP, portales y sesión aislada validados antes de declarar AI Ready operativo. Cada módulo entrega API, pruebas negativas y accesibilidad junto a UI, no como tarea final.

Versiones tentativas 0.0.1 laboratorio, 0.1 compositor, 0.2 glass, 0.3 UI, 0.4 shell, 0.5 ventanas, 0.6 sesión, 0.7 servicios, 0.8 apps, 0.9 beta. No crear tags/releases automáticamente.

## Puertas de riesgo previas a expansión
[Matriz experimental](testing/EXPERIMENT_MATRIX.md): toolkit y a11y, render offscreen y composición, región blur, cliente externo, contrato de acción, ciclo de sesión y modelo de permisos. No esperar a M11 para identificar bloqueos estructurales; incorporar pruebas tempranas. La puerta M00 requiere resolver o registrar formalmente cada incertidumbre crítica; no exigir medición física completa para comenzar fixtures sintéticos.

## Dependencias S01–S15
Consultar [puertas](testing/SUBSYSTEM_VALIDATION.md) para ordenar experimentos por riesgo. S07/S06 antes de tokens dispersos; S04/S02 antes de shell masivo; S05/S08/S09 antes de Files; S11 desde compositor inicial; S12 con app registry; S14 y S15 no bloquean sesión básica. Licencias y AI Ready transversales.

## Plataforma A00/P16–P30
[Puertas](testing/PLATFORM_VALIDATION.md). Diagnóstico y event loop desde M01/M02; composición y memoria antes de efectos avanzados; IME/a11y/i18n en primer toolkit; registro antes de Dock y Applications; gestión de sesión y energía antes de despliegue; remoting externo y formato propio no bloquean núcleo.

## Track D — distribución oficial candidata
[D00–D12](distribution/VALIDATION.md) paralelo al escritorio, sin bloquear M01/M02. D00 arquitectura; D01–D04 construir base y probar actualización/rollback en VM; D05–D08 módulos y aplicaciones; D09–D12 seguridad, hardware, instalador, distribución. Ninguna imagen estable sin aprobación. No sustituir el proyecto SupraLINUX existente.

## Siguiente corte verificable
[Prototipos D00 y M01/M02](distribution/PROTOTYPE_GATES.md). No ampliar scope antes de obtener evidencia. Mantener contratos portables del escritorio y de SupraDeploy; prototipo de distribución no bloquea compositor anidado. Backend candidato Arkdep, Calamares frontend adaptado, ninguna selección definitiva ni release.

## Límites de integración
[Repositorios](architecture/REPOSITORY_BOUNDARIES.md) y [laboratorio](testing/VALIDATION_LAB.md). M12 escritorio entrega paquete/integración reproducible; track D entrega imagen/ISO y release de distribución. Evitar que M12 obligue al escritorio a depender de Arkdep. Medición macOS nativa pendiente, fixtures sintéticos pueden comenzar.

## Separación ratificada
Track D se ejecuta en [SupraLINUX](https://github.com/SupraLINUX/SupraLINUX/blob/docs/m00-import/docs/distribution/VALIDATION.md), con CI y releases propios. M12 solo empaquetado/integración portable de escritorio. No bloquear compositor por distro ni viceversa. Nombre definitivo SupraLINUX; repositorio de la organización ya existente.

## Inicio de M01 — 2026-11-25
Workspace Rust, supra-geometry y supra-lab implementados como primer corte CPU; CI verifica tests, determinismo y artefactos. No implica M01 PASS ni fidelidad macOS. [Laboratorio](../tools/supra-lab/README.md). Licencia pendiente antes de importar dependencias.

M01 second increment: CPU Gaussian reference and local G2 cubic candidate, independent geometry/glass crates and three deterministic fixtures. CI pending. M01 full gate and M02 still pending.

## M01 evidence / M02 backend-independent preparation
[Run 45869337260](https://github.com/SupraLINUX/SupraDesktop/actions/runs/45869337260): 64 CPU incremental frames identical to full redraw; 414466 vs 1572864 stage-pixel evaluations, unchanged frame zero. No performance or GPU claim. M02 preparation: original dependency-free supra-window crate for toplevel lifecycle, internal configure token, commit, stacking, explicit activation, advisory close, unmap and destroy. Tests pending. Not an actual Wayland compositor, not M02 PASS. Smithay/license review remains open.

## M02 — Reference environment probe
[Run 45872169105](https://github.com/SupraLINUX/SupraDesktop/actions/runs/45872169105): SupraWindow lifecycle, existing workspace, Clippy and fixtures PASS. New independent Wayland lab runs packaged Weston headless/Pixman with an isolated runtime socket and external SHM client. Requires globals, per-object xdg configure/ack, non-null buffer attach/commit and a frame callback; emits trace and package versions. This validates reference infrastructure only, not SupraCompositor. No source or runtime dependency adopted; Smithay and license decisions remain open. CI pending.

## M02 — Smithay candidate lab
[Run 45934029405](https://github.com/SupraLINUX/SupraDesktop/actions/runs/45934029405): real external Weston/SHM trace PASS. Added isolated upstream Smallvil build and nested runtime at Smithay v0.7.0 commit a166cf4c94b5aedc332a65aa1dd753e8148829c3. Collect dependency/feature inventory, generated lock and toolchain; use existing trace validator. Does not import code into the project or select final backend. CI pending; license ratification and transitive review remain required.

## M02 — Upstream candidate PASS and multiple real clients
[Run 45998105413](https://github.com/SupraLINUX/SupraDesktop/actions/runs/45998105413): exact Smallvil revision built and served a real nested SHM client; configure/commit/frame evidence PASS. Added a reusable two-client test to both external reference backends: wait for real frame evidence on two independent connections, terminate one, record surviving client's callback count after exit, require at least three additional callbacks and both servers alive. Logs and JSON record actual counts. Not a security isolation or GPU test and not own compositor. CI pending. License audit/ratification still required before integrating Smithay into own runtime.

## M02 — Window lifecycle correction
The external Weston and Smallvil multiclient runs [45998837761](https://github.com/SupraLINUX/SupraDesktop/actions/runs/45998837761) and [45998837793](https://github.com/SupraLINUX/SupraDesktop/actions/runs/45998837793) passed. Correct backend-independent configure ordering, bounded backpressure, optional size hints and advisory close in SupraWindow, with regression tests; see [Wayland specification](specifications/WAYLAND_SPEC.md). CI pending. M02 is not complete, license and backend adoption remain open.

## M02 — Connection lifecycle preparation
[Run 46014792298](https://github.com/SupraLINUX/SupraDesktop/actions/runs/46014792298): nested reference runtime PASS under 1.100.0; offline cargo check with the same lock on Rust 1.88.0 exit 0, not a runtime test of 1.88. Keep workspace Rust unchanged. Original connection-scoped window dispatch and disconnect cleanup implemented in separate SupraWindow module, no external dependencies. Tests pending. Runtime adoption and license ratification remain open.

## M02 — Connection model PASS and pinned lab resolution
[Run 46020408962](https://github.com/SupraLINUX/SupraDesktop/actions/runs/46020408962): workspace tests, Clippy and deterministic fixtures PASS with connection dispatch and disconnect tests. Reference dependency resolution hash now gated against reviewed artifact; build/run on workspace Rust 1.88.0 instead of check-only, CI pending. This is the final outstanding toolchain runtime question, not own compositor implementation. Own license ratification and adoption review remain required.

## M02 — Rust runtime PASS / dependency evidence
[Run 46027514910](https://github.com/SupraLINUX/SupraDesktop/actions/runs/46027514910): exact reviewed lock and actual nested Smallvil runtime with Rust 1.88.0 PASS, two clients and survivor progress. Preserve workspace Rust; new crate license-file/native link inventory CI pending. License decision remains PROPOSED and no dependency adopted.

## M02 — Wire serial translation preparation
Original, dependency-free WireWindows adapter maps backend-supplied uint32 configure serials to internal u64 tokens, preserves FIFO across wrap, clears pending mappings with surface/connection lifecycle, tests invalid serial and queue backpressure. See Wayland specification. CI pending, not a real compositor or license adoption.

## M02 — Wire adapter PASS / buffer ledger
[Run 46032784564](https://github.com/SupraLINUX/SupraDesktop/actions/runs/46032784564): workspace tests, Clippy and deterministic fixtures PASS, including serial adapter. New original buffer ownership/retirement ledger with connection-scoped object reuse, trusted completion and disconnect tests. No actual graphics import, GPU fence or protocol runtime yet. CI pending.

## M02 — Buffer ledger PASS / surface state contract
[Run 46043642471](https://github.com/SupraLINUX/SupraDesktop/actions/runs/46043642471): workspace tests, Clippy and deterministic fixtures PASS including buffer ledger. Add original pure pending/current surface state, preview, commit generations, explicit detach vs unchanged, integer scale/transform and frame request handoff/discard. Tests pending. Actual transport, role validation, presentation scheduler and renderer remain to integrate. No external dependency or license adoption.
