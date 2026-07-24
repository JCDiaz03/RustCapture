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
