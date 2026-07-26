//! Fuentes de la UI (tokens §2): Segoe UI 13/12/11 y Consolas 10, en
//! unidades lógicas. Cacheadas por (rol, dpi) durante toda la vida del
//! proceso: el puñado de HFONT resultante no merece contabilidad RAII.

use std::cell::RefCell;
use std::collections::HashMap;

use windows::Win32::Graphics::Gdi::{
    CLIP_DEFAULT_PRECIS, CreateFontW, DEFAULT_CHARSET, DEFAULT_PITCH, DEFAULT_QUALITY, FW_NORMAL,
    HFONT, OUT_DEFAULT_PRECIS,
};
use windows::core::{PCWSTR, w};

use crate::dpi::Escala;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Rol {
    /// Segoe UI 13 — cuerpo (token del diseño; lo estrenará Ajustes V10).
    #[allow(dead_code)]
    Cuerpo,
    /// Segoe UI 12 — denso / chips.
    Denso,
    /// Segoe UI 11 — secundario / status bar.
    Secundario,
    /// Consolas 10 — coordenadas y valores hex.
    Mono,
}

impl Rol {
    const fn tam_logico(self) -> i32 {
        match self {
            Rol::Cuerpo => 13,
            Rol::Denso => 12,
            Rol::Secundario => 11,
            Rol::Mono => 10,
        }
    }

    const fn familia(self) -> PCWSTR {
        match self {
            Rol::Mono => w!("Consolas"),
            _ => w!("Segoe UI"),
        }
    }
}

thread_local! {
    static CACHE: RefCell<HashMap<(Rol, u32), HFONT>> = RefCell::new(HashMap::new());
}

/// HFONT cacheado del rol a un DPI. No destruir: lo posee la caché.
pub(crate) fn fuente(rol: Rol, escala: Escala) -> HFONT {
    CACHE.with(|cache| {
        *cache
            .borrow_mut()
            .entry((rol, escala.dpi()))
            .or_insert_with(|| {
                // SAFETY: CreateFontW no tiene precondiciones; un HFONT
                // inválido lo tratan los llamadores como "sin fuente".
                unsafe {
                    CreateFontW(
                        -escala.px(rol.tam_logico()), // negativo = alto de carácter
                        0,
                        0,
                        0,
                        FW_NORMAL.0 as i32,
                        0,
                        0,
                        0,
                        DEFAULT_CHARSET,
                        OUT_DEFAULT_PRECIS,
                        CLIP_DEFAULT_PRECIS,
                        DEFAULT_QUALITY,
                        DEFAULT_PITCH.0 as u32,
                        rol.familia(),
                    )
                }
            })
    })
}
