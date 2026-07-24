# F3/B — Motor de anotación en core — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** El motor D5+D6 en `crates/core/src/annotate/`: `Canvas` con mezcla alfa, rasterización pura de formas, siete anotaciones (rect, elipse, línea, flecha, lápiz, resaltador, texto con `fontdue`), `Document` + Commands con undo/redo ilimitado. Cero UI. Spec: `2026-07-24-motor-anotacion-design.md`.

**Architecture:** Un archivo por responsabilidad: `style` (tipos), `canvas` (única puerta de píxeles), `shapes` (rasterización interna), `annotations/` (Strategy, un archivo por tipo), `text` (`RenderContext` con fuentes inyectadas opcionales), `document` (D6). Los Commands poseen los `Box<dyn Annotation>` y los mueven con `Option::take` entre comando y documento — sin exigir `Clone`.

**Tech Stack:** `fontdue` 0.9 (rasterizador puro; ajustar versión si crates.io dice otra). Sin más deps.

## Global Constraints

- `rustcapture-core` puro: cero Win32, cero UI; la fuente SIEMPRE llega inyectada como bytes (los tests la leen de `C:\Windows\Fonts` — proyecto Windows-only; el resto del motor usa `RenderContext::sin_fuente()`).
- TDD estricto por píxeles (skills.md); sin antialiasing en el MVP.
- Convenios de la casa: rustdoc en español, `cargo fmt` antes de verificar, tests inline.
- **Commits: SOLO con aprobación humana previa.** Único commit: `v0.2.4 — F3/B: motor de anotación (D5+D6)`.
- Texto sin fuente cargada → no-op documentado (la GUI siempre la carga).
- Coordenadas de anotaciones en píxeles del frame (i32; fuera de rango se recorta en `Canvas`, nunca panica).

---

### Task 1: `style.rs` + `canvas.rs` (mezcla alfa)

**Files:**
- Create: `crates/core/src/annotate/style.rs`
- Create: `crates/core/src/annotate/canvas.rs`
- Modify: `crates/core/src/annotate/mod.rs` (hoy solo doc-comment)

**Interfaces:**
- Consumes: `ports::Frame`.
- Produces: `Color { r,g,b,a: u8 }` con `rgb(r,g,b)` (a=255) y `rgba(...)` (const fns); `Style { color: Color, thickness: u32 }`; `TextStyle { color: Color, size: f32, bold: bool }`; `Canvas::new(&mut Frame)`, `width()/height()`, `blend_pixel(x: i32, y: i32, color: Color)` (src-over; fuera de rango = no-op; el frame queda opaco).

- [ ] **Step 1: Tests que fallan** — `canvas.rs`:

```rust
//! Canvas (D5): única puerta de escritura de píxeles del motor. Envuelve
//! un `Frame` RGBA — da igual captura fija o fotograma de vídeo.

use crate::annotate::style::Color;
use crate::ports::Frame;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaco_sustituye_el_pixel() {
        let mut frame = Frame::filled(2, 2, [10, 10, 10, 255]);
        let mut canvas = Canvas::new(&mut frame);
        canvas.blend_pixel(1, 0, Color::rgb(200, 0, 0));
        assert_eq!(frame.pixel(1, 0), Some([200, 0, 0, 255]));
        assert_eq!(frame.pixel(0, 0), Some([10, 10, 10, 255]));
    }

    #[test]
    fn semitransparente_mezcla_src_over() {
        let mut frame = Frame::filled(1, 1, [0, 0, 0, 255]);
        let mut canvas = Canvas::new(&mut frame);
        canvas.blend_pixel(0, 0, Color::rgba(255, 255, 255, 128));
        let [r, g, b, a] = frame.pixel(0, 0).unwrap();
        assert!((127..=129).contains(&r) && r == g && g == b);
        assert_eq!(a, 255);
    }

    #[test]
    fn fuera_de_rango_es_noop() {
        let mut frame = Frame::filled(2, 2, [9, 9, 9, 255]);
        let mut canvas = Canvas::new(&mut frame);
        canvas.blend_pixel(-1, 0, Color::rgb(1, 1, 1));
        canvas.blend_pixel(0, 99, Color::rgb(1, 1, 1));
        assert_eq!(frame, Frame::filled(2, 2, [9, 9, 9, 255]));
    }
}
```

`style.rs` (tipos + un test de constructores):

```rust
//! Tipos de estilo compartidos por todas las anotaciones (D5).

/// Color RGBA; la opacidad de la herramienta viaja en `a`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

/// Estilo de las herramientas geométricas.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Style {
    pub color: Color,
    /// Grosor del trazo en píxeles (mínimo efectivo: 1).
    pub thickness: u32,
}

/// Estilo del texto (f.22).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TextStyle {
    pub color: Color,
    /// Altura de la fuente en píxeles.
    pub size: f32,
    pub bold: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_es_opaco() {
        assert_eq!(Color::rgb(1, 2, 3), Color::rgba(1, 2, 3, 255));
    }
}
```

`annotate/mod.rs` (conservar doc-comment existente):

