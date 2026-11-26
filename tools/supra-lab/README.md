# SupraLab — primer incremento M01
Ejecutar cargo test --workspace --locked y cargo run --locked -p supra-lab -- evidence. Fixture CPU 640x360, checkerboard sintético, arcos circulares a la izquierda y esquinas superelípticas locales n=4 a la derecha; dos proporciones. No usa recursos Apple. PPM y JSON; CI produce PNG, hash y compara dos ejecuciones. No es un screenshot del escritorio, ni una medición nativa, ni una implementación GPU. Geometría canónica separada de herramienta; contains es membership, no SDF real. El exponente es un candidato, no parámetro macOS. 
Rust 1.88 fijado provisionalmente; no dependencias externas ni código de terceros incorporado. Licencia propia pendiente. Próximos cortes: curvas Bézier locales, métricas y referencia blur CPU, compositor anidado y acción semántica. Estado IMPLEMENTED, CI pendiente al publicar; M01 completo no alcanzado.

## M01 second increment
Blur fixture: three independent bands sigma 0/4/12 physical samples (not macOS values), linear scalar convolution and sRGB export. Corners: circular, local n=4, cubic Bézier with both control points at outer corner, zero endpoint curvature (G2 to straight edge). Experimental candidate, not measured. Existing geometry fixture preserved. CI compares three PPMs across runs and exports PNG. No GPU or native calibration.

## Refraction fixture
`refraction.ppm` compares original grid panels (left identity, right experimental single-interface refraction). Synthetic geometry and optics, no calibrated macOS appearance. CI checks byte determinism and exports PNG with the existing encoder.

## Material fixture
`material.ppm`: left original RGB pattern, right synthetic glass composite (linear blur, RGB refraction, Fresnel, tint and coverage). PNG display encoded in sRGB. Parameters are not production design tokens; CI determinism is not macOS fidelity.

## Optical metrics
`optics-metrics.json` reports left/right differences for refraction and material inside/outside the fixed bounding rectangle. CI rejects effects leaking outside it and effects that change no pixel inside it; report bytes must agree across fixture runs. Values are encoded RGB differences, not Delta E or a macOS score.
