//! Tipos de estilo compartidos por todas las anotaciones (D5).

/// Color RGBA; la opacidad de la herramienta viaja en `a`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

/// Estilo de las herramientas geométricas.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Style {
    pub color: Color,
    /// Grosor del trazo en píxeles (mínimo efectivo: 1).
    pub thickness: u32,
}

/// Estilo del texto (f.22).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TextStyle {
    pub color: Color,
    /// Altura de la fuente en píxeles.
    pub size: f32,
    pub bold: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_es_opaco() {
        assert_eq!(Color::rgb(1, 2, 3), Color::rgba(1, 2, 3, 255));
    }
}