```rust
mod canvas;
mod style;

pub use canvas::Canvas;
pub use style::{Color, Style, TextStyle};
```

- [ ] **Step 2: Rojo** — `cargo test -p rustcapture-core` → FAIL (`Canvas` sin implementar).

- [ ] **Step 3: Implementar** — en `canvas.rs`:

```rust
/// Envuelve el frame y mezcla píxeles con alfa (src-over). El frame de
/// salida se mantiene opaco (las capturas lo son).
pub struct Canvas<'a> {
    frame: &'a mut Frame,
}

impl<'a> Canvas<'a> {
    pub fn new(frame: &'a mut Frame) -> Self {
        Self { frame }
    }

    pub fn width(&self) -> u32 {
        self.frame.width
    }

    pub fn height(&self) -> u32 {
        self.frame.height
    }

    /// Mezcla `color` sobre el píxel; fuera de rango no hace nada.
    pub fn blend_pixel(&mut self, x: i32, y: i32, color: Color) {
        if x < 0 || y < 0 || x as u32 >= self.frame.width || y as u32 >= self.frame.height {
            return;
        }
        let i = (y as usize * self.frame.width as usize + x as usize) * 4;
        let a = color.a as u32;
        let px = &mut self.frame.pixels[i..i + 4];
        px[0] = ((color.r as u32 * a + px[0] as u32 * (255 - a)) / 255) as u8;
        px[1] = ((color.g as u32 * a + px[1] as u32 * (255 - a)) / 255) as u8;
        px[2] = ((color.b as u32 * a + px[2] as u32 * (255 - a)) / 255) as u8;
        px[3] = 255;
    }
}
```

- [ ] **Step 4: Verde** — `cargo fmt && cargo test -p rustcapture-core` → PASS (82 + 4 = 86).

- [ ] **Step 5: Staging** — `git add crates/core/`

---

### Task 2: `shapes.rs` — rasterización pura

**Files:**
- Create: `crates/core/src/annotate/shapes.rs`
- Modify: `crates/core/src/annotate/mod.rs` (añadir `mod shapes;`)

**Interfaces:**
- Consumes: `Canvas`, `Color`, `Style`, `ports::Rect`.
- Produces (`pub(crate)`): `stamp_disc(canvas, cx, cy, thickness, color)`; `draw_line(canvas, a: (i32,i32), b: (i32,i32), &Style)` (Bresenham + disco); `draw_polyline(canvas, &[(i32,i32)], &Style)`; `draw_rect_outline(canvas, Rect, &Style)`; `draw_ellipse_outline(canvas, Rect, &Style)` (muestreo paramétrico); `fill_rect_blend(canvas, Rect, Color)`.

- [ ] **Step 1: Tests que fallan** — `shapes.rs`:

```rust
//! Rasterización pura de formas (sin antialiasing, MVP). Interna: las
//! anotaciones son la API pública.

use crate::annotate::canvas::Canvas;
use crate::annotate::style::{Color, Style};
use crate::ports::Rect;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::Frame;

    const NEGRO: [u8; 4] = [0, 0, 0, 255];
    const ROJO: Color = Color::rgb(255, 0, 0);

    fn lienzo() -> Frame {
        Frame::filled(20, 20, NEGRO)
    }

    fn es_rojo(frame: &Frame, x: u32, y: u32) -> bool {
        frame.pixel(x, y) == Some([255, 0, 0, 255])
    }

    #[test]
    fn linea_horizontal_grosor_uno_pinta_su_fila() {
        let mut frame = lienzo();
        draw_line(
            &mut Canvas::new(&mut frame),
            (2, 5),
            (8, 5),
            &Style { color: ROJO, thickness: 1 },
        );
        for x in 2..=8 {
            assert!(es_rojo(&frame, x, 5), "falta ({x},5)");
        }
        assert!(!es_rojo(&frame, 1, 5));
        assert!(!es_rojo(&frame, 5, 4));
    }

    #[test]
    fn linea_gruesa_cubre_vecinos() {
        let mut frame = lienzo();
        draw_line(
            &mut Canvas::new(&mut frame),
            (2, 10),
            (12, 10),
            &Style { color: ROJO, thickness: 3 },
        );
        assert!(es_rojo(&frame, 7, 9) && es_rojo(&frame, 7, 10) && es_rojo(&frame, 7, 11));
    }

    #[test]
    fn rect_outline_pinta_bordes_y_no_el_interior() {
        let mut frame = lienzo();
        draw_rect_outline(
            &mut Canvas::new(&mut frame),
            Rect::new(3, 3, 10, 8),
            &Style { color: ROJO, thickness: 1 },
        );
        assert!(es_rojo(&frame, 3, 3) && es_rojo(&frame, 12, 10));
        assert!(es_rojo(&frame, 7, 3) && es_rojo(&frame, 3, 7));
        assert!(!es_rojo(&frame, 7, 7)); // interior limpio
    }

    #[test]
    fn elipse_toca_los_cuatro_extremos_y_no_el_centro() {
        let mut frame = lienzo();
        draw_ellipse_outline(
            &mut Canvas::new(&mut frame),
            Rect::new(2, 4, 16, 10),
            &Style { color: ROJO, thickness: 1 },
        );
        assert!(es_rojo(&frame, 10, 4)); // arriba
        assert!(es_rojo(&frame, 10, 13)); // abajo
        assert!(es_rojo(&frame, 2, 9)); // izquierda
        assert!(es_rojo(&frame, 17, 9)); // derecha
        assert!(!es_rojo(&frame, 10, 9)); // centro limpio
    }

    #[test]
    fn fill_blend_mezcla_el_interior_completo() {
        let mut frame = lienzo();
        fill_rect_blend(
            &mut Canvas::new(&mut frame),
            Rect::new(5, 5, 4, 4),
            Color::rgba(255, 255, 0, 128),
        );
        let [r, g, b, _] = frame.pixel(6, 6).unwrap();
        assert!((127..=129).contains(&r) && (127..=129).contains(&g) && b == 0);
        assert_eq!(frame.pixel(4, 4), Some(NEGRO));
    }

    #[test]
    fn polyline_une_todos_los_tramos() {
        let mut frame = lienzo();
        draw_polyline(
            &mut Canvas::new(&mut frame),
            &[(2, 2), (10, 2), (10, 10)],
            &Style { color: ROJO, thickness: 1 },
        );
        assert!(es_rojo(&frame, 6, 2) && es_rojo(&frame, 10, 6));
    }
}
```

