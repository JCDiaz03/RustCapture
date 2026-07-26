//! Carga del icono de aplicación embebido como recurso 1 por
//! `crates/gui/build.rs`. Los binarios sin recursos (tests, cli) reciben
//! `None` y cada llamador decide su fallback.

use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    HICON, IMAGE_ICON, LR_DEFAULTCOLOR, LoadIconW, LoadImageW,
};
use windows::core::PCWSTR;

/// ID del recurso ICON que embebe `crates/gui/build.rs`.
const ID_RECURSO: PCWSTR = PCWSTR(1 as *const u16);

/// Icono a medida exacta (p. ej. bandeja a `SM_CXSMICON`).
pub(crate) fn a_medida(cx: i32, cy: i32) -> Option<HICON> {
    // SAFETY: LoadImageW sobre un recurso del propio módulo; devuelve
    // error si el binario no lo embebe.
    unsafe {
        let instance = GetModuleHandleW(None).ok()?;
        LoadImageW(
            Some(instance.into()),
            ID_RECURSO,
            IMAGE_ICON,
            cx,
            cy,
            LR_DEFAULTCOLOR,
        )
        .ok()
        .map(|h| HICON(h.0))
    }
}

/// Icono en tamaños por defecto del sistema, para `WNDCLASSW.hIcon`.
/// Sin recurso devuelve el HICON nulo (Windows pinta el genérico).
pub(crate) fn para_clase() -> HICON {
    // SAFETY: LoadIconW sobre un recurso del propio módulo.
    unsafe {
        GetModuleHandleW(None)
            .ok()
            .and_then(|instance| LoadIconW(Some(instance.into()), ID_RECURSO).ok())
            .unwrap_or_default()
    }
}
