//! Puerto de origen de píxeles (D2): lo implementan GDI/WGC en
//! `platform-win` y `MockScreenSource` en tests.

use super::{Frame, Rect};

#[derive(thiserror::Error, Clone, PartialEq, Eq, Debug)]
pub enum ScreenSourceError {
    #[error("la región {0:?} está fuera del escritorio")]
    OutOfBounds(Rect),
    /// Fallo del adapter (HRESULT, dispositivo perdido...). El texto ya viene
    /// formateado desde `platform-win`; `core` no conoce Win32.
    #[error("fallo de plataforma: {0}")]
    Platform(String),
}

pub trait ScreenSource {
    /// Rect del escritorio virtual completo (multi-monitor: el origen
    /// puede ser negativo).
    fn desktop_rect(&self) -> Rect;

    /// Rect de la ventana activa, si hay alguna (f.10). Vive aquí y no en
    /// un puerto propio mientras sea la única consulta de ventanas (D2:
    /// puertos solo en fronteras reales); si crece, se extrae.
    fn active_window_rect(&self) -> Option<Rect>;

    /// Captura `region` en coordenadas de escritorio virtual.
    fn capture_region(&mut self, region: Rect) -> Result<Frame, ScreenSourceError>;
}
