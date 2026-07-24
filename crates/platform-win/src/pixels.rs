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
}