- [ ] **Step 2: Rojo** — FAIL.

- [ ] **Step 3: Implementar** — sobre los tests:

```rust
/// Disco de diámetro `thickness` (mínimo 1) centrado en (cx, cy).
pub(crate) fn stamp_disc(canvas: &mut Canvas, cx: i32, cy: i32, thickness: u32, color: Color) {
    if thickness <= 1 {
        canvas.blend_pixel(cx, cy, color);
        return;
    }
    let radio = thickness as i32 / 2;
    for dy in -radio..=radio {
        for dx in -radio..=radio {
            if dx * dx + dy * dy <= radio * radio {
                canvas.blend_pixel(cx + dx, cy + dy, color);
            }
        }
    }
}

/// Bresenham con estampado de disco por punto.
pub(crate) fn draw_line(canvas: &mut Canvas, a: (i32, i32), b: (i32, i32), style: &Style) {
    let (mut x, mut y) = a;
    let dx = (b.0 - a.0).abs();
    let dy = -(b.1 - a.1).abs();
    let sx = if a.0 < b.0 { 1 } else { -1 };
    let sy = if a.1 < b.1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        stamp_disc(canvas, x, y, style.thickness, style.color);
        if x == b.0 && y == b.1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

pub(crate) fn draw_polyline(canvas: &mut Canvas, points: &[(i32, i32)], style: &Style) {
    for par in points.windows(2) {
        draw_line(canvas, par[0], par[1], style);
    }
}

pub(crate) fn draw_rect_outline(canvas: &mut Canvas, rect: Rect, style: &Style) {
    if rect.is_empty() {
        return;
    }
    let x2 = rect.x + rect.width as i32 - 1;
    let y2 = rect.y + rect.height as i32 - 1;
    draw_line(canvas, (rect.x, rect.y), (x2, rect.y), style);
    draw_line(canvas, (x2, rect.y), (x2, y2), style);
    draw_line(canvas, (x2, y2), (rect.x, y2), style);
    draw_line(canvas, (rect.x, y2), (rect.x, rect.y), style);
}

/// Contorno por muestreo paramétrico (pasos ∝ perímetro del rect).
pub(crate) fn draw_ellipse_outline(canvas: &mut Canvas, rect: Rect, style: &Style) {
    if rect.is_empty() {
        return;
    }
    let rx = (rect.width as f64 - 1.0) / 2.0;
    let ry = (rect.height as f64 - 1.0) / 2.0;
    let cx = rect.x as f64 + rx;
    let cy = rect.y as f64 + ry;
    let pasos = (4.0 * (rect.width + rect.height) as f64).max(16.0) as u32;
    for i in 0..pasos {
        let t = i as f64 / pasos as f64 * std::f64::consts::TAU;
        let x = (cx + rx * t.cos()).round() as i32;
        let y = (cy + ry * t.sin()).round() as i32;
        stamp_disc(canvas, x, y, style.thickness, style.color);
    }
}

pub(crate) fn fill_rect_blend(canvas: &mut Canvas, rect: Rect, color: Color) {
    for y in rect.y..rect.y + rect.height as i32 {
        for x in rect.x..rect.x + rect.width as i32 {
            canvas.blend_pixel(x, y, color);
        }
    }
}
```

(En `mod.rs`: `mod shapes;`. Nota: si la variable `r`/`_` sobra tras compilar, eliminarla — clippy mandará.)

- [ ] **Step 4: Verde** — `cargo fmt && cargo test -p rustcapture-core` → PASS (86 + 6 = 92).

- [ ] **Step 5: Staging** — `git add crates/core/`

---

### Task 3: `RenderContext` + anotaciones geométricas (Strategy)

