//! Adapters Win32 que implementan los puertos del core (D2): GDI, WGC,
//! DXGI, Media Foundation, WASAPI, hotkeys, WIA.
//!
//! Todo el código `unsafe` y el interop `windows-rs` del proyecto vive
//! encapsulado en este crate.

pub mod dpi;
pub mod gdi;
pub mod pixels;
