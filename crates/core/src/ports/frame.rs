//! Frame RGBA8: la unidad de píxeles que cruza todos los puertos (D4, D5).

use super::Rect;

/// Imagen RGBA8 en memoria. `pixels.len() == width * height * 4`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(thiserror::Error, Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameError {
    #[error("buffer de {got} bytes; se esperaban {expected}")]
    SizeMismatch { expected: usize, got: usize },
    #[error("la región {0:?} se sale del frame")]
    OutOfBounds(Rect),
    #[error("el PNG no se puede leer o no es RGBA de 8 bits")]
    PngIlegible,
}

impl Frame {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, FrameError> {
        let expected = width as usize * height as usize * 4;
        if pixels.len() != expected {
            return Err(FrameError::SizeMismatch {
                expected,
                got: pixels.len(),
            });
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Frame uniforme del color dado; útil sobre todo en tests y mocks.
    pub fn filled(width: u32, height: u32, rgba: [u8; 4]) -> Self {
        let pixels = rgba.repeat(width as usize * height as usize);
        Self {
            width,
            height,
            pixels,
        }
    }

    /// Píxel en coordenadas locales, `None` fuera de rango.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        Some([
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ])
    }

    /// Decodifica un PNG RGBA8 a `Frame`. Lo usa la carga del formato
    /// re-editable (f.31); el core no abre el archivo, recibe sus bytes.
    pub fn from_png(bytes: &[u8]) -> Result<Frame, FrameError> {
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut reader = decoder.read_info().map_err(|_| FrameError::PngIlegible)?;
        let mut buffer = vec![0u8; reader.output_buffer_size().unwrap_or(0)];
        let info = reader
            .next_frame(&mut buffer)
            .map_err(|_| FrameError::PngIlegible)?;
        if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
            return Err(FrameError::PngIlegible);
        }
        buffer.truncate(info.buffer_size());
        Frame::new(info.width, info.height, buffer)
    }

    /// Copia la subregión `region` (coordenadas locales al frame, origen 0,0).
    pub fn crop(&self, region: &Rect) -> Result<Frame, FrameError> {
        let propio = Rect::new(0, 0, self.width, self.height);
        if region.is_empty() || !propio.contains(region) {
            return Err(FrameError::OutOfBounds(*region));
        }
        let mut pixels = Vec::with_capacity(region.width as usize * region.height as usize * 4);
        for fila in 0..region.height {
            let y = (region.y as u32 + fila) as usize;
            let inicio = (y * self.width as usize + region.x as usize) * 4;
            let fin = inicio + region.width as usize * 4;
            pixels.extend_from_slice(&self.pixels[inicio..fin]);
        }
        Ok(Frame {
            width: region.width,
            height: region.height,
            pixels,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_valida_la_longitud_del_buffer() {
        assert!(Frame::new(2, 2, vec![0; 16]).is_ok());
        let err = Frame::new(2, 2, vec![0; 15]).unwrap_err();
        assert_eq!(
            err,
            FrameError::SizeMismatch {
                expected: 16,
                got: 15
            }
        );
    }

    #[test]
    fn filled_crea_frame_uniforme_y_pixel_lo_lee() {
        let f = Frame::filled(3, 2, [10, 20, 30, 255]);
        assert_eq!(f.pixel(2, 1), Some([10, 20, 30, 255]));
        assert_eq!(f.pixel(3, 0), None); // fuera de rango
    }

    #[test]
    fn crop_extrae_la_subregion_correcta() {
        // Frame 4x1: píxeles distinguibles por su canal R = columna.
        let pixels: Vec<u8> = (0..4u8).flat_map(|c| [c, 0, 0, 255]).collect();
        let f = Frame::new(4, 1, pixels).unwrap();
        let sub = f.crop(&Rect::new(1, 0, 2, 1)).unwrap();
        assert_eq!((sub.width, sub.height), (2, 1));
        assert_eq!(sub.pixel(0, 0), Some([1, 0, 0, 255]));
        assert_eq!(sub.pixel(1, 0), Some([2, 0, 0, 255]));
    }

    #[test]
    fn crop_fuera_de_limites_falla() {
        let f = Frame::filled(4, 4, [0; 4]);
        let region = Rect::new(2, 2, 4, 4);
        assert_eq!(
            f.crop(&region).unwrap_err(),
            FrameError::OutOfBounds(region)
        );
        let negativa = Rect::new(-1, 0, 2, 2);
        assert_eq!(
            f.crop(&negativa).unwrap_err(),
            FrameError::OutOfBounds(negativa)
        );
    }
}
