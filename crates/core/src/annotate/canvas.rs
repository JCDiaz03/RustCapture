//! Canvas (D5): única puerta de escritura de píxeles del motor. Envuelve
//! un `Frame` RGBA — da igual captura fija o fotograma de vídeo.

use crate::annotate::style::Color;
use crate::ports::Frame;

/// Envuelve el frame y mezcla píxeles con alfa (src-over). El frame de
/// salida se mantiene opaco (las capturas lo son).
pub struct Canvas<'a> {
    frame: &'a mut Frame,
}

impl<'a> Canvas<'a> {
    pub fn new(frame: &'a mut Frame) -> Self {
        Self { frame }
    }

    pub fn width(&self) -> u32 {
        self.frame.width
    }

    pub fn height(&self) -> u32 {
        self.frame.height
    }

    /// Lee el píxel YA compuesto (base + anotaciones anteriores); `None`
    /// fuera de rango. Lo necesitan las anotaciones que censuran lo que
    /// tienen debajo (pixelado/desenfoque): ven el z-order, no la base.
    pub fn pixel(&self, x: i32, y: i32) -> Option<Color> {
        if x < 0 || y < 0 || x as u32 >= self.frame.width || y as u32 >= self.frame.height {
            return None;
        }
        let i = (y as usize * self.frame.width as usize + x as usize) * 4;
        let px = &self.frame.pixels[i..i + 4];
        Some(Color::rgba(px[0], px[1], px[2], px[3]))
    }

    /// Mezcla `color` sobre el píxel; fuera de rango no hace nada.
    pub fn blend_pixel(&mut self, x: i32, y: i32, color: Color) {
        if x < 0 || y < 0 || x as u32 >= self.frame.width || y as u32 >= self.frame.height {
            return;
        }
        let i = (y as usize * self.frame.width as usize + x as usize) * 4;
        let a = color.a as u32;
        let px = &mut self.frame.pixels[i..i + 4];
        px[0] = ((color.r as u32 * a + px[0] as u32 * (255 - a)) / 255) as u8;
        px[1] = ((color.g as u32 * a + px[1] as u32 * (255 - a)) / 255) as u8;
        px[2] = ((color.b as u32 * a + px[2] as u32 * (255 - a)) / 255) as u8;
        px[3] = 255;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaco_sustituye_el_pixel() {
        let mut frame = Frame::filled(2, 2, [10, 10, 10, 255]);
        let mut canvas = Canvas::new(&mut frame);
        canvas.blend_pixel(1, 0, Color::rgb(200, 0, 0));
        assert_eq!(frame.pixel(1, 0), Some([200, 0, 0, 255]));
        assert_eq!(frame.pixel(0, 0), Some([10, 10, 10, 255]));
    }

    #[test]
    fn semitransparente_mezcla_src_over() {
        let mut frame = Frame::filled(1, 1, [0, 0, 0, 255]);
        let mut canvas = Canvas::new(&mut frame);
        canvas.blend_pixel(0, 0, Color::rgba(255, 255, 255, 128));
        let [r, g, b, a] = frame.pixel(0, 0).unwrap();
        assert!((127..=129).contains(&r) && r == g && g == b);
        assert_eq!(a, 255);
    }

    #[test]
    fn pixel_lee_lo_ya_compuesto_y_fuera_de_rango_es_none() {
        let mut frame = Frame::filled(2, 2, [10, 20, 30, 255]);
        let mut canvas = Canvas::new(&mut frame);
        assert_eq!(canvas.pixel(1, 1), Some(Color::rgba(10, 20, 30, 255)));
        // Lo que se escribe se vuelve a leer: la censura ve el z-order.
        canvas.blend_pixel(0, 0, Color::rgb(200, 0, 0));
        assert_eq!(canvas.pixel(0, 0), Some(Color::rgba(200, 0, 0, 255)));
        assert_eq!(canvas.pixel(-1, 0), None);
        assert_eq!(canvas.pixel(0, 2), None);
    }

    #[test]
    fn fuera_de_rango_es_noop() {
        let mut frame = Frame::filled(2, 2, [9, 9, 9, 255]);
        let mut canvas = Canvas::new(&mut frame);
        canvas.blend_pixel(-1, 0, Color::rgb(1, 1, 1));
        canvas.blend_pixel(0, 99, Color::rgb(1, 1, 1));
        assert_eq!(frame, Frame::filled(2, 2, [9, 9, 9, 255]));
    }
}
