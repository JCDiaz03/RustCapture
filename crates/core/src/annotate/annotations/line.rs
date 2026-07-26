//! Línea recta (f.22).

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::shapes;
use crate::annotate::style::Style;
use crate::annotate::text::RenderContext;

#[derive(Clone)]
pub struct LineAnnotation {
    pub from: (i32, i32),
    pub to: (i32, i32),
    pub style: Style,
}

impl Annotation for LineAnnotation {
    fn render(&self, canvas: &mut Canvas, _ctx: &RenderContext) {
        shapes::draw_line(canvas, self.from, self.to, &self.style);
    }
}
