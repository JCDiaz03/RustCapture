//! Flecha (f.22): línea + cabeza en V hacia atrás desde la punta.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::giro::Giro;
use crate::annotate::shapes;
use crate::annotate::style::Style;
use crate::annotate::text::RenderContext;
use crate::ports::Rect;

#[derive(Clone)]
pub struct ArrowAnnotation {
    pub from: (i32, i32),
    pub to: (i32, i32),
    pub style: Style,
}

/// Ángulo de los brazos respecto al sentido de la flecha.
const BRAZO_GRADOS: f64 = 150.0;

impl ArrowAnnotation {
    /// Longitud del eje en px.
    fn largo_eje(&self) -> f64 {
        let dx = f64::from(self.to.0 - self.from.0);
        let dy = f64::from(self.to.1 - self.from.1);
        (dx * dx + dy * dy).sqrt()
    }

    /// Longitud de los brazos de la cabeza. Pública porque `Objeto::bounds`
    /// la necesita para saber cuánto sobresale la cabeza del eje: si esta
    /// fórmula cambia, la caja de selección la sigue sola.
    pub fn head_len(&self) -> f64 {
        (f64::from(self.style.thickness) * 4.0)
            .max(10.0)
            .min(self.largo_eje())
    }
}

impl ArrowAnnotation {
    pub(crate) fn caja(&self) -> Rect {
        // Los brazos van a 150° del eje: sobresalen del eje
        // sin(150°) · largo = largo/2 en perpendicular.
        let cabeza = (self.head_len() / 2.0).ceil().max(0.0) as u32;
        Rect::bounding(
            &[self.from, self.to],
            self.style.thickness.max(1) / 2 + cabeza,
        )
    }

    /// Familia «puntos»: rotando los dos extremos la cabeza se recalcula
    /// sola (su ángulo se deriva del eje).
    pub(crate) fn render_girado(&self, canvas: &mut Canvas, ctx: &RenderContext, giro: Giro) {
        if giro.es_nulo() {
            return self.render(canvas, ctx);
        }
        let centro = self.caja().centro();
        ArrowAnnotation {
            from: giro.aplicar(self.from, centro),
            to: giro.aplicar(self.to, centro),
            style: self.style,
        }
        .render(canvas, ctx);
    }
}

impl Annotation for ArrowAnnotation {
    fn render(&self, canvas: &mut Canvas, _ctx: &RenderContext) {
        shapes::draw_line(canvas, self.from, self.to, &self.style);
        let dx = f64::from(self.to.0 - self.from.0);
        let dy = f64::from(self.to.1 - self.from.1);
        if self.largo_eje() < 1.0 {
            return;
        }
        let angulo = dy.atan2(dx);
        let largo = self.head_len();
        // Brazos a ±150° del sentido de la flecha.
        for signo in [-1.0, 1.0] {
            let a = angulo + signo * BRAZO_GRADOS.to_radians();
            let px = (self.to.0 as f64 + largo * a.cos()).round() as i32;
            let py = (self.to.1 as f64 + largo * a.sin()).round() as i32;
            shapes::draw_line(canvas, self.to, (px, py), &self.style);
        }
    }
}
