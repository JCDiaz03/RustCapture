//! Barra flotante V4 (f.1, D11): una fila de botones de icono (28×28
//! lógicos) con asa de arrastre, separadores y tooltips, pintada con el
//! tema activo. Solo produce eventos (D7). No-activate: no roba el foco,
//! así "ventana activa" es la correcta.
//!
//! Hilos: crear la barra y correr `run_message_loop` en el MISMO hilo
//! (el principal). El estado del wndproc vive en GWLP_USERDATA.

mod math;

use std::cell::RefCell;
use std::sync::mpsc::Sender;

use rustcapture_core::config::{HotkeysConfig, ThemeMode};
use rustcapture_core::orchestrator::{AppEvent, CaptureRequest, ModeRequest};
use rustcapture_core::ports::HotkeyId;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateSolidBrush, DeleteObject, EndPaint, FillRect, FrameRect, HBRUSH,
    InvalidateRect, PAINTSTRUCT, RDW_ALLCHILDREN, RDW_ERASE, RDW_INVALIDATE, RedrawWindow,
    SRCCOPY,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::DRAWITEMSTRUCT;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use crate::dpi::Escala;
use crate::gdi::raii::{Dib, MemDc, ScreenDc, Selected};
use crate::ui::{boton, iconos, layout, theme, tooltip::Tooltips};

use math::{Elemento, HotkeyTooltip};

/// Mensaje de callback del icono de bandeja (lo consume `tray`).
pub(crate) const WM_TRAY: u32 = WM_APP + 1;
/// Petición de selección de región (botón o hotkey): debe correr en el
/// hilo de UI, nunca en el orquestador.
pub(crate) const WM_APP_REGION: u32 = WM_APP + 2;

pub(crate) const MENU_FULLSCREEN: u16 = 2001;
pub(crate) const MENU_WINDOW: u16 = 2002;
pub(crate) const MENU_TOGGLE: u16 = 2003;
pub(crate) const MENU_QUIT: u16 = 2004;
pub(crate) const MENU_REPEAT: u16 = 2005;

struct BarState {
    tx: Sender<AppEvent>,
    destination: &'static str,
    /// Retardo del botón Delay (f.17), de `[capture].delay_seconds`.
    delay_ms: u64,
    /// Hotkeys de config: alimentan los tooltips "Nombre (hotkey)".
    hotkeys: HotkeysConfig,
    /// Preferencia de tema de config; se re-resuelve en WM_SETTINGCHANGE.
    theme_mode: ThemeMode,
    /// Control de tooltips; vive lo que la ventana (RefCell: hilo de UI).
    tooltips: RefCell<Option<Tooltips>>,
}

/// Handle de la barra. No expone tipos de `windows` (D2/D11).
pub struct Bar {
    hwnd: HWND,
}

impl Bar {
    /// Crea y muestra la barra. `destination` = sink por defecto de la
    /// config; `delay_ms` = retardo del botón Delay; `hotkeys` alimenta
    /// los tooltips y `theme_mode` la resolución de tema.
    pub fn create(
        tx: Sender<AppEvent>,
        destination: &'static str,
        delay_ms: u64,
        hotkeys: HotkeysConfig,
        theme_mode: ThemeMode,
    ) -> Result<Self, String> {
        Self::create_win32(tx, destination, delay_ms, hotkeys, theme_mode)
            .map_err(|e| e.to_string())
    }

