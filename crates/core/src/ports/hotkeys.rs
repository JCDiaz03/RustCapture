//! Puerto de atajos globales (D2, f.3). El trait cubre solo el registro;
//! las pulsaciones llegan como eventos por el canal mpsc del orquestador
//! (D7), con el que se construye cada adapter.

/// Teclas modificadoras. `win` es la tecla Windows.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

/// Tecla principal del atajo, independiente de códigos VK de Win32;
/// el adapter hace el mapeo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyCode {
    /// Letra o dígito, en minúscula ('a'..'z', '0'..'9').
    Char(char),
    /// Tecla de función F1..F24.
    F(u8),
    PrintScreen,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Hotkey {
    pub modifiers: Modifiers,
    pub key: KeyCode,
}

/// Identificador opaco que asigna el provider al registrar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HotkeyId(pub u32);

#[derive(thiserror::Error, Clone, PartialEq, Eq, Debug)]
pub enum HotkeyError {
    #[error("el atajo {0:?} ya está registrado")]
    AlreadyRegistered(Hotkey),
    #[error("id de atajo desconocido: {0:?}")]
    UnknownId(HotkeyId),
    /// `RegisterHotKey` falló (atajo tomado por otra app, etc.).
    #[error("fallo de plataforma: {0}")]
    Platform(String),
}

pub trait HotkeyProvider {
    fn register(&mut self, hotkey: Hotkey) -> Result<HotkeyId, HotkeyError>;
    fn unregister(&mut self, id: HotkeyId) -> Result<(), HotkeyError>;
}