**Files:**
- Modify: `Cargo.toml` + `crates/core/Cargo.toml` (dep `fontdue`)
- Create: `crates/core/src/annotate/text.rs` (solo `RenderContext` en esta task)
- Create: `crates/core/src/annotate/annotations/mod.rs` + `rect.rs`, `ellipse.rs`, `line.rs`, `arrow.rs`, `pen.rs`, `highlight.rs`
- Modify: `crates/core/src/annotate/mod.rs`

**Interfaces:**
- Consumes: Tasks 1-2.
- Produces:
  - `text::RenderContext` con `new(font: &[u8], bold: &[u8]) -> Result<Self, String>` y `sin_fuente()` (fuentes `Option<fontdue::Font>`; getter `pub(crate) fn font(&self, bold: bool) -> Option<&fontdue::Font>`).
  - `annotations::Annotation` (trait): `fn render(&self, canvas: &mut Canvas, ctx: &RenderContext);`
  - Structs públicos (campos pub): `RectAnnotation { rect, style }`, `EllipseAnnotation { rect, style }`, `LineAnnotation { from: (i32,i32), to, style }`, `ArrowAnnotation { from, to, style }` (línea + cabeza en V, brazos a ±150°, largo `max(10, thickness*4)`), `PenAnnotation { points: Vec<(i32,i32)>, style }`, `HighlightAnnotation { rect, color }`.

- [ ] **Step 1: Dep** — workspace: `fontdue = "0.9"`; core: `fontdue = { workspace = true }`.

- [ ] **Step 2: Tests que fallan** — en `annotations/mod.rs`:

```rust
//! Anotaciones (D5): una Strategy por tipo, un archivo por tipo.

mod arrow;
mod ellipse;
mod highlight;
mod line;
mod pen;
mod rect;
// `mod text;` + `pub use text::TextAnnotation;` se activan en la Task 4.

pub use arrow::ArrowAnnotation;
pub use ellipse::EllipseAnnotation;
pub use highlight::HighlightAnnotation;
pub use line::LineAnnotation;
pub use pen::PenAnnotation;
pub use rect::RectAnnotation;

use crate::annotate::canvas::Canvas;
use crate::annotate::text::RenderContext;

/// Strategy de anotación (D5): renderiza sobre el canvas; al motor le da
/// igual si debajo hay una captura o un fotograma de vídeo.
pub trait Annotation {
    fn render(&self, canvas: &mut Canvas, ctx: &RenderContext);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotate::style::{Color, Style};
    use crate::ports::{Frame, Rect};

    const ROJO: Color = Color::rgb(255, 0, 0);
    const ESTILO: Style = Style { color: ROJO, thickness: 1 };

    fn render(a: &dyn Annotation) -> Frame {
        let mut frame = Frame::filled(30, 30, [0, 0, 0, 255]);
        a.render(&mut Canvas::new(&mut frame), &RenderContext::sin_fuente());
        frame
    }

    fn es_rojo(frame: &Frame, x: u32, y: u32) -> bool {
        frame.pixel(x, y) == Some([255, 0, 0, 255])
    }

    #[test]
    fn rect_y_elipse_dibujan_contornos() {
        let frame = render(&RectAnnotation { rect: Rect::new(2, 2, 10, 10), style: ESTILO });
        assert!(es_rojo(&frame, 2, 2) && es_rojo(&frame, 11, 11) && !es_rojo(&frame, 6, 6));
        let frame = render(&EllipseAnnotation { rect: Rect::new(2, 2, 20, 10), style: ESTILO });
        assert!(es_rojo(&frame, 12, 2) && !es_rojo(&frame, 12, 7));
    }

    #[test]
    fn linea_y_lapiz_trazan_sus_puntos() {
        let frame = render(&LineAnnotation { from: (0, 0), to: (10, 10), style: ESTILO });
        assert!(es_rojo(&frame, 5, 5));
        let frame = render(&PenAnnotation {
            points: vec![(1, 1), (8, 1), (8, 8)],
            style: ESTILO,
        });
        assert!(es_rojo(&frame, 4, 1) && es_rojo(&frame, 8, 4));
    }

    #[test]
    fn la_flecha_tiene_cabeza_fuera_del_eje() {
        // Flecha horizontal → la cabeza pone píxeles por encima y por
        // debajo del eje y=10 cerca de la punta.
        let frame = render(&ArrowAnnotation { from: (2, 10), to: (25, 10), style: ESTILO });
        assert!(es_rojo(&frame, 10, 10)); // eje
        let cabeza_arriba = (18..25).any(|x| (5..10).any(|y| es_rojo(&frame, x, y)));
        let cabeza_abajo = (18..25).any(|x| (11..16).any(|y| es_rojo(&frame, x, y)));
        assert!(cabeza_arriba && cabeza_abajo);
    }

    #[test]
    fn el_resaltador_mezcla_sin_tapar() {
        let frame = render(&HighlightAnnotation {
            rect: Rect::new(5, 5, 8, 8),
            color: Color::rgba(255, 255, 0, 128),
        });
        let [r, g, b, _] = frame.pixel(8, 8).unwrap();
        assert!(r > 100 && g > 100 && b == 0); // amarillo a medias
    }
}
```

