//! Editor shell (f.21, Slice A de F3): la captura aterriza aquí. Ventana
//! con toolbar mínima (Guardar como / Copiar / Draw / Cerrar) y la
//! imagen encajada en el lienzo. Referencia visual: Ventana1.PNG.
//!
//! Hilos: `EditorSink::deliver` corre en el hilo orquestador y SOLO
//! publica un mensaje; `show_editor` corre en el hilo de UI (bucle
//! modal, patrón del overlay).

pub(crate) mod math;

use std::sync::atomic::{AtomicBool, Ordering};

use rustcapture_core::output::{ImageFormat, encode};
use rustcapture_core::ports::{Frame, OutputError, OutputSink};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, COLOR_APPWORKSPACE, COLOR_BTNSHADOW, EndPaint, FillRect, GetSysColorBrush,
    HALFTONE, HDC, InvalidateRect, PAINTSTRUCT, SRCCOPY, SetStretchBltMode, StretchBlt,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::Dialogs::{GetSaveFileNameW, OFN_OVERWRITEPROMPT, OPENFILENAMEW};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use crate::gdi::dib_from_frame;
use crate::gdi::raii::{Dib, MemDc, ScreenDc, Selected};

/// Mensaje al wndproc de la barra: wparam = `Box<Frame>` crudo, el
/// receptor toma posesión SIEMPRE (también si decide no abrir).
pub(crate) const WM_APP_EDITOR: u32 = WM_APP + 3;

/// Un editor cada vez (MVP sin tabs). Lo consulta el sink (hilo
/// orquestador) y lo mantiene `show_editor` (hilo UI).
static EDITOR_ABIERTO: AtomicBool = AtomicBool::new(false);

const TOOLBAR_H: i32 = 40;
const ID_GUARDAR: u16 = 3001;
const ID_COPIAR: u16 = 3002;
const ID_DRAW: u16 = 3003;
const ID_CERRAR: u16 = 3004;

/// `OutputSink` que entrega la captura al editor del hilo de UI.
pub struct EditorSink {
    bar_hwnd_raw: isize,
}

impl EditorSink {
    pub fn new(bar_hwnd_raw: isize) -> Self {
        Self { bar_hwnd_raw }
    }
}

impl OutputSink for EditorSink {
    fn id(&self) -> &'static str {
        "editor"
    }

    fn deliver(&mut self, frame: &Frame) -> Result<(), OutputError> {
        if EDITOR_ABIERTO.load(Ordering::SeqCst) {
            return Err(OutputError::Failed("editor ocupado".to_string()));
        }
        let boxed = Box::into_raw(Box::new(frame.clone()));
        // SAFETY: PostMessageW es seguro entre hilos; si falla, se
        // recupera el Box aquí mismo (sin fuga).
        let posted = unsafe {
            PostMessageW(
                Some(HWND(self.bar_hwnd_raw as *mut _)),
                WM_APP_EDITOR,
                WPARAM(boxed as usize),
                LPARAM(0),
            )
        };
        if posted.is_err() {
            // SAFETY: el mensaje no se publicó; el puntero sigue siendo nuestro.
            unsafe { drop(Box::from_raw(boxed)) };
            return Err(OutputError::Failed(
                "la barra no está disponible".to_string(),
            ));
        }
        Ok(())
    }
}

struct EditorState {
    frame: Frame,
    dib: Dib,
    cerrado: bool,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Abre el editor con la captura y bloquea el hilo de UI hasta cerrarlo.
pub fn show_editor(frame: Frame) {
    EDITOR_ABIERTO.store(true, Ordering::SeqCst);
    if let Err(e) = run(frame) {
        crate::alerts::error_box("RustCapture Editor", &e.to_string());
    }
    EDITOR_ABIERTO.store(false, Ordering::SeqCst);
}

fn run(frame: Frame) -> windows::core::Result<()> {
    let screen = ScreenDc::get()?;
    let dc = MemDc::compatible_with(&screen)?;
    let dib = dib_from_frame(&dc, &frame)?;
    drop(dc);
    drop(screen);

    let titulo = wide(&format!(
        "RustCapture Editor — {}×{}",
        frame.width, frame.height
    ));
    // Tamaño inicial: imagen + toolbar, acotado a un máximo razonable.
    let win_w = (frame.width as i32 + 60).clamp(520, 1280);
    let win_h = (frame.height as i32 + TOOLBAR_H + 100).clamp(360, 840);

    let state_ptr = Box::into_raw(Box::new(EditorState {
        frame,
        dib,
        cerrado: false,
    }));

    // SAFETY: patrón del overlay: el estado lo posee esta función; la
    // ventana se destruye antes del Box::from_raw.
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = w!("RustCaptureEditor");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: GetSysColorBrush(COLOR_APPWORKSPACE),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            PCWSTR(titulo.as_ptr()),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            win_w,
            win_h,
            None,
            None,
            Some(instance.into()),
            Some(state_ptr.cast()),
        )?;
        _ = SetForegroundWindow(hwnd);

        let mut msg = MSG::default();
        while !(*state_ptr).cerrado && GetMessageW(&mut msg, None, 0, 0).as_bool() {
            _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        drop(Box::from_raw(state_ptr));
    }
    Ok(())
}

fn state_mut<'a>(hwnd: HWND) -> Option<&'a mut EditorState> {
    // SAFETY: puntero puesto por WM_NCCREATE; liberado solo tras el
    // bucle modal (nunca aquí).
    unsafe { ((GetWindowLongPtrW(hwnd, GWLP_USERDATA)) as *mut EditorState).as_mut() }
}

