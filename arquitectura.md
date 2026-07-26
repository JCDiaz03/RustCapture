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
  /platform-win        (adapters e interfaz Win32: GDI, portapapeles, hotkeys, barra, bandeja, overlay, editor con anotación in situ, ui/ común de tema+iconos; futuros: WGC, DXGI, MF, WASAPI, WIA)
  /cli                 (binario fino)
  /gui                 (binario fino: cableado de barra + hilo orquestador)
```

El paquete del directorio `/crates/core` se llama `rustcapture-core`: un paquete llamado `core` colisiona con el crate homónimo de la biblioteca estándar y rompe las macros de std que expanden rutas `::core`.

## D4 — Strategy para los modos de captura

**Hacemos:** cada modo (región, ventana, objeto, scroll, mano alzada, fija...) implementa un trait `CaptureMode` con el mismo contrato: recibe un `ScreenSource`, devuelve un `Frame`.
**Para conseguir:** que añadir un modo nuevo (panorámica de fase 2) sea añadir un archivo, no tocar un `match` gigante; y que la CLI mapee flags a estrategias trivialmente. Cubre features 9-19.

## D5 — Anotación unificada imagen/vídeo (documento + Strategy + Factory)

**Hacemos:** el editor no manipula píxeles; mantiene un documento = lista de objetos de anotación. Trait `Annotation` con `render(&self, canvas: &mut Canvas)`; cada tipo (flecha, texto, pixelado, paso numerado...) es una Strategy en su propio archivo, creada vía Factory desde la toolbar o desde deserialización.

El documento guarda esos tipos en un **enum `Objeto` que cierra la jerarquía**, no en `Box<dyn Annotation>`. El trait sigue siendo el contrato que implementa cada tipo, pero un `dyn` solo ofrece lo que el trait declara, y el editor necesita tres cosas más sobre un objeto ya colocado: saber dónde está (`bounds`, para el hit-test de la selección), moverlo (`translate`) y serializarlo (f.31, que con `dyn` exigiría `typetag` — una dependencia, contra la prioridad de peso mínimo). El enum las da con un `match` en un solo archivo y el compilador señala los que faltan al añadir un tipo. El precio es perder el `dyn` como punto de extensión externo, que no cuesta nada en una app con los plugins descartados (`ideas.md` §Descartado). `Canvas` envuelve un frame RGBA — a la anotación le da igual si es una captura estática o el frame nº 4.812 de un vídeo. Para vídeo, cada objeto lleva un rango temporal `(t_inicio, t_fin)`; el pipeline de re-codificación pregunta por frame "¿qué anotaciones están activas en t?" y las renderiza. El `Canvas` expone lectura además de escritura: la censura (f.25) necesita ver lo que hay debajo, de modo que pixelar tapa también las anotaciones anteriores del z-order, no solo la base.

`RenderContext` es un **catálogo** de caras tipográficas indexadas por `(FamiliaId, negrita)`, no dos fuentes fijas (f.54). Las fuentes siguen entrando como bytes: quien lee el registro de Windows y la carpeta `fonts/` es `platform-win/fuentes_ttf`, así el core sigue sin abrir archivos. `TextStyle.familia` es un `FamiliaId(u16)` y no un `String` para que `TextStyle` siga siendo `Copy` — viaja por valor por todo el motor y convertirlo en no-`Copy` obligaría a rediseñar `draw_text`, `text_ink_box`, `draw_text_rotado` y el número de los pasos; el nombre vive en el catálogo, que será también quien lo resuelva al serializar (f.31). Pedir una cara nunca falla en seco: hay cadena de respaldo (la pedida → la misma familia sin negrita → la familia de respaldo), de modo que una fuente ausente o corrupta degrada en vez de dejar el texto sin pintar. Las familias se registran todas al abrir el editor (solo nombres) pero sus caras se cargan al elegirlas: parsear cientos de TTF de golpe no es aceptable.

El documento no guarda las formas sueltas, sino un `Objeto { forma: Forma, giro: Giro }`: el **giro (f.53) es propiedad de la colocación, no del tipo**, así que rotar es una línea en vez de nueve y el formato re-editable serializa un campo en vez de nueve. El centro de giro no se almacena — se deriva de la caja SIN girar, que es invariante al giro, y por eso girar y desgirar es exactamente reversible. Cada forma expone su `caja()`, que es a la vez la caja de selección y el origen del centro de giro: si divergieran, el objeto se desplazaría al rotarlo (hay un test que compara los píxeles pintados contra la caja para cada forma y varios ángulos). El rasterizado girado se resuelve por familias: las formas hechas de puntos (línea, flecha, lápiz, paso) rotan sus puntos y reutilizan los rasterizadores sin girar, la elipse añade el giro a su muestreo paramétrico, los contornos y rellenos de caja pasan a cuatro líneas y a barrido de cuadrilátero, y la censura y el texto van por **mapeo inverso** (recorrer el destino deshaciendo el giro), que es la única forma de rotar glifos ya rasterizados sin dejar huecos. Solo esa última familia pierde nitidez, y es inevitable con `fontdue`. Con `giro` nulo todas toman el camino directo, sin remuestrear: la calidad de lo que no se gira es intacta.
**Para conseguir:** un solo motor de anotación para imagen y vídeo (features 20-31 y 38). La decisión más rentable del proyecto en reutilización de código.

## D6 — Command pattern en el editor

**Hacemos:** cada acción del editor (añadir flecha, mover texto, pixelar zona) es un Command con `apply`/`revert` sobre el documento. `Move` y `Rotate` no guardan el estado anterior: revertir es aplicar el delta negado, así que mover y girar quedan deshacibles sin estado extra. Un delta nulo no se apila — arrastrar o soltar el asa sin mover nada no debe gastar un undo. `Replace` sí guarda el objeto sustituido, y existe para que reeditar un texto conserve su posición en el z-order (borrarlo y volver a añadirlo lo mandaría al frente). Los arrastres producen UN comando al soltar, no uno por `WM_MOUSEMOVE`: mientras duran, el documento no se toca y el resultado se pinta como preview (`render_onto_moved`/`render_onto_rotated`), de modo que un arrastre interrumpido no deja el documento cambiado fuera del historial.
**Para conseguir:** undo/redo ilimitado casi gratis, y el formato propio re-editable (feature 31) reducido a serializar el documento con serde. Command, Strategy y el formato propio son la misma decisión vista desde tres ángulos.

El `.rcap` es un **ZIP sin comprimir** con `imagen.png` (el frame BASE, sin hornear: por eso sigue siendo editable) y `documento.toml` (versión, familias y objetos). Se usa TOML y no JSON porque `toml` ya era dependencia directa, y el contenedor se escribe a mano en vez de usar el crate `zip` porque `crc32fast` y `flate2` ya estaban en el árbol vía `png`: así el formato entero no añade ni una dependencia ni un byte al binario. Se escribe sin comprimir (el PNG ya lo está) pero se **lee** aceptando deflate, para que un `.rcap` recomprimido con el Explorador de Windows siga abriéndose; y se lee por el directorio central, no recorriendo cabeceras locales, porque los zips de terceros pueden dejar los tamaños a cero ahí. Las familias tipográficas van por **nombre** en una lista compacta y los objetos indexan en ella: el `FamiliaId` es un índice del catálogo local y no significa nada en otra máquina. Al abrir, el editor recrea los buffers en vez de rellenarlos — hasta f.31 las dimensiones del frame eran inmutables durante toda la vida del editor y `refresh_committed` contaba con ello.

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
La carpeta `fonts/` junto al ejecutable es la otra pieza portable-first: el usuario suelta ahí sus `.ttf` y el editor las ofrece sin instalarlas en Windows, y en caso de coincidir el nombre tienen prioridad sobre las del sistema (f.54).
**Para conseguir:** que compilada/portable sea un detalle de runtime, no dos builds distintas (feature 4).

## D10 — Overlay por capas: selección primero, anotación encima

**Hacemos:** el overlay de captura es una ventana fullscreen (escritorio virtual completo) que renderiza el frame congelado, construida en dos capas independientes:
- **Capa de selección** `(platform-win/overlay)`: máscara blanca al 50 %, arrastre de rectángulo que se ve limpio, crosshair y lupa 200×200 con salto de esquina. Produce un `Rect` en coordenadas de escritorio — el mismo que la CLI parsea con `--region` — y publica `CaptureRequested(Region(rect))`; el pipeline no distingue productores. Corre en el hilo de UI con bucle modal; la barra se auto-oculta mientras tanto. Es la base sobre la que se montan mano alzada, región fija y el picking de ventana/objeto.
La capa de selección tiene **tres modos sobre la misma superficie** (arrastre libre f.13, señalar un objeto f.11/f.12 y rect de tamaño fijo f.15), y los tres publican `CaptureRequested(Region(rect))`: son formas distintas de elegir un rect, no modos de captura distintos, así que ninguno añade un `CaptureMode`. Internamente lo único que los distingue es de dónde sale la «selección vigente» que se pinta sin blanquear. Para f.12 el orden importa y es fácil de romper sin darse cuenta: la **instantánea del árbol de ventanas se toma ANTES de congelar la pantalla**, porque un menú desplegado se cierra en cuanto aparece el overlay — pero ya está en la imagen congelada, así que el hit-test se hace contra la instantánea y no contra las ventanas vivas. Se elige el rect de menor área que contiene el cursor: así un botón gana a su diálogo y el diálogo al escritorio.

- **Capa de anotación** (F4): el motor de D5 encima de la misma superficie; con ella la entrega pasa a ser el frame congelado editado (WYSIWYG estricto).

**Para conseguir:** anotar-antes-de-capturar al estilo Flameshot (feature 20) sin escribir un segundo editor, y que todos los modos interactivos de captura compartan una única superficie de selección.

## D11 — Barra, bandeja y hotkeys en Win32 puro

**Hacemos:** la barra flotante (f.1), el icono de bandeja (f.2) y los hotkeys globales (f.3) se implementan con `windows-rs` directo (ventanas Win32 clásicas, `Shell_NotifyIcon`, `RegisterHotKey`) en `platform-win`; `gui` es un binario fino que cablea config + canal + hilo orquestador + bucle de mensajes. La UI del hilo principal solo produce eventos (D7); el orquestador vive en su propio hilo y se construye dentro de él, de modo que ningún trait object necesita `Send`. La barra es no-activate (no roba el foco) para que "capturar ventana activa" apunte a la ventana correcta.
**Para conseguir:** peso y consumo mínimos — sin winit/egui/renderer. La misma decisión quedó ratificada para el editor y el resto de superficies: toda la UI es Win32 puro pintado con GDI sobre la infraestructura de D13.

## D12 — El editor como destino por defecto y las ventanas modales del hilo de UI

**Hacemos:** las capturas de la GUI desembocan por defecto en el editor (f.21) mediante un `OutputSink` más (`EditorSink`, id `"editor"`, default de `[output].destination`): su `deliver` corre en el hilo orquestador y solo publica el `Frame` al hilo de UI vía `PostMessageW` (Box crudo que el receptor siempre adopta). Las ventanas de trabajo (overlay de selección, editor) siguen el mismo patrón: corren en el hilo de UI con bucle modal anidado, su estado lo posee la función llamadora (`Box::into_raw` antes de crear la ventana, `Box::from_raw` después de destruirla) y el wndproc lo usa sin liberarlo jamás. Un `AtomicBool` garantiza un editor cada vez (capturas con el editor abierto se rechazan con aviso); la barra se auto-oculta mientras el editor vive.

La anotación (D5+D6) vive DENTRO del editor (diseño V4): toolbar con la herramienta activa, barra de propiedades contextual y canvas anotable con preview en vivo; el documento de objetos pertenece al editor y Guardar/Copiar hornean bajo demanda sobre el frame base, de modo que undo/redo sobrevive al guardado. El flag de sucio = documento cambiado desde el último guardado/copiado; cerrar sucio pide confirmación.
**Para conseguir:** que el flujo capturar→anotar→salida no rompa D7 (el sink es un productor de mensajes, no bloquea el bus), que ningún trait object necesite `Send`, un único patrón de ventana modal, y que la futura capa de anotación del overlay (D10/F4) reutilice el mismo patrón de herramientas inline sobre canvas.

## D13 — Assets pixel a pixel: atlas A8 offline + tema dual

**Hacemos:** los iconos de toolbar se generan OFFLINE desde los SVG de `design/icons/` con el tool aislado `design/tools/genassets` (resvg solo ahí, jamás en runtime): máscaras de cobertura A8 a 16/20/24/28/32 px (DPI 100-200 %) con el antialiasing horneado, embebidas con `include_bytes!` y tintadas en runtime (`AlphaBlend`) con el color del tema — un solo asset sirve para normal/deshabilitado/activo en claro y oscuro. El icono de app es un `.ico` multiresolución empaquetado desde los PNG del diseño e incrustado con `build.rs` (+ manifest comctl32 v6 y PMv2). La infraestructura común vive en `platform-win/src/ui/`: `theme.rs` (dos paletas const con los tokens del diseño, detección `AppsUseLightTheme`, `WM_SETTINGCHANGE`, DWM dark title bar), `iconos` (tinte premultiplicado con caché), `boton` (IconButton owner-draw con 5 estados), `botonera` (la fila declarativa `Elemento`/`BotonDef` y su ciclo crear/recolocar — cada superficie solo compone su fila), `lienzo` (`BackBuffer` RAII para pintar sin parpadeo + brochas efímeras), `ventana` (estado en GWLP_USERDATA y reacción común al cambio de tema), `tooltip`, `fuentes` y `layout` (unidades lógicas escaladas con `dpi::Escala`). Regla: layout en unidades LÓGICAS, escalar solo al pintar/posicionar; ninguna superficie duplica este ciclo — lo consume.
**Para conseguir:** UI nítida a cualquier DPI y en ambos temas con ~150 KB embebidos y cero dependencias de imagen en runtime (peso mínimo, f.4/f.5). Gotcha documentado: importar `GetWindowSubclass` mata el exe en el loader sin manifest v6 (comctl32 5.82 no lo exporta por nombre); el estado de los botones va en `GWLP_USERDATA`.

## Dependencias entre decisiones

D1-D3 son el esqueleto previo a todo; D4 y D7 habilitan la captura; D5+D6 forman el bloque de anotación y D12 lo integra en el editor; D13 da el sistema visual (tema, iconos, DPI) que consumen barra, overlay y editor; D10 depende de D5 maduro (su capa de selección ya existe; la de anotación reutilizará D5+D12); D11 y D12 comparten el patrón de ventana del hilo de UI; D8 es independiente del editor y es el módulo de mayor tamaño. Fases y estado → ver `roadmap.md`.
