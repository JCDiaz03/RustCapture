//! Rectángulo de contorno (f.22).

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::giro::Giro;
use crate::annotate::shapes;
use crate::annotate::style::Style;
use crate::annotate::text::RenderContext;
use crate::ports::Rect;

#[derive(Clone)]
pub struct RectAnnotation {
    pub rect: Rect,
    pub style: Style,
}

impl RectAnnotation {
    pub(crate) fn caja(&self) -> Rect {
        super::caja_con_trazo(self.rect, self.style.thickness)
    }

    /// Girado deja de ser cuatro líneas ortogonales y pasa a ser cuatro
    /// líneas entre las esquinas rotadas — mismo rasterizador, sin
    /// remuestrear.
    pub(crate) fn render_girado(&self, canvas: &mut Canvas, ctx: &RenderContext, giro: Giro) {
        if giro.es_nulo() {
            return self.render(canvas, ctx);
        }
        if self.rect.is_empty() {
            return;
        }
        let centro = self.rect.centro();
        let q = self.rect.corners().map(|c| giro.aplicar(c, centro));
        for i in 0..4 {
            shapes::draw_line(canvas, q[i], q[(i + 1) % 4], &self.style);
        }
    }
}

impl Annotation for RectAnnotation {
    fn render(&self, canvas: &mut Canvas, _ctx: &RenderContext) {
        shapes::draw_rect_outline(canvas, self.rect, &self.style);
    }
}
