//! Puertos (D2): traits en las fronteras reales del dominio.
//!
//! Aquí viven `ScreenSource`, `OutputSink` y `HotkeyProvider` con sus
//! tipos de frontera y mocks de test. `VideoEncoder` se define en F5 (D8).

mod frame;
mod geometry;
mod hotkeys;
mod output_sink;
mod screen_source;

pub mod mocks;

pub use frame::{Frame, FrameError};
pub use geometry::Rect;
pub use hotkeys::{Hotkey, HotkeyError, HotkeyId, HotkeyProvider, KeyCode, Modifiers};
pub use output_sink::{OutputError, OutputSink};
pub use screen_source::{ScreenSource, ScreenSourceError};
