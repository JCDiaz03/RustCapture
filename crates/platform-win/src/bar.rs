//! Barra flotante (f.1, D11): seis botones con el layout definitivo,
//! pantalla y ventana activos en F1. Solo produce eventos (D7).
//! No-activate: no roba el foco, así "ventana activa" es la correcta.
//!
//! Hilos: crear la barra y correr `run_message_loop` en el MISMO hilo
//! (el principal). El estado del wndproc vive en GWLP_USERDATA.

use std::sync::mpsc::Sender;

use rustcapture_core::orchestrator::{AppEvent, CaptureRequest, ModeRequest};
use rustcapture_core::ports::HotkeyId;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, COLOR_BTNFACE, COLOR_BTNTEXT, DFC_BUTTON, DFC_CAPTION, DFCS_BUTTONPUSH,
    DFCS_CAPTIONCLOSE, DFCS_PUSHED, DFCS_STATE, DT_SINGLELINE, DT_VCENTER, DrawFrameControl,
    DrawTextW, EndPaint, FillRect, GetSysColorBrush, PAINTSTRUCT, SetBkMode, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{DRAWITEMSTRUCT, ODS_SELECTED};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

/// Mensaje de callback del icono de bandeja (lo consume `tray`).
pub(crate) const WM_TRAY: u32 = WM_APP + 1;

const BTN_W: i32 = 78;
const BTN_H: i32 = 30;
const MARGIN: i32 = 6;
/// Cabecera propia: título + minimizar (ocultar) + cerrar (= Salir).
const HEADER_H: i32 = 24;
const HDR_BTN_W: i32 = 26;
const HDR_BTN_H: i32 = 18;

const ID_FULLSCREEN: u16 = 1001;
const ID_WINDOW: u16 = 1002;
const ID_REGION: u16 = 1003;
const ID_DELAY: u16 = 1004;
const ID_RECORD: u16 = 1005;
const ID_CONFIG: u16 = 1006;
const ID_MINIMIZE: u16 = 1007;
const ID_CLOSE: u16 = 1008;

pub(crate) const MENU_FULLSCREEN: u16 = 2001;
pub(crate) const MENU_WINDOW: u16 = 2002;
pub(crate) const MENU_TOGGLE: u16 = 2003;
pub(crate) const MENU_QUIT: u16 = 2004;

struct BarState {
    tx: Sender<AppEvent>,
    destination: &'static str,
}

/// Handle de la barra. No expone tipos de `windows` (D2/D11).
pub struct Bar {
    hwnd: HWND,
}

impl Bar {
    /// Crea y muestra la barra. `destination` = sink por defecto de la
    /// config ("clipboard"/"file").
    pub fn create(tx: Sender<AppEvent>, destination: &'static str) -> Result<Self, String> {
        Self::create_win32(tx, destination).map_err(|e| e.to_string())
    }

    fn create_win32(
        tx: Sender<AppEvent>,
        destination: &'static str,
    ) -> windows::core::Result<Self> {
        // SAFETY: registro de clase + creación de ventana estándar; el
        // Box de estado viaja como lpCreateParams y lo adopta WM_NCCREATE.
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class = w!("RustCaptureBar");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                lpszClassName: class,
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                hbrBackground: GetSysColorBrush(COLOR_BTNFACE),
                ..Default::default()
            };
            RegisterClassW(&wc); // 0 si ya estaba registrada: inofensivo
            let state = Box::into_raw(Box::new(BarState { tx, destination }));
            let width = 6 * BTN_W + 7 * MARGIN;
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class,
                w!("RustCapture"),
                WS_POPUP | WS_VISIBLE,
                40,
                40,
                width,
                HEADER_H + BTN_H + 2 * MARGIN,
                None,
                None,
                Some(instance.into()),
                Some(state.cast()),
            )?;
            Ok(Self { hwnd })
        }
    }

    /// HWND como entero opaco, para colgar el icono de bandeja.
    pub fn hwnd_raw(&self) -> isize {
        self.hwnd.0 as isize
    }
}

