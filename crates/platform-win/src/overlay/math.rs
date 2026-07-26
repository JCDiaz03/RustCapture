//! Geometría pura del overlay (TDD): coordenadas locales de la ventana.

use rustcapture_core::ports::Rect;

/// Lado (impar) de la fuente del zoom: el píxel del cursor queda en la
/// celda central exacta.
pub(crate) const LUPA_SRC: i32 = 21;
/// Lado LÓGICO de cada celda de zoom (≈6×).
pub(crate) const LUPA_CELDA: i32 = 6;
/// Alto LÓGICO del bloque de información (dos líneas de Consolas 10).
pub(crate) const LUPA_INFO_H: i32 = 36;
/// Separación LÓGICA de la caja respecto al cursor.
pub(crate) const LUPA_OFFSET: i32 = 16;

/// Píxeles que mueve cada paso de rueda en la región fija (f.15).
pub(crate) const PASO_FIJO: i32 = 10;
/// Lado mínimo, para que la rueda no lo deje en nada.
const MINIMO_FIJO: u32 = 8;

/// Rect de `tam` centrado en el cursor y empujado dentro de `limite`. Si no
/// cabe, se recorta al límite: mejor eso que devolver algo fuera de pantalla.
pub(crate) fn rect_fijo(cursor: (i32, i32), tam: (u32, u32), limite: Rect) -> Rect {
    let w = tam.0.min(limite.width).max(1);
    let h = tam.1.min(limite.height).max(1);
    let max_x = (limite.right() as i32 - w as i32).max(limite.x);
    let max_y = (limite.bottom() as i32 - h as i32).max(limite.y);
    Rect::new(
        (cursor.0 - w as i32 / 2).clamp(limite.x, max_x),
        (cursor.1 - h as i32 / 2).clamp(limite.y, max_y),
        w,
        h,
    )
}

/// Ajusta el tamaño con la rueda; `solo_ancho` = Shift pulsado.
pub(crate) fn ajustar_tam(tam: (u32, u32), pasos: i32, solo_ancho: bool) -> (u32, u32) {
    let mover = |v: u32| -> u32 {
        (v as i64 + (pasos * PASO_FIJO) as i64).clamp(MINIMO_FIJO as i64, u32::MAX as i64) as u32
    };
    if solo_ancho {
        (mover(tam.0), tam.1)
    } else {
        (mover(tam.0), mover(tam.1))
    }
}

/// Rect normalizado entre dos puntos de arrastre; mínimo 1×1 (f.19).
pub(crate) fn rect_between(a: (i32, i32), b: (i32, i32)) -> Rect {
    Rect::new(
        a.0.min(b.0),
        a.1.min(b.1),
        (a.0 - b.0).unsigned_abs().max(1),
        (a.1 - b.1).unsigned_abs().max(1),
    )
}

/// Fuente del zoom: 21×21 centrado en el cursor, sin salirse del frame.
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

/// Esquina de la caja de lupa: junto al cursor con `offset`, con FLIP al
/// otro lado cuando no cabe hacia la derecha/abajo del monitor. Todo en
/// px físicos (el llamador ya escaló caja y offset).
pub(crate) fn lupa_box_pos(
    monitor: Rect,
    cursor: (i32, i32),
    caja: (i32, i32),
    offset: i32,
) -> (i32, i32) {
    let mut x = cursor.0 + offset;
    if x + caja.0 > monitor.right() as i32 {
        x = cursor.0 - offset - caja.0;
    }
    let mut y = cursor.1 + offset;
    if y + caja.1 > monitor.bottom() as i32 {
        y = cursor.1 - offset - caja.1;
    }
    (x.max(monitor.x), y.max(monitor.y))
}

