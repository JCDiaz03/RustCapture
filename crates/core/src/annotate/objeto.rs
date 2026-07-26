//! Objeto de anotación: el enum que CIERRA la jerarquía del trait
//! `Annotation` (D5).
//!
//! Por qué un enum y no `Box<dyn Annotation>`: un `dyn` solo ofrece lo que
//! declara el trait, y el editor necesita tres cosas que el trait no puede
//! dar sin renunciar al objeto — saber dónde está cada objeto para el
//! hit-test (`bounds`), moverlo (`translate`) y serializarlo para el
//! formato re-editable (f.31, que con `dyn` exigiría `typetag`, es decir
//! una dependencia, contra la prioridad de peso mínimo). Al ser una app
//! cerrada (los plugins están descartados en `ideas.md`), cerrar la lista
//! no cuesta nada: cada tipo sigue en su archivo con su `impl Annotation`
//! y aquí solo se delega con un `match`.

use crate::annotate::annotations::{
    Annotation, ArrowAnnotation, EllipseAnnotation, HighlightAnnotation, LineAnnotation,
    PenAnnotation, PixelateAnnotation, RectAnnotation, StepAnnotation, TextAnnotation,
};
use crate::annotate::canvas::Canvas;
use crate::annotate::text::{RenderContext, text_ink_box};
use crate::ports::Rect;

/// Un objeto del documento. Añadir un tipo = un archivo en `annotations/`
/// + una variante aquí; el compilador señala los `match` que faltan.
#[derive(Clone)]
pub enum Objeto {
    Flecha(ArrowAnnotation),
    Elipse(EllipseAnnotation),
    Resaltador(HighlightAnnotation),
    Linea(LineAnnotation),
    Lapiz(PenAnnotation),
    Pixelado(PixelateAnnotation),
    Rect(RectAnnotation),
    Paso(StepAnnotation),
    Texto(TextAnnotation),
}

impl Objeto {
    /// Delega en la Strategy del tipo concreto (D5 intacto).
    pub fn render(&self, canvas: &mut Canvas, ctx: &RenderContext) {
        match self {
            Objeto::Flecha(a) => a.render(canvas, ctx),
            Objeto::Elipse(a) => a.render(canvas, ctx),
            Objeto::Resaltador(a) => a.render(canvas, ctx),
            Objeto::Linea(a) => a.render(canvas, ctx),
            Objeto::Lapiz(a) => a.render(canvas, ctx),
            Objeto::Pixelado(a) => a.render(canvas, ctx),
            Objeto::Rect(a) => a.render(canvas, ctx),
            Objeto::Paso(a) => a.render(canvas, ctx),
            Objeto::Texto(a) => a.render(canvas, ctx),
        }
    }

    /// Caja que encierra lo que el objeto pinta, para el hit-test de la
    /// herramienta de selección. Necesita el `ctx` porque medir el texto
    /// exige la fuente; sin fuente cargada el texto devuelve un rect vacío
    /// y por tanto no es seleccionable (la GUI siempre carga la fuente).
    pub fn bounds(&self, ctx: &RenderContext) -> Rect {
        match self {
            // Contornos: el trazo se reparte a ambos lados del borde, así
            // que la caja crece medio grosor (lo verifica el test que
            // compara la caja contra los píxeles realmente pintados).
            Objeto::Elipse(a) => caja_con_trazo(a.rect, a.style.thickness),
            Objeto::Rect(a) => caja_con_trazo(a.rect, a.style.thickness),
            // Rellenos: no se salen de su rect.
            Objeto::Resaltador(a) => a.rect,
            Objeto::Pixelado(a) => a.rect,
            // El trazo sobresale del eje geométrico: margen = medio grosor.
            Objeto::Linea(a) => {
                Rect::bounding(&[a.from, a.to], a.style.thickness.max(1) / 2)
            }
            Objeto::Flecha(a) => {
                // Los brazos van a 150° del eje: sobresalen del eje
                // sin(150°) · largo = largo/2 en perpendicular.
                let cabeza = (a.head_len() / 2.0).ceil().max(0.0) as u32;
                Rect::bounding(&[a.from, a.to], a.style.thickness.max(1) / 2 + cabeza)
            }
            Objeto::Lapiz(a) => Rect::bounding(&a.points, a.style.thickness.max(1) / 2),
            Objeto::Paso(a) => {
                let r = a.radius() as i32;
                Rect::bounding(&[(a.center.0 - r, a.center.1 - r), (a.center.0 + r, a.center.1 + r)], 0)
            }
            Objeto::Texto(a) => match text_ink_box(&a.text, a.style, ctx) {
                Some((dx, dy, w, h)) => Rect::new(a.pos.0 + dx, a.pos.1 + dy, w, h),
                None => Rect::new(0, 0, 0, 0),
            },
        }
    }

