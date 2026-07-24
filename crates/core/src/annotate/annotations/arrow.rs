//! Flecha (f.22): línea + cabeza en V hacia atrás desde la punta.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::shapes;
use crate::annotate::style::Style;
use crate::annotate::text::RenderContext;

pub struct ArrowAnnotation {
    pub from: (i32, i32),
    pub to: (i32, i32),
    pub style: Style,
}

impl Annotation for ArrowAnnotation {
    fn render(&self, canvas: &mut Canvas, _ctx: &RenderContext) {
        shapes::draw_line(canvas, self.from, self.to, &self.style);
        let dx = (self.to.0 - self.from.0) as f64;
        let dy = (self.to.1 - self.from.1) as f64;
        let largo_eje = (dx * dx + dy * dy).sqrt();
        if largo_eje < 1.0 {
            return;
        }
        let angulo = dy.atan2(dx);
        let largo = (self.style.thickness as f64 * 4.0).max(10.0).min(largo_eje);
        // Brazos a ±150° del sentido de la flecha.
        for signo in [-1.0, 1.0] {
            let a = angulo + signo * 150.0_f64.to_radians();
            let px = (self.to.0 as f64 + largo * a.cos()).round() as i32;
            let py = (self.to.1 as f64 + largo * a.sin()).round() as i32;
            shapes::draw_line(canvas, self.to, (px, py), &self.style);
        }
    }
}
