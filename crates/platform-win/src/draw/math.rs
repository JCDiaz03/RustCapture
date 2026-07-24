//! Mapeo puro entre coordenadas de la vista (lienzo encajado) y del
//! frame real (TDD): el dibujo debe caer exacto sobre la imagen.

use rustcapture_core::ports::Rect;

/// Punto de la vista → píxel del frame; `None` fuera del área encajada.
pub(crate) fn view_to_frame(p: (i32, i32), destino: Rect, frame: (u32, u32)) -> Option<(i32, i32)> {
    if destino.is_empty() || frame.0 == 0 || frame.1 == 0 {
        return None;
    }
    let dentro = p.0 >= destino.x
        && (p.0 as i64) < destino.right()
        && p.1 >= destino.y
        && (p.1 as i64) < destino.bottom();
    if !dentro {
        return None;
    }
    let fx = (p.0 - destino.x) as i64 * frame.0 as i64 / destino.width as i64;
    let fy = (p.1 - destino.y) as i64 * frame.1 as i64 / destino.height as i64;
    Some((
        (fx as i32).clamp(0, frame.0 as i32 - 1),
        (fy as i32).clamp(0, frame.1 as i32 - 1),
    ))
}

/// Píxel del frame → punto de la vista (esquina del píxel escalado).
pub(crate) fn frame_to_view(p: (i32, i32), destino: Rect, frame: (u32, u32)) -> (i32, i32) {
    if destino.is_empty() || frame.0 == 0 || frame.1 == 0 {
        return (0, 0);
    }
    (
        destino.x + (p.0 as i64 * destino.width as i64 / frame.0 as i64) as i32,
        destino.y + (p.1 as i64 * destino.height as i64 / frame.1 as i64) as i32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Imagen 200×100 encajada en destino (10,20,100,50): escala 0.5.
    const DESTINO: Rect = Rect {
        x: 10,
        y: 20,
        width: 100,
        height: 50,
    };
    const FRAME: (u32, u32) = (200, 100);

    #[test]
    fn dentro_del_destino_escala_al_frame() {
        assert_eq!(view_to_frame((10, 20), DESTINO, FRAME), Some((0, 0)));
        assert_eq!(view_to_frame((60, 45), DESTINO, FRAME), Some((100, 50)));
        assert_eq!(view_to_frame((109, 69), DESTINO, FRAME), Some((198, 98)));
    }

    #[test]
    fn fuera_del_destino_es_none() {
        assert_eq!(view_to_frame((9, 20), DESTINO, FRAME), None);
        assert_eq!(view_to_frame((10, 70), DESTINO, FRAME), None);
    }

    #[test]
    fn frame_to_view_es_el_inverso() {
        assert_eq!(frame_to_view((0, 0), DESTINO, FRAME), (10, 20));
        assert_eq!(frame_to_view((100, 50), DESTINO, FRAME), (60, 45));
    }

    #[test]
    fn degenerado_no_divide_por_cero() {
        let destino = Rect::new(0, 0, 0, 0);
        assert_eq!(view_to_frame((5, 5), destino, FRAME), None);
        assert_eq!(frame_to_view((5, 5), destino, FRAME), (0, 0));
    }
}
