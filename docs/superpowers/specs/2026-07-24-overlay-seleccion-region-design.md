# Spec — Overlay de selección de región (f.13 interactiva; base de f.14-f.16)

Slice de F2. Diseño validado con el humano (brainstorming 2026-07-24).

## Objetivo

Selección interactiva de región sobre la pantalla: activa el botón
«Región» de la barra y el hotkey `ctrl+printscreen`. Es la capa de
selección del overlay de D10; los siguientes modos de F2 (mano alzada,
región fija, picking de ventana/objeto, scroll) se montan encima, y la
capa de anotación de F4 llegará sobre esta misma superficie.

## Comportamiento (especificación del humano)

1. Al activarse: TODAS las pantallas se cubren con una máscara blanca
   `#FFFFFF` al 50 % de alfa.
2. `Esc` cancela; todo vuelve a la normalidad sin capturar.
3. Con el clic izquierdo mantenido se arrastra la selección; la región
   seleccionada se ve LIMPIA (sin máscara). Al soltar el botón, se
   captura esa región.
4. El cursor es un crosshair.
5. Caja de lupa de 300×500 px en la esquina inferior derecha del monitor
   donde está el cursor; si el cursor se acerca a la caja, esta salta a
   la esquina superior izquierda del mismo monitor. Composición vertical:
   - **Zoom (300×300):** la zona bajo el cursor ampliada a 5× — cada
     píxel real ocupa 5×5, luego la fuente es de 60×60 px.
   - **Barra de coordenadas (300×30, gris claro):** texto
     `X, Y = 560,267` con la posición actual del cursor en coordenadas
     de escritorio (origen (0,0) del escritorio virtual).
   - **Bloque de ayuda (300×170, azul):** texto de controles del estilo
     "ESC key to cancel" (copy final en español: «Arrastra para
     seleccionar · Suelta para capturar · ESC para cancelar»); durante
     el arrastre muestra además `Selección: W×H px`.

## Decisiones técnicas

- **Pantalla congelada:** al abrir, se captura el escritorio virtual
  completo una vez (`GdiScreenSource`). El overlay pinta ese frame; la
  máscara es una copia "blanqueada" precalculada
  (`px = (px + 255) / 2` por canal, alfa intacto — función pura).
  La lupa lee píxeles del frame congelado.
- **Coordenadas:** espacio único de escritorio virtual en píxeles
  físicos. El overlay traduce cliente→escritorio sumando el origen del
  `desktop_rect()`. Su salida es un `Rect` idéntico al que parsea la CLI
  con `--region X,Y,WxH`: ambos productores desembocan en
  `ModeRequest::Region(rect)` → pipeline existente sin cambios.
- **Entrega:** al soltar, el overlay se cierra y publica
  `AppEvent::CaptureRequested(Region(rect))`. El orquestador recaptura
  en vivo (ya sin overlay ni barra). «Repetir última» funciona con
  regiones gratis. La entrega del frame congelado tal cual (WYSIWYG
  estricto) llegará con la capa de anotación de F4.
- **La barra se auto-oculta** antes de congelar y reaparece al cerrar el
  overlay (resuelve la limitación anotada en F1: la barra salía en las
  capturas).
- **Enrutado UI:** el overlay DEBE correr en el hilo de UI. El botón
  «Región» y el hotkey de región llegan al wndproc de la barra como
  mensaje `WM_APP_REGION`; `run_message_loop` traduce el `WM_HOTKEY` de
  región a ese mensaje (recibe el id de región como parámetro). El
  overlay corre un bucle modal propio (como los menús).
- **Detalles visuales:** zoom de lupa 5× (fuente 60×60 px → 300×300) con
  cruz central de 1 px; borde de selección rojo de 1 px; sin tamaño
  mínimo de selección (f.19). Colores de la caja: zona de coordenadas
  gris claro (`COLOR_BTNFACE`), bloque de ayuda azul (RGB 30,80,160 con
  texto blanco).

## Estructura

- `platform-win/src/overlay/math.rs` — geometría pura con TDD:
  - `rect_between(a: (i32,i32), b: (i32,i32)) -> Rect` (normaliza el
    arrastre en cualquier dirección; ancho/alto ≥ 1).
  - `lupa_source(cursor_local, frame_w, frame_h) -> Rect` (60×60
    alrededor del cursor, con clamping en bordes).
  - `lupa_box_pos(monitor_local: Rect, cursor_local: (i32,i32)) -> (i32, i32)`
    (caja de 300×500 en inferior-derecha; salta a superior-izquierda si
    el cursor entra en la caja inflada por un margen).
- `platform-win/src/pixels.rs` — `whiten_half(pixels: &mut [u8])`
  (máscara 50 % blanca, pura, TDD).
- `platform-win/src/overlay/mod.rs` — ventana Win32 (popup topmost del
  tamaño del escritorio virtual, clase con cursor `IDC_CROSS`), doble
  bitmap (original y blanqueado), pintado con BitBlt + StretchBlt
  (lupa), bucle modal; `pub fn select_region() -> Option<Rect>`
  (None = cancelado). API pública sin tipos de `windows`.
- `platform-win/src/bar.rs` — `WM_APP_REGION`: ocultar barra → pausa
  breve (~150 ms para que el escritorio repinte) → `select_region()` →
  mostrar barra → publicar evento si hay rect. Botón «Región» habilitado.
- `gui/main.rs` — registra el hotkey `config.hotkeys.region`; su
  `WM_HOTKEY` se traduce a `WM_APP_REGION` (no pasa por el orquestador).

## Errores

- Falla la captura congelada → beep de error, el overlay no se abre, la
  barra reaparece.
- Selección de área 0 (clic sin arrastre) → se trata como 1×1 (f.19).

## Testing

- TDD: `rect_between`, `lupa_source` (clamping en las 4 esquinas),
  `lupa_box_pos` (posición normal y salto), `whiten_half`.
- Manual guiado: máscara en todos los monitores, arrastre en las cuatro
  direcciones, lupa siguiendo al cursor y saltando de esquina, Esc,
  captura al portapapeles, barra oculta durante la selección y de vuelta
  después, `ctrl+printscreen` equivalente al botón.

## Fuera de alcance

Mano alzada (f.14), región fija (f.15), picking de ventana/objeto
(f.11/f.12), anotación sobre el overlay (F4), entrega WYSIWYG del frame
congelado, guías de alineación y cambio de tamaño de la selección ya
hecha.
