//! DPI awareness per-monitor (f.6): en per-monitor V2 todas las APIs
//! devuelven píxeles físicos, que es lo que captura BitBlt.

use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
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
