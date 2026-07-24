//! Elipse de contorno (f.22), inscrita en su rect.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::shapes;
use crate::annotate::style::Style;
use crate::annotate::text::RenderContext;
use crate::ports::Rect;

pub struct EllipseAnnotation {
    pub rect: Rect,
    pub style: Style,
}

impl Annotation for EllipseAnnotation {
    fn render(&self, canvas: &mut Canvas, _ctx: &RenderContext) {
        shapes::draw_ellipse_outline(canvas, self.rect, &self.style);
    }
}
