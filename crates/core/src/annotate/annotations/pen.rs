//! Lápiz a mano alzada (f.22): polilínea de los puntos del arrastre.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::shapes;
use crate::annotate::style::Style;
use crate::annotate::text::RenderContext;

#[derive(Clone)]
pub struct PenAnnotation {
    pub points: Vec<(i32, i32)>,
    pub style: Style,
}

impl Annotation for PenAnnotation {
    fn render(&self, canvas: &mut Canvas, _ctx: &RenderContext) {
        shapes::draw_polyline(canvas, &self.points, &self.style);
    }
}
