//! Overlay de selección de región (capa de selección de D10): frame
//! congelado + máscara blanca 50 %, arrastre limpio, crosshair y lupa.
//!
//! Hilos: SOLO desde el hilo de UI (bucle modal anidado, como los
//! menús). El estado lo posee `select_region`; el wndproc lo usa vía
//! puntero crudo y nunca lo libera.

pub(crate) mod math;

use rustcapture_core::ports::{Frame, Rect, ScreenSource};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CLIP_DEFAULT_PRECIS, COLOR_BTNFACE, COLORONCOLOR, CreateFontW,
    CreateSolidBrush, DEFAULT_CHARSET, DEFAULT_PITCH, DEFAULT_QUALITY, DT_CENTER, DeleteObject,
    DrawTextW, EndPaint, FW_BOLD, FillRect, FrameRect, GetMonitorInfoW, GetSysColorBrush, HDC,
    InvalidateRect, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, OUT_DEFAULT_PRECIS,
    PAINTSTRUCT, SRCCOPY, SelectObject, SetBkMode, SetStretchBltMode, SetTextColor, StretchBlt,
    TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use crate::gdi::GdiScreenSource;
use crate::gdi::raii::{Dib, MemDc, ScreenDc, Selected};

/// Estado del overlay; lo posee `select_region`, el wndproc solo lo usa.
struct OverlayState {
    origin: (i32, i32),
    width: i32,
    height: i32,
    original: Dib,
    whitened: Dib,
    back: Dib,
    drag_start: Option<(i32, i32)>,
    cursor: (i32, i32),
    /// Crosshair negro propio: el IDC_CROSS estándar (XOR) se pierde
    /// sobre la máscara clara.
    cursor_cruz: Option<HCURSOR>,
    /// None = en curso; Some(None) = cancelado; Some(Some(r)) = elegido
    /// (coordenadas LOCALES; `select_region` las traduce a escritorio).
    outcome: Option<Option<Rect>>,
}

/// Selección interactiva. Bloquea el hilo de UI hasta soltar el botón o
/// Esc. `None` = cancelado o fallo al congelar (con beep).
pub fn select_region() -> Option<Rect> {
    match run() {
        Ok(resultado) => resultado,
        Err(_) => {
            crate::alerts::error_beep();
            None
        }
    }
}

/// Crosshair monocromo 32×32 negro con el punto origen (centro) en
/// blanco, hotspot en el centro. AND=0 & XOR=0 → negro; AND=0 & XOR=1 →
/// blanco; AND=1 & XOR=0 → transparente.
fn crear_cursor_cruz() -> Option<HCURSOR> {
    let mut and_mask = [0xFFu8; 128];
    let mut xor_mask = [0x00u8; 128];
    for b in 0..4 {
        and_mask[16 * 4 + b] = 0x00; // línea horizontal (fila 16)
    }
    for fila in 0..32 {
        and_mask[fila * 4 + 2] &= !0x80; // línea vertical (columna 16)
    }
    xor_mask[16 * 4 + 2] |= 0x80; // punto origen (16,16) en blanco
    // SAFETY: máscaras de 128 bytes exactos para 32×32 monocromo.
    unsafe {
        CreateCursor(
            None,
            16,
            16,
            32,
            32,
            and_mask.as_ptr().cast(),
            xor_mask.as_ptr().cast(),
        )
        .ok()
    }
}

fn dib_from_frame(dc: &MemDc, frame: &Frame) -> windows::core::Result<Dib> {
    let mut dib = Dib::new_32bpp(dc, frame.width, frame.height)?;
    let mut px = frame.pixels.clone();
    crate::pixels::rgba_to_bgra(&mut px);
    dib.bits_mut().copy_from_slice(&px);
    Ok(dib)
}

