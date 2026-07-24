//! Pantalla completa (f.9): todo el escritorio virtual.

use crate::capture::{CaptureError, CaptureMode};
use crate::ports::{Frame, ScreenSource};

/// Captura el escritorio virtual completo, multi-monitor incluido.
pub struct FullscreenMode;

impl CaptureMode for FullscreenMode {
    fn capture(&self, source: &mut dyn ScreenSource) -> Result<Frame, CaptureError> {
        let rect = source.desktop_rect();
        Ok(source.capture_region(rect)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::Rect;
    use crate::ports::mocks::MockScreenSource;

    #[test]
    fn captura_todo_el_escritorio_virtual() {
        // Origen negativo: monitor a la izquierda del primario.
        let pixels: Vec<u8> = (0..4u8).flat_map(|i| [i, 0, 0, 255]).collect();
        let mut source = MockScreenSource::new((-1, -1), Frame::new(2, 2, pixels).unwrap());

        let frame = FullscreenMode.capture(&mut source).unwrap();

        assert_eq!((frame.width, frame.height), (2, 2));
        assert_eq!(frame.pixel(0, 0), Some([0, 0, 0, 255]));
        assert_eq!(frame.pixel(1, 1), Some([3, 0, 0, 255]));
        // Pidió exactamente el rect del escritorio.
        assert_eq!(source.requests(), &[Rect::new(-1, -1, 2, 2)]);
    }
}
