//! Micro-utilidades internas del crate (antes duplicadas por módulo).

use rustcapture_core::annotate::Color;
use windows::Win32::Foundation::{COLORREF, LPARAM};

/// Color del motor → `COLORREF` de GDI, que va en BGR (no RGB) y sin alfa.
pub(crate) fn colorref(c: Color) -> COLORREF {
    COLORREF(c.r as u32 | (c.g as u32) << 8 | (c.b as u32) << 16)
}

/// Coordenadas del cursor empaquetadas en el lparam de los mensajes de
/// ratón (con signo: los monitores pueden tener origen negativo).
pub(crate) fn punto(lparam: LPARAM) -> (i32, i32) {
    (
        (lparam.0 & 0xFFFF) as i16 as i32,
        ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
    )
}

/// UTF-16 terminado en nulo para las APIs *W.
pub(crate) fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