    /// Desplaza el objeto `delta` píxeles. Es la operación que necesita
    /// `Command::Move`: aplicar y revertir es el mismo código con el
    /// delta negado, así que mover queda deshacible sin caso especial.
    pub fn translate(&mut self, delta: (i32, i32)) {
        let mover = |p: &mut (i32, i32)| {
            p.0 = p.0.saturating_add(delta.0);
            p.1 = p.1.saturating_add(delta.1);
        };
        match self {
            Objeto::Elipse(a) => a.rect = a.rect.translated(delta),
            Objeto::Rect(a) => a.rect = a.rect.translated(delta),
            Objeto::Resaltador(a) => a.rect = a.rect.translated(delta),
            Objeto::Pixelado(a) => a.rect = a.rect.translated(delta),
            Objeto::Linea(a) => {
                mover(&mut a.from);
                mover(&mut a.to);
            }
            Objeto::Flecha(a) => {
                mover(&mut a.from);
                mover(&mut a.to);
            }
            Objeto::Lapiz(a) => a.points.iter_mut().for_each(mover),
            Objeto::Paso(a) => mover(&mut a.center),
            Objeto::Texto(a) => mover(&mut a.pos),
        }
    }
}

/// Caja de una forma cuyo CONTORNO se estampa con `grosor`: el trazo
/// sobresale medio grosor del borde geométrico por los cuatro lados.
fn caja_con_trazo(rect: Rect, grosor: u32) -> Rect {
    if rect.is_empty() {
        return rect;
    }
    let esquinas = [
        (rect.x, rect.y),
        (
            rect.x + rect.width as i32 - 1,
            rect.y + rect.height as i32 - 1,
        ),
    ];
    Rect::bounding(&esquinas, grosor.max(1) / 2)
}

/// Conversiones para que los llamadores construyan sin nombrar la variante:
/// `Command::add(RectAnnotation { .. }.into())`.
macro_rules! desde {
    ($($tipo:ty => $variante:ident),* $(,)?) => {
        $(impl From<$tipo> for Objeto {
            fn from(a: $tipo) -> Self {
                Objeto::$variante(a)
            }
        })*
    };
}

