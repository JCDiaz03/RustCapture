//! Geometría pura del overlay (TDD): coordenadas locales de la ventana.

use rustcapture_core::ports::Rect;

pub(crate) const LUPA_SRC: i32 = 60;
pub(crate) const LUPA_W: i32 = 300;
pub(crate) const LUPA_ZOOM_H: i32 = 300;
pub(crate) const LUPA_COORD_H: i32 = 30;
pub(crate) const LUPA_HELP_H: i32 = 170;
pub(crate) const LUPA_H: i32 = LUPA_ZOOM_H + LUPA_COORD_H + LUPA_HELP_H;
const MARGEN: i32 = 20;
const ZONA_SALTO: i32 = 40;

/// Rect normalizado entre dos puntos de arrastre; mínimo 1×1 (f.19).
pub(crate) fn rect_between(a: (i32, i32), b: (i32, i32)) -> Rect {
    Rect::new(
        a.0.min(b.0),
        a.1.min(b.1),
        (a.0 - b.0).unsigned_abs().max(1),
        (a.1 - b.1).unsigned_abs().max(1),
    )
}

/// Fuente del zoom: 60×60 centrado en el cursor, sin salirse del frame.
pub(crate) fn lupa_source(cursor: (i32, i32), frame_w: u32, frame_h: u32) -> Rect {
    let max_x = (frame_w as i32 - LUPA_SRC).max(0);
    let max_y = (frame_h as i32 - LUPA_SRC).max(0);
    Rect::new(
        (cursor.0 - LUPA_SRC / 2).clamp(0, max_x),
        (cursor.1 - LUPA_SRC / 2).clamp(0, max_y),
        LUPA_SRC as u32,
        LUPA_SRC as u32,
    )
}

/// Esquina de la caja de lupa: inferior-derecha del monitor; si el
/// cursor entra en la caja inflada, salta a superior-izquierda.
pub(crate) fn lupa_box_pos(monitor: Rect, cursor: (i32, i32)) -> (i32, i32) {
    let br = (
        monitor.right() as i32 - LUPA_W - MARGEN,
        monitor.bottom() as i32 - LUPA_H - MARGEN,
    );
    let dentro = cursor.0 >= br.0 - ZONA_SALTO
        && cursor.0 < br.0 + LUPA_W + ZONA_SALTO
        && cursor.1 >= br.1 - ZONA_SALTO
        && cursor.1 < br.1 + LUPA_H + ZONA_SALTO;
    if dentro {
        (monitor.x + MARGEN, monitor.y + MARGEN)
    } else {
        br
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_between_normaliza_las_cuatro_direcciones() {
        let esperado = Rect::new(10, 20, 30, 40);
        assert_eq!(rect_between((10, 20), (40, 60)), esperado);
        assert_eq!(rect_between((40, 60), (10, 20)), esperado);
        assert_eq!(rect_between((40, 20), (10, 60)), esperado);
        assert_eq!(rect_between((10, 60), (40, 20)), esperado);
    }

    #[test]
    fn rect_between_de_un_clic_es_un_pixel() {
        assert_eq!(rect_between((5, 5), (5, 5)), Rect::new(5, 5, 1, 1));
    }

    #[test]
    fn lupa_source_centra_sobre_el_cursor() {
        assert_eq!(
            lupa_source((100, 100), 1920, 1080),
            Rect::new(70, 70, 60, 60)
        );
    }

    #[test]
    fn lupa_source_clampa_en_las_esquinas() {
        assert_eq!(lupa_source((0, 0), 1920, 1080), Rect::new(0, 0, 60, 60));
        assert_eq!(
            lupa_source((1919, 1079), 1920, 1080),
            Rect::new(1860, 1020, 60, 60)
        );
    }

    #[test]
    fn lupa_box_va_a_la_esquina_inferior_derecha() {
        let monitor = Rect::new(0, 0, 1920, 1080);
        assert_eq!(
            lupa_box_pos(monitor, (100, 100)),
            (1920 - 300 - 20, 1080 - 500 - 20)
        );
    }

    #[test]
    fn lupa_box_salta_cuando_el_cursor_se_acerca() {
        let monitor = Rect::new(0, 0, 1920, 1080);
        // Cursor dentro de la zona de la caja (esquina inferior derecha).
        assert_eq!(lupa_box_pos(monitor, (1700, 900)), (20, 20));
    }

    #[test]
    fn lupa_box_respeta_monitores_con_origen_negativo() {
        let monitor = Rect::new(-1920, 0, 1920, 1080);
        assert_eq!(
            lupa_box_pos(monitor, (-1800, 100)),
            (-1920 + 1920 - 300 - 20, 1080 - 500 - 20)
        );
    }
}
