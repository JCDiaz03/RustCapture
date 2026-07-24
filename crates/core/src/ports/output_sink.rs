//! Puerto de salida (D2): portapapeles, archivo, impresora, email...
//! El sink recibe el frame final ya compuesto; codificación de formato y
//! nombres automáticos (f.41, f.45) son responsabilidad del slice `output`.

use super::Frame;

#[derive(thiserror::Error, Clone, PartialEq, Eq, Debug)]
pub enum OutputError {
    /// El destino no está disponible (sin impresora, sin cliente de email...).
    #[error("destino no disponible: {0}")]
    Unavailable(String),
    /// La entrega empezó y falló (disco lleno, portapapeles bloqueado...).
    #[error("entrega fallida: {0}")]
    Failed(String),
}

pub trait OutputSink {
    /// Identificador estable ("clipboard", "file"...) para config y logs.
    fn id(&self) -> &'static str;

    /// Entrega el frame al destino.
    fn deliver(&mut self, frame: &Frame) -> Result<(), OutputError>;
}
