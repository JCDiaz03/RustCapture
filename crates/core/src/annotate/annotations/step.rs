//! Paso numerado (f.23): disco relleno con su número centrado dentro. El
//! radio se deriva del tamaño de fuente y de los dígitos, así el número
//! nunca sobresale del disco y la herramienta es un solo clic.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::shapes;
use crate::annotate::style::{Color, TextStyle};
use crate::annotate::text::{RenderContext, draw_text, text_ink_box};

#[derive(Clone)]
pub struct StepAnnotation {
    /// Centro del disco, en píxeles del frame.
    pub center: (i32, i32),
    pub number: u32,
    /// Color del disco; el número se pinta en blanco o negro según él.
    pub color: Color,
    /// Altura de la fuente del número (misma escala que `TextAnnotation`).
    pub font_size: f32,
}

impl StepAnnotation {
    /// Radio del disco: cubre el número con margen y crece con los
    /// dígitos, de modo que el 12 no queda apretado donde caía el 1.
    pub fn radius(&self) -> u32 {
        let digitos = self.number.to_string().len() as f32;
        (self.font_size * (0.75 + 0.22 * (digitos - 1.0)))
            .round()
            .max(2.0) as u32
    }
}

impl Annotation for StepAnnotation {
    fn render(&self, canvas: &mut Canvas, ctx: &RenderContext) {
        shapes::fill_disc_aa(canvas, self.center, self.radius(), self.color);
        let style = TextStyle {
            color: self.color.contraste(),
            size: self.font_size,
            bold: true,
        };
        let etiqueta = self.number.to_string();
        // Se centra la caja de TINTA, no la de línea: ver `text_ink_box`.
        if let Some((dx, dy, w, h)) = text_ink_box(&etiqueta, style, ctx) {
            let pos = (
                self.center.0 - dx - w as i32 / 2,
                self.center.1 - dy - h as i32 / 2,
            );
            draw_text(canvas, pos, &etiqueta, style, ctx);
        }
    }
}
