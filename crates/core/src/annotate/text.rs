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
