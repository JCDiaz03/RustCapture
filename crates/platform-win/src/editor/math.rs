//! Encaje de la captura en el lienzo del editor (puro, TDD).

use rustcapture_core::ports::Rect;

/// Rect destino de la imagen dentro del lienzo: centrada; si no cabe,
/// reducida manteniendo aspecto. Nunca se amplía.
pub(crate) fn fit_rect(imagen: (u32, u32), lienzo: (i32, i32)) -> Rect {
    let (iw, ih) = (imagen.0 as i64, imagen.1 as i64);
    let (lw, lh) = (lienzo.0 as i64, lienzo.1 as i64);
    if iw == 0 || ih == 0 || lw <= 0 || lh <= 0 {
        return Rect::new(0, 0, 0, 0);
    }
    let (w, h) = if iw <= lw && ih <= lh {
        (iw, ih)
    } else if iw * lh >= ih * lw {
        // Limita el ancho.
        (lw, (ih * lw / iw).max(1))
    } else {
        // Limita el alto.
        ((iw * lh / ih).max(1), lh)
    };
    Rect::new(
        ((lw - w) / 2) as i32,
        ((lh - h) / 2) as i32,
        w as u32,
        h as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imagen_pequena_se_centra_a_tamano_natural() {
        assert_eq!(
            fit_rect((100, 50), (400, 300)),
            Rect::new(150, 125, 100, 50)
        );
    }

    #[test]
    fn imagen_ancha_se_reduce_a_lo_ancho() {
        // 2000×1000 en 400×300 → escala 0.2 → 400×200, centrada en Y.
        assert_eq!(
            fit_rect((2000, 1000), (400, 300)),
            Rect::new(0, 50, 400, 200)
        );
    }

    #[test]
    fn imagen_alta_se_reduce_a_lo_alto() {
        // 500×1500 en 400×300 → escala 0.2 → 100×300, centrada en X.
        assert_eq!(
            fit_rect((500, 1500), (400, 300)),
            Rect::new(150, 0, 100, 300)
        );
    }

    #[test]
    fn lienzo_degenerado_da_rect_vacio() {
        assert_eq!(fit_rect((100, 100), (0, 300)), Rect::new(0, 0, 0, 0));
        assert_eq!(fit_rect((0, 0), (400, 300)), Rect::new(0, 0, 0, 0));
    }
}
