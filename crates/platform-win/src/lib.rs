//! Adapters Win32 que implementan los puertos del core (D2): GDI, WGC,
//! DXGI, Media Foundation, WASAPI, hotkeys, WIA.
//!
//! Todo el código `unsafe` y el interop `windows-rs` del proyecto vive
//! encapsulado en este crate.

pub mod alerts;
pub mod bar;
pub mod clipboard;
pub mod dpi;
pub mod editor;
mod fuentes_ttf;
pub mod gdi;
pub mod hotkeys;
pub mod overlay;
pub mod pixels;
pub mod tray;
pub(crate) mod ui;
pub(crate) mod util;