`text.rs` (esta task; `TextAnnotation` llega en la Task 4 pero su módulo ya se declara — crear `annotations/text.rs` con el struct SIN render de glifos aún no es posible porque el trait lo exige; así que `annotations/text.rs` se crea COMPLETO en la Task 4 y en esta task su `mod text;`/re-export se deja comentado):

```rust
//! Contexto de render: las fuentes llegan inyectadas como bytes (el core
//! nunca abre archivos). Sin fuente, el texto es no-op documentado.

pub struct RenderContext {
    normal: Option<fontdue::Font>,
    bold: Option<fontdue::Font>,
}

impl RenderContext {
    /// Carga ambas variantes desde bytes TTF/OTF.
    pub fn new(font: &[u8], bold: &[u8]) -> Result<Self, String> {
        let settings = fontdue::FontSettings::default();
        Ok(Self {
            normal: Some(fontdue::Font::from_bytes(font, settings.clone()).map_err(String::from)?),
            bold: Some(fontdue::Font::from_bytes(bold, settings).map_err(String::from)?),
        })
    }

    /// Contexto sin tipografía: todo salvo el texto funciona igual.
    pub fn sin_fuente() -> Self {
        Self {
            normal: None,
            bold: None,
        }
    }

    pub(crate) fn font(&self, bold: bool) -> Option<&fontdue::Font> {
        if bold { self.bold.as_ref() } else { self.normal.as_ref() }
    }
}
```

Ajuste para compilar esta task: en `annotations/mod.rs` dejar `mod text;` y `pub use text::TextAnnotation;` FUERA (se añaden en Task 4). `annotate/mod.rs` queda:

```rust
pub mod annotations;
mod canvas;
mod shapes;
mod style;
mod text;

pub use canvas::Canvas;
pub use style::{Color, Style, TextStyle};
pub use text::RenderContext;
```

- [ ] **Step 3: Rojo** — FAIL (structs sin implementar).

- [ ] **Step 4: Implementar** — un archivo por tipo. `rect.rs`:

```rust
//! Rectángulo de contorno (f.22).

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::shapes;
use crate::annotate::style::Style;
use crate::annotate::text::RenderContext;
use crate::ports::Rect;

pub struct RectAnnotation {
    pub rect: Rect,
    pub style: Style,
}

impl Annotation for RectAnnotation {
    fn render(&self, canvas: &mut Canvas, _ctx: &RenderContext) {
        shapes::draw_rect_outline(canvas, self.rect, &self.style);
    }
}
```

`ellipse.rs` (idéntico patrón con `draw_ellipse_outline`), `line.rs`:

```rust
//! Línea recta (f.22).

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::shapes;
use crate::annotate::style::Style;
use crate::annotate::text::RenderContext;

pub struct LineAnnotation {
    pub from: (i32, i32),
    pub to: (i32, i32),
    pub style: Style,
}

impl Annotation for LineAnnotation {
    fn render(&self, canvas: &mut Canvas, _ctx: &RenderContext) {
        shapes::draw_line(canvas, self.from, self.to, &self.style);
    }
}
```

`arrow.rs`:

```rust
//! Flecha (f.22): línea + cabeza en V hacia atrás desde la punta.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::shapes;
use crate::annotate::style::Style;
use crate::annotate::text::RenderContext;

pub struct ArrowAnnotation {
    pub from: (i32, i32),
    pub to: (i32, i32),
    pub style: Style,
}

impl Annotation for ArrowAnnotation {
    fn render(&self, canvas: &mut Canvas, ctx: &RenderContext) {
        let _ = ctx;
        shapes::draw_line(canvas, self.from, self.to, &self.style);
        let dx = (self.to.0 - self.from.0) as f64;
        let dy = (self.to.1 - self.from.1) as f64;
        let largo_eje = (dx * dx + dy * dy).sqrt();
        if largo_eje < 1.0 {
            return;
        }
        let angulo = dy.atan2(dx);
        let largo = (self.style.thickness as f64 * 4.0).max(10.0).min(largo_eje);
        // Brazos a ±150° del sentido de la flecha.
        for signo in [-1.0, 1.0] {
            let a = angulo + signo * 150.0_f64.to_radians();
            let px = (self.to.0 as f64 + largo * a.cos()).round() as i32;
            let py = (self.to.1 as f64 + largo * a.sin()).round() as i32;
            shapes::draw_line(canvas, self.to, (px, py), &self.style);
        }
    }
}
```

`pen.rs` (polilínea con `draw_polyline`), `highlight.rs`:

```rust
//! Resaltador (f.22): relleno semitransparente que no tapa el contenido.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::shapes;
use crate::annotate::style::Color;
use crate::annotate::text::RenderContext;
use crate::ports::Rect;

pub struct HighlightAnnotation {
    pub rect: Rect,
    /// Color CON alfa (típico: amarillo a 128).
    pub color: Color,
}

impl Annotation for HighlightAnnotation {
    fn render(&self, canvas: &mut Canvas, _ctx: &RenderContext) {
        shapes::fill_rect_blend(canvas, self.rect, self.color);
    }
}
```

