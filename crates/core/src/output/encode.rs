//! Codificación de `Frame` a formatos de imagen (f.45 parcial: PNG y
//! JPEG; el resto de formatos llega en F3).

use crate::ports::Frame;

/// Formatos de salida de imagen fija soportados por el MVP.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImageFormat {
    Png,
    /// Calidad fija 90 hasta que la config (D9) la parametrice.
    Jpeg,
}

impl ImageFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
        }
    }
}

#[derive(thiserror::Error, Clone, PartialEq, Eq, Debug)]
pub enum EncodeError {
    #[error("codificación {0} fallida: {1}")]
    Encoding(&'static str, String),
}

/// Codifica el frame RGBA al formato pedido.
pub fn encode(frame: &Frame, format: ImageFormat) -> Result<Vec<u8>, EncodeError> {
    match format {
        ImageFormat::Png => {
            let mut bytes = Vec::new();
            let mut encoder = png::Encoder::new(&mut bytes, frame.width, frame.height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder
                .write_header()
                .map_err(|e| EncodeError::Encoding("png", e.to_string()))?;
            writer
                .write_image_data(&frame.pixels)
                .map_err(|e| EncodeError::Encoding("png", e.to_string()))?;
            drop(writer);
            Ok(bytes)
        }
        ImageFormat::Jpeg => {
            let mut bytes = Vec::new();
            let encoder = jpeg_encoder::Encoder::new(&mut bytes, 90);
            encoder
                .encode(
                    &frame.pixels,
                    frame.width as u16,
                    frame.height as u16,
                    jpeg_encoder::ColorType::Rgba,
                )
                .map_err(|e| EncodeError::Encoding("jpeg", e.to_string()))?;
            Ok(bytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_2x2() -> Frame {
        let pixels: Vec<u8> = (0..4u8).flat_map(|i| [i * 60, 10, 200, 255]).collect();
        Frame::new(2, 2, pixels).unwrap()
    }

    #[test]
    fn png_empieza_con_la_firma_y_se_decodifica_igual() {
        let bytes = encode(&frame_2x2(), ImageFormat::Png).unwrap();
        assert_eq!(
            &bytes[..8],
            &[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']
        );

        // Ida y vuelta con el decoder del propio crate `png`.
        let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buf).unwrap();
        assert_eq!((info.width, info.height), (2, 2));
        assert_eq!(&buf[..info.buffer_size()], frame_2x2().pixels.as_slice());
    }

    #[test]
    fn jpeg_lleva_marcadores_soi_y_eoi() {
        let bytes = encode(&frame_2x2(), ImageFormat::Jpeg).unwrap();
        assert_eq!(&bytes[..2], &[0xFF, 0xD8]); // SOI
        assert_eq!(&bytes[bytes.len() - 2..], &[0xFF, 0xD9]); // EOI
    }

    #[test]
    fn las_extensiones_son_las_esperadas() {
        assert_eq!(ImageFormat::Png.extension(), "png");
        assert_eq!(ImageFormat::Jpeg.extension(), "jpg");
    }
}
