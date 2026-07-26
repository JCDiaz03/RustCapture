//! Esqueleto común de los wndproc del crate: estado en GWLP_USERDATA
//! (patrón D12: lo posee la función llamadora, el wndproc solo lo usa) y
//! reacción al cambio de tema del sistema.

use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::Graphics::Gdi::{RDW_ALLCHILDREN, RDW_ERASE, RDW_INVALIDATE, RedrawWindow};
use windows::Win32::UI::WindowsAndMessaging::{
    CREATESTRUCTW, GWLP_USERDATA, GetWindowLongPtrW, SetWindowLongPtrW,
};

use crate::ui::theme::{self, Tema};

/// Rama `WM_NCCREATE`: adopta el puntero de estado que viajó en
/// `lpCreateParams` y lo deja en GWLP_USERDATA.
pub(crate) fn adoptar_estado(hwnd: HWND, lparam: LPARAM) {
    // SAFETY: en WM_NCCREATE el lparam es el CREATESTRUCTW de la
    // creación; lpCreateParams es el Box::into_raw del llamador.
    unsafe {
        let cs = &*(lparam.0 as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
    }
}

/// Estado de la ventana. Precondición del llamador: GWLP_USERDATA de
/// `hwnd` contiene un `*mut T` puesto por `adoptar_estado` con un Box
/// vivo (se libera solo tras destruir la ventana / en WM_NCDESTROY).
pub(crate) fn estado<'a, T>(hwnd: HWND) -> Option<&'a mut T> {
    // SAFETY: ver precondición; los mensajes llegan por el hilo creador,
    // así que no hay acceso concurrente.
    unsafe { (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut T).as_mut() }
}

/// Rama `WM_SETTINGCHANGE`: si anuncia cambio de tema, lo re-resuelve y
/// repinta la ventana con sus hijos. Devuelve el tema nuevo para que el
/// llamador aplique sus extras (p. ej. DWM dark title bar).
pub(crate) fn cambio_de_tema(hwnd: HWND, lparam: LPARAM) -> Option<Tema> {
    if !theme::es_cambio_de_tema(lparam) {
        return None;
    }
    let tema = theme::refrescar_con_modo_actual();
    // SAFETY: repintado de una ventana propia viva.
    unsafe {
        _ = RedrawWindow(
            Some(hwnd),
            None,
            None,
            RDW_ERASE | RDW_INVALIDATE | RDW_ALLCHILDREN,
        );
    }
    Some(tema)
}
