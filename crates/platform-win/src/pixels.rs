//! Conversiones de píxel puras (sin `unsafe`, testeables sin hardware).

/// Convierte BGRA8 (nativo de GDI/DXGI) a RGBA8 in-place y fuerza alfa
/// opaco: BitBlt no escribe alfa útil y las capturas son siempre opacas.
/// Ignora bytes sobrantes si la longitud no es múltiplo de 4.
pub fn bgra_to_rgba_opaque(pixels: &mut [u8]) {
    for px in pixels.chunks_exact_mut(4) {
        px.swap(0, 2);
        px[3] = 255;
    }
}

/// Convierte RGBA8 a BGRA8 in-place (swap R↔B); el alfa no se toca.
pub fn rgba_to_bgra(pixels: &mut [u8]) {
    for px in pixels.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
}

/// Reordena las filas de arriba-abajo a abajo-arriba (DIB bottom-up,
/// el formato que más aplicaciones aceptan en CF_DIB).
pub fn rows_bottom_up(pixels: &[u8], width: u32, height: u32) -> Vec<u8> {
    let row = width as usize * 4;
    let mut out = Vec::with_capacity(pixels.len());
    for fila in (0..height as usize).rev() {
        out.extend_from_slice(&pixels[fila * row..(fila + 1) * row]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intercambia_b_y_r_y_fuerza_alfa_opaco() {
        // Dos píxeles BGRA; BitBlt suele dejar alfa 0.
        let mut px = vec![1u8, 2, 3, 0, 10, 20, 30, 128];
        bgra_to_rgba_opaque(&mut px);
        assert_eq!(px, vec![3, 2, 1, 255, 30, 20, 10, 255]);
    }

    #[test]
    fn buffer_vacio_no_hace_nada() {
        let mut px: Vec<u8> = Vec::new();
        bgra_to_rgba_opaque(&mut px);
        assert!(px.is_empty());
    }

    #[test]
    fn rgba_to_bgra_intercambia_canales_sin_tocar_alfa() {
        let mut px = vec![1u8, 2, 3, 40];
        rgba_to_bgra(&mut px);
        assert_eq!(px, vec![3, 2, 1, 40]);
    }

    #[test]
    fn rows_bottom_up_invierte_el_orden_de_filas() {
        // 1x3: filas A, B, C (un píxel por fila).
        let px: Vec<u8> = vec![1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3];
        assert_eq!(
            rows_bottom_up(&px, 1, 3),
            vec![3, 3, 3, 3, 2, 2, 2, 2, 1, 1, 1, 1]
        );
    }

    #[test]
    fn rows_bottom_up_de_una_fila_es_identidad() {
        let px: Vec<u8> = vec![7, 8, 9, 255, 1, 2, 3, 255];
        assert_eq!(rows_bottom_up(&px, 2, 1), px);
    }
}
