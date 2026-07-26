//! Paso numerado (f.23): disco relleno con su número centrado dentro. El
//! radio se deriva del tamaño de fuente y de los dígitos, así el número
//! nunca sobresale del disco y la herramienta es un solo clic.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::giro::Giro;
use crate::annotate::shapes;
use crate::annotate::style::{Color, FamiliaId, TextStyle};
use crate::annotate::text::{RenderContext, draw_text, draw_text_rotado, text_ink_box};
use crate::ports::Rect;

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

impl StepAnnotation {
    pub(crate) fn caja(&self) -> Rect {
        let r = self.radius() as i32;
        Rect::bounding(
            &[
                (self.center.0 - r, self.center.1 - r),
                (self.center.0 + r, self.center.1 + r),
            ],
            0,
        )
    }

    /// El disco es invariante al giro (es redondo) y su centro coincide con
    /// el centro de giro, así que solo el NÚMERO rota.
    pub(crate) fn render_girado(&self, canvas: &mut Canvas, ctx: &RenderContext, giro: Giro) {
        if giro.es_nulo() {
            return self.render(canvas, ctx);
        }
        shapes::fill_disc_aa(canvas, self.center, self.radius(), self.color);
        let (style, etiqueta) = self.estilo_numero();
        if let Some((dx, dy, w, h)) = text_ink_box(&etiqueta, style, ctx) {
            let pos = (
                self.center.0 - dx - w as i32 / 2,
                self.center.1 - dy - h as i32 / 2,
            );
            // El número gira alrededor del centro del disco, que es también
            // el centro de giro del objeto.
            let centro = (self.center.0 as f32, self.center.1 as f32);
            draw_text_rotado(canvas, pos, &etiqueta, style, ctx, giro, centro);
        }
    }

    /// Estilo y etiqueta del número: un solo sitio, lo comparten el render
    /// normal y el girado. El número usa la familia de respaldo a propósito:
    /// es un indicador, no texto del usuario.
    fn estilo_numero(&self) -> (TextStyle, String) {
        (
            TextStyle {
                color: self.color.contraste(),
                size: self.font_size,
                bold: true,
                familia: FamiliaId::default(),
            },
            self.number.to_string(),
        )
    }
}

impl Annotation for StepAnnotation {
    fn render(&self, canvas: &mut Canvas, ctx: &RenderContext) {
        shapes::fill_disc_aa(canvas, self.center, self.radius(), self.color);
        let (style, etiqueta) = self.estilo_numero();
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