fn run() -> windows::core::Result<Option<Rect>> {
    // Congelar el escritorio completo.
    let mut source = GdiScreenSource::new();
    let desktop = GdiScreenSource::desktop_rect(&source);
    let frozen = ScreenSource::capture_region(&mut source, desktop).map_err(|e| {
        windows::core::Error::new(windows::Win32::Foundation::E_FAIL, e.to_string())
    })?;
    let mut blanqueado = frozen.clone();
    crate::pixels::whiten_half(&mut blanqueado.pixels);

    // Bitmaps GDI (original, máscara y back buffer del doble buffer).
    let screen = ScreenDc::get()?;
    let dc = MemDc::compatible_with(&screen)?;
    let state = Box::new(OverlayState {
        origin: (desktop.x, desktop.y),
        width: desktop.width as i32,
        height: desktop.height as i32,
        original: dib_from_frame(&dc, &frozen)?,
        whitened: dib_from_frame(&dc, &blanqueado)?,
        back: Dib::new_32bpp(&dc, desktop.width, desktop.height)?,
        drag_start: None,
        cursor: (0, 0),
        cursor_cruz: crear_cursor_cruz(),
        outcome: None,
    });
    drop(dc);
    drop(screen);
    let state_ptr = Box::into_raw(state);

    // SAFETY: creación de ventana estándar; state_ptr vive hasta el
    // Box::from_raw del final, siempre después de destruir la ventana.
    let resultado = unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = w!("RustCaptureOverlay");
        // Sin cursor de clase: WM_SETCURSOR pone el crosshair negro.
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST,
            class,
            w!("RustCapture Selección"),
            WS_POPUP | WS_VISIBLE,
            desktop.x,
            desktop.y,
            desktop.width as i32,
            desktop.height as i32,
            None,
            None,
            Some(instance.into()),
            Some(state_ptr.cast()),
        )?;
        _ = SetForegroundWindow(hwnd); // necesario para recibir Esc

        // Bucle modal: consume mensajes hasta que el wndproc resuelva.
        let mut msg = MSG::default();
        while (*state_ptr).outcome.is_none() && GetMessageW(&mut msg, None, 0, 0).as_bool() {
            _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let state = Box::from_raw(state_ptr);
        if let Some(cruz) = state.cursor_cruz {
            // SAFETY: la ventana ya fue destruida; el cursor no está en uso.
            _ = DestroyCursor(cruz);
        }
        let origin = state.origin;
        state.outcome.flatten().map(|local| {
            Rect::new(
                local.x + origin.0,
                local.y + origin.1,
                local.width,
                local.height,
            )
        })
    };
    Ok(resultado)
}

fn state_mut<'a>(hwnd: HWND) -> Option<&'a mut OverlayState> {
    // SAFETY: puntero puesto por WM_NCCREATE; el dueño (select_region)
    // no lo toca mientras el bucle despacha mensajes.
    unsafe { ((GetWindowLongPtrW(hwnd, GWLP_USERDATA)) as *mut OverlayState).as_mut() }
}

