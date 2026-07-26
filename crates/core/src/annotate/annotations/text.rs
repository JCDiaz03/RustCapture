//! Texto (f.22): render con la fuente inyectada en el RenderContext.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::giro::Giro;
use crate::annotate::style::TextStyle;
use crate::annotate::text::{RenderContext, draw_text, draw_text_rotado, text_ink_box};
use crate::ports::Rect;

#[derive(Clone)]
pub struct TextAnnotation {
    pub pos: (i32, i32),
    pub text: String,
    pub style: TextStyle,
}

impl TextAnnotation {
    /// Caja de la tinta del texto; vacía sin fuente cargada (y entonces el
    /// texto no es seleccionable, coherente con que tampoco se pinta).
    pub(crate) fn caja(&self, ctx: &RenderContext) -> Rect {
        match text_ink_box(&self.text, self.style, ctx) {
            Some((dx, dy, w, h)) => Rect::new(self.pos.0 + dx, self.pos.1 + dy, w, h),
            None => Rect::new(0, 0, 0, 0),
        }
    }

    pub(crate) fn render_girado(&self, canvas: &mut Canvas, ctx: &RenderContext, giro: Giro) {
        if giro.es_nulo() {
            return self.render(canvas, ctx);
        }
        let centro = self.caja(ctx).centro();
        draw_text_rotado(canvas, self.pos, &self.text, self.style, ctx, giro, centro);
    }
}

impl Annotation for TextAnnotation {
    fn render(&self, canvas: &mut Canvas, ctx: &RenderContext) {
        draw_text(canvas, self.pos, &self.text, self.style, ctx);
    }
}
