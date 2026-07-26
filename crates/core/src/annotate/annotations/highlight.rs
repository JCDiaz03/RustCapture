//! Resaltador (f.22): relleno semitransparente que no tapa el contenido.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::giro::Giro;
use crate::annotate::shapes;
use crate::annotate::style::Color;
use crate::annotate::text::RenderContext;
use crate::ports::Rect;

#[derive(Clone)]
pub struct HighlightAnnotation {
    pub rect: Rect,
    /// Color CON alfa (típico: amarillo a 128).
    pub color: Color,
}

impl HighlightAnnotation {
    /// Relleno: no sobresale de su rect.
    pub(crate) fn caja(&self) -> Rect {
        self.rect
    }

    /// Girado, el relleno deja de poder recorrerse por filas del rect y
    /// pasa a ser el barrido de un cuadrilátero.
    pub(crate) fn render_girado(&self, canvas: &mut Canvas, ctx: &RenderContext, giro: Giro) {
        if giro.es_nulo() {
            return self.render(canvas, ctx);
        }
        if self.rect.is_empty() {
            return;
        }
        let centro = self.rect.centro();
        let q = self.rect.corners().map(|c| giro.aplicar(c, centro));
        shapes::fill_quad_blend(canvas, q, self.color);
    }
}

impl Annotation for HighlightAnnotation {
    fn render(&self, canvas: &mut Canvas, _ctx: &RenderContext) {
        shapes::fill_rect_blend(canvas, self.rect, self.color);
    }
}
