# Roadmap — RustCapture

> **Mantenimiento de este documento — capa PLANES/HOJA DE RUTA.**
>
> - Qué es: hacia dónde va el proyecto y en qué punto está. Rastrea ESTADO, no cambios.
> - Estado con marcadores (✅ hecho · 🔵 en curso · ⏳ pendiente · 🚫 descartado), NO con fechas de commit ni "antes/ahora". Una fecha solo si es un hito/objetivo real.
> - El "qué se construye y por qué" → `ideas.md` (no duplicar; resumir + enlazar). Las decisiones técnicas y sus dependencias → `arquitectura.md`.
> - Ideas sin comprometer → §7 Diferido. No mezclar con las fases comprometidas.
> - Los números de característica (f.N) referencian `ideas.md`; las decisiones (D.N) referencian `arquitectura.md`.

## 0. Estado general

🔵 **Fase actual: F3 — Editor y anotación (adelantada por decisión de producto).** El ciclo capturar → anotar in situ → guardar/copiar funciona en el editor V4 (F3.5 completada: rediseño visual con tema dual, iconos y fusión de la ventana de dibujo en el editor, D12+D13); quedan la leyenda, crop/resize, el formato re-editable y el resto de salidas. F1 completada; F2 en pausa (picking de ventana/objeto, mano alzada, región fija, scroll y f.7/f.19).

## 1. F0 — Diseño y preparación del entorno

- ✅ Comparativa de mercado y análisis de FastStone.
- ✅ Catálogo de características (`ideas.md`).
- ✅ Decisiones de arquitectura D1-D10 (`arquitectura.md`).
- ✅ Instalar skills de flujo de trabajo del agente → selección y delegación en `skills.md`.
- ✅ Skill propia del proyecto (`windows-rs`, HRESULT, adapters) → definida en `skills.md`, se crea con el esqueleto.
- ✅ Esqueleto del workspace (D1-D3): crates `core` (paquete `rustcapture-core`), `platform-win`, `cli`, `gui` compilando en vacío.

## 2. F1 — MVP de captura (D4 + D7)

Objetivo: capturar y sacar por portapapeles/archivo desde barra, hotkey y CLI. Primer binario usable a diario.

- ✅ Puertos `ScreenSource`, `OutputSink`, `HotkeyProvider` + mocks de test (D2).
- ✅ Adapter de captura GDI en `platform-win` con DPI per-monitor (f.6) — WGC diferido como adapter alternativo.
- ✅ Bus de eventos mpsc + orquestador (D7).
- ✅ Modos: pantalla completa, ventana activa, región rectangular (f.9, f.10, f.13) como strategies `CaptureMode` (D4).
- ✅ Salidas: portapapeles y archivo con nombres automáticos (f.40, f.41); PNG y JPEG.
- ✅ Barra flotante mínima + icono en bandeja + hotkeys globales (f.1-f.3).
- ✅ CLI fina sobre el core (f.8, D1).
- ✅ Config TOML portable-first (f.4, D9).

## 3. F2 — Resto de modos de captura

- ✅ Overlay de selección de región (capa de selección de D10): activa botón «Región» y `ctrl+printscreen`; base de f.11-f.16.
- ✅ Objeto de ventana y menús (f.11, f.12): instantánea del árbol de ventanas ANTES de congelar (por eso un menú desplegado sigue siendo señalable) y resaltado del rect más pequeño bajo el cursor.
- ✅ Región fija (f.15): rect de tamaño de config siguiendo al cursor, ajustable con la rueda (Shift = solo ancho).
- 🚫 Región a mano alzada (antigua f.14): descartada, ver `ideas.md` §Descartado.
- 🚫 Scroll capture (antigua f.16) y escritorios virtuales (antigua f.7): descartadas, ver `ideas.md` §Descartado.
- ✅ Retardo/temporizador y repetir última captura (f.17, f.18).
- ⏳ Interruptor de tamaño mínimo de captura (f.19): último ítem de la fase.

## 4. F3 — Editor y anotación (D5 + D6 + D12)

- ✅ Slice A — Editor shell (f.21): captura → ventana de editor, barra auto-oculta, destino "editor" por defecto.
- ✅ Slice C — Motor de anotación con UI (histórico: nació como ventana de dibujo/Ventana2; F3.5/S6 la fusionó dentro del editor).
- ✅ Modelo de documento: objetos `Annotation`, Strategy y `Canvas` sobre frame RGBA (D5); la toolbar del editor hace de Factory.
- ✅ Command pattern con undo/redo (D6); sobrevive al guardado (hornear bajo demanda, D12).
- ✅ Herramientas (motor en core): texto, flechas, líneas, formas, resaltado, lápiz, pixelado/desenfoque, pasos numerados y goma, todas integradas en el editor. 🚫 Leyendas (antigua f.24) descartada.
- ✅ Selección, movimiento y reedición de objetos (f.52): `Objeto::bounds`/`translate`, `Document::hit_test` y Commands `Move`/`Replace` en el motor (D5+D6), y en el editor la herramienta Selección (clic elige, arrastre mueve, doble clic reedita un texto, marco punteado con 8 asas, Esc suelta) más la goma y Supr que borran el objeto (f.27).
- ✅ Rotación de objetos (f.53): `Objeto { forma, giro }` con rasterizado girado en las nueve formas y `Command::Rotate`; asa redonda en el recuadro, arrastre circular y salto de 15° con Shift.
- ⏳ Recorte y redimensionado (f.26), marca de agua (f.29) y efectos de borde (f.30). 🚫 Nitidez (antigua f.28) descartada.
- ✅ Formato propio re-editable `.rcap` (f.31): ZIP con la base en PNG y los objetos en TOML, con botón Abrir en el editor; cero dependencias nuevas.
- ⏳ Resto de salidas: impresora, email, editor externo (f.42-f.44) y PDF, que se genera a mano envolviendo el JPEG con `/DCTDecode` sin dependencias (f.45). 🚫 WebP, BMP, GIF y TIFF descartados; el GIF sigue vivo como exportación de vídeo (f.39).

