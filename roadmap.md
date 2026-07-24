# Roadmap — RustCapture

> **Mantenimiento de este documento — capa PLANES/HOJA DE RUTA.**
>
> - Qué es: hacia dónde va el proyecto y en qué punto está. Rastrea ESTADO, no cambios.
> - Estado con marcadores (✅ hecho · 🔵 en curso · ⏳ pendiente · 🚫 descartado), NO con fechas de commit ni "antes/ahora". Una fecha solo si es un hito/objetivo real.
> - El "qué se construye y por qué" → `ideas.md` (no duplicar; resumir + enlazar). Las decisiones técnicas y sus dependencias → `arquitectura.md`.
> - Ideas sin comprometer → §7 Diferido. No mezclar con las fases comprometidas.
> - Los números de característica (f.N) referencian `ideas.md`; las decisiones (D.N) referencian `arquitectura.md`.

## 0. Estado general

🔵 **Fase actual: F2 — Resto de modos de captura.** F1 completada: el MVP captura a diario desde barra, bandeja, hotkeys y CLI.

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

- ⏳ Objeto de ventana y menús (f.11, f.12).
- ⏳ Región a mano alzada y región fija (f.14, f.15).
- ⏳ Scroll capture (f.16) — el módulo de mayor riesgo técnico de la fase; referencia: implementación de ShareX.
- ⏳ Retardo/temporizador y repetir última captura (f.17, f.18).
- ⏳ Escritorios virtuales y capturas diminutas (f.7, f.19).

## 4. F3 — Editor y anotación (D5 + D6)

- ⏳ Modelo de documento: objetos `Annotation`, Strategy + Factory, `Canvas` sobre frame RGBA (D5).
- ⏳ Command pattern con undo/redo (D6).
- ⏳ Herramientas: texto, flechas, líneas, formas, resaltado, pasos numerados, leyendas, pixelado, goma (f.22-f.27).
- ⏳ Recorte, redimensionado, nitidez, marca de agua, efectos de borde (f.26, f.28-f.30).
- ⏳ Formato propio re-editable: PNG base + JSON de objetos en contenedor zip (f.31).
- ⏳ Resto de salidas: impresora, email, editor externo; WebP, BMP, GIF, TIFF, PDF (f.42-f.45).

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

Descartes con su porqué → `ideas.md` §Descartado.
