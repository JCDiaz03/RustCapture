//! Editor V4 (f.21): la captura aterriza aquí y se anota IN SITU con el
//! motor del core (D5+D6) — toolbar de herramientas con la activa en
//! acento, barra de propiedades contextual, canvas con preview en vivo,
//! texto in situ y barra de estado. Guardar/Copiar hornean bajo demanda
//! el documento sobre la base, así undo/redo sobrevive al guardado.
//! (La antigua ventana de dibujo/Ventana2 vive fusionada aquí.)
//!
//! Hilos: `EditorSink::deliver` corre en el hilo orquestador y SOLO
//! publica un mensaje; `show_editor` corre en el hilo de UI (bucle
//! modal, patrón del overlay).

mod estado;
pub(crate) mod math;
mod props;
mod texto;

use std::sync::atomic::{AtomicBool, Ordering};

use rustcapture_core::output::{ImageFormat, encode};
use rustcapture_core::ports::{Frame, OutputError, OutputSink, Rect};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateSolidBrush, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteObject,
    DrawTextW, EndPaint, FillRect, HALFTONE, HBRUSH, HDC, InvalidateRect, PAINTSTRUCT,
    RDW_ALLCHILDREN, RDW_ERASE, RDW_INVALIDATE, RedrawWindow, SRCCOPY, SelectObject, SetBkMode,
    SetStretchBltMode, SetTextColor, StretchBlt, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::DRAWITEMSTRUCT;
use windows::Win32::UI::Controls::Dialogs::{GetSaveFileNameW, OFN_OVERWRITEPROMPT, OPENFILENAMEW};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetKeyState, ReleaseCapture, SetCapture, VK_CONTROL, VK_ESCAPE,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use crate::dpi::{self, Escala};
use crate::gdi::raii::{Dib, MemDc, ScreenDc, Selected};
use crate::ui::{boton, fuentes, layout, theme, tooltip::Tooltips};

use estado::{DragState, EditorState};
use math::Elemento;
use texto::{ID_EDIT_TEXT, WM_APP_CANCEL_TEXT};

/// Mensaje al wndproc de la barra: wparam = `Box<Frame>` crudo, el
/// receptor toma posesión SIEMPRE (también si decide no abrir).
pub(crate) const WM_APP_EDITOR: u32 = WM_APP + 3;

/// Un editor cada vez (MVP sin tabs). Lo consulta el sink (hilo
/// orquestador) y lo mantiene `show_editor` (hilo UI).
static EDITOR_ABIERTO: AtomicBool = AtomicBool::new(false);

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

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// PENDIENTE(limpieza): duplicada en overlay/mod.rs (y `wide` está en
// alerts); extraer a un módulo util interno del crate.
fn punto(lparam: LPARAM) -> (i32, i32) {
    (
        (lparam.0 & 0xFFFF) as i16 as i32,
        ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
    )
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
    let titulo = wide(&format!(
        "Captura {}×{} — RustCapture",
        frame.width, frame.height
    ));
    // Tamaño inicial: imagen + chrome, acotado a un máximo razonable.
    let win_w = (frame.width as i32 + 60).clamp(720, 1280);
    let win_h = (frame.height as i32 + 190).clamp(430, 840);

    let state_ptr = Box::into_raw(Box::new(EditorState::new(frame)?));

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
            hIcon: crate::ui::icono_app::para_clase(),
            // Sin brocha de clase: el back buffer pinta todo el cliente.
            hbrBackground: HBRUSH::default(),
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
        theme::aplicar_titulo_oscuro(hwnd, theme::actual().es_oscuro());
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

/// Layout actual de la toolbar al DPI/ancho de la ventana.
fn distribuye_toolbar(hwnd: HWND) -> (Vec<Elemento>, Vec<layout::Caja>) {
    let escala = Escala::from_hwnd(hwnd);
    let mut client = RECT::default();
    // SAFETY: consulta del client rect de una ventana viva.
    unsafe { _ = GetClientRect(hwnd, &mut client) };
    let fila = math::toolbar();
    let items = math::a_items(&fila);
    let (cajas, _) = layout::distribuir(
        &items,
        escala,
        escala.px(math::TOOLBAR_LOGICO),
        Some(client.right - client.left),
    );
    (fila, cajas)
}

fn crear_toolbar(hwnd: HWND) {
    let (fila, cajas) = distribuye_toolbar(hwnd);
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
                grabacion: false,
            },
        ) else {
            continue;
        };
        if def.habilitado && let Some(tt) = tooltips.as_mut() {
            _ = tt.agregar(control, def.nombre);
        }
    }
    if let Some(state) = state_mut(hwnd) {
        state.tooltips = tooltips;
        // Sin fuentes del sistema no hay herramienta de texto.
        if !state.tiene_fuente {
            // SAFETY: deshabilitar un control hijo propio.
            unsafe {
                if let Ok(btn) = GetDlgItem(Some(hwnd), i32::from(math::ID_TEXTO)) {
                    _ = EnableWindow(btn, false);
                }
            }
        }
        marcar_herramienta(hwnd, None, state.herramienta);
    }
}