- [ ] **Step 5: Verde** — `cargo fmt && cargo test -p rustcapture-core` → PASS (92 + 4 = 96).

- [ ] **Step 6: Staging** — `git add Cargo.toml Cargo.lock crates/core/`

---

### Task 4: Texto con `fontdue`

**Files:**
- Modify: `crates/core/src/annotate/text.rs` (añadir `draw_text`)
- Create: `crates/core/src/annotate/annotations/text.rs`
- Modify: `crates/core/src/annotate/annotations/mod.rs` (activar `mod text; pub use text::TextAnnotation;`)

**Interfaces:**
- Consumes: `RenderContext`, `Canvas`, `TextStyle`.
- Produces: `TextAnnotation { pos: (i32,i32), text: String, style: TextStyle }`; interna `text::draw_text(canvas, pos, &str, TextStyle, &RenderContext)` — multilínea por `\n` (interlineado `size * 1.2`), glifos de `fontdue` mezclados con la cobertura como alfa; sin fuente → no-op.

- [ ] **Step 1: Tests que fallan** — añadir a `annotations/mod.rs` tests:

```rust
    fn ctx_con_fuente() -> RenderContext {
        let normal = std::fs::read("C:/Windows/Fonts/segoeui.ttf").expect("fuente del sistema");
        let bold = std::fs::read("C:/Windows/Fonts/segoeuib.ttf").expect("fuente del sistema");
        RenderContext::new(&normal, &bold).unwrap()
    }

    #[test]
    fn el_texto_ocupa_su_caja_y_sin_fuente_es_noop() {
        let anotacion = TextAnnotation {
            pos: (5, 5),
            text: "Hola".to_string(),
            style: crate::annotate::style::TextStyle {
                color: ROJO,
                size: 20.0,
                bold: false,
            },
        };
        // Con fuente: aparecen píxeles rojos en la zona del texto.
        let mut frame = Frame::filled(100, 40, [0, 0, 0, 255]);
        anotacion.render(&mut Canvas::new(&mut frame), &ctx_con_fuente());
        let pintados = (5..60)
            .flat_map(|x| (5..35).map(move |y| (x, y)))
            .filter(|&(x, y)| {
                frame.pixel(x, y).is_some_and(|[r, _, _, _]| r > 100)
            })
            .count();
        assert!(pintados > 20, "solo {pintados} píxeles de texto");
        // Sin fuente: no-op.
        let mut vacio = Frame::filled(100, 40, [0, 0, 0, 255]);
        anotacion.render(&mut Canvas::new(&mut vacio), &RenderContext::sin_fuente());
        assert_eq!(vacio, Frame::filled(100, 40, [0, 0, 0, 255]));
    }

    #[test]
    fn el_texto_multilinea_baja_de_linea() {
        let anotacion = TextAnnotation {
            pos: (2, 2),
            text: "A\nA".to_string(),
            style: crate::annotate::style::TextStyle {
                color: ROJO,
                size: 16.0,
                bold: false,
            },
        };
        let mut frame = Frame::filled(60, 60, [0, 0, 0, 255]);
        anotacion.render(&mut Canvas::new(&mut frame), &ctx_con_fuente());
        let fila_ocupada = |y0: u32, y1: u32| {
            (0..60).any(|x| (y0..y1).any(|y| frame.pixel(x, y).is_some_and(|[r, ..]| r > 100)))
        };
        assert!(fila_ocupada(2, 20) && fila_ocupada(21, 45));
    }
```

- [ ] **Step 2: Rojo** — FAIL (`TextAnnotation` no existe).

- [ ] **Step 3: Implementar** — en `text.rs` añadir:

```rust
use crate::annotate::canvas::Canvas;
use crate::annotate::style::TextStyle;

/// Dibuja texto multilínea; la cobertura del glifo modula el alfa del
/// color. Sin fuente cargada, no hace nada (la GUI siempre la carga).
pub(crate) fn draw_text(
    canvas: &mut Canvas,
    pos: (i32, i32),
    text: &str,
    style: TextStyle,
    ctx: &RenderContext,
) {
    let Some(font) = ctx.font(style.bold) else {
        return;
    };
    let line_height = (style.size * 1.2).round() as i32;
    for (n, linea) in text.split('\n').enumerate() {
        let base_y = pos.1 + n as i32 * line_height;
        let mut cursor_x = pos.0 as f32;
        for c in linea.chars() {
            let (metrics, bitmap) = font.rasterize(c, style.size);
            let gx = cursor_x.round() as i32 + metrics.xmin;
            // ymin es respecto a la línea base; colocamos la base a
            // `size` píxeles del tope de la línea.
            let gy = base_y + style.size.round() as i32 - metrics.height as i32 - metrics.ymin;
            for (i, cobertura) in bitmap.iter().enumerate() {
                if *cobertura == 0 {
                    continue;
                }
                let px = gx + (i % metrics.width) as i32;
                let py = gy + (i / metrics.width) as i32;
                let alfa = (style.color.a as u16 * *cobertura as u16 / 255) as u8;
                canvas.blend_pixel(
                    px,
                    py,
                    crate::annotate::style::Color::rgba(
                        style.color.r,
                        style.color.g,
                        style.color.b,
                        alfa,
                    ),
                );
            }
            cursor_x += metrics.advance_width;
        }
    }
}
```

