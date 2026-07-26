//! Overlay de selección de región (capa de selección de D10): frame
//! congelado + máscara blanca 50 %, arrastre limpio, crosshair y lupa.
//!
//! Hilos: SOLO desde el hilo de UI (bucle modal anidado, como los
//! menús). El estado lo posee `select_region`; el wndproc lo usa vía
//! puntero crudo y nunca lo libera.

pub(crate) mod math;

use rustcapture_core::ports::{Rect, ScreenSource};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, COLORONCOLOR, DT_NOPREFIX, DrawTextW, EndPaint, GetMonitorInfoW, HDC,
    InvalidateRect, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, PAINTSTRUCT,
    SRCCOPY, SelectObject, SetBkMode, SetStretchBltMode, SetTextColor, StretchBlt, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use crate::dpi::Escala;
use crate::gdi::GdiScreenSource;
use crate::gdi::raii::{Dib, MemDc, ScreenDc, Selected};
use crate::ui::{fuentes, lienzo, theme, ventana};
use crate::util::punto;

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
    /// Última zona pintada de la caja de lupa: invalidación mínima.
    zona_lupa_previa: Option<RECT>,
    /// Última selección pintada: invalidación mínima durante el arrastre.
    sel_previa: Option<Rect>,
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

use crate::gdi::dib_from_frame;

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
        zona_lupa_previa: None,
        sel_previa: None,
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
    // El dueño (select_region) no toca el Box mientras el bucle despacha.
    ventana::estado::<OverlayState>(hwnd)
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: cada rama documenta su invariante; el estado nunca se
    // libera aquí (lo posee select_region).
    unsafe {
        match msg {
            WM_NCCREATE => {
                ventana::adoptar_estado(hwnd, lparam);
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
                    // Invalidación mínima: la escena es estática salvo la
                    // caja de lupa y la selección; se invalida la unión
                    // de sus zonas vieja y nueva (BeginPaint recorta los
                    // blits del back buffer a esa región).
                    let escala = Escala::from_hwnd(hwnd);
                    let lupa_previa = state.zona_lupa_previa;
                    let sel_previa = state.sel_previa;
                    state.cursor = punto(lparam);
                    let lupa = zona_lupa(state, escala);
                    let sel = state.drag_start.map(|s| math::rect_between(s, state.cursor));
                    for zona in [lupa_previa, Some(lupa)].into_iter().flatten() {
                        _ = InvalidateRect(Some(hwnd), Some(&zona), false);
                    }
                    for s in [sel_previa, sel].into_iter().flatten() {
                        // +2: cubre el marco de 1 px pintado alrededor.
                        let zona = RECT {
                            left: s.x - 2,
                            top: s.y - 2,
                            right: s.x + s.width as i32 + 2,
                            bottom: s.y + s.height as i32 + 2,
                        };
                        _ = InvalidateRect(Some(hwnd), Some(&zona), false);
                    }
                    state.zona_lupa_previa = Some(lupa);
                    state.sel_previa = sel;
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
                    _ = pintar(hdc, state, Escala::from_hwnd(hwnd));
                }
                _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// Compone la escena en el back buffer y la vuelca de un BitBlt.
fn pintar(hdc: HDC, state: &mut OverlayState, escala: Escala) -> windows::core::Result<()> {
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
            let marco = RECT {
                left: sel.x - 1,
                top: sel.y - 1,
                right: sel.x + sel.width as i32 + 1,
                bottom: sel.y + sel.height as i32 + 1,
            };
            lienzo::marco(back_dc.0, &marco, theme::actual().paleta().acento);
        }
        // 3. Caja de lupa.
        pintar_lupa(&back_dc, &src_dc, state, seleccion, escala)?;
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

/// Monitor bajo el cursor, en coordenadas locales de la ventana.
fn monitor_local(state: &OverlayState) -> Rect {
    let cursor_desktop = POINT {
        x: state.cursor.0 + state.origin.0,
        y: state.cursor.1 + state.origin.1,
    };
    // SAFETY: consultas de monitor sin precondiciones.
    unsafe {
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
    }
}

/// Zona (px físicos, coordenadas locales) de la caja de la lupa para el
/// cursor actual: rejilla de zoom + bloque de información debajo.
fn zona_lupa(state: &OverlayState, escala: Escala) -> RECT {
    let celda = escala.px(math::LUPA_CELDA).max(1);
    let zoom = math::LUPA_SRC * celda;
    let info = escala.px(math::LUPA_INFO_H);
    let (bx, by) = math::lupa_box_pos(
        monitor_local(state),
        state.cursor,
        (zoom, zoom + info),
        escala.px(math::LUPA_OFFSET),
    );
    RECT { left: bx, top: by, right: bx + zoom, bottom: by + zoom + info }
}

/// Lupa V3: caja compacta junto al cursor (flip en bordes) con zoom
/// 21×21 a ~6×, píxel central marcado en acento y dos líneas de info:
/// `#RRGGBB · X, Y` y `sel W × H` durante el arrastre.
fn pintar_lupa(
    back_dc: &MemDc,
    src_dc: &MemDc,
    state: &OverlayState,
    seleccion: Option<Rect>,
    escala: Escala,
) -> windows::core::Result<()> {
    let paleta = theme::actual().paleta();
    let celda = escala.px(math::LUPA_CELDA).max(1);
    let zoom = math::LUPA_SRC * celda;
    let caja = zona_lupa(state, escala);
    let (bx, by) = (caja.left, caja.top);
    let fuente = math::lupa_source(state.cursor, state.width as u32, state.height as u32);

    // SAFETY: DCs vivos; dibujo GDI estándar con brochas propias
    // liberadas en la misma función.
    unsafe {
        // Rejilla de zoom desde el frame original (píxeles nítidos).
        {
            let _o = Selected::bitmap(src_dc, &state.original)?;
            SetStretchBltMode(back_dc.0, COLORONCOLOR);
            _ = StretchBlt(
                back_dc.0,
                bx,
                by,
                zoom,
                zoom,
                Some(src_dc.0),
                fuente.x,
                fuente.y,
                math::LUPA_SRC,
                math::LUPA_SRC,
                SRCCOPY,
            );
        }
        // Píxel del cursor recuadrado en acento (doble marco = 2 px).
        let px = bx + (state.cursor.0 - fuente.x) * celda;
        let py = by + (state.cursor.1 - fuente.y) * celda;
        for inflado in [1, 0] {
            let recuadro = RECT {
                left: px - inflado,
                top: py - inflado,
                right: px + celda + inflado,
                bottom: py + celda + inflado,
            };
            lienzo::marco(back_dc.0, &recuadro, paleta.acento);
        }

        // Bloque de información: hex + coordenadas y tamaño de selección.
        let info_rect = RECT {
            left: bx,
            top: by + zoom,
            right: caja.right,
            bottom: caja.bottom,
        };
        lienzo::rellenar(back_dc.0, &info_rect, paleta.superficie);

        let (b, g, r) = pixel_bajo_el_cursor(state);
        let linea1 = format!(
            "{} · {}, {}",
            math::hex_de_bgra(b, g, r),
            state.cursor.0 + state.origin.0,
            state.cursor.1 + state.origin.1
        );
        let linea2 = seleccion
            .map(|s| format!("sel {} × {}", s.width, s.height))
            .unwrap_or_default();

        SetBkMode(back_dc.0, TRANSPARENT);
        SetTextColor(back_dc.0, paleta.texto);
        let mono = fuentes::fuente(fuentes::Rol::Mono, escala);
        let fuente_previa = SelectObject(back_dc.0, mono.into());
        let margen = escala.px(6);
        let alto_linea = (info_rect.bottom - info_rect.top) / 2;
        for (i, texto) in [linea1, linea2].iter().enumerate() {
            if texto.is_empty() {
                continue;
            }
            let mut wide: Vec<u16> = texto.encode_utf16().collect();
            let mut rc = RECT {
                left: info_rect.left + margen,
                top: info_rect.top + margen / 2 + i as i32 * alto_linea,
                right: info_rect.right - margen,
                bottom: info_rect.bottom,
            };
            DrawTextW(back_dc.0, &mut wide, &mut rc, DT_NOPREFIX);
        }
        SelectObject(back_dc.0, fuente_previa);

        // Marco exterior de la caja completa.
        lienzo::marco(back_dc.0, &caja, paleta.borde);
    }
    Ok(())
}

/// BGRA del píxel del frame congelado bajo el cursor.
fn pixel_bajo_el_cursor(state: &OverlayState) -> (u8, u8, u8) {
    let x = state.cursor.0.clamp(0, state.width - 1) as usize;
    let y = state.cursor.1.clamp(0, state.height - 1) as usize;
    let bits = state.original.bits();
    let i = (y * state.width as usize + x) * 4;
    match bits.get(i..i + 3) {
        Some([b, g, r]) => (*b, *g, *r),
        _ => (0, 0, 0),
    }
}
