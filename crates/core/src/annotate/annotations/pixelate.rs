//! Pixelado / desenfoque (f.25): censura una región tal y como se ve en
//! su punto del z-order — lee del canvas y lo reescribe.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::censor;
use crate::annotate::giro::Giro;
use crate::annotate::style::CensorMode;
use crate::annotate::text::RenderContext;
use crate::ports::Rect;

#[derive(Clone)]
pub struct PixelateAnnotation {
    pub rect: Rect,
    pub mode: CensorMode,
}

impl PixelateAnnotation {
    /// La censura es un relleno: no sobresale de su rect.
    pub(crate) fn caja(&self) -> Rect {
        self.rect
    }

    pub(crate) fn render_girado(&self, canvas: &mut Canvas, ctx: &RenderContext, giro: Giro) {
        if giro.es_nulo() {
            return self.render(canvas, ctx);
        }
        let (px, desenfocar) = match self.mode {
            CensorMode::Mosaic { block } => (block, false),
            CensorMode::Blur { radius } => (radius, true),
        };
        censor::censurar_girado(canvas, self.rect, px, desenfocar, giro);
    }
}

impl Annotation for PixelateAnnotation {
    fn render(&self, canvas: &mut Canvas, _ctx: &RenderContext) {
        match self.mode {
            CensorMode::Mosaic { block } => censor::mosaico(canvas, self.rect, block),
            CensorMode::Blur { radius } => censor::desenfoque(canvas, self.rect, radius),
        }
    }
}