    fn create_win32(
        tx: Sender<AppEvent>,
        destination: &'static str,
        delay_ms: u64,
        hotkeys: HotkeysConfig,
        theme_mode: ThemeMode,
    ) -> windows::core::Result<Self> {
        theme::refrescar(theme_mode);
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
                // Sin brocha de clase: WM_PAINT pinta todo el fondo con el
                // tema y WM_ERASEBKGND devuelve 1 (sin parpadeo).
                hbrBackground: HBRUSH::default(),
                ..Default::default()
            };
            RegisterClassW(&wc); // 0 si ya estaba registrada: inofensivo
            let state = Box::into_raw(Box::new(BarState {
                tx,
                destination,
                delay_ms,
                hotkeys,
                theme_mode,
                tooltips: RefCell::new(None),
            }));
            // Tamaño provisional: WM_CREATE lo corrige con el DPI real.
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class,
                w!("RustCapture"),
                WS_POPUP | WS_VISIBLE,
                40,
                40,
                600,
                36,
                None,
                None,
                Some(instance.into()),
                Some(state.cast()),
            )?;
            // Esquinas redondeadas en Win11; no-op silencioso en Win10.
            let esquinas = DWMWCP_ROUND;
            _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &esquinas as *const _ as *const _,
                size_of_val(&esquinas) as u32,
            );
            Ok(Self { hwnd })
        }
    }

    /// HWND como entero opaco, para colgar el icono de bandeja.
    pub fn hwnd_raw(&self) -> isize {
        self.hwnd.0 as isize
    }
}

/// Bucle de mensajes del hilo UI. Los `WM_HOTKEY` (hwnd nulo) van al bus
/// salvo el de región, que se traduce a `WM_APP_REGION` (UI).
pub fn run_message_loop(tx: &Sender<AppEvent>, region_hotkey: Option<HotkeyId>, bar: &Bar) {
    let mut msg = MSG::default();
    // SAFETY: bucle GetMessage estándar del hilo que posee las ventanas.
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_HOTKEY && msg.hwnd.is_invalid() {
                let id = HotkeyId(msg.wParam.0 as u32);
                if Some(id) == region_hotkey {
                    _ = PostMessageW(Some(bar.hwnd), WM_APP_REGION, WPARAM(0), LPARAM(0));
                } else {
                    let _ = tx.send(AppEvent::HotkeyPressed(id));
                }
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
        math::ID_FULLSCREEN | MENU_FULLSCREEN => enviar_captura(hwnd, ModeRequest::Fullscreen),
        math::ID_WINDOW | MENU_WINDOW => enviar_captura(hwnd, ModeRequest::ActiveWindow),
        // f.13 interactiva: el overlay corre en el hilo de UI; se
        // despacha como mensaje para salir del contexto del clic.
        math::ID_REGION => {
            // SAFETY: post a la propia ventana del wndproc.
            unsafe { _ = PostMessageW(Some(hwnd), WM_APP_REGION, WPARAM(0), LPARAM(0)) };
        }
        // f.17: captura del monitor activo tras el retardo configurado.
        math::ID_DELAY => {
            if let Some(state) = state_ref(hwnd) {
                let _ = state.tx.send(AppEvent::DelayedCapture {
                    request: CaptureRequest {
                        mode: ModeRequest::Fullscreen,
                        destination: state.destination,
                    },
                    delay_ms: state.delay_ms,
                });
            }
        }
        // f.18: repetir la última captura ejecutada con éxito.
        MENU_REPEAT => {
            if let Some(state) = state_ref(hwnd) {
                let _ = state.tx.send(AppEvent::RepeatLast);
            }
        }
        MENU_TOGGLE => {
            // SAFETY: hwnd válido (viene del wndproc de esa ventana).
            unsafe {
                let visible = IsWindowVisible(hwnd).as_bool();
                _ = ShowWindow(hwnd, if visible { SW_HIDE } else { SW_SHOW });
            }
        }
        math::ID_CLOSE | MENU_QUIT => {
            // SAFETY: destruir la propia ventana dispara WM_DESTROY
            // (Shutdown + PostQuitMessage).
            unsafe { _ = DestroyWindow(hwnd) };
        }
        _ => {}
    }
}

/// Oculta la barra, abre el overlay y publica la región elegida.
fn flujo_region(hwnd: HWND) {
    // SAFETY: hwnd válido (viene del wndproc); ocultar/mostrar la propia
    // ventana alrededor del overlay para no salir en la captura.
    unsafe {
        _ = ShowWindow(hwnd, SW_HIDE);
        std::thread::sleep(std::time::Duration::from_millis(150));
        let resultado = crate::overlay::select_region();
        _ = ShowWindow(hwnd, SW_SHOW);
        if let (Some(rect), Some(state)) = (resultado, state_ref(hwnd)) {
            let _ = state.tx.send(AppEvent::CaptureRequested(CaptureRequest {
                mode: ModeRequest::Region(rect),
                destination: state.destination,
            }));
        }
    }
}

