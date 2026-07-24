//! Sink de portapapeles (f.40): publica la captura como CF_DIB
//! (BITMAPINFOHEADER 32 bpp bottom-up + BGRA), el formato de imagen
//! más aceptado por las aplicaciones Windows.
//!
//! Hilos: el portapapeles es global; abrir/escribir/cerrar sucede
//! íntegro dentro de `deliver`, sin estado retenido.

use rustcapture_core::ports::{Frame, OutputError, OutputSink};
use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use windows::Win32::Graphics::Gdi::{BI_RGB, BITMAPINFOHEADER};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_DIB;
use windows::core::{Error, Result as WinResult};

/// Guard RAII: portapapeles abierto y vaciado; se cierra en `Drop`.
struct ClipboardGuard;

impl ClipboardGuard {
    fn open() -> WinResult<Self> {
        // SAFETY: OpenClipboard(None) lo asocia a la tarea actual; sin
        // precondiciones de memoria.
        unsafe { OpenClipboard(None)? };
        let guard = Self; // a partir de aquí, Drop garantiza el cierre
        // SAFETY: el portapapeles está abierto por este hilo.
        unsafe { EmptyClipboard()? };
        Ok(guard)
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        // SAFETY: emparejado con el OpenClipboard de `open`.
        unsafe { _ = CloseClipboard() };
    }
}

/// `OutputSink` real del portapapeles.
pub struct ClipboardSink;

impl ClipboardSink {
    #[expect(clippy::new_without_default, reason = "simetría con GdiScreenSource")]
    pub fn new() -> Self {
        Self
    }

    fn put_dib(&self, frame: &Frame) -> WinResult<()> {
        // Preparación pura, fuera de todo unsafe.
        let mut pixels = crate::pixels::rows_bottom_up(&frame.pixels, frame.width, frame.height);
        crate::pixels::rgba_to_bgra(&mut pixels);
        let header = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: frame.width as i32,
            // Positivo = bottom-up, el convenio clásico de CF_DIB.
            biHeight: frame.height as i32,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };
        let total = size_of::<BITMAPINFOHEADER>() + pixels.len();

        let _guard = ClipboardGuard::open()?;
        // SAFETY: GMEM_MOVEABLE es obligatorio para SetClipboardData.
        let hglobal: HGLOBAL = unsafe { GlobalAlloc(GMEM_MOVEABLE, total)? };
        // SAFETY: hglobal recién asignado con `total` bytes; se copia
        // header y después los píxeles, sin salirse del bloque.
        unsafe {
            let ptr = GlobalLock(hglobal) as *mut u8;
            if ptr.is_null() {
                let err = Error::from_thread();
                _ = GlobalFree(Some(hglobal));
                return Err(err);
            }
            core::ptr::copy_nonoverlapping(
                (&raw const header).cast::<u8>(),
                ptr,
                size_of::<BITMAPINFOHEADER>(),
            );
            core::ptr::copy_nonoverlapping(
                pixels.as_ptr(),
                ptr.add(size_of::<BITMAPINFOHEADER>()),
                pixels.len(),
            );
            _ = GlobalUnlock(hglobal);
        }
        // SAFETY: hglobal es válido; si SetClipboardData acepta, el
        // sistema pasa a poseer la memoria y NO debe liberarse.
        match unsafe { SetClipboardData(CF_DIB.0 as u32, Some(HANDLE(hglobal.0))) } {
            Ok(_) => Ok(()),
            Err(e) => {
                // SAFETY: el sistema rechazó el handle; sigue siendo nuestro.
                unsafe { _ = GlobalFree(Some(hglobal)) };
                Err(e)
            }
        }
    }
}

impl OutputSink for ClipboardSink {
    fn id(&self) -> &'static str {
        "clipboard"
    }

    fn deliver(&mut self, frame: &Frame) -> Result<(), OutputError> {
        self.put_dib(frame)
            .map_err(|e| OutputError::Failed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Humo: SOBRESCRIBE el portapapeles del usuario. Ejecutar a mano con
    /// `cargo test -p platform-win -- --ignored`.
    #[test]
    #[ignore = "toca el portapapeles real"]
    fn deliver_publica_cf_dib_disponible() {
        use windows::Win32::System::DataExchange::IsClipboardFormatAvailable;

        let mut sink = ClipboardSink::new();
        sink.deliver(&Frame::filled(2, 2, [255, 0, 0, 255]))
            .unwrap();
        // SAFETY: consulta sin estado; no requiere el portapapeles abierto.
        let disponible = unsafe { IsClipboardFormatAvailable(CF_DIB.0 as u32) };
        assert!(disponible.is_ok());
    }
}
