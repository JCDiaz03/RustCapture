//! Anotaciones (D5): una Strategy por tipo, un archivo por tipo.

mod arrow;
mod ellipse;
mod highlight;
mod line;
mod pen;
mod pixelate;
mod rect;
mod step;
mod text;

pub use arrow::ArrowAnnotation;
pub use ellipse::EllipseAnnotation;
pub use highlight::HighlightAnnotation;
pub use line::LineAnnotation;
pub use pen::PenAnnotation;
pub use pixelate::PixelateAnnotation;
pub use rect::RectAnnotation;
pub use step::StepAnnotation;
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
    use crate::annotate::style::{CensorMode, Color, Style};
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

    #[test]
    fn el_pixelado_censura_su_rect_en_los_dos_modos() {
        // Frame con un cuadrado blanco de 8×8 dentro de fondo negro.
        let mut frame = Frame::filled(30, 30, [0, 0, 0, 255]);
        for y in 4..12u32 {
            for x in 4..12u32 {
                let i = (y as usize * 30 + x as usize) * 4;
                frame.pixels[i..i + 3].copy_from_slice(&[255, 255, 255]);
            }
        }
        let original = frame.clone();

        // Mosaico con bloque 8 sobre el rect 0..16: el cuadrado blanco se
        // diluye en la media de su celda y deja de ser blanco puro.
        let mut mosaico = original.clone();
        PixelateAnnotation {
            rect: Rect::new(0, 0, 16, 16),
            mode: CensorMode::Mosaic { block: 8 },
        }
        .render(
            &mut Canvas::new(&mut mosaico),
            &RenderContext::sin_fuente(),
        );
        assert_ne!(mosaico.pixel(6, 6), Some([255, 255, 255, 255]));
        // Fuera del rect no se toca nada.
        assert_eq!(mosaico.pixel(20, 20), Some([0, 0, 0, 255]));

        // Desenfoque: el borde del cuadrado deja de ser un salto seco.
        let mut borroso = original.clone();
        PixelateAnnotation {
            rect: Rect::new(0, 0, 16, 16),
            mode: CensorMode::Blur { radius: 3 },
        }
        .render(
            &mut Canvas::new(&mut borroso),
            &RenderContext::sin_fuente(),
        );
        let [r, ..] = borroso.pixel(3, 8).unwrap();
        assert!(r > 0 && r < 255, "el borde no se difuminó (r = {r})");
        assert_eq!(borroso.pixel(20, 20), Some([0, 0, 0, 255]));
    }

    #[test]
    fn el_pixelado_tapa_las_anotaciones_de_debajo() {
        // El z-order manda: una línea pintada ANTES queda censurada.
        let mut frame = Frame::filled(30, 30, [0, 0, 0, 255]);
        let mut canvas = Canvas::new(&mut frame);
        let ctx = RenderContext::sin_fuente();
        LineAnnotation {
            from: (2, 8),
            to: (14, 8),
            style: ESTILO,
        }
        .render(&mut canvas, &ctx);
        assert!(es_rojo(&frame, 8, 8));
        PixelateAnnotation {
            rect: Rect::new(0, 0, 16, 16),
            mode: CensorMode::Mosaic { block: 16 },
        }
        .render(&mut Canvas::new(&mut frame), &ctx);
        assert!(!es_rojo(&frame, 8, 8), "la línea sobrevivió a la censura");
    }

    fn ctx_con_fuente() -> RenderContext {
        let normal = std::fs::read("C:/Windows/Fonts/segoeui.ttf").expect("fuente del sistema");
        let bold = std::fs::read("C:/Windows/Fonts/segoeuib.ttf").expect("fuente del sistema");
        RenderContext::new(&normal, &bold).unwrap()
    }

    #[test]
    fn el_radio_del_paso_crece_con_los_digitos() {
        let paso = |number| StepAnnotation {
            center: (0, 0),
            number,
            color: ROJO,
            font_size: 20.0,
        };
        assert_eq!(paso(1).radius(), paso(9).radius());
        assert!(paso(10).radius() > paso(9).radius());
        assert!(paso(100).radius() > paso(10).radius());
        // Una fuente diminuta no da radio 0 (el disco sigue viéndose).
        assert!(
            StepAnnotation {
                center: (0, 0),
                number: 1,
                color: ROJO,
                font_size: 1.0,
            }
            .radius()
                >= 2
        );
    }

    #[test]
    fn el_paso_pinta_disco_con_el_numero_centrado_y_en_contraste() {
        let mut frame = Frame::filled(60, 60, [0, 0, 0, 255]);
        let paso = StepAnnotation {
            center: (30, 30),
            number: 3,
            color: ROJO,
            font_size: 24.0,
        };
        paso.render(&mut Canvas::new(&mut frame), &ctx_con_fuente());
        let radio = paso.radius() as i32;

        // El disco llega casi hasta su radio en los dos ejes. Se muestrea
        // LEJOS del centro a propósito: la tinta del número ocupa el
        // centro y allí el píxel no es rojo puro.
        assert!(es_rojo(&frame, 30, (30 - radio + 2) as u32), "arriba");
        assert!(es_rojo(&frame, (30 - radio + 2) as u32, 30), "izquierda");
        // Fuera del disco, intacto.
        assert_eq!(frame.pixel(30, (30 - radio - 3) as u32), Some([0, 0, 0, 255]));

        // El número va en blanco (contraste del rojo) y dentro del disco.
        let blancos: Vec<(u32, u32)> = (0..60)
            .flat_map(|x| (0..60).map(move |y| (x, y)))
            .filter(|&(x, y)| {
                frame
                    .pixel(x, y)
                    .is_some_and(|[r, g, b, _]| r > 200 && g > 200 && b > 200)
            })
            .collect();
        assert!(!blancos.is_empty(), "el número no se pintó");
        // Centrado: el centroide de la tinta cae a ≤2 px del centro.
        let n = blancos.len() as i32;
        let cx = blancos.iter().map(|p| p.0 as i32).sum::<i32>() / n;
        let cy = blancos.iter().map(|p| p.1 as i32).sum::<i32>() / n;
        assert!(
            (cx - 30).abs() <= 2 && (cy - 30).abs() <= 2,
            "centroide ({cx}, {cy})"
        );
        // Y toda la tinta queda dentro del disco.
        for (x, y) in blancos {
            let (dx, dy) = (x as i32 - 30, y as i32 - 30);
            assert!(
                dx * dx + dy * dy <= radio * radio,
                "número fuera en ({x}, {y})"
            );
        }
    }

    #[test]
    fn el_paso_sin_fuente_pinta_solo_el_disco() {
        let mut frame = Frame::filled(40, 40, [0, 0, 0, 255]);
        StepAnnotation {
            center: (20, 20),
            number: 5,
            color: ROJO,
            font_size: 20.0,
        }
        .render(&mut Canvas::new(&mut frame), &RenderContext::sin_fuente());
        assert!(es_rojo(&frame, 20, 20));
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
