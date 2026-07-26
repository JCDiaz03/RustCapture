//! Lápiz a mano alzada (f.22): polilínea de los puntos del arrastre.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::giro::Giro;
use crate::annotate::shapes;
use crate::annotate::style::Style;
use crate::annotate::text::RenderContext;
use crate::ports::Rect;

#[derive(Clone)]
pub struct PenAnnotation {
    pub points: Vec<(i32, i32)>,
    pub style: Style,
}

impl PenAnnotation {
    pub(crate) fn caja(&self) -> Rect {
        Rect::bounding(&self.points, self.style.thickness.max(1) / 2)
    }

    /// Familia «puntos»: se rota cada punto del trazo.
    pub(crate) fn render_girado(&self, canvas: &mut Canvas, ctx: &RenderContext, giro: Giro) {
        if giro.es_nulo() {
            return self.render(canvas, ctx);
        }
        let centro = self.caja().centro();
        let girados: Vec<(i32, i32)> = self
            .points
            .iter()
            .map(|&p| giro.aplicar(p, centro))
            .collect();
        shapes::draw_polyline(canvas, &girados, &self.style);
    }
}

impl Annotation for PenAnnotation {
    fn render(&self, canvas: &mut Canvas, _ctx: &RenderContext) {
        shapes::draw_polyline(canvas, &self.points, &self.style);
    }
}