`annotations/text.rs`:

```rust
//! Texto (f.22): render con la fuente inyectada en el RenderContext.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::style::TextStyle;
use crate::annotate::text::{RenderContext, draw_text};

pub struct TextAnnotation {
    pub pos: (i32, i32),
    pub text: String,
    pub style: TextStyle,
}

impl Annotation for TextAnnotation {
    fn render(&self, canvas: &mut Canvas, ctx: &RenderContext) {
        draw_text(canvas, self.pos, &self.text, self.style, ctx);
    }
}
```

(y en `annotations/mod.rs`: activar `mod text;` + `pub use text::TextAnnotation;`.)

- [ ] **Step 4: Verde** — `cargo fmt && cargo test -p rustcapture-core` → PASS (96 + 2 = 98).

- [ ] **Step 5: Staging** — `git add crates/core/`

---

### Task 5: `Document` + Commands + `History` (D6)

**Files:**
- Create: `crates/core/src/annotate/document.rs`
- Modify: `crates/core/src/annotate/mod.rs` (añadir `mod document; pub use document::{Command, Document, History};`)

**Interfaces:**
- Consumes: `annotations::Annotation`, `Canvas`, `RenderContext`.
- Produces:
  - `Document::new()/default`, `len()`, `is_empty()`, `render_onto(&self, frame: &mut Frame, ctx: &RenderContext)`.
  - `Command::add(annotation: Box<dyn Annotation>)` y `Command::remove(index: usize)`.
  - `History::new()/default`, `apply(&mut self, doc, cmd) -> bool` (false = comando inválido, no se apila; vacía redo), `undo(&mut self, doc) -> bool`, `redo(&mut self, doc) -> bool`, `can_undo()/can_redo()`.

- [ ] **Step 1: Tests que fallan** — `document.rs`:

```rust
//! Documento de anotaciones + Commands con undo/redo ilimitado (D6).
//! Los Commands POSEEN las anotaciones y las mueven con Option::take:
//! nada exige Clone.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::text::RenderContext;
use crate::ports::Frame;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotate::annotations::RectAnnotation;
    use crate::annotate::style::{Color, Style};
    use crate::ports::Rect;

    fn caja(x: i32) -> Box<dyn Annotation> {
        Box::new(RectAnnotation {
            rect: Rect::new(x, 1, 3, 3),
            style: Style {
                color: Color::rgb(255, 0, 0),
                thickness: 1,
            },
        })
    }

    fn render(doc: &Document) -> Frame {
        let mut frame = Frame::filled(20, 10, [0, 0, 0, 255]);
        doc.render_onto(&mut frame, &RenderContext::sin_fuente());
        frame
    }

    fn es_rojo(frame: &Frame, x: u32, y: u32) -> bool {
        frame.pixel(x, y) == Some([255, 0, 0, 255])
    }

    #[test]
    fn add_pinta_y_undo_lo_quita() {
        let mut doc = Document::new();
        let mut historia = History::new();
        assert!(historia.apply(&mut doc, Command::add(caja(2))));
        assert_eq!(doc.len(), 1);
        assert!(es_rojo(&render(&doc), 2, 1));

        assert!(historia.undo(&mut doc));
        assert!(doc.is_empty());
        assert!(!es_rojo(&render(&doc), 2, 1));

        assert!(historia.redo(&mut doc));
        assert!(es_rojo(&render(&doc), 2, 1));
    }

    #[test]
    fn remove_borra_el_objeto_y_undo_lo_restaura_en_su_sitio() {
        let mut doc = Document::new();
        let mut historia = History::new();
        historia.apply(&mut doc, Command::add(caja(2)));
        historia.apply(&mut doc, Command::add(caja(10)));

        assert!(historia.apply(&mut doc, Command::remove(0)));
        let frame = render(&doc);
        assert!(!es_rojo(&frame, 2, 1) && es_rojo(&frame, 10, 1));

        assert!(historia.undo(&mut doc));
        let frame = render(&doc);
        assert!(es_rojo(&frame, 2, 1) && es_rojo(&frame, 10, 1));
        assert_eq!(doc.len(), 2);
    }

    #[test]
    fn remove_invalido_no_se_apila() {
        let mut doc = Document::new();
        let mut historia = History::new();
        assert!(!historia.apply(&mut doc, Command::remove(5)));
        assert!(!historia.can_undo());
    }

    #[test]
    fn un_comando_nuevo_vacia_el_redo() {
        let mut doc = Document::new();
        let mut historia = History::new();
        historia.apply(&mut doc, Command::add(caja(2)));
        historia.undo(&mut doc);
        assert!(historia.can_redo());
        historia.apply(&mut doc, Command::add(caja(10)));
        assert!(!historia.can_redo());
        assert!(!historia.redo(&mut doc));
    }

    #[test]
    fn undo_sin_historia_devuelve_false() {
        let mut doc = Document::new();
        let mut historia = History::new();
        assert!(!historia.undo(&mut doc));
        assert!(!historia.redo(&mut doc));
    }
}
```

