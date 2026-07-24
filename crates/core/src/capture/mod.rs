//! Slice de captura: modos (f.9-f.19) como strategies `CaptureMode` (D4),
//! selección y scroll-stitching.

use crate::ports::{Frame, ScreenSource, ScreenSourceError};

#[derive(thiserror::Error, Clone, PartialEq, Eq, Debug)]
pub enum CaptureError {
    #[error(transparent)]
    Source(#[from] ScreenSourceError),
    /// El modo no tiene nada que capturar (sin ventana activa, etc.).
    #[error("nada que capturar: {0}")]
    NothingToCapture(String),
}

/// Strategy de captura (D4): recibe un `ScreenSource`, devuelve un `Frame`.
/// Las estrategias concretas (pantalla completa, ventana, región...) se
/// construyen desde un `ModeRequest` vía la mode factory del orquestador.
pub trait CaptureMode {
    fn capture(&self, source: &mut dyn ScreenSource) -> Result<Frame, CaptureError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::mocks::MockScreenSource;
    use crate::ports::{Frame, ScreenSource, ScreenSourceError};

    /// Estrategia mínima para validar el contrato del trait.
    struct DesktopMode;

    impl CaptureMode for DesktopMode {
        fn capture(&self, source: &mut dyn ScreenSource) -> Result<Frame, CaptureError> {
            let rect = source.desktop_rect();
            // `?` prueba la conversión From<ScreenSourceError>.
            Ok(source.capture_region(rect)?)
        }
    }

    #[test]
    fn una_estrategia_captura_a_traves_del_puerto() {
        let mut source = MockScreenSource::new((0, 0), Frame::filled(2, 2, [9, 9, 9, 255]));
        let frame = DesktopMode.capture(&mut source).unwrap();
        assert_eq!((frame.width, frame.height), (2, 2));
        assert_eq!(frame.pixel(1, 1), Some([9, 9, 9, 255]));
    }

    #[test]
    fn los_errores_del_puerto_se_convierten_a_capture_error() {
        let mut source = MockScreenSource::new((0, 0), Frame::filled(1, 1, [0; 4]));
        source.fail_next(ScreenSourceError::Platform("GDI caído".into()));
        let err = DesktopMode.capture(&mut source).unwrap_err();
        assert_eq!(
            err,
            CaptureError::Source(ScreenSourceError::Platform("GDI caído".into()))
        );
    }
}
