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
      /orchestrator    (bus de eventos + consumidor, D7)
      /ports           (traits: ScreenSource, VideoEncoder, OutputSink, HotkeyProvider; mocks públicos de test)
  /platform-win        (adapters e interfaz Win32: GDI, portapapeles, hotkeys, barra, bandeja, overlay, editor, dibujo; futuros: WGC, DXGI, MF, WASAPI, WIA)
  /cli                 (binario fino)
  /gui                 (binario fino: cableado de barra + hilo orquestador)
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

**Hacemos:** hotkeys, clics de la barra y comandos CLI no llaman funciones: publican eventos (`CaptureRequested { mode, destination }`) en un canal mpsc que consume un orquestador en el core. El orquestador dispone además de un canal *loopback* (un `Sender` propio) para eventos programados: la captura con retardo lanza un hilo temporizador que reenvía el evento cuando toca — el bus nunca duerme.
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

## D10 — Overlay por capas: selección primero, anotación encima

**Hacemos:** el overlay de captura es una ventana fullscreen (escritorio virtual completo) que renderiza el frame congelado, construida en dos capas independientes:
- **Capa de selección** `(platform-win/overlay)`: máscara blanca al 50 %, arrastre de rectángulo que se ve limpio, crosshair y lupa 200×200 con salto de esquina. Produce un `Rect` en coordenadas de escritorio — el mismo que la CLI parsea con `--region` — y publica `CaptureRequested(Region(rect))`; el pipeline no distingue productores. Corre en el hilo de UI con bucle modal; la barra se auto-oculta mientras tanto. Es la base sobre la que se montan mano alzada, región fija y el picking de ventana/objeto.
- **Capa de anotación** (F4): el motor de D5 encima de la misma superficie; con ella la entrega pasa a ser el frame congelado editado (WYSIWYG estricto).

**Para conseguir:** anotar-antes-de-capturar al estilo Flameshot (feature 20) sin escribir un segundo editor, y que todos los modos interactivos de captura compartan una única superficie de selección.

## D11 — Barra, bandeja y hotkeys en Win32 puro

**Hacemos:** la barra flotante (f.1), el icono de bandeja (f.2) y los hotkeys globales (f.3) se implementan con `windows-rs` directo (ventanas Win32 clásicas, `Shell_NotifyIcon`, `RegisterHotKey`) en `platform-win`; `gui` es un binario fino que cablea config + canal + hilo orquestador + bucle de mensajes. La UI del hilo principal solo produce eventos (D7); el orquestador vive en su propio hilo y se construye dentro de él, de modo que ningún trait object necesita `Send`. La barra es no-activate (no roba el foco) para que "capturar ventana activa" apunte a la ventana correcta.
**Para conseguir:** peso y consumo mínimos — sin winit/egui/renderer para una barra de seis botones — y sin hipotecar la decisión de tecnología del editor (F3), que se tomará por separado.

## D12 — El editor como destino por defecto y las ventanas modales del hilo de UI

**Hacemos:** las capturas de la GUI desembocan por defecto en el editor (f.21) mediante un `OutputSink` más (`EditorSink`, id `"editor"`, default de `[output].destination`): su `deliver` corre en el hilo orquestador y solo publica el `Frame` al hilo de UI vía `PostMessageW` (Box crudo que el receptor siempre adopta). Todas las ventanas de trabajo (overlay de selección, editor, dibujo) siguen el mismo patrón: corren en el hilo de UI con bucle modal anidado, su estado lo posee la función llamadora (`Box::into_raw` antes de crear la ventana, `Box::from_raw` después de destruirla) y el wndproc lo usa sin liberarlo jamás. Un `AtomicBool` garantiza un editor cada vez (capturas con el editor abierto se rechazan con aviso); la barra se auto-oculta mientras el editor vive. El editor lleva flag de sucio: Draw con OK lo marca, Guardar/Copiar con éxito lo limpian, y cerrar sucio pide confirmación.
**Para conseguir:** que el flujo capturar→editar→dibujar→salida no rompa D7 (el sink es un productor de mensajes, no bloquea el bus), que ningún trait object necesite `Send`, y un único patrón de ventana modal que cada pieza nueva reutiliza en vez de reinventar.

## Dependencias entre decisiones

D1-D3 son el esqueleto previo a todo; D4 y D7 habilitan la captura; D5+D6 forman el bloque del editor y D12 lo cablea al flujo de captura; D10 depende de D5 maduro (su capa de selección ya existe; la de anotación reutilizará D5+D12); D11 y D12 comparten el patrón de ventana del hilo de UI; D8 es independiente del editor y es el módulo de mayor tamaño. Fases y estado → ver `roadmap.md`.