- [ ] **Step 2: Rojo** — FAIL.

- [ ] **Step 3: Implementar**:

```rust
/// Lista ordenada de anotaciones: el orden es el orden de pintado.
#[derive(Default)]
pub struct Document {
    annotations: Vec<Box<dyn Annotation>>,
}

impl Document {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.annotations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.annotations.is_empty()
    }

    /// Hornea todas las anotaciones sobre el frame, en orden.
    pub fn render_onto(&self, frame: &mut Frame, ctx: &RenderContext) {
        let mut canvas = Canvas::new(frame);
        for annotation in &self.annotations {
            annotation.render(&mut canvas, ctx);
        }
    }
}

/// Comando de edición (D6): posee la anotación que mueve.
pub enum Command {
    Add {
        annotation: Option<Box<dyn Annotation>>,
    },
    Remove {
        index: usize,
        removed: Option<Box<dyn Annotation>>,
    },
}

impl Command {
    pub fn add(annotation: Box<dyn Annotation>) -> Self {
        Command::Add {
            annotation: Some(annotation),
        }
    }

    pub fn remove(index: usize) -> Self {
        Command::Remove {
            index,
            removed: None,
        }
    }

    /// Ejecuta sobre el documento; false = inválido (no aplicar).
    fn apply(&mut self, doc: &mut Document) -> bool {
        match self {
            Command::Add { annotation } => match annotation.take() {
                Some(a) => {
                    doc.annotations.push(a);
                    true
                }
                None => false,
            },
            Command::Remove { index, removed } => {
                if *index >= doc.annotations.len() {
                    return false;
                }
                *removed = Some(doc.annotations.remove(*index));
                true
            }
        }
    }

    /// Deshace lo hecho por `apply` (solo se llama tras un apply con éxito).
    fn revert(&mut self, doc: &mut Document) {
        match self {
            Command::Add { annotation } => {
                *annotation = doc.annotations.pop();
            }
            Command::Remove { index, removed } => {
                if let Some(a) = removed.take() {
                    doc.annotations.insert(*index, a);
                }
            }
        }
    }
}

/// Pilas de undo/redo ilimitadas (D6).
#[derive(Default)]
pub struct History {
    undo: Vec<Command>,
    redo: Vec<Command>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// Aplica y apila; un comando nuevo invalida el redo.
    pub fn apply(&mut self, doc: &mut Document, mut cmd: Command) -> bool {
        if !cmd.apply(doc) {
            return false;
        }
        self.undo.push(cmd);
        self.redo.clear();
        true
    }

    pub fn undo(&mut self, doc: &mut Document) -> bool {
        match self.undo.pop() {
            Some(mut cmd) => {
                cmd.revert(doc);
                self.redo.push(cmd);
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self, doc: &mut Document) -> bool {
        match self.redo.pop() {
            Some(mut cmd) => {
                if !cmd.apply(doc) {
                    return false;
                }
                self.undo.push(cmd);
                true
            }
            None => false,
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}
```

Nota redo de un Add: `revert` devolvió el Box al comando (`annotation = pop()`), así que el `apply` del redo vuelve a tener `Some` y funciona. En `mod.rs`: `mod document; pub use document::{Command, Document, History};`.

- [ ] **Step 4: Verde** — `cargo fmt && cargo test -p rustcapture-core` → PASS (98 + 5 = 103).

- [ ] **Step 5: Staging** — `git add crates/core/`

---

### Task 6: Verificación final y propuesta de commit

- [ ] **Step 1:** `cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check`
Expected: 103 core + 22 platform-win + 10 cli = 135 tests; clippy y formato limpios. Confirmar que `crates/core/Cargo.toml` no tiene nada de Win32 (solo `fontdue` nuevo).

- [ ] **Step 2: Roadmap** — en §4 F3:
- `- ⏳ Modelo de documento…(D5).` → `- 🔵 Modelo de documento: objetos `Annotation`, Strategy + `Canvas` sobre frame RGBA (D5) — hecho el motor; falta la Factory desde la toolbar (Slice C).`
- `- ⏳ Command pattern con undo/redo (D6).` → `- ✅ …`
- `- ⏳ Herramientas: texto, flechas…` → `- 🔵 Herramientas (motor): texto, flechas, líneas, formas, resaltado y lápiz hechos en core; pasos numerados, leyendas y pixelado pendientes; goma = eliminar objeto (llega con la UI del Slice C).`

- [ ] **Step 3: Proponer commit (NO ejecutar sin aprobación)**

```
v0.2.4 — F3/B: motor de anotación (D5+D6)

annotate/ en core: Canvas con mezcla alfa sobre Frame, rasterización
pura de formas, siete anotaciones Strategy (rect, elipse, línea,
flecha, lápiz, resaltador, texto con fontdue y fuente inyectada) y
Document + Commands con undo/redo ilimitado. 21 tests nuevos por
píxeles; cero UI, listo para la ventana de dibujo (Slice C).
```
