//! Resaltador (f.22): relleno semitransparente que no tapa el contenido.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::shapes;
use crate::annotate::style::Color;
use crate::annotate::text::RenderContext;
use crate::ports::Rect;

pub struct HighlightAnnotation {
    pub rect: Rect,
    /// Color CON alfa (típico: amarillo a 128).
    pub color: Color,
}

impl Annotation for HighlightAnnotation {
    fn render(&self, canvas: &mut Canvas, _ctx: &RenderContext) {
        shapes::fill_rect_blend(canvas, self.rect, self.color);
    }
}
