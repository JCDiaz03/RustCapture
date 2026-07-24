//! Adapter GDI del puerto `ScreenSource` (D2): BitBlt sobre un DIB de
//! 32 bits. Elegido para el MVP frente a WGC por simplicidad y soporte
//! universal en Windows 10; WGC llegará como adapter alternativo.
//!
//! Hilos: `GdiScreenSource` no es `Send` a propósito — los HDC que crea
//! `capture_region` viven y mueren dentro de la llamada, pero el uso
//! previsto es un único hilo orquestador.

pub(crate) mod raii;

use rustcapture_core::ports::{Frame, Rect, ScreenSource, ScreenSourceError};
use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CAPTUREBLT, GdiFlush, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MonitorFromPoint, SRCCOPY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetForegroundWindow, GetSystemMetrics, GetWindowRect, SM_CXVIRTUALSCREEN,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};
use windows::core::Result as WinResult;

/// Vuelca un `Frame` RGBA del core a un DIB BGRA seleccionable en GDI.
/// Compartido por el overlay y el editor.
pub(crate) fn dib_from_frame(dc: &raii::MemDc, frame: &Frame) -> windows::core::Result<raii::Dib> {
    let mut dib = raii::Dib::new_32bpp(dc, frame.width, frame.height)?;
    let mut px = frame.pixels.clone();
    crate::pixels::rgba_to_bgra(&mut px);
    dib.bits_mut().copy_from_slice(&px);
    Ok(dib)
}

/// `ScreenSource` real sobre GDI. Sin estado entre capturas: cada
/// `capture_region` crea y destruye sus recursos (RAII).
pub struct GdiScreenSource;

impl GdiScreenSource {
    #[expect(
        clippy::new_without_default,
        reason = "constructor con futuro estado (config WGC)"
    )]
    pub fn new() -> Self {
        Self
    }

    /// Rect del escritorio virtual en píxeles físicos (per-monitor V2).
    pub fn desktop_rect(&self) -> Rect {
        // SAFETY: GetSystemMetrics no tiene precondiciones; devuelve 0
        // para métricas desconocidas (escritorio degenerado → rect vacío,
        // que el core rechaza como OutOfBounds).
        unsafe {
            let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let w = GetSystemMetrics(SM_CXVIRTUALSCREEN).max(0) as u32;
            let h = GetSystemMetrics(SM_CYVIRTUALSCREEN).max(0) as u32;
            Rect::new(x, y, w, h)
        }
    }

    /// Rect del monitor que contiene el cursor (f.9: "pantalla completa"
    /// es la pantalla del usuario). Si algo falla, cae al escritorio.
    pub fn active_monitor_rect(&self) -> Rect {
        // SAFETY: consultas sin precondiciones; ante cualquier fallo se
        // devuelve el escritorio virtual completo.
        unsafe {
            let mut pt = POINT::default();
            if GetCursorPos(&mut pt).is_err() {
                return GdiScreenSource::desktop_rect(self);
            }
            let monitor = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
            let mut info = MONITORINFO {
                cbSize: size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(monitor, &mut info).as_bool() {
                let r = info.rcMonitor;
                Rect::new(
                    r.left,
                    r.top,
                    r.right.saturating_sub(r.left).max(0) as u32,
                    r.bottom.saturating_sub(r.top).max(0) as u32,
                )
            } else {
                GdiScreenSource::desktop_rect(self)
            }
        }
    }

    /// Rect de la ventana en primer plano. DWM da el marco visible real
    /// (`DWMWA_EXTENDED_FRAME_BOUNDS`); si DWM falla, `GetWindowRect`
    /// (incluye bordes invisibles). Errores → `None`: para el dominio,
    /// "no hay ventana capturable".
    pub fn active_window_rect(&self) -> Option<Rect> {
        // SAFETY: GetForegroundWindow no tiene precondiciones; NULL si
        // no hay ventana en primer plano.
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.is_invalid() {
            return None;
        }
        let mut rect = RECT::default();
        // SAFETY: hwnd es una ventana válida ahora mismo (puede morir en
        // paralelo: en ese caso las APIs fallan y devolvemos None).
        let via_dwm = unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut rect as *mut RECT as *mut core::ffi::c_void,
                size_of::<RECT>() as u32,
            )
        }
        .is_ok();
        if !via_dwm {
            // SAFETY: mismas precondiciones que arriba.
            unsafe { GetWindowRect(hwnd, &mut rect) }.ok()?;
        }
        let width = rect.right.saturating_sub(rect.left).max(0) as u32;
        let height = rect.bottom.saturating_sub(rect.top).max(0) as u32;
        Some(Rect::new(rect.left, rect.top, width, height))
    }

    /// Captura real. Errores Win32 quedan en `windows::core::Result`;
    /// la frontera del puerto los aplana a `Platform(String)`.
    fn grab(&self, region: Rect) -> WinResult<Frame> {
        let screen = raii::ScreenDc::get()?;
        let mem = raii::MemDc::compatible_with(&screen)?;
        let dib = raii::Dib::new_32bpp(&mem, region.width, region.height)?;
        let _selected = raii::Selected::bitmap(&mem, &dib)?;
        // SAFETY: ambos DC son válidos (RAII vivos); CAPTUREBLT incluye
        // ventanas por capas (tooltips, popups).
        unsafe {
            BitBlt(
                mem.0,
                0,
                0,
                region.width as i32,
                region.height as i32,
                Some(screen.0),
                region.x,
                region.y,
                SRCCOPY | CAPTUREBLT,
            )?;
            // SAFETY: fuerza a GDI a terminar antes de leer los bits.
            _ = GdiFlush();
        }
        let mut pixels = dib.bits().to_vec();
        crate::pixels::bgra_to_rgba_opaque(&mut pixels);
        Frame::new(region.width, region.height, pixels).map_err(|e| {
            windows::core::Error::new(windows::Win32::Foundation::E_FAIL, e.to_string())
        })
    }
}

