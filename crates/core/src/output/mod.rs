//! Slice de salidas: sinks, generación automática de nombres y formatos
//! (features f.40-f.45).

pub mod contenedor;
mod destination;
mod encode;
mod file_sink;
mod naming;

pub use destination::DestinationKind;
pub use encode::{EncodeError, ImageFormat, encode};
pub use file_sink::FileSink;
pub use naming::auto_name;