/// Bucle de mensajes del hilo UI. Los `WM_HOTKEY` (registrados con hwnd
/// nulo) se traducen aquí a eventos del bus.
pub fn run_message_loop(tx: &Sender<AppEvent>) {
    let mut msg = MSG::default();
    // SAFETY: bucle GetMessage estándar del hilo que posee las ventanas.
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_HOTKEY && msg.hwnd.is_invalid() {
                let _ = tx.send(AppEvent::HotkeyPressed(HotkeyId(msg.wParam.0 as u32)));
                continue;
            }
            _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn state_ref<'a>(hwnd: HWND) -> Option<&'a BarState> {
    // SAFETY: el puntero lo puso WM_NCCREATE desde un Box válido y solo
    // se libera en WM_NCDESTROY (después de todo uso).
    unsafe { ((GetWindowLongPtrW(hwnd, GWLP_USERDATA)) as *const BarState).as_ref() }
}

fn enviar_captura(hwnd: HWND, mode: ModeRequest) {
    if let Some(state) = state_ref(hwnd) {
        let _ = state.tx.send(AppEvent::CaptureRequested(CaptureRequest {
            mode,
            destination: state.destination,
        }));
    }
}

pub(crate) fn on_command(hwnd: HWND, id: u16) {
    match id {
        ID_FULLSCREEN | MENU_FULLSCREEN => enviar_captura(hwnd, ModeRequest::Fullscreen),
        ID_WINDOW | MENU_WINDOW => enviar_captura(hwnd, ModeRequest::ActiveWindow),
        MENU_TOGGLE => {
            // SAFETY: hwnd válido (viene del wndproc de esa ventana).
            unsafe {
                let visible = IsWindowVisible(hwnd).as_bool();
                _ = ShowWindow(hwnd, if visible { SW_HIDE } else { SW_SHOW });
            }
        }
        // Minimizar = ocultar; se recupera desde la bandeja.
        ID_MINIMIZE => {
            // SAFETY: hwnd válido (viene del wndproc de esa ventana).
            unsafe { _ = ShowWindow(hwnd, SW_HIDE) };
        }
        ID_CLOSE | MENU_QUIT => {
            // SAFETY: destruir la propia ventana dispara WM_DESTROY
            // (Shutdown + PostQuitMessage).
            unsafe { _ = DestroyWindow(hwnd) };
        }
        _ => {}
    }
}

fn crear_botones(hwnd: HWND) {
    // Cabecera: minimizar (ocultar a bandeja) y cerrar (= Salir).
    // Owner-draw: WM_DRAWITEM los pinta con DrawFrameControl, los
    // glifos de caption del propio sistema — centrados a cualquier
    // tamaño, sin depender de fuentes.
    let ancho_total = 6 * BTN_W + 7 * MARGIN;
    let cabecera: [(u16, i32); 2] = [
        (ID_MINIMIZE, ancho_total - 2 * (HDR_BTN_W + 3)),
        (ID_CLOSE, ancho_total - (HDR_BTN_W + 3)),
    ];
    for (id, x) in cabecera {
        // SAFETY: hwnd padre válido durante WM_CREATE; controles BUTTON
        // estándar, el sistema los destruye con el padre.
        unsafe {
            _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                PCWSTR::null(),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_OWNERDRAW as u32),
                x,
                3,
                HDR_BTN_W,
                HDR_BTN_H,
                Some(hwnd),
                Some(HMENU(id as usize as *mut _)),
                None,
                None,
            );
        }
    }
    // (id, texto, habilitado en F1)
    let botones: [(u16, PCWSTR, bool); 6] = [
        (ID_FULLSCREEN, w!("Pantalla"), true),
        (ID_WINDOW, w!("Ventana"), true),
        (ID_REGION, w!("Región"), false),
        (ID_DELAY, w!("Delay"), false),
        (ID_RECORD, w!("Grabar"), false),
        (ID_CONFIG, w!("Config"), false),
    ];
    for (i, (id, texto, habilitado)) in botones.iter().enumerate() {
        // SAFETY: ver arriba.
        unsafe {
            if let Ok(btn) = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                *texto,
                WS_CHILD | WS_VISIBLE,
                MARGIN + i as i32 * (BTN_W + MARGIN),
                HEADER_H + MARGIN,
                BTN_W,
                BTN_H,
                Some(hwnd),
                Some(HMENU(*id as usize as *mut _)),
                None,
                None,
            ) {
                _ = EnableWindow(btn, *habilitado);
            }
        }
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: cada rama documenta su invariante; el estado de
    // GWLP_USERDATA se libera únicamente en WM_NCDESTROY.
    unsafe {
        match msg {
            WM_NCCREATE => {
                let cs = &*(lparam.0 as *const CREATESTRUCTW);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CREATE => {
                crear_botones(hwnd);
                LRESULT(0)
            }
            WM_COMMAND => {
                on_command(hwnd, (wparam.0 & 0xFFFF) as u16);
                LRESULT(0)
            }
            WM_PAINT => {
                // Título de la cabecera pintado a mano: así el fondo
                // sigue siendo arrastrable (sin control STATIC encima).
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                SetBkMode(hdc, TRANSPARENT);
                let mut titulo: Vec<u16> = "RustCapture".encode_utf16().collect();
                let mut rect = windows::Win32::Foundation::RECT {
                    left: MARGIN + 2,
                    top: 0,
                    right: 220,
                    bottom: HEADER_H,
                };
                DrawTextW(hdc, &mut titulo, &mut rect, DT_SINGLELINE | DT_VCENTER);
                _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_DRAWITEM => {
                // Botones de cabecera owner-draw: la ✕ es el glifo de
                // caption del sistema; el minimizar se pinta a mano como
                // guion CENTRADO (el glifo estándar es una barra baja).
                let dis = &*(lparam.0 as *const DRAWITEMSTRUCT);
                let pulsado = if (dis.itemState.0 & ODS_SELECTED.0) != 0 {
                    DFCS_PUSHED.0
                } else {
                    0
                };
                let mut rect = dis.rcItem;
                match dis.CtlID as u16 {
                    ID_CLOSE => {
                        _ = DrawFrameControl(
                            dis.hDC,
                            &mut rect,
                            DFC_CAPTION,
                            DFCS_STATE(DFCS_CAPTIONCLOSE.0 | pulsado),
                        );
                        LRESULT(1)
                    }
                    ID_MINIMIZE => {
                        _ = DrawFrameControl(
                            dis.hDC,
                            &mut rect,
                            DFC_BUTTON,
                            DFCS_STATE(DFCS_BUTTONPUSH.0 | pulsado),
                        );
                        // Guion centrado; se desplaza 1 px al pulsarse,
                        // como los botones nativos.
                        let desplazado = i32::from(pulsado != 0);
                        let cx = (rect.left + rect.right) / 2 + desplazado;
                        let cy = (rect.top + rect.bottom) / 2 + desplazado;
                        let guion = RECT {
                            left: cx - 5,
                            top: cy - 1,
                            right: cx + 5,
                            bottom: cy + 1,
                        };
                        FillRect(dis.hDC, &guion, GetSysColorBrush(COLOR_BTNTEXT));
                        LRESULT(1)
                    }
                    _ => DefWindowProcW(hwnd, msg, wparam, lparam),
                }
            }
            m if m == WM_TRAY => {
                crate::tray::on_tray_message(hwnd, lparam);
                LRESULT(0)
            }
            WM_NCHITTEST => {
                // Arrastrable desde cualquier punto del fondo.
                let hit = DefWindowProcW(hwnd, msg, wparam, lparam);
                if hit.0 == HTCLIENT as isize {
                    LRESULT(HTCAPTION as isize)
                } else {
                    hit
                }
            }
            WM_DESTROY => {
                if let Some(state) = state_ref(hwnd) {
                    let _ = state.tx.send(AppEvent::Shutdown);
                }
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let ptr = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut BarState;
                if !ptr.is_null() {
                    drop(Box::from_raw(ptr));
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