fn punto(lparam: LPARAM) -> (i32, i32) {
    (
        (lparam.0 & 0xFFFF) as i16 as i32,
        ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
    )
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: cada rama documenta su invariante; el estado nunca se
    // libera aquí (lo posee select_region).
    unsafe {
        match msg {
            WM_NCCREATE => {
                let cs = &*(lparam.0 as *const CREATESTRUCTW);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_ERASEBKGND => LRESULT(1), // el doble buffer pinta todo
            WM_SETCURSOR => {
                if let Some(cruz) = state_mut(hwnd).and_then(|s| s.cursor_cruz) {
                    SetCursor(Some(cruz));
                    return LRESULT(1);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_LBUTTONDOWN => {
                if let Some(state) = state_mut(hwnd) {
                    state.drag_start = Some(punto(lparam));
                    state.cursor = punto(lparam);
                    SetCapture(hwnd);
                    _ = InvalidateRect(Some(hwnd), None, false);
                }
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                if let Some(state) = state_mut(hwnd) {
                    state.cursor = punto(lparam);
                    _ = InvalidateRect(Some(hwnd), None, false);
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                if let Some(state) = state_mut(hwnd)
                    && let Some(start) = state.drag_start
                {
                    _ = ReleaseCapture();
                    state.outcome = Some(Some(math::rect_between(start, punto(lparam))));
                    _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            WM_KEYDOWN if wparam.0 as u16 == VK_ESCAPE.0 => {
                if let Some(state) = state_mut(hwnd) {
                    state.outcome = Some(None);
                    _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                if let Some(state) = state_mut(hwnd) {
                    _ = pintar(hdc, state);
                }
                _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// Compone la escena en el back buffer y la vuelca de un BitBlt.
fn pintar(hdc: HDC, state: &mut OverlayState) -> windows::core::Result<()> {
    let screen = ScreenDc::get()?;
    let back_dc = MemDc::compatible_with(&screen)?;
    let src_dc = MemDc::compatible_with(&screen)?;
    let _back = Selected::bitmap(&back_dc, &state.back)?;

    // SAFETY: DCs y bitmaps vivos (RAII); operaciones GDI estándar.
    unsafe {
        // 1. Máscara blanca en todo el escritorio.
        {
            let _s = Selected::bitmap(&src_dc, &state.whitened)?;
            BitBlt(
                back_dc.0,
                0,
                0,
                state.width,
                state.height,
                Some(src_dc.0),
                0,
                0,
                SRCCOPY,
            )?;
        }
        // 2. Región seleccionada limpia + borde rojo.
        let seleccion = state
            .drag_start
            .map(|s| math::rect_between(s, state.cursor));
        if let Some(sel) = seleccion {
            let _o = Selected::bitmap(&src_dc, &state.original)?;
            BitBlt(
                back_dc.0,
                sel.x,
                sel.y,
                sel.width as i32,
                sel.height as i32,
                Some(src_dc.0),
                sel.x,
                sel.y,
                SRCCOPY,
            )?;
            let rojo = CreateSolidBrush(COLORREF(0x0000FF));
            let marco = RECT {
                left: sel.x - 1,
                top: sel.y - 1,
                right: sel.x + sel.width as i32 + 1,
                bottom: sel.y + sel.height as i32 + 1,
            };
            FrameRect(back_dc.0, &marco, rojo);
            _ = DeleteObject(rojo.into());
        }
        // 3. Caja de lupa.
        pintar_lupa(&back_dc, &src_dc, state, seleccion)?;
        // 4. Volcado único a pantalla.
        BitBlt(
            hdc,
            0,
            0,
            state.width,
            state.height,
            Some(back_dc.0),
            0,
            0,
            SRCCOPY,
        )?;
    }
    Ok(())
}

/// Caja 300×500: zoom 5× (300×300) + coordenadas (300×30) + ayuda (300×170).
fn pintar_lupa(
    back_dc: &MemDc,
    src_dc: &MemDc,
    state: &OverlayState,
    seleccion: Option<Rect>,
) -> windows::core::Result<()> {
    // Monitor del cursor, en coordenadas locales de la ventana.
    let cursor_desktop = POINT {
        x: state.cursor.0 + state.origin.0,
        y: state.cursor.1 + state.origin.1,
    };
    // SAFETY: consultas de monitor sin precondiciones.
    let monitor_local = unsafe {
        let monitor = MonitorFromPoint(cursor_desktop, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            Rect::new(
                info.rcMonitor.left - state.origin.0,
                info.rcMonitor.top - state.origin.1,
                (info.rcMonitor.right - info.rcMonitor.left).max(0) as u32,
                (info.rcMonitor.bottom - info.rcMonitor.top).max(0) as u32,
            )
        } else {
            Rect::new(0, 0, state.width as u32, state.height as u32)
        }
    };
    let (bx, by) = math::lupa_box_pos(monitor_local, state.cursor);
    let fuente = math::lupa_source(state.cursor, state.width as u32, state.height as u32);

    // SAFETY: DCs vivos; dibujo GDI estándar.
    unsafe {
        // Zoom 5× desde el frame original.
        {
            let _o = Selected::bitmap(src_dc, &state.original)?;
            SetStretchBltMode(back_dc.0, COLORONCOLOR);
            _ = StretchBlt(
                back_dc.0,
                bx,
                by,
                math::LUPA_W,
                math::LUPA_ZOOM_H,
                Some(src_dc.0),
                fuente.x,
                fuente.y,
                math::LUPA_SRC,
                math::LUPA_SRC,
                SRCCOPY,
            );
        }
        // Cruz roja de 1 px centrada en el bloque 5×5 del píxel del
        // cursor (coincide con el centro salvo clamping en bordes), y el
        // propio píxel origen pintado en blanco.
        const ZOOM: i32 = 5;
        let px = bx + (state.cursor.0 - fuente.x) * ZOOM;
        let py = by + (state.cursor.1 - fuente.y) * ZOOM;
        let rojo = CreateSolidBrush(COLORREF(0x0000FF));
        FillRect(
            back_dc.0,
            &RECT {
                left: bx,
                top: py + 2,
                right: bx + math::LUPA_W,
                bottom: py + 3,
            },
            rojo,
        );
        FillRect(
            back_dc.0,
            &RECT {
                left: px + 2,
                top: by,
                right: px + 3,
                bottom: by + math::LUPA_ZOOM_H,
            },
            rojo,
        );
        _ = DeleteObject(rojo.into());
        let blanco = CreateSolidBrush(COLORREF(0x00FFFFFF));
        FillRect(
            back_dc.0,
            &RECT {
                left: px,
                top: py,
                right: px + ZOOM,
                bottom: py + ZOOM,
            },
            blanco,
        );
        _ = DeleteObject(blanco.into());

        // Barra de coordenadas (gris claro).
        let coord_rect = RECT {
            left: bx,
            top: by + math::LUPA_ZOOM_H,
            right: bx + math::LUPA_W,
            bottom: by + math::LUPA_ZOOM_H + math::LUPA_COORD_H,
        };
        FillRect(back_dc.0, &coord_rect, GetSysColorBrush(COLOR_BTNFACE));
        SetBkMode(back_dc.0, TRANSPARENT);
        // Coordenadas en morado y negrita.
        SetTextColor(back_dc.0, COLORREF(0x00800080));
        let negrita = CreateFontW(
            22,
            0,
            0,
            0,
            FW_BOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            DEFAULT_QUALITY,
            DEFAULT_PITCH.0 as u32,
            w!("Segoe UI"),
        );
        let fuente_anterior = SelectObject(back_dc.0, negrita.into());
        let mut coords: Vec<u16> = format!("X, Y = {},{}", cursor_desktop.x, cursor_desktop.y)
            .encode_utf16()
            .collect();
        let mut rc = coord_rect;
        rc.top += 3;
        DrawTextW(back_dc.0, &mut coords, &mut rc, DT_CENTER);
        SelectObject(back_dc.0, fuente_anterior);
        _ = DeleteObject(negrita.into());

        // Bloque de ayuda (azul RGB(30,80,160), texto blanco).
        let ayuda_rect = RECT {
            left: bx,
            top: by + math::LUPA_ZOOM_H + math::LUPA_COORD_H,
            right: bx + math::LUPA_W,
            bottom: by + math::LUPA_H,
        };
        let azul = CreateSolidBrush(COLORREF(0x00A0501E));
        FillRect(back_dc.0, &ayuda_rect, azul);
        _ = DeleteObject(azul.into());
        SetTextColor(back_dc.0, COLORREF(0x00FFFFFF));
        let ayuda = match seleccion {
            Some(sel) => format!(
                "Arrastra para seleccionar\nSuelta para capturar\nESC para cancelar\n\nSelección: {}×{} px",
                sel.width, sel.height
            ),
            None => {
                "Arrastra para seleccionar\nSuelta para capturar\nESC para cancelar".to_string()
            }
        };
        let mut ayuda: Vec<u16> = ayuda.encode_utf16().collect();
        let mut rc = ayuda_rect;
        rc.top += 24;
        DrawTextW(back_dc.0, &mut ayuda, &mut rc, DT_CENTER);

        // Marco exterior de la caja completa.
        let caja = RECT {
            left: bx,
            top: by,
            right: bx + math::LUPA_W,
            bottom: by + math::LUPA_H,
        };
        let negro = CreateSolidBrush(COLORREF(0x000000));
        FrameRect(back_dc.0, &caja, negro);
        _ = DeleteObject(negro.into());
    }
    Ok(())
}
