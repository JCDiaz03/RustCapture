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

/// Disco relleno con antialiasing: cada píxel se mezcla UNA vez con el
/// alfa de su cobertura (supermuestreo 4×4 solo en la corona del borde),
/// así el contorno sale suave y sin las manchas del estampado solapado
/// que describe la cabecera de este módulo.
pub(crate) fn fill_disc_aa(canvas: &mut Canvas, centro: (i32, i32), radio: u32, color: Color) {
    if radio == 0 {
        return;
    }
    let r = f64::from(radio);
    // Centro geométrico = centro del píxel `centro`.
    let (cx, cy) = (f64::from(centro.0) + 0.5, f64::from(centro.1) + 0.5);
    let borde = radio as i32 + 1;
    for py in centro.1 - borde..=centro.1 + borde {
        for px in centro.0 - borde..=centro.0 + borde {
            let dx = f64::from(px) + 0.5 - cx;
            let dy = f64::from(py) + 0.5 - cy;
            let d = (dx * dx + dy * dy).sqrt();
            // Interior y exterior se resuelven sin muestrear.
            let cobertura = if d <= r - 0.75 {
                1.0
            } else if d >= r + 0.75 {
                0.0
            } else {
                cobertura_4x4(px, py, cx, cy, r)
            };
            if cobertura > 0.0 {
                let a = (f64::from(color.a) * cobertura).round() as u8;
                canvas.blend_pixel(px, py, Color::rgba(color.r, color.g, color.b, a));
            }
        }
    }
}

/// Fracción de las 16 submuestras del píxel que caen dentro del disco.
fn cobertura_4x4(px: i32, py: i32, cx: f64, cy: f64, r: f64) -> f64 {
    let mut dentro = 0;
    for sy in 0..4 {
        for sx in 0..4 {
            let x = f64::from(px) + (f64::from(sx) + 0.5) / 4.0;
            let y = f64::from(py) + (f64::from(sy) + 0.5) / 4.0;
            let (dx, dy) = (x - cx, y - cy);
            if dx * dx + dy * dy <= r * r {
                dentro += 1;
            }
        }
    }
    f64::from(dentro) / 16.0
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
    fn el_disco_aa_rellena_el_centro_y_suaviza_el_borde() {
        let mut frame = Frame::filled(20, 20, NEGRO);
        fill_disc_aa(&mut Canvas::new(&mut frame), (10, 10), 5, ROJO);
        // Centro y radio interior: rojo puro.
        assert!(es_rojo(&frame, 10, 10) && es_rojo(&frame, 10, 7));
        // Justo en el borde: cobertura parcial → rojo a medias.
        let [r, ..] = frame.pixel(10, 5).unwrap();
        assert!(r > 0 && r < 255, "el borde no tiene AA (r = {r})");
        // Fuera del disco: intacto.
        assert_eq!(frame.pixel(10, 3), Some(NEGRO));
        assert_eq!(frame.pixel(3, 3), Some(NEGRO));
    }

    #[test]
    fn el_disco_de_radio_cero_es_noop() {
        let mut frame = Frame::filled(6, 6, NEGRO);
        fill_disc_aa(&mut Canvas::new(&mut frame), (3, 3), 0, ROJO);
        assert_eq!(frame, Frame::filled(6, 6, NEGRO));
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
