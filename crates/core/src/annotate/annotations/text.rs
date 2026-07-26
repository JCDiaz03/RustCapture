//! Texto (f.22): render con la fuente inyectada en el RenderContext.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::style::TextStyle;
use crate::annotate::text::{RenderContext, draw_text};

#[derive(Clone)]
pub struct TextAnnotation {
    pub pos: (i32, i32),
    pub text: String,
    pub style: TextStyle,
}

impl Annotation for TextAnnotation {
    fn render(&self, canvas: &mut Canvas, ctx: &RenderContext) {
        draw_text(canvas, self.pos, &self.text, self.style, ctx);
    }
}