fn crear_toolbar(hwnd: HWND) {
    let botones: [(u16, PCWSTR, bool, i32, i32); 4] = [
        (ID_GUARDAR, w!("Guardar como…"), true, 10, 120),
        (ID_COPIAR, w!("Copiar"), true, 140, 90),
        (ID_DRAW, w!("Draw"), false, 240, 90),
        (ID_CERRAR, w!("Cerrar"), true, 340, 90),
    ];
    for (id, texto, habilitado, x, ancho) in botones {
        // SAFETY: padre válido durante WM_CREATE; el sistema destruye
        // los hijos con la ventana.
        unsafe {
            if let Ok(btn) = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                texto,
                WS_CHILD | WS_VISIBLE,
                x,
                6,
                ancho,
                26,
                Some(hwnd),
                Some(HMENU(id as usize as *mut _)),
                None,
                None,
            ) {
                _ = EnableWindow(btn, habilitado);
            }
        }
    }
}

/// Diálogo "Guardar como" → codifica y escribe. Errores → MessageBox.
fn guardar_como(hwnd: HWND, frame: &Frame) {
    let mut buffer = [0u16; 260];
    let inicial: Vec<u16> = "captura".encode_utf16().collect();
    buffer[..inicial.len()].copy_from_slice(&inicial);
    let filtro: Vec<u16> = "PNG (*.png)\0*.png\0JPEG (*.jpg)\0*.jpg\0\0"
        .encode_utf16()
        .collect();
    let mut ofn = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: hwnd,
        lpstrFilter: PCWSTR(filtro.as_ptr()),
        nFilterIndex: 1,
        lpstrFile: windows::core::PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        Flags: OFN_OVERWRITEPROMPT,
        ..Default::default()
    };
    // SAFETY: struct completo con punteros a buffers locales vivos.
    let aceptado = unsafe { GetSaveFileNameW(&mut ofn) }.as_bool();
    if !aceptado {
        return; // canceló
    }
    let fin = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    let mut ruta = String::from_utf16_lossy(&buffer[..fin]);
    let format = if ofn.nFilterIndex == 2 {
        ImageFormat::Jpeg
    } else {
        ImageFormat::Png
    };
    let ext = format.extension();
    let minuscula = ruta.to_ascii_lowercase();
    let tiene_extension =
        minuscula.ends_with(&format!(".{ext}")) || (ext == "jpg" && minuscula.ends_with(".jpeg"));
    if !tiene_extension {
        ruta.push('.');
        ruta.push_str(ext);
    }
    let resultado = encode(frame, format)
        .map_err(|e| e.to_string())
        .and_then(|bytes| std::fs::write(&ruta, bytes).map_err(|e| e.to_string()));
    match resultado {
        Ok(()) => crate::alerts::capture_beep(),
        Err(e) => crate::alerts::error_box("RustCapture Editor", &format!("{ruta}: {e}")),
    }
}

fn on_command(hwnd: HWND, id: u16) {
    match id {
        ID_GUARDAR => {
            if let Some(state) = state_mut(hwnd) {
                guardar_como(hwnd, &state.frame);
            }
        }
        ID_COPIAR => {
            if let Some(state) = state_mut(hwnd) {
                match crate::clipboard::ClipboardSink::new().deliver(&state.frame) {
                    Ok(()) => crate::alerts::capture_beep(),
                    Err(_) => crate::alerts::error_beep(),
                }
            }
        }
        ID_CERRAR => {
            // Sin ediciones posibles todavía: cierre silencioso (spec).
            // SAFETY: destruir la propia ventana desde su wndproc.
            unsafe { _ = DestroyWindow(hwnd) };
        }
        _ => {}
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: cada rama documenta su invariante; el estado se libera en
    // `run`, nunca aquí.
    unsafe {
        match msg {
            WM_NCCREATE => {
                let cs = &*(lparam.0 as *const CREATESTRUCTW);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CREATE => {
                crear_toolbar(hwnd);
                LRESULT(0)
            }
            WM_COMMAND => {
                on_command(hwnd, (wparam.0 & 0xFFFF) as u16);
                LRESULT(0)
            }
            WM_SIZE => {
                _ = InvalidateRect(Some(hwnd), None, true);
                LRESULT(0)
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                if let Some(state) = state_mut(hwnd) {
                    _ = pintar(hwnd, hdc, state);
                }
                _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_DESTROY => {
                if let Some(state) = state_mut(hwnd) {
                    state.cerrado = true;
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// Pinta la imagen encajada bajo la toolbar.
fn pintar(hwnd: HWND, hdc: HDC, state: &EditorState) -> windows::core::Result<()> {
    // SAFETY: hdc de BeginPaint; consultas de rect sin precondiciones.
    unsafe {
        let mut client = RECT::default();
        _ = GetClientRect(hwnd, &mut client);
        let lienzo = (
            client.right - client.left,
            client.bottom - client.top - TOOLBAR_H,
        );
        let destino = math::fit_rect((state.frame.width, state.frame.height), lienzo);
        if destino.is_empty() {
            return Ok(());
        }
        // Separador de la toolbar.
        FillRect(
            hdc,
            &RECT {
                left: 0,
                top: TOOLBAR_H - 2,
                right: client.right,
                bottom: TOOLBAR_H,
            },
            GetSysColorBrush(COLOR_BTNSHADOW),
        );
        let screen = ScreenDc::get()?;
        let src_dc = MemDc::compatible_with(&screen)?;
        let _s = Selected::bitmap(&src_dc, &state.dib)?;
        SetStretchBltMode(hdc, HALFTONE);
        _ = StretchBlt(
            hdc,
            destino.x,
            destino.y + TOOLBAR_H,
            destino.width as i32,
            destino.height as i32,
            Some(src_dc.0),
            0,
            0,
            state.frame.width as i32,
            state.frame.height as i32,
            SRCCOPY,
        );
    }
    Ok(())
}
