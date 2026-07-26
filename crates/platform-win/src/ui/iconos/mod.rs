//! Iconos de toolbar: máscaras de cobertura A8 generadas offline por
//! `design/tools/genassets` (antialiasing horneado), embebidas en el
//! binario y tintadas en runtime con el color del tema. Un solo asset
//! sirve para normal/deshabilitado/activo en claro y oscuro.

use std::cell::RefCell;
use std::collections::HashMap;

use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, AlphaBlend, BLENDFUNCTION, GdiFlush, HDC,
};
use windows::core::Result;

use crate::gdi::raii::{Dib, MemDc, ScreenDc, Selected};

mod atlas;
pub(crate) use atlas::Icono;
use atlas::{NUM_ICONOS, TALLAS};

static ATLAS: [(u32, &[u8]); 5] = [
    (16, include_bytes!("atlas_16.bin")),
    (20, include_bytes!("atlas_20.bin")),
    (24, include_bytes!("atlas_24.bin")),
    (28, include_bytes!("atlas_28.bin")),
    (32, include_bytes!("atlas_32.bin")),
];

/// Opacidad del tinte: 255 normal, ~40 % para deshabilitado.
pub(crate) const OPACO: u8 = 255;
pub(crate) const DESHABILITADO: u8 = 102;

/// Talla física del icono para un DPI: la menor talla generada que cubre
/// `16 lógicos` sin quedarse corta; por encima de 200 % se usa la mayor
/// (el que pinta escala desde 32 si de verdad necesita más).
pub(crate) fn talla_para_dpi(dpi: u32) -> u32 {
    let ideal = (16 * dpi).div_ceil(96);
    TALLAS
        .into_iter()
        .find(|&t| t >= ideal)
        .unwrap_or(TALLAS[TALLAS.len() - 1])
}

/// Máscara A8 de un icono a una talla generada (lado × lado bytes).
pub(crate) fn mascara(icono: Icono, talla: u32) -> &'static [u8] {
    let (_, blob) = ATLAS
        .iter()
        .find(|(t, _)| *t == talla)
        .expect("talla no generada en el atlas");
    debug_assert_eq!(blob.len(), NUM_ICONOS * (talla * talla) as usize);
    let lado2 = (talla * talla) as usize;
    let inicio = icono as usize * lado2;
    &blob[inicio..inicio + lado2]
}

/// Tinta una máscara A8 con un color: BGRA premultiplicada, que es lo que
/// exige `AlphaBlend` con `AC_SRC_ALPHA`. `opacidad` atenúa el conjunto
/// (deshabilitado) sin necesitar otro asset.
pub(crate) fn tinte_premultiplicado(mascara: &[u8], color_0xbbggrr: u32, opacidad: u8) -> Vec<u8> {
    let r = (color_0xbbggrr & 0xFF) as u32;
    let g = ((color_0xbbggrr >> 8) & 0xFF) as u32;
    let b = ((color_0xbbggrr >> 16) & 0xFF) as u32;
    let mut out = Vec::with_capacity(mascara.len() * 4);
    for &cobertura in mascara {
        let a = (u32::from(cobertura) * u32::from(opacidad) + 127) / 255;
        let premult = |c: u32| ((c * a + 127) / 255) as u8;
        out.extend_from_slice(&[premult(b), premult(g), premult(r), a as u8]);
    }
    out
}

thread_local! {
    // Caché de DIBs tintados por (icono, talla, color, opacidad). Vive en
    // el hilo de UI; acotado por iconos × tallas × pocos colores de tema.
    static CACHE: RefCell<HashMap<(Icono, u32, u32, u8), Dib>> = RefCell::new(HashMap::new());
}

/// Pinta un icono tintado en `dc` con su esquina superior izquierda en
/// (x, y) físicos. `talla` debe ser una de las generadas (`talla_para_dpi`).
pub(crate) fn pintar(
    dc: HDC,
    icono: Icono,
    talla: u32,
    x: i32,
    y: i32,
    color: windows::Win32::Foundation::COLORREF,
    opacidad: u8,
) -> Result<()> {
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let clave = (icono, talla, color.0, opacidad);
        if !cache.contains_key(&clave) {
            let pantalla = ScreenDc::get()?;
            let mem = MemDc::compatible_with(&pantalla)?;
            let mut dib = Dib::new_32bpp(&mem, talla, talla)?;
            dib.bits_mut().copy_from_slice(&tinte_premultiplicado(
                mascara(icono, talla),
                color.0,
                opacidad,
            ));
            cache.insert(clave, dib);
        }
        let dib = &cache[&clave];

        let pantalla = ScreenDc::get()?;
        let mem = MemDc::compatible_with(&pantalla)?;
        let _sel = Selected::bitmap(&mem, dib)?;
        let mezcla = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        // SAFETY: ambos DCs son válidos (RAII vivos) y el DIB fuente mide
        // exactamente talla × talla; GdiFlush garantiza que la escritura
        // previa de bits terminó antes del blit.
        unsafe {
            _ = GdiFlush();
            AlphaBlend(
                dc,
                x,
                y,
                talla as i32,
                talla as i32,
                mem.0,
                0,
                0,
                talla as i32,
                talla as i32,
                mezcla,
            )
            .ok()?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_talla_cubre_los_dpi_estandar() {
        assert_eq!(talla_para_dpi(96), 16);
        assert_eq!(talla_para_dpi(120), 20);
        assert_eq!(talla_para_dpi(144), 24);
        assert_eq!(talla_para_dpi(168), 28);
        assert_eq!(talla_para_dpi(192), 32);
    }

    #[test]
    fn una_talla_intermedia_redondea_hacia_arriba() {
        assert_eq!(talla_para_dpi(110), 20); // 18,3 px ideales → 20
        assert_eq!(talla_para_dpi(97), 20); // apenas por encima de 16
    }

    #[test]
    fn por_encima_del_atlas_se_usa_la_mayor() {
        assert_eq!(talla_para_dpi(240), 32);
    }

    #[test]
    fn cada_mascara_tiene_su_offset_y_no_esta_vacia() {
        for talla in TALLAS {
            let m = mascara(Icono::CaptureRegion, talla);
            assert_eq!(m.len(), (talla * talla) as usize);
            assert!(m.iter().any(|&b| b > 0));
        }
        // El último icono del enum también resuelve dentro del blob.
        let ultimo = mascara(Icono::OutputPrint, 16);
        assert_eq!(ultimo.len(), 256);
        assert!(ultimo.iter().any(|&b| b > 0));
    }

    #[test]
    fn el_tinte_premultiplica_y_respeta_la_opacidad() {
        // Color #0067C0 (acento) en COLORREF: 0x00C06700.
        let color = 0x00C06700;
        let tintado = tinte_premultiplicado(&[0, 255, 128], color, 255);
        assert_eq!(&tintado[0..4], &[0, 0, 0, 0]); // cobertura 0 → todo 0
        assert_eq!(&tintado[4..8], &[0xC0, 0x67, 0x00, 255]); // opaco → BGRA del color
        let a = tintado[11];
        assert_eq!(a, 128);
        assert_eq!(tintado[8], ((0xC0u32 * 128 + 127) / 255) as u8); // B premultiplicado

        // Deshabilitado: la cobertura plena queda al 40 %.
        let atenuado = tinte_premultiplicado(&[255], color, DESHABILITADO);
        assert_eq!(atenuado[3], 102);
    }
}
