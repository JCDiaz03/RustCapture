//! Pixelado / desenfoque (f.25): censura una región tal y como se ve en
//! su punto del z-order — lee del canvas y lo reescribe.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::censor;
use crate::annotate::style::CensorMode;
use crate::annotate::text::RenderContext;
use crate::ports::Rect;

#[derive(Clone)]
pub struct PixelateAnnotation {
    pub rect: Rect,
    pub mode: CensorMode,
}

impl Annotation for PixelateAnnotation {
    fn render(&self, canvas: &mut Canvas, _ctx: &RenderContext) {
        match self.mode {
            CensorMode::Mosaic { block } => censor::mosaico(canvas, self.rect, block),
            CensorMode::Blur { radius } => censor::desenfoque(canvas, self.rect, radius),
        }
    }
}