impl ScreenSource for GdiScreenSource {
    fn desktop_rect(&self) -> Rect {
        GdiScreenSource::desktop_rect(self)
    }

    fn active_monitor_rect(&self) -> Rect {
        GdiScreenSource::active_monitor_rect(self)
    }

    fn active_window_rect(&self) -> Option<Rect> {
        GdiScreenSource::active_window_rect(self)
    }

    fn capture_region(&mut self, region: Rect) -> Result<Frame, ScreenSourceError> {
        if region.is_empty() || !GdiScreenSource::desktop_rect(self).contains(&region) {
            return Err(ScreenSourceError::OutOfBounds(region));
        }
        self.grab(region)
            .map_err(|e| ScreenSourceError::Platform(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcapture_core::ports::{ScreenSource, ScreenSourceError};

    /// No toca GDI: la validación de límites es previa a toda captura.
    /// Ejecutable sin `--ignored` (no exige sesión gráfica, solo métricas).
    #[test]
    fn una_region_absurda_devuelve_out_of_bounds_sin_capturar() {
        let mut source = GdiScreenSource::new();
        let region = Rect::new(i32::MIN, i32::MIN, 1, 1);
        assert_eq!(
            source.capture_region(region).unwrap_err(),
            ScreenSourceError::OutOfBounds(region)
        );
    }

    /// Humo: captura real de la esquina del escritorio.
    #[test]
    #[ignore = "requiere escritorio real"]
    fn captura_una_region_pequena_con_dimensiones_y_alfa_correctos() {
        crate::dpi::ensure_per_monitor_dpi_awareness();
        let mut source = GdiScreenSource::new();
        let desktop = source.desktop_rect();
        let region = Rect::new(desktop.x, desktop.y, 8, 8);
        let frame = source.capture_region(region).unwrap();
        assert_eq!((frame.width, frame.height), (8, 8));
        // Alfa forzado a opaco en la conversión BGRA→RGBA.
        assert!(frame.pixels.chunks_exact(4).all(|px| px[3] == 255));
    }

    /// Humo: el monitor activo cabe dentro del escritorio virtual.
    #[test]
    #[ignore = "requiere escritorio real"]
    fn el_monitor_activo_esta_dentro_del_escritorio() {
        crate::dpi::ensure_per_monitor_dpi_awareness();
        let source = GdiScreenSource::new();
        let monitor = source.active_monitor_rect();
        assert!(!monitor.is_empty());
        assert!(source.desktop_rect().contains(&monitor));
    }

    /// Humo: exige sesión gráfica real. Ejecutar con
    /// `cargo test -p platform-win -- --ignored`.
    #[test]
    #[ignore = "requiere escritorio real"]
    fn el_escritorio_virtual_tiene_area() {
        crate::dpi::ensure_per_monitor_dpi_awareness();
        let source = GdiScreenSource::new();
        let rect = source.desktop_rect();
        assert!(rect.width > 0 && rect.height > 0);
    }

    /// Humo: en una sesión interactiva casi siempre hay ventana activa;
    /// si la hay, su rect interseca el escritorio.
    #[test]
    #[ignore = "requiere escritorio real"]
    fn la_ventana_activa_si_existe_esta_en_el_escritorio() {
        crate::dpi::ensure_per_monitor_dpi_awareness();
        let source = GdiScreenSource::new();
        if let Some(win) = source.active_window_rect() {
            assert!(source.desktop_rect().intersection(&win).is_some());
        }
    }
}
