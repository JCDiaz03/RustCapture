//! Eventos del bus (D7): datos, no comportamiento. Cualquier productor
//! (hotkey, barra, CLI, auto-captura futura) publica estos valores en el
//! canal mpsc que consume el orquestador.

use crate::ports::{HotkeyId, Rect};

/// Qué capturar. La mode factory lo convierte en una strategy `CaptureMode`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModeRequest {
    Fullscreen,
    ActiveWindow,
    Region(Rect),
}

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
    HotkeyPressed(HotkeyId),
    Shutdown,
}
