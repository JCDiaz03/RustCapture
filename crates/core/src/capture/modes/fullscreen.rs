//! Pantalla completa (f.9): el monitor activo — el que contiene el
//! cursor. Con un monitor equivale al escritorio; con varios, captura
//! solo la pantalla del usuario (feedback de verificación manual).

use crate::capture::{CaptureError, CaptureMode};
use crate::ports::{Frame, ScreenSource};

/// Captura el monitor activo completo.
pub struct FullscreenMode;

impl CaptureMode for FullscreenMode {
    fn capture(&self, source: &mut dyn ScreenSource) -> Result<Frame, CaptureError> {
        let rect = source.active_monitor_rect();
        Ok(source.capture_region(rect)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::Rect;
    use crate::ports::mocks::MockScreenSource;

    #[test]
    fn con_un_solo_monitor_captura_todo_el_escritorio() {
        // Origen negativo: monitor a la izquierda del primario.
        let pixels: Vec<u8> = (0..4u8).flat_map(|i| [i, 0, 0, 255]).collect();
        let mut source = MockScreenSource::new((-1, -1), Frame::new(2, 2, pixels).unwrap());

        let frame = FullscreenMode.capture(&mut source).unwrap();

        assert_eq!((frame.width, frame.height), (2, 2));
        assert_eq!(frame.pixel(0, 0), Some([0, 0, 0, 255]));
        assert_eq!(frame.pixel(1, 1), Some([3, 0, 0, 255]));
        // Sin monitor activo configurado, el mock lo iguala al escritorio.
        assert_eq!(source.requests(), &[Rect::new(-1, -1, 2, 2)]);
    }

    #[test]
    fn con_varios_monitores_captura_solo_el_activo() {
        // Escritorio 4x1 = dos "monitores" de 2x1; el activo es el derecho.
        let pixels: Vec<u8> = (0..4u8).flat_map(|i| [i, 0, 0, 255]).collect();
        let mut source = MockScreenSource::new((0, 0), Frame::new(4, 1, pixels).unwrap());
        source.set_active_monitor(Some(Rect::new(2, 0, 2, 1)));

        let frame = FullscreenMode.capture(&mut source).unwrap();

        assert_eq!((frame.width, frame.height), (2, 1));
        assert_eq!(frame.pixel(0, 0), Some([2, 0, 0, 255]));
        assert_eq!(source.requests(), &[Rect::new(2, 0, 2, 1)]);
    }
}
