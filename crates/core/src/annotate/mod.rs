//! Slice de anotación: documento de objetos `Annotation`, render sobre
//! `Canvas` RGBA (D5) y commands con undo/redo (D6). Features f.20-f.31.

pub mod annotations;
mod canvas;
mod censor;
mod document;
pub mod formato;
mod giro;
mod objeto;
mod shapes;
mod style;
mod text;

pub use canvas::Canvas;
pub use document::{Command, Document, History};
pub use giro::Giro;
pub use objeto::{Forma, Objeto};
pub use style::{CensorMode, Color, FamiliaId, Style, TextStyle};
pub use text::RenderContext;
