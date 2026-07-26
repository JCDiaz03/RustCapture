//! Línea recta (f.22).

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::giro::Giro;
use crate::annotate::shapes;
use crate::annotate::style::Style;
use crate::annotate::text::RenderContext;
use crate::ports::Rect;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct LineAnnotation {
    pub from: (i32, i32),
    pub to: (i32, i32),
    pub style: Style,
}

impl LineAnnotation {
    /// Caja sin girar. Es el MISMO punto de verdad que usa `render_girado`
    /// para el centro de giro: si divergieran, la línea se desplazaría al
    /// rotarla respecto a su recuadro de selección.
    pub(crate) fn caja(&self) -> Rect {
        Rect::bounding(&[self.from, self.to], self.style.thickness.max(1) / 2)
    }

    /// Familia «puntos»: se rotan los extremos y se reutiliza el mismo
    /// rasterizado, sin remuestrear — la calidad es la de una línea sin
    /// girar.
    pub(crate) fn render_girado(&self, canvas: &mut Canvas, ctx: &RenderContext, giro: Giro) {
        if giro.es_nulo() {
            return self.render(canvas, ctx);
        }
        let centro = self.caja().centro();
        shapes::draw_line(
            canvas,
            giro.aplicar(self.from, centro),
            giro.aplicar(self.to, centro),
            &self.style,
        );
    }
}

impl Annotation for LineAnnotation {
    fn render(&self, canvas: &mut Canvas, _ctx: &RenderContext) {
        shapes::draw_line(canvas, self.from, self.to, &self.style);
    }
}