/// Layout actual de la fila al DPI de la ventana.
fn layout_actual(hwnd: HWND) -> (Vec<Elemento>, Vec<layout::Caja>, i32, i32) {
    let escala = Escala::from_hwnd(hwnd);
    let fila = math::fila();
    let items = math::a_items(&fila);
    let alto = escala.px(math::ALTO_LOGICO);
    let (cajas, ancho) = layout::distribuir(&items, escala, alto, None);
    (fila, cajas, ancho, alto)
}

fn texto_tooltip(def: &math::BotonDef, hotkeys: &HotkeysConfig) -> String {
    match def.hotkey {
        Some(HotkeyTooltip::Fullscreen) => format!("{} ({})", def.nombre, hotkeys.fullscreen),
        Some(HotkeyTooltip::Window) => format!("{} ({})", def.nombre, hotkeys.window),
        Some(HotkeyTooltip::Region) => format!("{} ({})", def.nombre, hotkeys.region),
        Some(HotkeyTooltip::Delay) => format!("{} ({})", def.nombre, hotkeys.delay),
        None => def.nombre.to_string(),
    }
}

/// Crea los botones de la fila, ajusta el tamaño real de la ventana al
/// DPI y registra los tooltips.
fn crear_botones(hwnd: HWND) {
    let (fila, cajas, ancho, alto) = layout_actual(hwnd);
    // SAFETY: redimensionar la propia ventana durante WM_CREATE.
    unsafe {
        _ = SetWindowPos(hwnd, None, 0, 0, ancho, alto, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
    }
    let mut tooltips = Tooltips::nuevo(hwnd).ok();
    for (elemento, caja) in fila.iter().zip(&cajas) {
        let Elemento::Boton(def) = elemento else {
            continue;
        };
        let Ok(control) = boton::crear(
            hwnd,
            def.id,
            *caja,
            boton::Opciones {
                icono: def.icono,
                habilitado: def.habilitado,
                grabacion: def.grabacion,
            },
        ) else {
            continue;
        };
        if def.habilitado
            && let (Some(tt), Some(state)) = (tooltips.as_mut(), state_ref(hwnd))
        {
            _ = tt.agregar(control, &texto_tooltip(def, &state.hotkeys));
        }
    }
    if let Some(state) = state_ref(hwnd) {
        *state.tooltips.borrow_mut() = tooltips;
    }
}

/// Recoloca los botones tras un cambio de DPI y ajusta el tamaño total.
fn reposicionar(hwnd: HWND, sugerido: Option<(i32, i32)>) {
    let (fila, cajas, ancho, alto) = layout_actual(hwnd);
    // SAFETY: mover ventanas propias (padre e hijos) desde su hilo.
    unsafe {
        let (x, y) = sugerido.unwrap_or((0, 0));
        let flags = if sugerido.is_some() {
            SWP_NOZORDER | SWP_NOACTIVATE
        } else {
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE
        };
        _ = SetWindowPos(hwnd, None, x, y, ancho, alto, flags);
        for (elemento, caja) in fila.iter().zip(&cajas) {
            let Elemento::Boton(def) = elemento else {
                continue;
            };
            if let Ok(control) = GetDlgItem(Some(hwnd), i32::from(def.id)) {
                _ = SetWindowPos(
                    control,
                    None,
                    caja.x,
                    caja.y,
                    caja.ancho,
                    caja.alto,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }
        _ = InvalidateRect(Some(hwnd), None, false);
    }
}

/// Fondo, borde, asa y separadores, compuestos en un back buffer y
/// volcados con un único BitBlt (sin parpadeo).
fn pintar(hwnd: HWND) {
    let paleta = theme::actual().paleta();
    let escala = Escala::from_hwnd(hwnd);
    let (fila, cajas, _, _) = layout_actual(hwnd);
    // SAFETY: pintado estándar con recursos RAII; brochas propias se
    // liberan en la misma función.
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let mut rc = RECT::default();
        _ = GetClientRect(hwnd, &mut rc);
        let (ancho, alto) = (rc.right - rc.left, rc.bottom - rc.top);
        if ancho <= 0 || alto <= 0 {
            _ = EndPaint(hwnd, &ps);
            return;
        }
        let Ok(pantalla) = ScreenDc::get() else {
            _ = EndPaint(hwnd, &ps);
            return;
        };
        let Ok(mem) = MemDc::compatible_with(&pantalla) else {
            _ = EndPaint(hwnd, &ps);
            return;
        };
        let Ok(back) = Dib::new_32bpp(&mem, ancho as u32, alto as u32) else {
            _ = EndPaint(hwnd, &ps);
            return;
        };
        let Ok(_sel) = Selected::bitmap(&mem, &back) else {
            _ = EndPaint(hwnd, &ps);
            return;
        };

        let fondo = CreateSolidBrush(paleta.superficie);
        FillRect(mem.0, &rc, fondo);
        _ = DeleteObject(fondo.into());

        let talla = iconos::talla_para_dpi(escala.dpi());
        let sep = CreateSolidBrush(paleta.borde);
        for (elemento, caja) in fila.iter().zip(&cajas) {
            match elemento {
                Elemento::Asa => {
                    let x = caja.x + (caja.ancho - talla as i32) / 2;
                    let y = caja.y + (caja.alto - talla as i32) / 2;
                    _ = iconos::pintar(
                        mem.0,
                        iconos::Icono::SysDragHandle,
                        talla,
                        x,
                        y,
                        paleta.texto_secundario,
                        iconos::OPACO,
                    );
                }
                Elemento::Separador => {
                    let linea = RECT {
                        left: caja.x,
                        top: caja.y,
                        right: caja.x + caja.ancho,
                        bottom: caja.y + caja.alto,
                    };
                    FillRect(mem.0, &linea, sep);
                }
                Elemento::Boton(_) => {}
            }
        }
        let borde = RECT { left: 0, top: 0, right: ancho, bottom: alto };
        FrameRect(mem.0, &borde, sep);
        _ = DeleteObject(sep.into());

        _ = BitBlt(hdc, 0, 0, ancho, alto, Some(mem.0), 0, 0, SRCCOPY);
        _ = EndPaint(hwnd, &ps);
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
                pintar(hwnd);
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_DRAWITEM => {
                let dis = &*(lparam.0 as *const DRAWITEMSTRUCT);
                if boton::pintar_drawitem(dis) {
                    LRESULT(1)
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
            WM_DPICHANGED => {
                // El rect sugerido trae posición y tamaño para el DPI
                // nuevo; el tamaño real lo recalcula el layout.
                let rect = &*(lparam.0 as *const RECT);
                reposicionar(hwnd, Some((rect.left, rect.top)));
                LRESULT(0)
            }
            WM_SETTINGCHANGE => {
                if theme::es_cambio_de_tema(lparam) {
                    if let Some(state) = state_ref(hwnd) {
                        theme::refrescar(state.theme_mode);
                    }
                    _ = RedrawWindow(
                        Some(hwnd),
                        None,
                        None,
                        RDW_ERASE | RDW_INVALIDATE | RDW_ALLCHILDREN,
                    );
                }
                LRESULT(0)
            }
            m if m == WM_APP_REGION => {
                flujo_region(hwnd);
                LRESULT(0)
            }
            m if m == crate::editor::WM_APP_EDITOR => {
                // SAFETY: wparam es un Box<Frame> publicado por
                // EditorSink; se toma posesión SIEMPRE.
                let frame = *Box::from_raw(wparam.0 as *mut rustcapture_core::ports::Frame);
                _ = ShowWindow(hwnd, SW_HIDE);
                crate::editor::show_editor(frame);
                _ = ShowWindow(hwnd, SW_SHOW);
                LRESULT(0)
            }
            m if m == WM_TRAY => {
                crate::tray::on_tray_message(hwnd, lparam);
                LRESULT(0)
            }
            WM_NCHITTEST => {
                // Arrastrable desde cualquier punto del fondo (los
                // botones capturan sus propios clics).
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