/// Color de un píxel BGRA como `#RRGGBB` (lo que muestra la lupa).
pub(crate) fn hex_de_bgra(b: u8, g: u8, r: u8) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
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
    fn lupa_source_centra_el_cursor_en_la_celda_central() {
        // 21 de lado → el cursor queda en la celda 10 (la central).
        assert_eq!(
            lupa_source((100, 100), 1920, 1080),
            Rect::new(90, 90, 21, 21)
        );
    }

    #[test]
    fn lupa_source_clampa_en_las_esquinas() {
        assert_eq!(lupa_source((0, 0), 1920, 1080), Rect::new(0, 0, 21, 21));
        assert_eq!(
            lupa_source((1919, 1079), 1920, 1080),
            Rect::new(1899, 1059, 21, 21)
        );
    }

    const CAJA: (i32, i32) = (126, 162);
    const OFFSET: i32 = 16;

    #[test]
    fn lupa_box_va_junto_al_cursor() {
        let monitor = Rect::new(0, 0, 1920, 1080);
        assert_eq!(
            lupa_box_pos(monitor, (100, 100), CAJA, OFFSET),
            (116, 116)
        );
    }

    #[test]
    fn lupa_box_flipa_en_horizontal_cerca_del_borde_derecho() {
        let monitor = Rect::new(0, 0, 1920, 1080);
        assert_eq!(
            lupa_box_pos(monitor, (1850, 100), CAJA, OFFSET),
            (1850 - 16 - 126, 116)
        );
    }

    #[test]
    fn lupa_box_flipa_en_vertical_cerca_del_borde_inferior() {
        let monitor = Rect::new(0, 0, 1920, 1080);
        assert_eq!(
            lupa_box_pos(monitor, (100, 1000), CAJA, OFFSET),
            (116, 1000 - 16 - 162)
        );
    }

    #[test]
    fn lupa_box_flipa_en_ambos_ejes_en_la_esquina() {
        let monitor = Rect::new(0, 0, 1920, 1080);
        assert_eq!(
            lupa_box_pos(monitor, (1900, 1060), CAJA, OFFSET),
            (1900 - 16 - 126, 1060 - 16 - 162)
        );
    }

    #[test]
    fn lupa_box_no_se_sale_por_arriba_izquierda_al_flipar() {
        // Monitor diminuto: el flip horizontal daría x negativa (120-16-126)
        // → clamp al origen del monitor.
        let monitor = Rect::new(0, 0, 200, 200);
        assert_eq!(lupa_box_pos(monitor, (120, 190), CAJA, OFFSET), (0, 12));
    }

    #[test]
    fn lupa_box_respeta_monitores_con_origen_negativo() {
        let monitor = Rect::new(-1920, 0, 1920, 1080);
        assert_eq!(
            lupa_box_pos(monitor, (-1800, 100), CAJA, OFFSET),
            (-1784, 116)
        );
        // Pegado al borde derecho del monitor izquierdo (x = 0): flip.
        assert_eq!(
            lupa_box_pos(monitor, (-50, 100), CAJA, OFFSET),
            (-50 - 16 - 126, 116)
        );
    }

    #[test]
    fn el_hex_va_en_orden_rgb_y_mayusculas() {
        assert_eq!(hex_de_bgra(0xAB, 0x7D, 0x4A), "#4A7DAB");
        assert_eq!(hex_de_bgra(0, 0, 0), "#000000");
        assert_eq!(hex_de_bgra(255, 255, 255), "#FFFFFF");
    }
}

#[cfg(test)]
mod tests_fijo {
    use super::*;

    const LIMITE: Rect = Rect { x: 0, y: 0, width: 1000, height: 800 };

    #[test]
    fn el_rect_fijo_se_centra_en_el_cursor() {
        assert_eq!(rect_fijo((500, 400), (200, 100), LIMITE), Rect::new(400, 350, 200, 100));
    }

    #[test]
    fn el_rect_fijo_no_se_sale_del_limite() {
        // Pegado a la esquina superior izquierda y a la inferior derecha.
        assert_eq!(rect_fijo((10, 10), (200, 100), LIMITE), Rect::new(0, 0, 200, 100));
        assert_eq!(rect_fijo((995, 795), (200, 100), LIMITE), Rect::new(800, 700, 200, 100));
    }

    #[test]
    fn un_rect_mayor_que_el_limite_se_recorta_al_limite() {
        let pequeno = Rect::new(0, 0, 100, 80);
        assert_eq!(rect_fijo((50, 40), (400, 300), pequeno), Rect::new(0, 0, 100, 80));
    }

    #[test]
    fn el_limite_con_origen_negativo_funciona() {
        let limite = Rect::new(-1920, -100, 3840, 1180);
        assert_eq!(
            rect_fijo((-1900, -90), (200, 100), limite),
            Rect::new(-1920, -100, 200, 100)
        );
        // Y en el centro sigue centrándose.
        assert_eq!(rect_fijo((0, 400), (200, 100), limite), Rect::new(-100, 350, 200, 100));
    }

    #[test]
    fn la_rueda_ajusta_el_tamano_con_minimo() {
        assert_eq!(ajustar_tam((200, 100), 1, false), (210, 110));
        assert_eq!(ajustar_tam((200, 100), -1, false), (190, 90));
        // Shift: solo el ancho.
        assert_eq!(ajustar_tam((200, 100), 2, true), (220, 100));
        // Nunca baja del mínimo usable.
        assert_eq!(ajustar_tam((10, 10), -5, false), (8, 8));
    }
}
