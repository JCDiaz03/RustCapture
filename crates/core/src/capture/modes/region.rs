//! Región rectangular (f.13). El rect llega elegido por el usuario
//! (overlay, CLI); el modo solo lo ejecuta. Sin tamaño mínimo (f.19).

use crate::capture::{CaptureError, CaptureMode};
use crate::ports::{Frame, Rect, ScreenSource};

/// Captura un rect fijo en coordenadas de escritorio virtual.
pub struct RegionMode {
    region: Rect,
}

impl RegionMode {
    pub fn new(region: Rect) -> Self {
        Self { region }
    }
}

impl CaptureMode for RegionMode {
    fn capture(&self, source: &mut dyn ScreenSource) -> Result<Frame, CaptureError> {
        Ok(source.capture_region(self.region)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::ScreenSourceError;
    use crate::ports::mocks::MockScreenSource;

    fn source_4x4() -> MockScreenSource {
        let pixels: Vec<u8> = (0..16u8).flat_map(|i| [i, 0, 0, 255]).collect();
        MockScreenSource::new((0, 0), Frame::new(4, 4, pixels).unwrap())
    }

    #[test]
    fn captura_exactamente_la_region_pedida() {
        let mut source = source_4x4();
        let frame = RegionMode::new(Rect::new(2, 3, 1, 1))
            .capture(&mut source)
            .unwrap();
        assert_eq!((frame.width, frame.height), (1, 1));
        // Escritorio (2,3) = índice 14.
        assert_eq!(frame.pixel(0, 0), Some([14, 0, 0, 255]));
    }

    #[test]
    fn una_region_fuera_del_escritorio_propaga_el_error_del_puerto() {
        let mut source = source_4x4();
        let region = Rect::new(3, 3, 5, 5);
        let err = RegionMode::new(region).capture(&mut source).unwrap_err();
        assert_eq!(
            err,
            CaptureError::Source(ScreenSourceError::OutOfBounds(region))
        );
    }
}
