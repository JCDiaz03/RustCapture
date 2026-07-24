//! Strategies de captura (D4): un archivo por modo. Añadir un modo nuevo
//! es añadir un archivo aquí y un brazo en `capture::create_mode`.

mod active_window;
mod fullscreen;
mod region;

pub use active_window::ActiveWindowMode;
pub use fullscreen::FullscreenMode;
pub use region::RegionMode;
