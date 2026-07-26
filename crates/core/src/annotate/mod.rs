//! Slice de anotación: documento de objetos `Annotation`, render sobre
//! `Canvas` RGBA (D5) y commands con undo/redo (D6). Features f.20-f.31.

pub mod annotations;
mod canvas;
mod censor;
mod document;
mod objeto;
mod shapes;
mod style;
mod text;

pub use canvas::Canvas;
pub use document::{Command, Document, History};
pub use objeto::Objeto;
pub use style::{CensorMode, Color, Style, TextStyle};
pub use text::RenderContext;
