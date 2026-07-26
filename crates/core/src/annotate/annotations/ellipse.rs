//! Elipse de contorno (f.22), inscrita en su rect.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::giro::Giro;
use crate::annotate::shapes;
use crate::annotate::style::Style;
use crate::annotate::text::RenderContext;
use crate::ports::Rect;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct EllipseAnnotation {
    pub rect: Rect,
    pub style: Style,
}

impl EllipseAnnotation {
    pub(crate) fn caja(&self) -> Rect {
        super::caja_con_trazo(self.rect, self.style.thickness)
    }

    /// El contorno ya se obtiene por muestreo paramétrico: girar es rotar
    /// cada muestra antes de estampar el disco. Sin remuestreo.
    pub(crate) fn render_girado(&self, canvas: &mut Canvas, ctx: &RenderContext, giro: Giro) {
        if giro.es_nulo() {
            return self.render(canvas, ctx);
        }
        shapes::draw_ellipse_outline_girada(canvas, self.rect, &self.style, giro);
    }
}

impl Annotation for EllipseAnnotation {
    fn render(&self, canvas: &mut Canvas, _ctx: &RenderContext) {
        shapes::draw_ellipse_outline(canvas, self.rect, &self.style);
    }
}
