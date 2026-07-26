//! Tipos de estilo compartidos por todas las anotaciones (D5).

/// Color RGBA; la opacidad de la herramienta viaja en `a`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
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

    /// Blanco o negro — el que contraste con este color. Luminancia
    /// percibida ITU-R BT.601 en milésimas para no usar coma flotante.
    /// Sirve para pintar texto legible sobre un relleno (f.23).
    pub const fn contraste(&self) -> Color {
        let luz = 299 * self.r as u32 + 587 * self.g as u32 + 114 * self.b as u32;
        if luz > 140_000 {
            Color::rgb(0, 0, 0)
        } else {
            Color::rgb(255, 255, 255)
        }
    }
}

/// Estilo de las herramientas geométricas.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Style {
    pub color: Color,
    /// Grosor del trazo en píxeles (mínimo efectivo: 1).
    pub thickness: u32,
}

/// Estilo de censura del pixelado (f.25): las dos variantes recorren el
/// mismo camino (leer el canvas → reescribirlo), solo cambia el filtro.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum CensorMode {
    /// Celdas de `block`×`block` aplanadas a su color medio.
    Mosaic { block: u32 },
    /// Desenfoque de caja de radio `radius`.
    Blur { radius: u32 },
}

/// Familia tipográfica, como índice en el catálogo del `RenderContext`.
///
/// Es un `u16` y no un `String` a propósito: `TextStyle` viaja por valor por
/// todo el motor (`draw_text`, `text_ink_box`, `draw_text_rotado`, el número
/// de los pasos) y tiene que seguir siendo `Copy`. El nombre vive en el
/// catálogo, que será también quien lo resuelva al serializar (f.31).
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct FamiliaId(pub u16);

/// Estilo del texto (f.22).
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Debug)]
pub struct TextStyle {
    pub color: Color,
    /// Altura de la fuente en píxeles.
    pub size: f32,
    pub bold: bool,
    /// Familia tipográfica (f.54); `FamiliaId::default()` = la de respaldo.
    pub familia: FamiliaId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_es_opaco() {
        assert_eq!(Color::rgb(1, 2, 3), Color::rgba(1, 2, 3, 255));
    }

    #[test]
    fn el_contraste_es_negro_sobre_claro_y_blanco_sobre_oscuro() {
        assert_eq!(Color::rgb(255, 255, 255).contraste(), Color::rgb(0, 0, 0));
        assert_eq!(Color::rgb(0, 0, 0).contraste(), Color::rgb(255, 255, 255));
        // Acento del diseño (#D83B01): número blanco encima.
        assert_eq!(
            Color::rgb(0xD8, 0x3B, 0x01).contraste(),
            Color::rgb(255, 255, 255)
        );
        // Amarillo puro es claro: número negro.
        assert_eq!(Color::rgb(255, 255, 0).contraste(), Color::rgb(0, 0, 0));
    }
}
