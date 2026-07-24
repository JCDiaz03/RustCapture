//! Ventana activa (f.10): captura el rect que reporta el `ScreenSource`,
//! recortado al escritorio visible.

use crate::capture::{CaptureError, CaptureMode};
use crate::ports::{Frame, ScreenSource};

/// Captura la ventana activa (f.10), recortada al escritorio visible.
pub struct ActiveWindowMode;

impl CaptureMode for ActiveWindowMode {
    fn capture(&self, source: &mut dyn ScreenSource) -> Result<Frame, CaptureError> {
        let window = source
            .active_window_rect()
            .ok_or_else(|| CaptureError::NothingToCapture("no hay ventana activa".into()))?;
        let visible = source.desktop_rect().intersection(&window).ok_or_else(|| {
            CaptureError::NothingToCapture("la ventana activa está fuera de la pantalla".into())
        })?;
        Ok(source.capture_region(visible)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::Rect;
    use crate::ports::mocks::MockScreenSource;

    /// Escritorio 4x4 en (0,0), canal R = índice 0..16.
    fn source_4x4() -> MockScreenSource {
        let pixels: Vec<u8> = (0..16u8).flat_map(|i| [i, 0, 0, 255]).collect();
        MockScreenSource::new((0, 0), Frame::new(4, 4, pixels).unwrap())
    }

    #[test]
    fn captura_el_rect_de_la_ventana_activa() {
        let mut source = source_4x4();
        source.set_active_window(Some(Rect::new(1, 1, 2, 2)));

        let frame = ActiveWindowMode.capture(&mut source).unwrap();

        assert_eq!((frame.width, frame.height), (2, 2));
        // Píxel (0,0) del recorte = escritorio (1,1) = índice 5.
        assert_eq!(frame.pixel(0, 0), Some([5, 0, 0, 255]));
    }

    #[test]
    fn recorta_la_ventana_que_asoma_fuera_del_escritorio() {
        let mut source = source_4x4();
        // Asoma 2 px por la izquierda y 1 por arriba.
        source.set_active_window(Some(Rect::new(-2, -1, 4, 3)));

        let frame = ActiveWindowMode.capture(&mut source).unwrap();

        // Solo la parte visible: (0,0)-(2,2).
        assert_eq!((frame.width, frame.height), (2, 2));
        assert_eq!(source.requests(), &[Rect::new(0, 0, 2, 2)]);
    }

    #[test]
    fn sin_ventana_activa_devuelve_nothing_to_capture() {
        let mut source = source_4x4();
        let err = ActiveWindowMode.capture(&mut source).unwrap_err();
        assert!(matches!(err, CaptureError::NothingToCapture(_)));
    }

    #[test]
    fn ventana_totalmente_fuera_del_escritorio_devuelve_nothing_to_capture() {
        let mut source = source_4x4();
        source.set_active_window(Some(Rect::new(100, 100, 2, 2)));
        let err = ActiveWindowMode.capture(&mut source).unwrap_err();
        assert!(matches!(err, CaptureError::NothingToCapture(_)));
    }
}
