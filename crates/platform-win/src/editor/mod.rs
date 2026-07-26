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

use rustcapture_core::annotate::annotations::StepAnnotation;
use rustcapture_core::output::{ImageFormat, encode};
use rustcapture_core::ports::{Frame, OutputError, OutputSink, Rect};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DrawTextW, EndPaint, HALFTONE, HBRUSH,
    HDC, InvalidateRect, PAINTSTRUCT, SRCCOPY, SelectObject, SetBkMode, SetStretchBltMode,
    SetTextColor, StretchBlt, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::DRAWITEMSTRUCT;
use windows::Win32::UI::Controls::Dialogs::{GetSaveFileNameW, OFN_OVERWRITEPROMPT, OPENFILENAMEW};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetKeyState, ReleaseCapture, SetCapture, VK_CONTROL, VK_DELETE, VK_ESCAPE,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use crate::dpi::{self, Escala};
use crate::gdi::raii::Selected;
use crate::ui::botonera;
use crate::ui::{boton, fuentes, lienzo, theme, ventana};
use crate::util::{punto, wide};

use estado::{DragState, EditorState, MoverDrag};
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
            // CS_DBLCLKS: sin él Windows manda dos WM_LBUTTONDOWN en vez de
            // WM_LBUTTONDBLCLK, y el doble clic para reeditar texto no
            // llegaría nunca.
            style: CS_DBLCLKS,
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
            // CLIPCHILDREN: el volcado del back buffer no debe pintar encima
            // de los botones hijos de la toolbar. Sin esto, cada repintado
            // los tapa y ellos se repintan después — el parpadeo que se ve
            // al mover un objeto, donde se invalida en cada mousemove.
            WS_OVERLAPPEDWINDOW | WS_VISIBLE | WS_CLIPCHILDREN,
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
    // El Box vive hasta después del bucle modal (nunca se libera aquí).
    ventana::estado::<EditorState>(hwnd)
}

fn crear_toolbar(hwnd: HWND) {
    let fila = math::toolbar();
    let (cajas, _) = botonera::cajas(hwnd, &fila, math::TOOLBAR_LOGICO, true);
    let tooltips = botonera::crear(hwnd, &fila, &cajas, |def| def.nombre.to_string());
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
    let fila = math::toolbar();
    let (cajas, _) = botonera::cajas(hwnd, &fila, math::TOOLBAR_LOGICO, true);
    botonera::reposicionar(hwnd, &fila, &cajas);
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
        // La selección solo tiene sentido con las herramientas que operan
        // sobre objetos; al pasar a dibujar se suelta.
        if !math::opera_sobre_objetos(nueva) {
            state.seleccionado = None;
            state.mover = None;
        }
        marcar_herramienta(hwnd, Some(previa), nueva);
        // La property bar cambia con la herramienta.
        // SAFETY: invalidación de la propia ventana.
        unsafe { _ = InvalidateRect(Some(hwnd), None, false) };
    }
}

