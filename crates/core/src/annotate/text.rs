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
            normal: Some(fontdue::Font::from_bytes(font, settings).map_err(String::from)?),
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
        if bold {
            self.bold.as_ref()
        } else {
            self.normal.as_ref()
        }
    }
}

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

/// Caja de la TINTA del texto, relativa al `pos` que recibe `draw_text`:
/// `(dx, dy, ancho, alto)`. `None` sin fuente cargada o si nada pinta
/// (p. ej. solo espacios). Replica la colocación de `draw_text` glifo a
/// glifo — si una cambia, la otra tiene que cambiar igual.
///
/// Existe para centrar ópticamente: la caja de línea incluye hueco de
/// ascendente/descendente que las cifras no usan, y centrar por ella
/// deja el número visiblemente bajo dentro del disco (f.23).
pub(crate) fn text_ink_box(
    text: &str,
    style: TextStyle,
    ctx: &RenderContext,
) -> Option<(i32, i32, u32, u32)> {
    let font = ctx.font(style.bold)?;
    let line_height = (style.size * 1.2).round() as i32;
    let (mut min_x, mut min_y) = (i32::MAX, i32::MAX);
    let (mut max_x, mut max_y) = (i32::MIN, i32::MIN);
    for (n, linea) in text.split('\n').enumerate() {
        let base_y = n as i32 * line_height;
        let mut cursor_x = 0.0f32;
        for c in linea.chars() {
            let metrics = font.metrics(c, style.size);
            if metrics.width > 0 && metrics.height > 0 {
                let gx = cursor_x.round() as i32 + metrics.xmin;
                let gy = base_y + style.size.round() as i32 - metrics.height as i32 - metrics.ymin;
                min_x = min_x.min(gx);
                min_y = min_y.min(gy);
                max_x = max_x.max(gx + metrics.width as i32);
                max_y = max_y.max(gy + metrics.height as i32);
            }
            cursor_x += metrics.advance_width;
        }
    }
    (min_x < max_x && min_y < max_y).then(|| {
        (
            min_x,
            min_y,
            (max_x - min_x) as u32,
            (max_y - min_y) as u32,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotate::canvas::Canvas;
    use crate::annotate::style::Color;
    use crate::ports::Frame;

    fn ctx_con_fuente() -> RenderContext {
        let normal = std::fs::read("C:/Windows/Fonts/segoeui.ttf").expect("fuente del sistema");
        let bold = std::fs::read("C:/Windows/Fonts/segoeuib.ttf").expect("fuente del sistema");
        RenderContext::new(&normal, &bold).unwrap()
    }

    fn estilo(size: f32) -> TextStyle {
        TextStyle {
            color: Color::rgb(255, 0, 0),
            size,
            bold: true,
        }
    }

    #[test]
    fn sin_fuente_no_hay_caja_de_tinta() {
        assert_eq!(
            text_ink_box("7", estilo(20.0), &RenderContext::sin_fuente()),
            None
        );
    }

    #[test]
    fn el_espacio_no_tiene_tinta() {
        assert_eq!(text_ink_box("   ", estilo(20.0), &ctx_con_fuente()), None);
    }

    #[test]
    fn la_caja_de_tinta_encierra_exactamente_los_pixeles_pintados() {
        // La caja se mide y luego se compara con lo que de verdad se pinta.
        let ctx = ctx_con_fuente();
        let style = estilo(24.0);
        let (dx, dy, w, h) = text_ink_box("12", style, &ctx).expect("hay tinta");
        assert!(w > 0 && h > 0);

        let origen = (10, 10);
        let mut frame = Frame::filled(80, 60, [0, 0, 0, 255]);
        draw_text(&mut Canvas::new(&mut frame), origen, "12", style, &ctx);
        let pintados: Vec<(u32, u32)> = (0..80)
            .flat_map(|x| (0..60).map(move |y| (x, y)))
            .filter(|&(x, y)| frame.pixel(x, y).is_some_and(|[r, ..]| r > 0))
            .collect();
        assert!(!pintados.is_empty());
        let min_x = pintados.iter().map(|p| p.0).min().unwrap() as i32;
        let min_y = pintados.iter().map(|p| p.1).min().unwrap() as i32;
        let max_x = pintados.iter().map(|p| p.0).max().unwrap() as i32;
        let max_y = pintados.iter().map(|p| p.1).max().unwrap() as i32;
        // La caja predicha contiene la tinta real y no se pasa de holgada.
        assert_eq!((min_x, min_y), (origen.0 + dx, origen.1 + dy));
        assert_eq!(
            (max_x + 1, max_y + 1),
            (origen.0 + dx + w as i32, origen.1 + dy + h as i32)
        );
    }

    #[test]
    fn dos_lineas_dan_una_caja_mas_alta_que_una() {
        let ctx = ctx_con_fuente();
        let (_, _, _, h1) = text_ink_box("A", estilo(20.0), &ctx).unwrap();
        let (_, _, _, h2) = text_ink_box("A\nA", estilo(20.0), &ctx).unwrap();
        assert!(h2 > h1 * 2 - 4, "h1 = {h1}, h2 = {h2}");
    }
}
