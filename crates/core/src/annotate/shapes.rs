//! Rasterización pura de formas (sin antialiasing, MVP). Interna: las
//! anotaciones son la API pública.
//!
//! PENDIENTE(corrección futura): línea y elipse estampan discos con
//! solape, y cada estampado mezcla con alfa — con colores alfa < 255 los
//! píxeles solapados se mezclan varias veces (manchas oscuras). Hoy no
//! afecta (solo el resaltador usa alfa y es un fill sin solape), pero al
//! añadir opacidad por herramienta habrá que acumular una máscara de
//! cobertura por trazo y mezclar UNA vez.

use crate::annotate::canvas::Canvas;
use crate::annotate::style::{Color, Style};
use crate::ports::Rect;

/// Disco de diámetro `thickness` (mínimo 1) centrado en (cx, cy).
pub(crate) fn stamp_disc(canvas: &mut Canvas, cx: i32, cy: i32, thickness: u32, color: Color) {
    if thickness <= 1 {
        canvas.blend_pixel(cx, cy, color);
        return;
    }
    let radio = thickness as i32 / 2;
    for dy in -radio..=radio {
        for dx in -radio..=radio {
            if dx * dx + dy * dy <= radio * radio {
                canvas.blend_pixel(cx + dx, cy + dy, color);
            }
        }
    }
}

/// Bresenham con estampado de disco por punto.
pub(crate) fn draw_line(canvas: &mut Canvas, a: (i32, i32), b: (i32, i32), style: &Style) {
    let (mut x, mut y) = a;
    let dx = (b.0 - a.0).abs();
    let dy = -(b.1 - a.1).abs();
    let sx = if a.0 < b.0 { 1 } else { -1 };
    let sy = if a.1 < b.1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        stamp_disc(canvas, x, y, style.thickness, style.color);
        if x == b.0 && y == b.1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

pub(crate) fn draw_polyline(canvas: &mut Canvas, points: &[(i32, i32)], style: &Style) {
    for par in points.windows(2) {
        draw_line(canvas, par[0], par[1], style);
    }
}

pub(crate) fn draw_rect_outline(canvas: &mut Canvas, rect: Rect, style: &Style) {
    if rect.is_empty() {
        return;
    }
    let x2 = rect.x + rect.width as i32 - 1;
    let y2 = rect.y + rect.height as i32 - 1;
    draw_line(canvas, (rect.x, rect.y), (x2, rect.y), style);
    draw_line(canvas, (x2, rect.y), (x2, y2), style);
    draw_line(canvas, (x2, y2), (rect.x, y2), style);
    draw_line(canvas, (rect.x, y2), (rect.x, rect.y), style);
}

/// Contorno por muestreo paramétrico (pasos ∝ perímetro del rect).
pub(crate) fn draw_ellipse_outline(canvas: &mut Canvas, rect: Rect, style: &Style) {
    if rect.is_empty() {
        return;
    }
    let rx = (rect.width as f64 - 1.0) / 2.0;
    let ry = (rect.height as f64 - 1.0) / 2.0;
    let cx = rect.x as f64 + rx;
    let cy = rect.y as f64 + ry;
    let pasos = (4.0 * (rect.width + rect.height) as f64).max(16.0) as u32;
    for i in 0..pasos {
        let t = i as f64 / pasos as f64 * std::f64::consts::TAU;
        let x = (cx + rx * t.cos()).round() as i32;
        let y = (cy + ry * t.sin()).round() as i32;
        stamp_disc(canvas, x, y, style.thickness, style.color);
    }
}

// PENDIENTE(rendimiento): pasa por blend_pixel píxel a píxel (bounds
// check + índice por píxel). Si los resaltadores grandes se notan
// lentos, clampear el rect una vez y recorrer por filas (~2-3×).
pub(crate) fn fill_rect_blend(canvas: &mut Canvas, rect: Rect, color: Color) {
    for y in rect.y..rect.y + rect.height as i32 {
        for x in rect.x..rect.x + rect.width as i32 {
            canvas.blend_pixel(x, y, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::Frame;

    const NEGRO: [u8; 4] = [0, 0, 0, 255];
    const ROJO: Color = Color::rgb(255, 0, 0);

    fn lienzo() -> Frame {
        Frame::filled(20, 20, NEGRO)
    }

    fn es_rojo(frame: &Frame, x: u32, y: u32) -> bool {
        frame.pixel(x, y) == Some([255, 0, 0, 255])
    }

    #[test]
    fn linea_horizontal_grosor_uno_pinta_su_fila() {
        let mut frame = lienzo();
        draw_line(
            &mut Canvas::new(&mut frame),
            (2, 5),
            (8, 5),
            &Style {
                color: ROJO,
                thickness: 1,
            },
        );
        for x in 2..=8 {
            assert!(es_rojo(&frame, x, 5), "falta ({x},5)");
        }
        assert!(!es_rojo(&frame, 1, 5));
        assert!(!es_rojo(&frame, 5, 4));
    }

    #[test]
    fn linea_gruesa_cubre_vecinos() {
        let mut frame = lienzo();
        draw_line(
            &mut Canvas::new(&mut frame),
            (2, 10),
            (12, 10),
            &Style {
                color: ROJO,
                thickness: 3,
            },
        );
        assert!(es_rojo(&frame, 7, 9) && es_rojo(&frame, 7, 10) && es_rojo(&frame, 7, 11));
    }

    #[test]
    fn rect_outline_pinta_bordes_y_no_el_interior() {
        let mut frame = lienzo();
        draw_rect_outline(
            &mut Canvas::new(&mut frame),
            Rect::new(3, 3, 10, 8),
            &Style {
                color: ROJO,
                thickness: 1,
            },
        );
        assert!(es_rojo(&frame, 3, 3) && es_rojo(&frame, 12, 10));
        assert!(es_rojo(&frame, 7, 3) && es_rojo(&frame, 3, 7));
        assert!(!es_rojo(&frame, 7, 7)); // interior limpio
    }

    #[test]
    fn elipse_toca_los_cuatro_extremos_y_no_el_centro() {
        let mut frame = lienzo();
        draw_ellipse_outline(
            &mut Canvas::new(&mut frame),
            Rect::new(2, 4, 16, 10),
            &Style {
                color: ROJO,
                thickness: 1,
            },
        );
        assert!(es_rojo(&frame, 10, 4)); // arriba
        assert!(es_rojo(&frame, 10, 13)); // abajo
        assert!(es_rojo(&frame, 2, 9)); // izquierda
        assert!(es_rojo(&frame, 17, 9)); // derecha
        assert!(!es_rojo(&frame, 10, 9)); // centro limpio
    }

    #[test]
    fn fill_blend_mezcla_el_interior_completo() {
        let mut frame = lienzo();
        fill_rect_blend(
            &mut Canvas::new(&mut frame),
            Rect::new(5, 5, 4, 4),
            Color::rgba(255, 255, 0, 128),
        );
        let [r, g, b, _] = frame.pixel(6, 6).unwrap();
        assert!((127..=129).contains(&r) && (127..=129).contains(&g) && b == 0);
        assert_eq!(frame.pixel(4, 4), Some(NEGRO));
    }

    #[test]
    fn polyline_une_todos_los_tramos() {
        let mut frame = lienzo();
        draw_polyline(
            &mut Canvas::new(&mut frame),
            &[(2, 2), (10, 2), (10, 10)],
            &Style {
                color: ROJO,
                thickness: 1,
            },
        );
        assert!(es_rojo(&frame, 6, 2) && es_rojo(&frame, 10, 6));
    }
}
