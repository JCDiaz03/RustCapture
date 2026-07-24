# Arquitectura — RustCapture

> **Mantenimiento de este documento — capa REFERENCIA.**
>
> - Qué es: foto del estado ACTUAL del diseño técnico para que cualquier programador (o IA) entienda cómo se construye y por qué. NO es un registro de cambios ni una hoja de ruta.
> - Presente, sin fechas: nada de "(2026-...)", "última actualización", "antes era X / ahora Y", "se añadió/eliminó/decidió". El historial está en git.
> - Conserva el porqué, no el cuándo: cada decisión mantiene el formato "hacemos X para conseguir Y"; documenta invariantes y gotchas no obvios; fuera anécdotas.
> - Estado, no fecha: si una decisión está a medio implementar, márcala con un estado — `(parcial)`, `(no cableado)`, `(mock)` —, nunca con una fecha.
> - Una sola casa por dato: aquí vive el CÓMO (decisiones técnicas). El QUÉ (catálogo de características) → ver `ideas.md`; referenciar features por número, no duplicar su descripción.
> - §Dependencias entre decisiones expresa relaciones técnicas, no un calendario. Fases y estado → `roadmap.md`.

Complemento de `ideas.md`. Cada decisión sigue el formato: hacemos X para conseguir Y.
Los números de característica referencian la numeración de `ideas.md`.

## D1 — Core como biblioteca, frontends como binarios finos

**Hacemos:** un workspace de Cargo donde `core` es una biblioteca pura (cero UI) y `cli` y `gui` son binarios delgados que traducen argumentos/clics a llamadas del core.
**Para conseguir:** la CLI (feature 8) gratis — `app.exe --region --clipboard` y el botón de la barra ejecutan la misma función — y un core testeable sin abrir ventanas.

## D2 — Hexagonal pragmática: puertos solo en fronteras reales

**Hacemos:** traits (puertos) únicamente donde hay una frontera de verdad:
- `ScreenSource` — origen de píxeles (GDI, Windows.Graphics.Capture, mock de test)
- `VideoEncoder` — Media Foundation hoy, otro mañana
- `OutputSink` — portapapeles, archivo, impresora, email
- `HotkeyProvider` — registro de atajos globales

Todo el código Win32 vive en el crate `platform-win`, que implementa estos traits.
**Para conseguir:** que la IA trabaje cada adapter aislado, tests sin Windows real, y poder reescribir scroll capture o vídeo sin tocar el dominio. Sin la burocracia de una hexagonal ortodoxa.

## D3 — Vertical slicing dentro del core

**Hacemos:** organización por feature, no por capa técnica. Cada slice contiene su dominio, servicios y tipos juntos.
**Para conseguir:** que cada sesión con la IA sea "trabaja en el slice X" con contexto acotado — los slices son las unidades de trabajo del desarrollo IA-first.

```
/Cargo.toml            (workspace)
/crates
  /core                (biblioteca: dominio + puertos, cero Win32)
    /src
      /capture         (modos, selección, scroll-stitching)
      /annotate        (objetos, render, documento)
      /record          (sesión de grabación, timeline)
      /output          (sinks, nombres automáticos, formatos)
      /tools           (lupa, regla, cuentagotas, pin)
      /config
      ports.rs         (traits: ScreenSource, VideoEncoder, OutputSink, HotkeyProvider)
  /platform-win        (adapters: GDI, WGC, DXGI, MF, WASAPI, hotkeys, WIA)
  /cli                 (binario fino)
  /gui                 (binario fino: barra flotante, overlay, editor)
```

El paquete del directorio `/crates/core` se llama `rustcapture-core`: un paquete llamado `core` colisiona con el crate homónimo de la biblioteca estándar y rompe las macros de std que expanden rutas `::core`.

## D4 — Strategy para los modos de captura

**Hacemos:** cada modo (región, ventana, objeto, scroll, mano alzada, fija...) implementa un trait `CaptureMode` con el mismo contrato: recibe un `ScreenSource`, devuelve un `Frame`.
**Para conseguir:** que añadir un modo nuevo (panorámica de fase 2) sea añadir un archivo, no tocar un `match` gigante; y que la CLI mapee flags a estrategias trivialmente. Cubre features 9-19.

## D5 — Anotación unificada imagen/vídeo (documento + Strategy + Factory)