/// Recoloca la toolbar tras WM_SIZE / WM_DPICHANGED (el muelle depende
/// del ancho del cliente).
fn reposicionar_toolbar(hwnd: HWND) {
    let (fila, cajas) = distribuye_toolbar(hwnd);
    for (elemento, caja) in fila.iter().zip(&cajas) {
        let Elemento::Boton(def) = elemento else {
            continue;
        };
        // SAFETY: mover controles hijos propios desde su hilo.
        unsafe {
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
    }
}

/// Refleja la herramienta activa en la toolbar (estado 'activo' en acento).
fn marcar_herramienta(hwnd: HWND, previa: Option<math::Herramienta>, nueva: math::Herramienta) {
    // SAFETY: consulta de controles hijos propios.
    unsafe {
        if let Some(previa) = previa
            && let Ok(btn) = GetDlgItem(Some(hwnd), i32::from(math::id_de_herramienta(previa)))
        {
            boton::set_activo(btn, false);
        }
        if let Ok(btn) = GetDlgItem(Some(hwnd), i32::from(math::id_de_herramienta(nueva))) {
            boton::set_activo(btn, true);
        }
    }
}

fn cambiar_herramienta(hwnd: HWND, state: &mut EditorState, nueva: math::Herramienta) {
    // Cambiar de herramienta confirma la caja de texto abierta.
    texto::commit_text(hwnd, state);
    if state.herramienta != nueva {
        let previa = state.herramienta;
        state.herramienta = nueva;
        marcar_herramienta(hwnd, Some(previa), nueva);
        // La property bar cambia con la herramienta.
        // SAFETY: invalidación de la propia ventana.
        unsafe { _ = InvalidateRect(Some(hwnd), None, false) };
    }
}

/// Rect destino (coordenadas de cliente) de la imagen encajada en el canvas.
fn dest_rect(hwnd: HWND, state: &EditorState) -> Rect {
    let escala = Escala::from_hwnd(hwnd);
    let mut client = RECT::default();
    // SAFETY: consulta sin precondiciones.
    unsafe { _ = GetClientRect(hwnd, &mut client) };
    let reparto = math::reparto(
        client.bottom - client.top,
        escala.px(math::TOOLBAR_LOGICO),
        escala.px(math::PROPS_LOGICO),
        escala.px(math::STATUS_LOGICO),
    );
    let lienzo = (
        client.right - client.left,
        reparto.status_inicio - reparto.props_fin,
    );
    let encajado = math::fit_rect((state.committed.width, state.committed.height), lienzo);
    Rect::new(
        encajado.x,
        encajado.y + reparto.props_fin,
        encajado.width,
        encajado.height,
    )
}

/// Diálogo "Guardar como" → codifica y escribe. Errores → MessageBox.
/// Devuelve la ruta escrita (limpia el flag de sucio en el llamador).
fn guardar_como(hwnd: HWND, frame: &Frame) -> Option<String> {
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
        return None; // canceló
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
        Ok(()) => {
            crate::alerts::capture_beep();
            Some(ruta)
        }
        Err(e) => {
            crate::alerts::error_box("RustCapture Editor", &format!("{ruta}: {e}"));
            None
        }
    }
}

fn deshacer(hwnd: HWND, state: &mut EditorState) {
    texto::commit_text(hwnd, state);
    if state.history.undo(&mut state.doc) {
        state.refresh_committed();
        state.dirty = true;
        // SAFETY: invalidación de la propia ventana.
        unsafe { _ = InvalidateRect(Some(hwnd), None, false) };
    }
}