desde! {
    ArrowAnnotation => Flecha,
    EllipseAnnotation => Elipse,
    HighlightAnnotation => Resaltador,
    LineAnnotation => Linea,
    PenAnnotation => Lapiz,
    PixelateAnnotation => Pixelado,
    RectAnnotation => Rect,
    StepAnnotation => Paso,
    TextAnnotation => Texto,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotate::style::{CensorMode, Color, Style, TextStyle};

    const ESTILO: Style = Style {
        color: Color::rgb(255, 0, 0),
        thickness: 4,
    };

    fn ctx() -> RenderContext {
        let normal = std::fs::read("C:/Windows/Fonts/segoeui.ttf").expect("fuente del sistema");
        let bold = std::fs::read("C:/Windows/Fonts/segoeuib.ttf").expect("fuente del sistema");
        RenderContext::new(&normal, &bold).unwrap()
    }

    /// Un objeto de cada variante, para recorrerlas todas en los tests.
    fn todos() -> Vec<Objeto> {
        vec![
            ArrowAnnotation { from: (10, 10), to: (40, 30), style: ESTILO }.into(),
            EllipseAnnotation { rect: Rect::new(5, 6, 20, 10), style: ESTILO }.into(),
            HighlightAnnotation { rect: Rect::new(1, 2, 30, 8), color: Color::rgba(255, 255, 0, 128) }.into(),
            LineAnnotation { from: (3, 4), to: (50, 40), style: ESTILO }.into(),
            PenAnnotation { points: vec![(2, 2), (9, 20), (30, 5)], style: ESTILO }.into(),
            PixelateAnnotation { rect: Rect::new(7, 8, 12, 12), mode: CensorMode::Mosaic { block: 4 } }.into(),
            RectAnnotation { rect: Rect::new(0, 0, 15, 15), style: ESTILO }.into(),
            StepAnnotation { center: (25, 25), number: 7, color: Color::rgb(1, 2, 3), font_size: 20.0 }.into(),
            TextAnnotation {
                pos: (12, 14),
                text: "Hola".to_string(),
                style: TextStyle { color: Color::rgb(9, 9, 9), size: 18.0, bold: false },
            }
            .into(),
        ]
    }

    #[test]
    fn todas_las_variantes_tienen_caja_no_vacia() {
        let ctx = ctx();
        for (i, o) in todos().iter().enumerate() {
            assert!(!o.bounds(&ctx).is_empty(), "variante {i} sin caja");
        }
    }

    #[test]
    fn la_caja_contiene_los_extremos_de_la_geometria() {
        let ctx = ctx();
        // Línea de (3,4) a (50,40) con grosor 4 → margen 2.
        let linea: Objeto = LineAnnotation { from: (3, 4), to: (50, 40), style: ESTILO }.into();
        let caja = linea.bounds(&ctx);
        assert!(caja.contains_point((3, 4)) && caja.contains_point((50, 40)));
        assert!(caja.contains_point((1, 2)), "el margen del grosor no está");
        assert!(!caja.contains_point((0, 4)));
        // El paso encierra su disco completo.
        let paso = StepAnnotation { center: (25, 25), number: 7, color: Color::rgb(1, 2, 3), font_size: 20.0 };
        let radio = paso.radius() as i32;
        let caja = Objeto::from(paso).bounds(&ctx);
        assert!(caja.contains_point((25 - radio, 25)) && caja.contains_point((25, 25 + radio)));
    }

    /// La caja tiene que cubrir TODO lo pintado: se comprueba contra el
    /// render real, no contra la fórmula. Es lo que impide que la caja se
    /// quede corta con la cabeza de la flecha o el disco del paso.
    #[test]
    fn la_caja_cubre_todos_los_pixeles_que_pinta_el_objeto() {
        use crate::ports::Frame;
        let ctx = ctx();
        let casos: Vec<Objeto> = vec![
            // Flecha en varias direcciones: la cabeza sobresale del eje.
            ArrowAnnotation { from: (60, 60), to: (110, 60), style: ESTILO }.into(),
            ArrowAnnotation { from: (60, 60), to: (60, 110), style: ESTILO }.into(),
            ArrowAnnotation { from: (110, 110), to: (60, 60), style: ESTILO }.into(),
            LineAnnotation { from: (30, 40), to: (120, 90), style: ESTILO }.into(),
            PenAnnotation { points: vec![(40, 40), (90, 70), (60, 110)], style: ESTILO }.into(),
            StepAnnotation { center: (90, 90), number: 12, color: Color::rgb(255, 0, 0), font_size: 24.0 }.into(),
            EllipseAnnotation { rect: Rect::new(30, 30, 60, 40), style: ESTILO }.into(),
            RectAnnotation { rect: Rect::new(30, 30, 60, 40), style: ESTILO }.into(),
            TextAnnotation {
                pos: (40, 40),
                text: "Ag".to_string(),
                style: TextStyle { color: Color::rgb(255, 0, 0), size: 28.0, bold: true },
            }
            .into(),
        ];
        for (i, o) in casos.iter().enumerate() {
            let mut frame = Frame::filled(200, 200, [0, 0, 0, 255]);
            o.render(&mut Canvas::new(&mut frame), &ctx);
            let caja = o.bounds(&ctx);
            for y in 0..200u32 {
                for x in 0..200u32 {
                    let pintado = frame.pixel(x, y).is_some_and(|[r, g, b, _]| {
                        r > 0 || g > 0 || b > 0
                    });
                    if pintado {
                        assert!(
                            caja.contains_point((x as i32, y as i32)),
                            "caso {i}: pinta en ({x}, {y}) fuera de la caja {caja:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn translate_mueve_la_caja_y_conserva_su_tamano() {
        let ctx = ctx();
        for (i, mut o) in todos().into_iter().enumerate() {
            let antes = o.bounds(&ctx);
            o.translate((7, -3));
            let despues = o.bounds(&ctx);
            assert_eq!(
                (despues.x, despues.y),
                (antes.x + 7, antes.y - 3),
                "variante {i}: la caja no se movió igual"
            );
            assert_eq!(
                (despues.width, despues.height),
                (antes.width, antes.height),
                "variante {i}: cambió de tamaño al moverse"
            );
        }
    }

    #[test]
    fn translate_es_reversible_con_el_delta_negado() {
        let ctx = ctx();
        for mut o in todos() {
            let original = o.bounds(&ctx);
            o.translate((25, 40));
            o.translate((-25, -40));
            assert_eq!(o.bounds(&ctx), original);
        }
    }

    #[test]
    fn mover_un_objeto_mueve_lo_que_pinta() {
        use crate::ports::Frame;
        // Un rectángulo pintado y el mismo rectángulo desplazado deben dar
        // exactamente el mismo frame que dibujarlo ya desplazado.
        let ctx = RenderContext::sin_fuente();
        let pintar = |o: &Objeto| {
            let mut f = Frame::filled(60, 60, [0, 0, 0, 255]);
            o.render(&mut Canvas::new(&mut f), &ctx);
            f
        };
        let mut movido: Objeto = RectAnnotation { rect: Rect::new(5, 5, 10, 10), style: ESTILO }.into();
        movido.translate((20, 12));
        let directo: Objeto = RectAnnotation { rect: Rect::new(25, 17, 10, 10), style: ESTILO }.into();
        assert_eq!(pintar(&movido), pintar(&directo));
    }

    #[test]
    fn el_texto_sin_fuente_no_tiene_caja_y_no_es_seleccionable() {
        let o: Objeto = TextAnnotation {
            pos: (12, 14),
            text: "Hola".to_string(),
            style: TextStyle { color: Color::rgb(9, 9, 9), size: 18.0, bold: false },
        }
        .into();
        assert!(o.bounds(&RenderContext::sin_fuente()).is_empty());
    }
}
