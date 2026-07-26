//! Micro-utilidades internas del crate (antes duplicadas por módulo).

use windows::Win32::Foundation::LPARAM;

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
