//! Back buffer RAII y brochas de un solo uso: el esqueleto común del
//! pintado sin parpadeo (componer todo en memoria, volcar de un BitBlt).

use windows::Win32::Foundation::{COLORREF, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateSolidBrush, DeleteObject, FillRect, FrameRect, HDC, HGDIOBJ, SRCCOPY,
    SelectObject,
};
use windows::core::Result;

use crate::gdi::raii::{Dib, MemDc, ScreenDc};

/// DIB del tamaño del cliente ya seleccionado en un DC de memoria.
/// En Drop restaura la selección y libera todo (orden de campos).
pub(crate) struct BackBuffer {
    // La pantalla vive para poder crear DCs fuente compatibles.
    pantalla: ScreenDc,
    dc: MemDc,
    // Solo keep-alive: el bitmap queda seleccionado en `dc`.
    _dib: Dib,
    anterior: HGDIOBJ,
}

impl BackBuffer {
    pub(crate) fn nuevo(ancho: i32, alto: i32) -> Result<Self> {
        let pantalla = ScreenDc::get()?;
        let dc = MemDc::compatible_with(&pantalla)?;
        let dib = Dib::new_32bpp(&dc, ancho.max(1) as u32, alto.max(1) as u32)?;
        // SAFETY: DC y bitmap recién creados y vivos (campos propios).
        let anterior = unsafe { SelectObject(dc.0, dib.bitmap.into()) };
        Ok(Self { pantalla, dc, _dib: dib, anterior })
    }

    /// DC de memoria con el back buffer seleccionado: pintar aquí.
    pub(crate) fn dc(&self) -> HDC {
        self.dc.0
    }

    /// DC fuente compatible para blitear otros DIBs sobre el buffer.
    pub(crate) fn dc_fuente(&self) -> Result<MemDc> {
        MemDc::compatible_with(&self.pantalla)
    }

    /// Vuelca el buffer completo al DC destino (el de BeginPaint).
    pub(crate) fn volcar(&self, destino: HDC, ancho: i32, alto: i32) {
        // SAFETY: ambos DCs vivos; blit estándar.
        unsafe { _ = BitBlt(destino, 0, 0, ancho, alto, Some(self.dc.0), 0, 0, SRCCOPY) };
    }
}

impl Drop for BackBuffer {
    fn drop(&mut self) {
        // SAFETY: restaura el bitmap que desplazó nuevo(); después caen
        // dib y dc por orden de campos.
        unsafe { SelectObject(self.dc.0, self.anterior) };
    }
}

/// FillRect con una brocha efímera del color dado.
pub(crate) fn rellenar(dc: HDC, rect: &RECT, color: COLORREF) {
    // SAFETY: brocha propia creada y liberada aquí mismo.
    unsafe {
        let brocha = CreateSolidBrush(color);
        FillRect(dc, rect, brocha);
        _ = DeleteObject(brocha.into());
    }
}

/// FrameRect (marco de 1 px) con una brocha efímera del color dado.
pub(crate) fn marco(dc: HDC, rect: &RECT, color: COLORREF) {
    // SAFETY: brocha propia creada y liberada aquí mismo.
    unsafe {
        let brocha = CreateSolidBrush(color);
        FrameRect(dc, rect, brocha);
        _ = DeleteObject(brocha.into());
    }
}