## 4c. F3.6 — Manipulación de objetos

Convierte el editor en un editor de verdad: los objetos colocados dejan de ser inmutables.

- ✅ S-D — Pixelado/desenfoque (f.25) y pasos numerados (f.23).
- ✅ S-E — Motor cerrado en un enum (`Objeto`), selección, movimiento, reedición de texto y goma (f.52, f.27).
- ✅ S-F — Rotación con asa y snap (f.53).
- ✅ S-G — Catálogo de fuentes: familias del sistema y de la carpeta `fonts/` portable, chip de fuente y edición en vivo de fuente/tamaño/negrita/color con la caja abierta (f.54).
- ✅ S-H — Formato re-editable `.rcap` (f.31): contenedor ZIP propio, documento en TOML, familias por nombre y botón Abrir.

## 4b. F3.5 — Rediseño visual V4 (D13)

Integración del diseño de `design/` (tokens de `diseno-frontend.md`) en toda la GUI existente.

- ✅ S0 — Assets: iconos annotate-ellipse/annotate-pencil nuevos + tool `design/tools/genassets` (atlas A8 16-32 px con AA horneado, `.ico` multiresolución).
- ✅ S1 — Infraestructura `platform-win/src/ui/`: tema claro/oscuro con autodetección, iconos tintados con caché, IconButton owner-draw (5 estados), tooltips, fuentes y layout en unidades lógicas; `dpi::Escala` + `WM_DPICHANGED`; sección `[theme]` en config.
- ✅ S3 — Barra V4: fila de iconos 28×28 con asa, separadores, tooltips con hotkey, double buffering y tema en vivo.
- ✅ S2 — Identidad: `.ico` embebido (build.rs + winresource), manifest comctl32 v6 + PMv2, perfil release (GUI ~1 MB), bandeja e iconos de ventana propios.
- ✅ S4 — Lupa V3: caja compacta junto al cursor con flip, zoom 21×21 con píxel central en acento, `#RRGGBB · X, Y` y `sel W × H`; invalidación mínima del overlay (resuelto el PENDIENTE de rendimiento).
- ✅ S5 — Editor V4 (chrome): toolbar de iconos, status bar, back buffer sin parpadeo, DWM dark, título con nombre de archivo.
- ✅ S6 — Fusión: la ventana de dibujo desaparece; anotación in situ en el editor con property bar contextual (ver F3 Slice C).
- ✅ S7 — Documentación.
- ✅ Consolidación post-fusión: `ui/botonera`, `ui/lienzo` (BackBuffer), `ui/ventana` y `util` extraen la duplicación que dejó el rediseño; comportamiento intacto.

## 5. F4 — Overlay de anotación en captura (D10)

- ⏳ Overlay fullscreen con frame congelado + motor de anotación de F3 embebido (f.20).
- ⏳ Flujo seleccionar → anotar → Enter → pipeline de salida, estilo Flameshot.

## 6. F5 — Vídeo y utilidades (D8)

- ⏳ Grabación DXGI Desktop Duplication + composición de cursor y clics (f.32, f.35).
- ⏳ Encoder H.264/MP4 vía Media Foundation, hardware con fallback software (f.36).
- ⏳ Audio WASAPI: micrófono + loopback, mezcla, AAC (f.33).
- ⏳ Webcam overlay y ventana de inicio simple/detallada (f.34, f.37).
- ⏳ Trim básico y exportación GIF sobre el mismo pipeline (f.38, f.39).
- ⏳ Utilidades: pin-to-screen, OCR, cuentagotas, lupa, crosshair, regla (f.46-f.51).

## 7. Diferido (sin comprometer)

Resumen de `ideas.md` §2 Fase 2 → ver detalle allí:

- ⏳ Flujos post-captura simplificados.
- ⏳ Historial de capturas.
- ⏳ Auto-captura por intervalo.
- ⏳ Escaneo desde escáner (WIA).
- ⏳ Instalador opcional (Inno Setup / WiX).
- ⏳ Editor de vídeo ampliado.
- ⏳ Barra flotante: orientación vertical, estado colapsado y acoplado al borde con auto-ocultado (diseño V1 completo).
- ⏳ Zoom Ctrl+rueda y pan del canvas del editor (la status bar ya muestra el % de encaje).
- ⏳ Pestañas multi-captura del editor (diseño V4).
- ⏳ Icono de bandeja monocromo theme-aware (`app-icon-tray.svg`).

Descartes con su porqué → `ideas.md` §Descartado.