/// Zona a invalidar durante un arrastre: solo el canvas. Invalidar la
/// ventana entera repintaría toolbar y status en cada `WM_MOUSEMOVE` sin
/// que hayan cambiado.
fn zona_canvas(destino: Rect) -> RECT {
    RECT {
        left: destino.x,
        top: destino.y,
        right: destino.x + destino.width as i32,
        bottom: destino.y + destino.height as i32,
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

/// Coloca un paso numerado en el punto del clic (f.23): sin arrastre, con
/// el número que toca según el contador sincronizado con la historia.
fn colocar_paso(hwnd: HWND, state: &mut EditorState, pf: (i32, i32)) {
    let objeto = StepAnnotation {
        center: pf,
        number: state.pasos.siguiente(),
        color: state.props.color,
        font_size: state.props.tamano_texto,
    };
    if state.history.apply(
        &mut state.doc,
        rustcapture_core::annotate::Command::add(objeto.into()),
    ) {
        state.pasos.aplicado(true);
        state.refresh_committed();
        state.dirty = true;
        // SAFETY: invalidación de la propia ventana.
        unsafe { _ = InvalidateRect(Some(hwnd), None, false) };
    }
}

/// Borra un objeto entero (f.27: objetos, no píxeles). La goma y Supr
/// pasan por aquí.
fn borrar_objeto(hwnd: HWND, state: &mut EditorState, index: usize) {
    if state.history.apply(
        &mut state.doc,
        rustcapture_core::annotate::Command::remove(index),
    ) {
        state.pasos.aplicado(false);
        // Los índices de detrás se han desplazado: la selección muere.
        state.seleccionado = None;
        state.refresh_committed();
        state.dirty = true;
        // SAFETY: invalidación de la propia ventana.
        unsafe { _ = InvalidateRect(Some(hwnd), None, false) };
    }
}

/// Cierra el arrastre de un objeto convirtiéndolo en un `Command::Move`
/// (un solo comando por arrastre, no uno por `WM_MOUSEMOVE`).
fn soltar_movimiento(hwnd: HWND, state: &mut EditorState) {
    let Some(mover) = state.mover.take() else {
        return;
    };
    // SAFETY: se liberó la captura que tomó el WM_LBUTTONDOWN.
    unsafe { _ = ReleaseCapture() };
    // Un delta nulo lo rechaza el propio Command: no gasta un undo.
    if state.history.apply(
        &mut state.doc,
        rustcapture_core::annotate::Command::move_by(mover.index, mover.delta()),
    ) {
        state.pasos.aplicado(false);
        state.refresh_committed();
        state.dirty = true;
    }
    // SAFETY: invalidación de la propia ventana.
    unsafe { _ = InvalidateRect(Some(hwnd), None, false) };
}

fn deshacer(hwnd: HWND, state: &mut EditorState) {
    texto::commit_text(hwnd, state);
    if state.history.undo(&mut state.doc) {
        state.pasos.deshecho();
        state.seleccionado = None;
        state.refresh_committed();
        state.dirty = true;
        // SAFETY: invalidación de la propia ventana.
        unsafe { _ = InvalidateRect(Some(hwnd), None, false) };
    }
}

fn rehacer(hwnd: HWND, state: &mut EditorState) {
    texto::commit_text(hwnd, state);
    if state.history.redo(&mut state.doc) {
        state.pasos.rehecho();
        state.seleccionado = None;
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
                ventana::adoptar_estado(hwnd, lparam);
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
                        // Texto y Pasos se colocan con un clic; el resto
                        // arranca un arrastre con preview en vivo.
                        match state.herramienta {
                            math::Herramienta::Texto => {
                                texto::commit_text(hwnd, state);
                                if let Some(state) = state_mut(hwnd) {
                                    texto::abrir_edit(hwnd, state, pf, destino);
                                }
                            }
                            math::Herramienta::Pasos => colocar_paso(hwnd, state, pf),
                            math::Herramienta::Seleccion => {
                                // Clic en un objeto lo elige y arma el
                                // arrastre; en vacío, deselecciona.
                                let indice = state.doc.hit_test(pf, &state.ctx);
                                state.seleccionado = indice;
                                if let Some(index) = indice {
                                    state.mover = Some(MoverDrag {
                                        index,
                                        start: pf,
                                        current: pf,
                                    });
                                    SetCapture(hwnd);
                                }
                                _ = InvalidateRect(Some(hwnd), None, false);
                            }
                            math::Herramienta::Goma => {
                                if let Some(index) = state.doc.hit_test(pf, &state.ctx) {
                                    borrar_objeto(hwnd, state, index);
                                }
                            }
                            _ => {
                                state.drag = Some(DragState {
                                    start: pf,
                                    current: pf,
                                    points: vec![pf],
                                });
                                SetCapture(hwnd);
                            }
                        }
                    }
                }
                LRESULT(0)
            }
            WM_LBUTTONDBLCLK => {
                // Doble clic con Selección sobre un texto → reeditarlo.
                // El WM_LBUTTONDOWN previo ya armó un arrastre y tomó la
                // captura: hay que deshacer las dos cosas o el texto se
                // movería al soltar.
                if let Some(state) = state_mut(hwnd)
                    && state.herramienta == math::Herramienta::Seleccion
                {
                    let destino = dest_rect(hwnd, state);
                    let tam = (state.committed.width, state.committed.height);
                    if let Some(pf) = math::view_to_frame(punto(lparam), destino, tam)
                        && let Some(index) = state.doc.hit_test(pf, &state.ctx)
                    {
                        state.mover = None;
                        _ = ReleaseCapture();
                        state.seleccionado = Some(index);
                        if texto::abrir_reedicion(hwnd, state, index, destino) {
                            _ = InvalidateRect(Some(hwnd), None, false);
                        }
                    }
                }
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                if let Some(state) = state_mut(hwnd) {
                    let destino = dest_rect(hwnd, state);
                    let tam = (state.committed.width, state.committed.height);
                    // Arrastre de un objeto: solo mueve el preview.
                    if state.mover.is_some()
                        && let Some(pf) = math::view_to_frame(punto(lparam), destino, tam)
                        && let Some(mover) = state.mover.as_mut()
                    {
                        mover.current = pf;
                        let zona = zona_canvas(destino);
                        _ = InvalidateRect(Some(hwnd), Some(&zona), false);
                        return LRESULT(0);
                    }
                    if state.drag.is_some()
                        && let Some(pf) = math::view_to_frame(punto(lparam), destino, tam)
                        && let Some(drag) = state.drag.as_mut()
                    {
                        drag.current = pf;
                        if state.herramienta == math::Herramienta::Lapiz {
                            drag.points.push(pf);
                        }
                        // Solo cambia el canvas: invalidar su rect encajado.
                        let zona = zona_canvas(destino);
                        _ = InvalidateRect(Some(hwnd), Some(&zona), false);
                    }
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                if let Some(state) = state_mut(hwnd)
                    && state.mover.is_some()
                {
                    soltar_movimiento(hwnd, state);
                    return LRESULT(0);
                }
                if let Some(state) = state_mut(hwnd)
                    && state.drag.is_some()
                {
                    _ = ReleaseCapture();
                    if let Some(anotacion) = state.anotacion_en_curso()
                        && state.history.apply(
                            &mut state.doc,
                            rustcapture_core::annotate::Command::add(anotacion),
                        )
                    {
                        state.pasos.aplicado(false);
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
                            // Esc cancela la caja de texto abierta o, si no
                            // hay ninguna, suelta la selección.
                            if let Some(edit) = state.edit.take() {
                                texto::cerrar_edit(edit);
                                _ = InvalidateRect(Some(hwnd), None, false);
                            } else if state.seleccionado.take().is_some() {
                                _ = InvalidateRect(Some(hwnd), None, false);
                            }
                        }
                        k if k == VK_DELETE.0 => {
                            if let Some(index) = state.seleccionado {
                                borrar_objeto(hwnd, state, index);
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
                if let Some(tema) = ventana::cambio_de_tema(hwnd, lparam) {
                    theme::aplicar_titulo_oscuro(hwnd, tema.es_oscuro());
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

/// Marco punteado + 8 asas del objeto seleccionado, en el color de acento.
/// Se pinta en coordenadas de VISTA (la caja del objeto está en píxeles del
/// frame, así que se mapea con `frame_to_view`); durante el arrastre sigue
/// al objeto sumándole el delta.
fn pintar_seleccion(dc: HDC, state: &EditorState, destino: Rect, escala: Escala) {
    let Some(index) = state.seleccionado else {
        return;
    };
    let Some(objeto) = state.doc.get(index) else {
        return;
    };
    let mut caja = objeto.bounds(&state.ctx);
    if caja.is_empty() {
        return;
    }
    if let Some(mover) = state.mover.as_ref().filter(|m| m.index == index) {
        caja = caja.translated(mover.delta());
    }
    let tam = (state.committed.width, state.committed.height);
    // Esquinas del objeto → vista. La inferior-derecha usa el borde
    // exclusivo para que el marco encierre el último píxel escalado.
    let (vx0, vy0) = math::frame_to_view((caja.x, caja.y), destino, tam);
    let (vx1, vy1) = math::frame_to_view(
        (caja.right() as i32, caja.bottom() as i32),
        destino,
        tam,
    );
    let vista = Rect::new(
        vx0,
        vy0,
        (vx1 - vx0).max(1) as u32,
        (vy1 - vy0).max(1) as u32,
    );
    let paleta = theme::actual().paleta();
    let grosor = escala.px(1).max(1);
    let guion = escala.px(4).max(2);
    // SAFETY: DC del back buffer vivo; brochas efímeras de `lienzo`.
    unsafe {
        marco_punteado(dc, vista, grosor, guion, paleta.acento);
        for asa in math::asas(vista, escala.px(math::ASA_LOGICA)) {
            let rc = RECT {
                left: asa.x,
                top: asa.y,
                right: asa.right() as i32,
                bottom: asa.bottom() as i32,
            };
            lienzo::rellenar(dc, &rc, paleta.acento);
            lienzo::marco(dc, &rc, paleta.superficie);
        }
    }
}

/// Marco de guiones dibujado con rectángulos (GDI puro, sin pens punteados:
/// `PS_DOT` ignora el grosor > 1 y a 200 % se vería mal).
unsafe fn marco_punteado(
    dc: HDC,
    caja: Rect,
    grosor: i32,
    guion: i32,
    color: windows::Win32::Foundation::COLORREF,
) {
    let paso = guion * 2;
    let (x0, y0) = (caja.x, caja.y);
    let (x1, y1) = (caja.right() as i32, caja.bottom() as i32);
    let mut x = x0;
    while x < x1 {
        let fin = (x + guion).min(x1);
        for y in [y0, y1 - grosor] {
            lienzo::rellenar(
                dc,
                &RECT { left: x, top: y, right: fin, bottom: y + grosor },
                color,
            );
        }
        x += paso;
    }
    let mut y = y0;
    while y < y1 {
        let fin = (y + guion).min(y1);
        for x in [x0, x1 - grosor] {
            lienzo::rellenar(
                dc,
                &RECT { left: x, top: y, right: x + grosor, bottom: fin },
                color,
            );
        }
        y += paso;
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

        let back = lienzo::BackBuffer::nuevo(ancho, alto)?;
        let back_dc_h = back.dc();

        // Bandas superiores y status: superficie. Canvas: fondo de canvas.
        lienzo::rellenar(
            back_dc_h,
            &RECT { left: 0, top: 0, right: ancho, bottom: reparto.props_fin },
            paleta.superficie,
        );
        lienzo::rellenar(
            back_dc_h,
            &RECT { left: 0, top: reparto.status_inicio, right: ancho, bottom: alto },
            paleta.superficie,
        );
        lienzo::rellenar(
            back_dc_h,
            &RECT {
                left: 0,
                top: reparto.props_fin,
                right: ancho,
                bottom: reparto.status_inicio,
            },
            paleta.canvas,
        );
        // Separadores de 1 px: toolbar/props, props/canvas y canvas/status.
        let linea = escala.px(1).max(1);
        for y in [reparto.toolbar_fin - linea, reparto.props_fin - linea, reparto.status_inicio] {
            lienzo::rellenar(
                back_dc_h,
                &RECT { left: 0, top: y, right: ancho, bottom: y + linea },
                paleta.borde,
            );
        }

        // Property bar contextual (guarda las zonas para el hit-test).
        let banda_props = RECT {
            left: 0,
            top: reparto.toolbar_fin,
            right: ancho,
            bottom: reparto.props_fin - linea,
        };
        state.chips = props::pintar(back_dc_h, banda_props, state, escala);

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
                // Dibujando: comprometido + la anotación provisional.
                state.preview.pixels.copy_from_slice(&state.committed.pixels);
                {
                    let mut canvas = rustcapture_core::annotate::Canvas::new(&mut state.preview);
                    anotacion.render(&mut canvas, &state.ctx);
                }
                crate::gdi::copy_frame_to_dib(&state.preview, &mut state.preview_dib);
                &state.preview_dib
            } else if let Some(mover) = state.mover.as_ref().filter(|m| m.delta() != (0, 0)) {
                // Moviendo: se re-hornea el documento entero con ese objeto
                // desplazado. Es lo más caro del editor (un re-horneado por
                // WM_MOUSEMOVE) y está asumido: si se nota con muchos
                // objetos, cachear el documento sin el objeto movido.
                let (index, delta) = (mover.index, mover.delta());
                state.preview.pixels.copy_from_slice(&state.base.pixels);
                state
                    .doc
                    .render_onto_moved(&mut state.preview, &state.ctx, index, delta);
                crate::gdi::copy_frame_to_dib(&state.preview, &mut state.preview_dib);
                &state.preview_dib
            } else {
                &state.committed_dib
            };
            let src_dc = back.dc_fuente()?;
            let _s = Selected::bitmap(&src_dc, dib)?;
            SetStretchBltMode(back_dc_h, HALFTONE);
            _ = StretchBlt(
                back_dc_h,
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

        // Marco y asas del objeto seleccionado, sobre la imagen.
        if !encajado.is_empty() {
            let destino = Rect::new(
                encajado.x,
                encajado.y + reparto.props_fin,
                encajado.width,
                encajado.height,
            );
            pintar_seleccion(back_dc_h, state, destino, escala);
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
        SetBkMode(back_dc_h, TRANSPARENT);
        SetTextColor(back_dc_h, paleta.texto_secundario);
        let fuente = fuentes::fuente(fuentes::Rol::Secundario, escala);
        let fuente_previa = SelectObject(back_dc_h, fuente.into());
        let mut wide: Vec<u16> = status.encode_utf16().collect();
        let mut rc = RECT {
            left: escala.px(12),
            top: reparto.status_inicio + linea,
            right: ancho - escala.px(12),
            bottom: alto,
        };
        DrawTextW(
            back_dc_h,
            &mut wide,
            &mut rc,
            DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX,
        );
        SelectObject(back_dc_h, fuente_previa);

        back.volcar(hdc, ancho, alto);
    }
    Ok(())
}
