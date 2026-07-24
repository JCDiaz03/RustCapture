# Spec — Ventana de dibujo (Slice C de F3; f.20-f.27 parcial)

Diseño validado con el humano (brainstorming 2026-07-24). Referencia
visual: `docs/superpowers/Ventana2.jpg`. Consume el motor del Slice B
(`annotate/`: anotaciones, `Document`, `History`, `RenderContext`).

## Ventana

Modal desde el botón «Draw» del editor (que se activa con este slice),
patrón del editor/overlay (estado poseído por el llamador, bucle modal):

- **Paleta izquierda** (columna de botones): Rect, Elipse, Línea,
  Flecha, Lápiz, Resaltador, Texto, Deshacer, Rehacer.
- **Lienzo central**: la captura encajada (`fit_rect`); mapeo PURO
  vista↔frame para que el dibujo caiga exacto sobre la imagen real
  (`view_to_frame`, testeable; inverso del encaje).
- **Barra inferior**: 8 colores predefinidos (negro, blanco, rojo,
  verde, azul, amarillo, naranja, morado) + «Más…» (`ChooseColorW`);
  grosor 1/2/3/5/8; con Texto activa: tamaño 12/16/20/28/36 y botón
  «B» (negrita); OK / Cancelar a la derecha.
- **Atajos**: `Ctrl+Z` deshacer, `Ctrl+Y` rehacer, `Esc` = Cancelar
  (con confirmación si hay anotaciones).

## Comportamiento de dibujo

- Arrastre con preview en vivo del objeto provisional; al soltar,
  `Command::add` vía `History` (undo/redo ilimitado, D6).
- Lápiz: acumula puntos del arrastre (`PenAnnotation`).
- Resaltador: usa el color actual con alfa 128.
- **Texto flotante in situ**: clic con Texto activa → control EDIT
  multilínea en ese punto del lienzo con fuente/tamaño/negrita/color
  actuales (Enter = nueva línea); clic fuera (pérdida de foco) confirma
  y crea la `TextAnnotation`; `Esc` dentro del EDIT cancela; texto
  vacío no crea nada.
- **Rendimiento**: frame "comprometido" (base + documento) cacheado
  como DIB, regenerado SOLO al cambiar el documento (add/undo/redo);
  el preview del arrastre renderiza sobre una copia del comprometido.

## Flujo OK / Cancelar / sucio

- **OK**: hornea (`Document::render_onto` sobre la captura) y devuelve
  el frame al editor, que sustituye imagen y título y queda **editado**.
- Editor editado: «Cerrar»/✕ preguntan «¿Descartar los cambios?»
  (Sí/No). «Guardar como…» o «Copiar» con éxito limpian la marca.
- **Cancelar** (o Esc): descarta las anotaciones; si el documento no
  está vacío, confirma antes. El editor queda como estaba.

## Fuentes

`platform-win` lee `C:\Windows\Fonts\segoeui.ttf` y `segoeuib.ttf` al
abrir la ventana y las inyecta al `RenderContext`; si faltan, el texto
se deshabilita (botón gris) y el resto funciona.

## Testing

- TDD: `view_to_frame`/`frame_to_view` (mapeo con encaje y bordes),
  elección de alfa del resaltador, la tabla de colores/grosores.
- Manual guiado: cada herramienta, preview en vivo, texto in situ
  (confirmar/cancelar/multilínea), undo/redo (botones y atajos),
  colores/grosores, OK→editor sucio→aviso al cerrar, Cancelar limpio.

## Fuera de alcance

Selección/movimiento/edición de objetos ya puestos, goma por objeto,
pasos numerados, leyendas, pixelado/blur, zoom del lienzo, guardar el
documento re-editable (f.31).
