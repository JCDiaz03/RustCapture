# Spec — Motor de anotación en core (Slice B de F3; D5+D6, f.22-f.27 parcial)

Diseño validado con el humano (brainstorming 2026-07-24). Es el motor que
alimentará la ventana de dibujo (Ventana2.jpg) en el Slice C.

## Alcance

Set esencial de herramientas, todo en `crates/core/src/annotate/`, puro y
con TDD (cero UI, cero Win32):

- **Tipos**: rectángulo (contorno), elipse (contorno), línea, flecha
  (línea + cabeza triangular), lápiz (polilínea a mano alzada),
  resaltador (relleno semitransparente) y texto.
- **Propiedades**: `Color` RGBA (la opacidad viaja en el alfa),
  `Style { color, thickness }`; texto con `TextStyle { size, bold, color }`.
- **Undo/redo ilimitado** (D6): Commands `AddAnnotation` /
  `RemoveAnnotation` con `apply`/`revert` sobre el documento e `History`
  con pilas de undo/redo (redo se vacía al aplicar un comando nuevo).
  El "borrador" del Slice C será eliminar objetos (Command), no goma de
  píxeles.

## Estructura

- `style.rs` — `Color`, `Style`, `TextStyle`.
- `canvas.rs` — `Canvas` sobre `&mut Frame` con `blend_pixel` (mezcla
  alfa estándar). Única puerta de escritura de píxeles del motor.
- `shapes.rs` — rasterización pura: línea con grosor (estampado de
  disco), rect/elipse de contorno, polilínea, relleno mezclado.
- `annotations/` — trait `Annotation { fn render(&self, canvas: &mut Canvas, ctx: &RenderContext) }`
  (Strategy, un archivo por tipo, D5).
- `text.rs` — rasterización con `fontdue`; la fuente llega inyectada
  como bytes en `RenderContext { font, font_bold }` (la GUI pasará
  `C:\Windows\Fonts\segoeui.ttf` / `segoeuib.ttf`; proyecto
  Windows-only). Multilínea por `\n`.
- `document.rs` — `Document` (Vec ordenado de anotaciones),
  `Command`, `History`, `Document::render_onto(&self, frame, ctx)`.

## Decisiones

- Render sin antialiasing en el MVP (estampado directo); la calidad
  visual se itera después sin tocar la API.
- La fuente NUNCA se abre desde el core (bytes inyectados): el core
  sigue puro y este mismo motor renderizará sobre frames de vídeo (D5).
- `fontdue` como única dependencia nueva (rasterizador puro, ligero).
- Los Commands poseen las anotaciones (`Box<dyn Annotation>`): `revert`
  de un Add es quitar del documento; `revert` de un Remove es devolver
  el Box guardado — sin exigir `Clone` a las anotaciones.

## Testing

TDD por píxeles concretos: la línea horizontal pinta su fila con el
grosor pedido, el resaltador mezcla ~50 %, la flecha tiene píxeles de
cabeza fuera del eje, undo/redo restauran el render anterior. El texto
se verifica por ocupación (píxeles del color del texto dentro de la caja
esperada), no por glifos exactos; sus tests leen la fuente del sistema.

## Fuera de alcance

UI de dibujo (Slice C), selección/movimiento/edición de objetos ya
puestos, serialización re-editable (f.31), callouts, pasos numerados,
polígonos, blur/pixelado, antialiasing.