fn rehacer(hwnd: HWND, state: &mut EditorState) {
    texto::commit_text(hwnd, state);
    if state.history.redo(&mut state.doc) {
        state.refresh_committed();
        state.dirty = true;
        // SAFETY: invalidación de la propia ventana.
        unsafe { _ = InvalidateRect(Some(hwnd), None, false) };
    }
}

fn on_command(hwnd: HWND, id: u16) {
    let Some(state) = state_mut(hwnd) else {
        return;
    };
    if let Some(herramienta) = math::herramienta_de_id(id) {
        if herramienta != math::Herramienta::Texto || state.tiene_fuente {
            cambiar_herramienta(hwnd, state, herramienta);
        }
        return;
    }
    match id {
        math::ID_UNDO => deshacer(hwnd, state),
        math::ID_REDO => rehacer(hwnd, state),
        math::ID_GUARDAR => {
            // Hornear bajo demanda: committed = base + documento vigente.
            texto::commit_text(hwnd, state);
            if let Some(ruta) = guardar_como(hwnd, &state.committed) {
                state.dirty = false;
                let nombre = std::path::Path::new(&ruta)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or(ruta);
                let titulo = wide(&format!("{nombre} — RustCapture"));
                // SAFETY: SetWindowTextW sobre la propia ventana.
                unsafe {
                    _ = SetWindowTextW(hwnd, PCWSTR(titulo.as_ptr()));
                    _ = InvalidateRect(Some(hwnd), None, false);
                }
                state.nombre = Some(nombre);
            }
        }
        math::ID_COPIAR => {
            texto::commit_text(hwnd, state);
            match crate::clipboard::ClipboardSink::new().deliver(&state.committed) {
                Ok(()) => {
                    state.dirty = false;
                    crate::alerts::capture_beep();
                    // SAFETY: invalidación de la propia ventana.
                    unsafe { _ = InvalidateRect(Some(hwnd), None, false) };
                }
                Err(_) => crate::alerts::error_beep(),
            }
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
                let id = (wparam.0 & 0xFFFF) as u16;
                let code = ((wparam.0 >> 16) & 0xFFFF) as u32;
                if id == ID_EDIT_TEXT {
                    if code == EN_KILLFOCUS
                        && let Some(state) = state_mut(hwnd)
                    {
                        texto::commit_text(hwnd, state);
                    }
                } else {
                    on_command(hwnd, id);
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                if let Some(state) = state_mut(hwnd) {
                    let p = punto(lparam);
                    if props::on_click(hwnd, state, p) {
                        return LRESULT(0);
                    }
                    let destino = dest_rect(hwnd, state);
                    let tam = (state.committed.width, state.committed.height);
                    if let Some(pf) = math::view_to_frame(p, destino, tam) {
                        if state.herramienta == math::Herramienta::Texto {
                            texto::commit_text(hwnd, state);
                            if let Some(state) = state_mut(hwnd) {
                                texto::abrir_edit(hwnd, state, pf, destino);
                            }
                        } else {
                            state.drag = Some(DragState {
                                start: pf,
                                current: pf,
                                points: vec![pf],
                            });
                            SetCapture(hwnd);
                        }
                    }
                }
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                if let Some(state) = state_mut(hwnd) {
                    let destino = dest_rect(hwnd, state);
                    let tam = (state.committed.width, state.committed.height);
                    if state.drag.is_some()
                        && let Some(pf) = math::view_to_frame(punto(lparam), destino, tam)
                        && let Some(drag) = state.drag.as_mut()
                    {
                        drag.current = pf;
                        if state.herramienta == math::Herramienta::Lapiz {
                            drag.points.push(pf);
                        }
                        // Solo cambia el canvas: invalidar su rect encajado.
                        let zona = RECT {
                            left: destino.x,
                            top: destino.y,
                            right: destino.x + destino.width as i32,
                            bottom: destino.y + destino.height as i32,
                        };
                        _ = InvalidateRect(Some(hwnd), Some(&zona), false);
                    }
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                if let Some(state) = state_mut(hwnd)
                    && state.drag.is_some()
                {
                    _ = ReleaseCapture();
                    if let Some(anotacion) = state.anotacion_en_curso() {
                        state
                            .history
                            .apply(&mut state.doc, rustcapture_core::annotate::Command::add(anotacion));
                        state.refresh_committed();
                        state.dirty = true;
                    }
                    state.drag = None;
                    _ = InvalidateRect(Some(hwnd), None, false);
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if let Some(state) = state_mut(hwnd) {
                    let ctrl = GetKeyState(VK_CONTROL.0 as i32) < 0;
                    match wparam.0 as u16 {
                        k if k == VK_ESCAPE.0 => {
                            // Esc solo cancela la caja de texto abierta.
                            if let Some(edit) = state.edit.take() {
                                texto::cerrar_edit(edit);
                                _ = InvalidateRect(Some(hwnd), None, false);
                            }
                        }
                        k if ctrl && k == b'Z' as u16 => deshacer(hwnd, state),
                        k if ctrl && k == b'Y' as u16 => rehacer(hwnd, state),
                        _ => {}
                    }
                }
                LRESULT(0)
            }
            m if m == WM_APP_CANCEL_TEXT => {
                if let Some(state) = state_mut(hwnd)
                    && let Some(edit) = state.edit.take()
                {
                    texto::cerrar_edit(edit);
                    _ = InvalidateRect(Some(hwnd), None, false);
                }
                LRESULT(0)
            }
            WM_SIZE => {
                reposicionar_toolbar(hwnd);
                _ = InvalidateRect(Some(hwnd), None, false);
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1), // el back buffer pinta todo
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                if let Some(state) = state_mut(hwnd) {
                    _ = pintar(hwnd, hdc, state);
                }
                _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_DRAWITEM => {
                let dis = &*(lparam.0 as *const DRAWITEMSTRUCT);
                if boton::pintar_drawitem(dis) {
                    LRESULT(1)
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
            WM_DPICHANGED => {
                dpi::aplicar_rect_sugerido(hwnd, lparam);
                reposicionar_toolbar(hwnd);
                _ = InvalidateRect(Some(hwnd), None, false);
                LRESULT(0)
            }
            WM_SETTINGCHANGE => {
                if theme::es_cambio_de_tema(lparam) {
                    let tema = theme::refrescar_con_modo_actual();
                    theme::aplicar_titulo_oscuro(hwnd, tema.es_oscuro());
                    _ = RedrawWindow(
                        Some(hwnd),
                        None,
                        None,
                        RDW_ERASE | RDW_INVALIDATE | RDW_ALLCHILDREN,
                    );
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                // Sucio → confirmar el descarte (regla del humano).
                let descartar = match state_mut(hwnd) {
                    Some(state) if state.dirty => {
                        MessageBoxW(
                            Some(hwnd),
                            w!("Hay cambios sin guardar. ¿Descartarlos?"),
                            w!("RustCapture Editor"),
                            MB_YESNO | MB_ICONQUESTION,
                        ) == IDYES
                    }
                    _ => true,
                };
                if descartar {
                    _ = DestroyWindow(hwnd);
                }
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

/// Compone todo el cliente (toolbar, property bar, canvas anotable y
/// status bar) en un back buffer y lo vuelca de un BitBlt.
fn pintar(hwnd: HWND, hdc: HDC, state: &mut EditorState) -> windows::core::Result<()> {
    let paleta = theme::actual().paleta();
    let escala = Escala::from_hwnd(hwnd);
    // SAFETY: hdc de BeginPaint; recursos RAII; brochas propias
    // liberadas en la misma función.
    unsafe {
        let mut client = RECT::default();
        _ = GetClientRect(hwnd, &mut client);
        let (ancho, alto) = (client.right - client.left, client.bottom - client.top);
        if ancho <= 0 || alto <= 0 {
            return Ok(());
        }
        let reparto = math::reparto(
            alto,
            escala.px(math::TOOLBAR_LOGICO),
            escala.px(math::PROPS_LOGICO),
            escala.px(math::STATUS_LOGICO),
        );

        let screen = ScreenDc::get()?;
        let back_dc = MemDc::compatible_with(&screen)?;
        let back = Dib::new_32bpp(&back_dc, ancho as u32, alto as u32)?;
        let _b = Selected::bitmap(&back_dc, &back)?;

        // Bandas superiores y status: superficie. Canvas: fondo de canvas.
        let superficie = CreateSolidBrush(paleta.superficie);
        FillRect(
            back_dc.0,
            &RECT { left: 0, top: 0, right: ancho, bottom: reparto.props_fin },
            superficie,
        );
        FillRect(
            back_dc.0,
            &RECT { left: 0, top: reparto.status_inicio, right: ancho, bottom: alto },
            superficie,
        );
        _ = DeleteObject(superficie.into());
        let fondo_canvas = CreateSolidBrush(paleta.canvas);
        FillRect(
            back_dc.0,
            &RECT {
                left: 0,
                top: reparto.props_fin,
                right: ancho,
                bottom: reparto.status_inicio,
            },
            fondo_canvas,
        );
        _ = DeleteObject(fondo_canvas.into());
        // Separadores de 1 px: toolbar/props, props/canvas y canvas/status.
        let borde = CreateSolidBrush(paleta.borde);
        let linea = escala.px(1).max(1);
        for y in [reparto.toolbar_fin - linea, reparto.props_fin - linea, reparto.status_inicio] {
            FillRect(
                back_dc.0,
                &RECT { left: 0, top: y, right: ancho, bottom: y + linea },
                borde,
            );
        }
        _ = DeleteObject(borde.into());

        // Property bar contextual (guarda las zonas para el hit-test).
        let banda_props = RECT {
            left: 0,
            top: reparto.toolbar_fin,
            right: ancho,
            bottom: reparto.props_fin - linea,
        };
        state.chips = props::pintar(back_dc.0, banda_props, state, escala);

        // Canvas: comprometido, o comprometido + provisional durante el
        // arrastre (buffers persistentes: dos memcpy, cero asignaciones).
        let lienzo = (ancho, reparto.status_inicio - reparto.props_fin);
        let encajado = math::fit_rect((state.committed.width, state.committed.height), lienzo);
        let porcentaje = if state.committed.width > 0 {
            (u64::from(encajado.width) * 100 / u64::from(state.committed.width)) as u32
        } else {
            100
        };
        if !encajado.is_empty() {
            let dib = if let Some(anotacion) = state.anotacion_en_curso() {
                state.preview.pixels.copy_from_slice(&state.committed.pixels);
                {
                    let mut canvas = rustcapture_core::annotate::Canvas::new(&mut state.preview);
                    anotacion.render(&mut canvas, &state.ctx);
                }
                crate::gdi::copy_frame_to_dib(&state.preview, &mut state.preview_dib);
                &state.preview_dib
            } else {
                &state.committed_dib
            };
            let src_dc = MemDc::compatible_with(&screen)?;
            let _s = Selected::bitmap(&src_dc, dib)?;
            SetStretchBltMode(back_dc.0, HALFTONE);
            _ = StretchBlt(
                back_dc.0,
                encajado.x,
                encajado.y + reparto.props_fin,
                encajado.width as i32,
                encajado.height as i32,
                Some(src_dc.0),
                0,
                0,
                state.committed.width as i32,
                state.committed.height as i32,
                SRCCOPY,
            );
        }

        // Status bar: dimensiones · % de encaje · destino/estado.
        let mut status = format!(
            "{} × {} px · {} %",
            state.committed.width, state.committed.height, porcentaje
        );
        match &state.nombre {
            Some(nombre) => {
                status.push_str(" · ");
                status.push_str(nombre);
            }
            None => status.push_str(" · PNG"),
        }
        if state.dirty {
            status.push_str(" · sin guardar");
        }
        SetBkMode(back_dc.0, TRANSPARENT);
        SetTextColor(back_dc.0, paleta.texto_secundario);
        let fuente = fuentes::fuente(fuentes::Rol::Secundario, escala);
        let fuente_previa = SelectObject(back_dc.0, fuente.into());
        let mut wide: Vec<u16> = status.encode_utf16().collect();
        let mut rc = RECT {
            left: escala.px(12),
            top: reparto.status_inicio + linea,
            right: ancho - escala.px(12),
            bottom: alto,
        };
        DrawTextW(
            back_dc.0,
            &mut wide,
            &mut rc,
            DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
        );
        SelectObject(back_dc.0, fuente_previa);

        _ = BitBlt(hdc, 0, 0, ancho, alto, Some(back_dc.0), 0, 0, SRCCOPY);
    }
    Ok(())
}
