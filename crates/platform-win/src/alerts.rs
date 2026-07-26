//! Avisos mínimos de la GUI: beep no bloqueante y MessageBox para
//! errores fatales de arranque (la config rota, spec §Errores).

use windows::Win32::System::Diagnostics::Debug::MessageBeep;
use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
use windows::core::PCWSTR;

use crate::util::wide;

/// Beep de error no bloqueante (hotkey no registrable, captura fallida).
pub fn error_beep() {
    // SAFETY: sin precondiciones; el resultado no importa.
    unsafe { _ = MessageBeep(MB_ICONERROR) };
}

/// Sonido de confirmación de captura (feedback de verificación manual).
pub fn capture_beep() {
    // SAFETY: sin precondiciones; el resultado no importa.
    unsafe { _ = MessageBeep(MB_OK) };
}

/// MessageBox modal de error (solo errores fatales de arranque).
pub fn error_box(titulo: &str, texto: &str) {
    let titulo = wide(titulo);
    let texto = wide(texto);
    // SAFETY: los buffers viven hasta después de la llamada (locals).
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(texto.as_ptr()),
            PCWSTR(titulo.as_ptr()),
            MB_OK | MB_ICONERROR,
        )
    };
}
