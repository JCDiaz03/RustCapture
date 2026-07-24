//! Eventos del bus (D7): datos, no comportamiento. Cualquier productor
//! (hotkey, barra, CLI, auto-captura futura) publica estos valores en el
//! canal mpsc que consume el orquestador.

use crate::ports::HotkeyId;

pub use crate::capture::ModeRequest;

/// `CaptureRequested { mode, destination }` de D7.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CaptureRequest {
    pub mode: ModeRequest,
    /// Id del sink registrado ("clipboard", "file"...).
    pub destination: &'static str,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AppEvent {
    CaptureRequested(CaptureRequest),
    /// Captura programada (f.17): el orquestador lanza un hilo que espera
    /// `delay_ms` y reenvía `CaptureRequested` por su loopback.
    DelayedCapture {
        request: CaptureRequest,
        delay_ms: u64,
    },
    /// Repite la última captura ejecutada con éxito (f.18).
    RepeatLast,
    HotkeyPressed(HotkeyId),
    Shutdown,
}
