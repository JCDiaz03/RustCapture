//! DPI awareness per-monitor (f.6): en per-monitor V2 todas las APIs
//! devuelven píxeles físicos, que es lo que captura BitBlt.
//!
//! La UI rediseñada (F3.5) define su layout en unidades LÓGICAS (rejilla
//! de `diseno-frontend.md` §2) y las convierte a físicas con `Escala` en
//! el momento de crear/pintar/posicionar. Regla: ninguna constante de
//! layout nueva en px físicos.

use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos,
};

/// Fija el proceso a per-monitor V2. Llamar UNA vez al arrancar cada
/// binario, antes de tocar ninguna ventana o captura. Devuelve `false`
/// si el sistema la rechaza (ya fijada por manifest o llamada previa):
/// no es un error, la awareness ya es definitiva.
pub fn ensure_per_monitor_dpi_awareness() -> bool {
    // SAFETY: cambia estado global del proceso; sin precondiciones de
    // memoria. Idempotente a efectos prácticos (la segunda llamada falla
    // y se ignora).
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_ok() }
}

/// Conversor de unidades lógicas (rejilla a 96 dpi) a píxeles físicos.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Escala(u32);

impl Escala {
    pub(crate) const fn nueva(dpi: u32) -> Self {
        // 0 = GetDpiForWindow con HWND inválido; tratarlo como 100 %.
        Self(if dpi == 0 { 96 } else { dpi })
    }

    /// DPI real de la ventana (per-monitor V2 → cambia al moverla de monitor).
    pub(crate) fn from_hwnd(hwnd: HWND) -> Self {
        // SAFETY: GetDpiForWindow acepta cualquier HWND; devuelve 0 si es
        // inválido, caso que `nueva` normaliza a 96.
        Self::nueva(unsafe { GetDpiForWindow(hwnd) })
    }

    pub(crate) const fn dpi(self) -> u32 {
        self.0
    }

    /// Lógico → físico con redondeo al más cercano (mitades hacia arriba).
    pub(crate) const fn px(self, logico: i32) -> i32 {
        ((logico as i64 * self.0 as i64 + 48) / 96) as i32
    }
}

/// DPI nuevo que trae `WM_DPICHANGED` en su wparam (ambos ejes son iguales).
// Documenta el contrato del mensaje; las superficies actuales resuelven
// con GetDpiForWindow tras aplicar el rect y no lo necesitan.
#[allow(dead_code)]
pub(crate) const fn dpi_de_wparam(wparam: WPARAM) -> u32 {
    (wparam.0 & 0xFFFF) as u32
}

/// Aplica el rect sugerido por `WM_DPICHANGED`: mover/redimensionar la
/// ventana exactamente ahí evita bucles de rebote entre monitores.
pub(crate) fn aplicar_rect_sugerido(hwnd: HWND, lparam: LPARAM) {
    // SAFETY: en WM_DPICHANGED el lparam apunta a un RECT válido que posee
    // el sistema durante el mensaje; solo se lee.
    let rect = unsafe { &*(lparam.0 as *const RECT) };
    // SAFETY: SetWindowPos sobre la propia ventana del mensaje.
    unsafe {
        _ = SetWindowPos(
            hwnd,
            None,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn px_escala_lineal_en_los_dpi_estandar() {
        // Botón de 28 lógicos en 100/125/150/175/200 %.
        assert_eq!(Escala::nueva(96).px(28), 28);
        assert_eq!(Escala::nueva(120).px(28), 35);
        assert_eq!(Escala::nueva(144).px(28), 42);
        assert_eq!(Escala::nueva(168).px(28), 49);
        assert_eq!(Escala::nueva(192).px(28), 56);
    }

    #[test]
    fn px_redondea_al_mas_cercano() {
        assert_eq!(Escala::nueva(120).px(1), 1); // 1,25 → 1
        assert_eq!(Escala::nueva(144).px(1), 2); // 1,5 → 2 (borde visible)
        assert_eq!(Escala::nueva(96).px(0), 0);
    }

    #[test]
    fn un_dpi_de_cero_cae_a_96() {
        assert_eq!(Escala::nueva(0).px(10), 10);
        assert_eq!(Escala::nueva(0).dpi(), 96);
    }

    #[test]
    fn el_wparam_de_dpichanged_lleva_el_dpi_en_el_word_bajo() {
        assert_eq!(dpi_de_wparam(WPARAM((144 << 16) | 144)), 144);
    }
}
