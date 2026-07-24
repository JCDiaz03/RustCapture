//! Anotaciones (D5): una Strategy por tipo, un archivo por tipo.

mod arrow;
mod ellipse;
mod highlight;
mod line;
mod pen;
mod rect;
mod text;

pub use arrow::ArrowAnnotation;
pub use ellipse::EllipseAnnotation;
pub use highlight::HighlightAnnotation;
pub use line::LineAnnotation;
pub use pen::PenAnnotation;
pub use rect::RectAnnotation;
pub use text::TextAnnotation;

use crate::annotate::canvas::Canvas;
use crate::annotate::text::RenderContext;

/// Strategy de anotación (D5): renderiza sobre el canvas; al motor le da
/// igual si debajo hay una captura o un fotograma de vídeo.
pub trait Annotation {
    fn render(&self, canvas: &mut Canvas, ctx: &RenderContext);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotate::style::{Color, Style};
    use crate::ports::{Frame, Rect};

    const ROJO: Color = Color::rgb(255, 0, 0);
    const ESTILO: Style = Style {
        color: ROJO,
        thickness: 1,
    };

    fn render(a: &dyn Annotation) -> Frame {
        let mut frame = Frame::filled(30, 30, [0, 0, 0, 255]);
        a.render(&mut Canvas::new(&mut frame), &RenderContext::sin_fuente());
        frame
    }

    fn es_rojo(frame: &Frame, x: u32, y: u32) -> bool {
        frame.pixel(x, y) == Some([255, 0, 0, 255])
    }

    #[test]
    fn rect_y_elipse_dibujan_contornos() {
        let frame = render(&RectAnnotation {
            rect: Rect::new(2, 2, 10, 10),
            style: ESTILO,
        });
        assert!(es_rojo(&frame, 2, 2) && es_rojo(&frame, 11, 11) && !es_rojo(&frame, 6, 6));
        let frame = render(&EllipseAnnotation {
            rect: Rect::new(2, 2, 20, 10),
            style: ESTILO,
        });
        assert!(es_rojo(&frame, 12, 2) && !es_rojo(&frame, 12, 7));
    }

    #[test]
    fn linea_y_lapiz_trazan_sus_puntos() {
        let frame = render(&LineAnnotation {
            from: (0, 0),
            to: (10, 10),
            style: ESTILO,
        });
        assert!(es_rojo(&frame, 5, 5));
        let frame = render(&PenAnnotation {
            points: vec![(1, 1), (8, 1), (8, 8)],
            style: ESTILO,
        });
        assert!(es_rojo(&frame, 4, 1) && es_rojo(&frame, 8, 4));
    }

    #[test]
    fn la_flecha_tiene_cabeza_fuera_del_eje() {
        // Flecha horizontal → la cabeza pone píxeles por encima y por
        // debajo del eje y=10 cerca de la punta.
        let frame = render(&ArrowAnnotation {
            from: (2, 10),
            to: (25, 10),
            style: ESTILO,
        });
        assert!(es_rojo(&frame, 10, 10)); // eje
        let cabeza_arriba = (18..25).any(|x| (5..10).any(|y| es_rojo(&frame, x, y)));
        let cabeza_abajo = (18..25).any(|x| (11..16).any(|y| es_rojo(&frame, x, y)));
        assert!(cabeza_arriba && cabeza_abajo);
    }

    #[test]
    fn el_resaltador_mezcla_sin_tapar() {
        let frame = render(&HighlightAnnotation {
            rect: Rect::new(5, 5, 8, 8),
            color: Color::rgba(255, 255, 0, 128),
        });
        let [r, g, b, _] = frame.pixel(8, 8).unwrap();
        assert!(r > 100 && g > 100 && b == 0); // amarillo a medias
    }

    fn ctx_con_fuente() -> RenderContext {
        let normal = std::fs::read("C:/Windows/Fonts/segoeui.ttf").expect("fuente del sistema");
        let bold = std::fs::read("C:/Windows/Fonts/segoeuib.ttf").expect("fuente del sistema");
        RenderContext::new(&normal, &bold).unwrap()
    }

    #[test]
    fn el_texto_ocupa_su_caja_y_sin_fuente_es_noop() {
        let anotacion = TextAnnotation {
            pos: (5, 5),
            text: "Hola".to_string(),
            style: crate::annotate::style::TextStyle {
                color: ROJO,
                size: 20.0,
                bold: false,
            },
        };
        // Con fuente: aparecen píxeles rojos en la zona del texto.
        let mut frame = Frame::filled(100, 40, [0, 0, 0, 255]);
        anotacion.render(&mut Canvas::new(&mut frame), &ctx_con_fuente());
        let pintados = (5..60)
            .flat_map(|x| (5..35).map(move |y| (x, y)))
            .filter(|&(x, y)| frame.pixel(x, y).is_some_and(|[r, _, _, _]| r > 100))
            .count();
        assert!(pintados > 20, "solo {pintados} píxeles de texto");
        // Sin fuente: no-op.
        let mut vacio = Frame::filled(100, 40, [0, 0, 0, 255]);
        anotacion.render(&mut Canvas::new(&mut vacio), &RenderContext::sin_fuente());
        assert_eq!(vacio, Frame::filled(100, 40, [0, 0, 0, 255]));
    }

    #[test]
    fn el_texto_multilinea_baja_de_linea() {
        let anotacion = TextAnnotation {
            pos: (2, 2),
            text: "A\nA".to_string(),
            style: crate::annotate::style::TextStyle {
                color: ROJO,
                size: 16.0,
                bold: false,
            },
        };
        let mut frame = Frame::filled(60, 60, [0, 0, 0, 255]);
        anotacion.render(&mut Canvas::new(&mut frame), &ctx_con_fuente());
        let fila_ocupada = |y0: u32, y1: u32| {
            (0..60).any(|x| (y0..y1).any(|y| frame.pixel(x, y).is_some_and(|[r, ..]| r > 100)))
        };
        assert!(fila_ocupada(2, 20) && fila_ocupada(21, 45));
    }
}
