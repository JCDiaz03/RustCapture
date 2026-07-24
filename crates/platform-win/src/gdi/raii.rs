//! Envoltorios RAII de recursos GDI. Internos al adapter: ningún tipo
//! de `windows` sale de este crate.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, GetDC, HBITMAP, HDC, HGDIOBJ, ReleaseDC, SelectObject,
};
use windows::core::{Error, Result};

/// DC de pantalla (`GetDC(None)`); se libera con `ReleaseDC`.
pub(crate) struct ScreenDc(pub(crate) HDC);

impl ScreenDc {
    pub(crate) fn get() -> Result<Self> {
        // SAFETY: GetDC(None) pide el DC del escritorio; no hay
        // precondiciones. NULL indica fallo.
        let dc = unsafe { GetDC(Some(HWND::default())) };
        if dc.is_invalid() {
            return Err(Error::from_thread());
        }
        Ok(Self(dc))
    }
}

impl Drop for ScreenDc {
    fn drop(&mut self) {
        // SAFETY: el HDC fue obtenido con GetDC(None) y no se ha liberado.
        unsafe { ReleaseDC(Some(HWND::default()), self.0) };
    }
}

/// DC de memoria compatible; se libera con `DeleteDC`.
pub(crate) struct MemDc(pub(crate) HDC);

impl MemDc {
    pub(crate) fn compatible_with(screen: &ScreenDc) -> Result<Self> {
        // SAFETY: el HDC de origen es válido mientras viva `screen`.
        let dc = unsafe { CreateCompatibleDC(Some(screen.0)) };
        if dc.is_invalid() {
            return Err(Error::from_thread());
        }
        Ok(Self(dc))
    }
}

impl Drop for MemDc {
    fn drop(&mut self) {
        // SAFETY: el HDC fue creado con CreateCompatibleDC.
        unsafe { _ = DeleteDC(self.0) };
    }
}

/// DIB de 32 bits top-down: los bits viven en memoria del proceso.
pub(crate) struct Dib {
    pub(crate) bitmap: HBITMAP,
    bits: *const u8,
    len: usize,
}

impl Dib {
    pub(crate) fn new_32bpp(dc: &MemDc, width: u32, height: u32) -> Result<Self> {
        let header = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            // Negativo = top-down: la fila 0 es la de arriba.
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };
        let info = BITMAPINFO {
            bmiHeader: header,
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = core::ptr::null_mut();
        // SAFETY: `info` describe un DIB válido y `bits` recibe el puntero
        // al buffer que posee el propio HBITMAP.
        let bitmap =
            unsafe { CreateDIBSection(Some(dc.0), &info, DIB_RGB_COLORS, &mut bits, None, 0)? };
        Ok(Self {
            bitmap,
            bits: bits as *const u8,
            len: width as usize * height as usize * 4,
        })
    }

    /// Bits BGRA del bitmap. Llamar tras `GdiFlush` para que GDI haya
    /// terminado de escribir.
    pub(crate) fn bits(&self) -> &[u8] {
        // SAFETY: el buffer pertenece al HBITMAP vivo (self) y mide
        // exactamente `len` bytes (32 bpp * w * h).
        unsafe { core::slice::from_raw_parts(self.bits, self.len) }
    }
}

impl Drop for Dib {
    fn drop(&mut self) {
        // SAFETY: el HBITMAP fue creado con CreateDIBSection.
        unsafe { _ = DeleteObject(self.bitmap.into()) };
    }
}

/// Selección temporal de un objeto en un DC; restaura el anterior en Drop.
pub(crate) struct Selected<'a> {
    dc: &'a MemDc,
    old: HGDIOBJ,
}

impl<'a> Selected<'a> {
    pub(crate) fn bitmap(dc: &'a MemDc, dib: &Dib) -> Result<Self> {
        // SAFETY: DC y bitmap son válidos (RAII vivos).
        let old = unsafe { SelectObject(dc.0, dib.bitmap.into()) };
        if old.is_invalid() {
            return Err(Error::from_thread());
        }
        Ok(Self { dc, old })
    }
}

impl Drop for Selected<'_> {
    fn drop(&mut self) {
        // SAFETY: restaura el objeto que este mismo guard desplazó.
        unsafe { SelectObject(self.dc.0, self.old) };
    }
}