**Hacemos:** el editor no manipula píxeles; mantiene un documento = lista de objetos de anotación. Trait `Annotation` con `render(&self, canvas: &mut Canvas)`; cada tipo (flecha, texto, pixelado, paso numerado...) es una Strategy, creada vía Factory desde la toolbar o desde deserialización. `Canvas` envuelve un frame RGBA — a la anotación le da igual si es una captura estática o el frame nº 4.812 de un vídeo. Para vídeo, cada objeto lleva un rango temporal `(t_inicio, t_fin)`; el pipeline de re-codificación pregunta por frame "¿qué anotaciones están activas en t?" y las renderiza.
**Para conseguir:** un solo motor de anotación para imagen y vídeo (features 20-31 y 38). La decisión más rentable del proyecto en reutilización de código.

## D6 — Command pattern en el editor

**Hacemos:** cada acción del editor (añadir flecha, mover texto, pixelar zona) es un Command con `apply`/`revert` sobre el documento.
**Para conseguir:** undo/redo ilimitado casi gratis, y el formato propio re-editable (feature 31) reducido a serializar el documento con serde: PNG base + JSON de objetos en un contenedor zip. Command, Strategy y el formato propio son la misma decisión vista desde tres ángulos.

## D7 — Eventos con canales (mpsc) para desacoplar entrada de acción

**Hacemos:** hotkeys, clics de la barra y comandos CLI no llaman funciones: publican eventos (`CaptureRequested { mode, destination }`) en un canal mpsc que consume un orquestador en el core.
**Para conseguir:** que el hilo de UI nunca se bloquee (el hook de teclado de Windows penaliza callbacks lentos), y que grabar-mientras-anotas o el auto-capture por intervalo (fase 2) sean solo productores adicionales del mismo canal.

## D8 — Especificación de grabación de vídeo

**Hacemos:**
- **Captura de frames:** DXGI Desktop Duplication — solo entrega frames cuando la pantalla cambia (pantalla estática = CPU ~0).
- **Cursor:** compuesto manualmente sobre el frame (DXGI no siempre lo incluye). Resaltado de clics como overlay propio (feature 35).
- **Conversión:** BGRA → NV12 para el encoder.
- **Codificación:** H.264 en MP4 vía Media Foundation. Encoder hardware (Quick Sync / NVENC / AMF) con fallback a software. Cero dependencias externas, cero ffmpeg.
- **Parámetros por defecto:** 30 fps (configurable 15/30/60), VBR ~4-6 Mbps a 1080p (el screen content comprime muy bien), keyframe cada 2 s.
- **Audio:** micrófono + loopback de altavoces vía WASAPI, mezcla y codificación AAC.
- **GIF (feature 39):** mismo pipeline hasta la fase de frames; solo cambia el encoder final (cuantización de paleta, 10-15 fps, aviso si región/duración disparan el tamaño).

**Para conseguir:** consumo mínimo grabando, binario pequeño y sin dependencias, calidad adecuada a contenido de pantalla.

## D9 — Configuración transversal con detección de modo

**Hacemos:** un solo struct `Config` serializado a TOML. Al arrancar: si existe `config.toml` junto al exe → modo portable; si no → `%APPDATA%`.
**Para conseguir:** que compilada/portable sea un detalle de runtime, no dos builds distintas (feature 4).

## D10 — Overlay de selección y de anotación: la misma superficie

**Hacemos:** el overlay de captura es una ventana fullscreen sin bordes que renderiza el frame congelado, y le montamos encima el motor de anotación de D5.
**Para conseguir:** anotar-antes-de-capturar al estilo Flameshot (feature 20) sin escribir un segundo editor: seleccionar, anotar ahí mismo, Enter ejecuta el pipeline de salida. El editor completo queda para la edición con calma.

## D11 — Barra, bandeja y hotkeys en Win32 puro

**Hacemos:** la barra flotante (f.1), el icono de bandeja (f.2) y los hotkeys globales (f.3) se implementan con `windows-rs` directo (ventanas Win32 clásicas, `Shell_NotifyIcon`, `RegisterHotKey`) en `platform-win`; `gui` es un binario fino que cablea config + canal + hilo orquestador + bucle de mensajes. La UI del hilo principal solo produce eventos (D7); el orquestador vive en su propio hilo y se construye dentro de él, de modo que ningún trait object necesita `Send`. La barra es no-activate (no roba el foco) para que "capturar ventana activa" apunte a la ventana correcta.
**Para conseguir:** peso y consumo mínimos — sin winit/egui/renderer para una barra de seis botones — y sin hipotecar la decisión de tecnología del editor (F3), que se tomará por separado.

## Dependencias entre decisiones

D1-D3 son el esqueleto previo a todo; D4 y D7 habilitan la captura; D5+D6 forman el bloque del editor; D10 depende de D5 maduro; D8 es independiente del editor y es el módulo de mayor tamaño. Fases y estado → ver `roadmap.md`.
