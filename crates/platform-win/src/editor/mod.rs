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
    BeginPaint, CreatePen, CreateSolidBrush, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteObject,
    DrawTextW, Ellipse, EndPaint, HALFTONE, HBRUSH, HDC, InvalidateRect, PAINTSTRUCT, PS_SOLID,
    SRCCOPY, SelectObject, SetBkColor, SetBkMode, SetStretchBltMode, SetTextColor, StretchBlt,
    TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::DRAWITEMSTRUCT;
use windows::Win32::UI::Controls::Dialogs::{GetSaveFileNameW, OFN_OVERWRITEPROMPT, OPENFILENAMEW};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetKeyState, ReleaseCapture, SetCapture, VK_CONTROL, VK_DELETE, VK_ESCAPE,
    VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use crate::dpi::{self, Escala};
use crate::gdi::raii::Selected;
use crate::ui::botonera;
use crate::ui::{boton, fuentes, lienzo, theme, ventana};
use crate::util::{punto, wide};

use estado::{DragState, EditorState, GirarDrag, MoverDrag};
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
/// `familia` es la tipografía por defecto del texto (`[text].familia`).
pub fn show_editor(frame: Frame, familia: &str) {
    EDITOR_ABIERTO.store(true, Ordering::SeqCst);
    if let Err(e) = run(frame, familia) {
        crate::alerts::error_box("RustCapture Editor", &e.to_string());
    }
    EDITOR_ABIERTO.store(false, Ordering::SeqCst);
}

fn run(frame: Frame, familia: &str) -> windows::core::Result<()> {
    let titulo = wide(&format!(
        "Captura {}×{} — RustCapture",
        frame.width, frame.height
    ));
    // Tamaño inicial: imagen + chrome, acotado a un máximo razonable.
    let win_w = (frame.width as i32 + 60).clamp(720, 1280);
    let win_h = (frame.height as i32 + 190).clamp(430, 840);

    let state_ptr = Box::into_raw(Box::new(EditorState::con_fuente(frame, familia)?));

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
            state.girar = None;
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

/// Arranca el arrastre de rotación si el clic (en coordenadas de CLIENTE)
/// cae sobre el asa del objeto seleccionado. Devuelve true si lo hizo.
fn empezar_giro(
    hwnd: HWND,
    state: &mut EditorState,
    p: (i32, i32),
    destino: Rect,
) -> bool {
    let Some(index) = state.seleccionado else {
        return false;
    };
    let Some(objeto) = state.doc.get(index) else {
        return false;
    };
    let caja = objeto.bounds(&state.ctx);
    if caja.is_empty() {
        return false;
    }
    let escala = Escala::from_hwnd(hwnd);
    let vista = caja_en_vista(caja, destino, &state.committed);
    let (asa, _) = asa_rotacion_y_talla(vista, escala);
    // Zona clicable algo más generosa que el dibujo, para no exigir
    // precisión de píxel.
    let holgura = escala.px(3);
    let zona = Rect::new(
        asa.x - holgura,
        asa.y - holgura,
        asa.width + 2 * holgura as u32,
        asa.height + 2 * holgura as u32,
    );
    if !zona.contains_point(p) {
        return false;
    }
    // El centro de giro es el de la caja, en píxeles del frame.
    let centro = (
        caja.x + caja.width as i32 / 2,
        caja.y + caja.height as i32 / 2,
    );
    let Some(pf) = math::view_to_frame(p, destino, (state.committed.width, state.committed.height))
    else {
        return false;
    };
    let inicial = math::angulo_hacia(centro, pf);
    state.girar = Some(GirarDrag {
        index,
        centro,
        inicial,
        actual: inicial,
        snap: false,
    });
    // SAFETY: captura del ratón en la propia ventana.
    unsafe { SetCapture(hwnd) };
    true
}

/// Caja del objeto (píxeles del frame) → rect en coordenadas de cliente.
fn caja_en_vista(caja: Rect, destino: Rect, frame: &Frame) -> Rect {
    let tam = (frame.width, frame.height);
    let (x0, y0) = math::frame_to_view((caja.x, caja.y), destino, tam);
    let (x1, y1) = math::frame_to_view(
        (caja.right() as i32, caja.bottom() as i32),
        destino,
        tam,
    );
    Rect::new(x0, y0, (x1 - x0).max(1) as u32, (y1 - y0).max(1) as u32)
}

/// Cierra el arrastre del asa de rotación como un `Command::Rotate`.
fn soltar_giro(hwnd: HWND, state: &mut EditorState) {
    let Some(mut girar) = state.girar.take() else {
        return;
    };
    // SAFETY: se liberó la captura que tomó el WM_LBUTTONDOWN.
    unsafe {
        _ = ReleaseCapture();
        // El snap se decide al soltar: así se puede pulsar o soltar Shift a
        // mitad del arrastre y vale lo último.
        girar.snap = GetKeyState(VK_SHIFT.0 as i32) < 0;
    }
    // Un giro nulo lo rechaza el propio Command: no gasta un undo.
    if state.history.apply(
        &mut state.doc,
        rustcapture_core::annotate::Command::rotate_by(girar.index, girar.delta()),
    ) {
        state.pasos.aplicado(false);
        state.refresh_committed();
        state.dirty = true;
    }
    // SAFETY: invalidación de la propia ventana.
    unsafe { _ = InvalidateRect(Some(hwnd), None, false) };
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
                    // A propósito NO se confirma con EN_KILLFOCUS: los menús
                    // de los chips y el diálogo de color son MODALES, así que
                    // le quitan el foco a la caja y confirmarla aquí la
                    // cerraría justo cuando el usuario va a cambiarle el
                    // estilo. Cerrar la caja lo cubren los demás caminos: un
                    // clic fuera de los chips, cambiar de herramienta,
                    // cualquier botón de la toolbar, Esc o abrir otra caja.
                    _ = code;
                } else {
                    on_command(hwnd, id);
                }
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                if let Some(state) = state_mut(hwnd) {
                    let p = punto(lparam);
                    // Los chips van PRIMERO y NO confirman la caja: su razón
                    // de ser es cambiar el estilo de lo que estás escribiendo
                    // (f.54), así que cerrarla sería justo lo contrario.
                    if props::on_click(hwnd, state, p) {
                        return LRESULT(0);
                    }
                    // Cualquier OTRO clic sí la confirma. No basta con
                    // EN_KILLFOCUS: pulsar en el cliente del padre NO le
                    // quita el foco al EDIT hijo, así que ese aviso no llega
                    // y la caja se quedaba abierta mientras se manipulaba
                    // otro objeto.
                    texto::commit_text(hwnd, state);
                    let destino = dest_rect(hwnd, state);
                    let tam = (state.committed.width, state.committed.height);
                    if let Some(pf) = math::view_to_frame(p, destino, tam) {
                        // Texto y Pasos se colocan con un clic; el resto
                        // arranca un arrastre con preview en vivo.
                        match state.herramienta {
                            // La caja anterior ya se confirmó arriba.
                            math::Herramienta::Texto => {
                                texto::abrir_edit(hwnd, state, pf, destino)
                            }
                            math::Herramienta::Pasos => colocar_paso(hwnd, state, pf),
                            math::Herramienta::Seleccion => {
                                // El asa de rotación gana al hit-test: está
                                // FUERA del objeto, así que si no se
                                // comprobara antes, ese clic caería en vacío
                                // y deseleccionaría.
                                if empezar_giro(hwnd, state, p, destino) {
                                    return LRESULT(0);
                                }
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
            WM_CTLCOLOREDIT => {
                // El color del chip tiene que verse mientras escribes, no
                // solo al confirmar. Un EDIT solo colorea su texto por aquí.
                if let Some(state) = state_mut(hwnd) {
                    let dc = HDC(wparam.0 as *mut _);
                    let paleta = theme::actual().paleta();
                    SetTextColor(dc, crate::util::colorref(state.props.color));
                    SetBkColor(dc, paleta.superficie);
                    // Brocha del tema, cacheada en el estado para no filtrar
                    // una por cada repintado del control.
                    return LRESULT(state.brocha_caja(paleta.superficie).0 as isize);
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
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
                    // Arrastre del asa: el ángulo sigue al puntero.
                    if state.girar.is_some()
                        && let Some(pf) = math::view_to_frame(punto(lparam), destino, tam)
                        && let Some(girar) = state.girar.as_mut()
                    {
                        girar.actual = math::angulo_hacia(girar.centro, pf);
                        girar.snap = GetKeyState(VK_SHIFT.0 as i32) < 0;
                        let zona = zona_canvas(destino);
                        _ = InvalidateRect(Some(hwnd), Some(&zona), false);
                        return LRESULT(0);
                    }
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
                    && state.girar.is_some()
                {
                    soltar_giro(hwnd, state);
                    return LRESULT(0);
                }
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
    // Girando, el recuadro sigue al objeto: se usa la caja del objeto ya
    // girado, así el marco no se queda quieto mientras el objeto rota.
    if let Some(girar) = state.girar.as_ref().filter(|g| g.index == index) {
        let mut previsto = objeto.clone();
        previsto.rotar(girar.delta());
        let girada = previsto.bounds(&state.ctx);
        if !girada.is_empty() {
            caja = girada;
        }
    }
    let vista = caja_en_vista(caja, destino, &state.committed);
    let paleta = theme::actual().paleta();
    let grosor = escala.px(1).max(1);
    let guion = escala.px(4).max(2);
    let lado_asa = escala.px(math::ASA_LOGICA);
    // SAFETY: DC del back buffer vivo; brochas efímeras de `lienzo`.
    unsafe {
        marco_punteado(dc, vista, grosor, guion, paleta.acento);
        for asa in math::asas(vista, lado_asa) {
            let rc = RECT {
                left: asa.x,
                top: asa.y,
                right: asa.right() as i32,
                bottom: asa.bottom() as i32,
            };
            lienzo::rellenar(dc, &rc, paleta.acento);
            lienzo::marco(dc, &rc, paleta.superficie);
        }
        // Asa de rotación: botón redondo con el icono de girar, junto a la
        // esquina superior derecha. Solo con el selector activo — con la
        // goma la selección sigue viva pero girar no viene al caso.
        if state.herramienta == math::Herramienta::Seleccion {
            let (rot, talla) = asa_rotacion_y_talla(vista, escala);
            circulo_relleno(dc, rot, paleta.acento, paleta.superficie);
            // Icono centrado en el botón, tintado con el color de superficie
            // para que contraste con el relleno de acento.
            let (cx, cy) = (
                rot.x + rot.width as i32 / 2,
                rot.y + rot.height as i32 / 2,
            );
            _ = crate::ui::iconos::pintar(
                dc,
                crate::ui::iconos::Icono::EditRotate,
                talla,
                cx - talla as i32 / 2,
                cy - talla as i32 / 2,
                paleta.superficie,
                crate::ui::iconos::OPACO,
            );
        }
    }
}

/// Caja del botón de rotación y talla del icono que lleva dentro. Un solo
/// sitio: lo consumen el pintado y el hit-test del clic, que tienen que
/// coincidir exactamente o el asa se vería donde no se puede pulsar.
fn asa_rotacion_y_talla(vista: Rect, escala: Escala) -> (Rect, u32) {
    let talla = crate::ui::iconos::talla_para_dpi(escala.dpi());
    let diametro = talla as i32 + 2 * escala.px(math::ASA_ROT_MARGEN);
    let asa = math::asa_rotacion(vista, diametro, escala.px(math::SEPARACION_LOGICA));
    (asa, talla)
}

/// Círculo relleno con borde, dibujado con `Ellipse` de GDI (el asa de
/// rotación; las de redimensionado son cuadradas a propósito).
unsafe fn circulo_relleno(
    dc: HDC,
    caja: Rect,
    relleno: windows::Win32::Foundation::COLORREF,
    borde: windows::Win32::Foundation::COLORREF,
) {
    // SAFETY: brochas y lápiz efímeros, seleccionados y restaurados aquí.
    unsafe {
        let brocha = CreateSolidBrush(relleno);
        let lapiz = CreatePen(PS_SOLID, 1, borde);
        let brocha_previa = SelectObject(dc, brocha.into());
        let lapiz_previo = SelectObject(dc, lapiz.into());
        _ = Ellipse(
            dc,
            caja.x,
            caja.y,
            caja.right() as i32,
            caja.bottom() as i32,
        );
        SelectObject(dc, brocha_previa);
        SelectObject(dc, lapiz_previo);
        _ = DeleteObject(brocha.into());
        _ = DeleteObject(lapiz.into());
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
            } else if let Some(girar) = state.girar.as_ref().filter(|g| g.delta() != 0.0) {
                // Girando: mismo coste que mover (un re-horneado por
                // mousemove), asumido en el plan.
                let (index, delta) = (girar.index, girar.delta());
                state.preview.pixels.copy_from_slice(&state.base.pixels);
                state
                    .doc
                    .render_onto_rotated(&mut state.preview, &state.ctx, index, delta);
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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::gdi::raii::{Dib, MemDc, ScreenDc, Selected};
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::Graphics::Gdi::GdiFlush;

    /// Pinta con `f` sobre un DIB de 40×40 y devuelve sus píxeles BGRA.
    fn pintar_en_dib(f: impl FnOnce(HDC)) -> Vec<u8> {
        let screen = ScreenDc::get().expect("DC de pantalla");
        let mem = MemDc::compatible_with(&screen).expect("DC de memoria");
        let mut dib = Dib::new_32bpp(&mem, 40, 40).expect("DIB");
        dib.bits_mut().fill(0); // negro
        let sel = Selected::bitmap(&mem, &dib).expect("seleccionar DIB");
        f(mem.0);
        // SAFETY: GDI debe terminar de escribir antes de leer los bits.
        unsafe { _ = GdiFlush() };
        drop(sel);
        dib.bits().to_vec()
    }

    fn pintado(bits: &[u8], x: usize, y: usize) -> bool {
        let i = (y * 40 + x) * 4;
        bits[i] != 0 || bits[i + 1] != 0 || bits[i + 2] != 0
    }

    /// El asa de rotación se dibuja con `Ellipse`, que es la única primitiva
    /// GDI del editor que no pasa por `ui::lienzo`. Este test confirma que
    /// de verdad pinta píxeles (y no falla en silencio).
    #[test]
    fn el_circulo_del_asa_pinta_pixeles() {
        let rojo = COLORREF(0x0000FF);
        let bits = pintar_en_dib(|dc| {
            // SAFETY: DC de memoria vivo con un DIB seleccionado.
            unsafe { circulo_relleno(dc, Rect::new(10, 10, 12, 12), rojo, rojo) };
        });
        // Centro del círculo relleno.
        assert!(pintado(&bits, 16, 16), "el círculo no pintó su centro");
        // Borde superior e izquierdo del círculo.
        assert!(pintado(&bits, 16, 10) || pintado(&bits, 16, 11), "borde arriba");
        assert!(pintado(&bits, 10, 16) || pintado(&bits, 11, 16), "borde izq.");
        // Fuera del círculo, intacto.
        assert!(!pintado(&bits, 2, 2), "pintó fuera de su caja");
        assert!(!pintado(&bits, 30, 30), "pintó fuera de su caja");
    }

    /// Pinta el chrome de selección REAL sobre un DIB y devuelve sus bits.
    /// Es la única forma de comprobar sin ojos que el asa de rotación llega
    /// a dibujarse: `circulo_relleno` puede funcionar aislado y aun así no
    /// pintarse nada si el resto de `pintar_seleccion` no llega ahí.
    fn pintar_chrome(lado: u32) -> (Vec<u8>, Rect, usize) {
        let screen = ScreenDc::get().expect("DC de pantalla");
        let mem = MemDc::compatible_with(&screen).expect("DC de memoria");
        let mut dib = Dib::new_32bpp(&mem, lado, lado).expect("DIB");
        dib.bits_mut().fill(0);
        let sel = Selected::bitmap(&mem, &dib).expect("seleccionar DIB");

        // Estado con un rectángulo colocado y seleccionado.
        let base = Frame::filled(lado, lado, [0, 0, 0, 255]);
        let mut state = EditorState::con_fuente(base, "Segoe UI").expect("estado");
        state.herramienta = math::Herramienta::Seleccion;
        let objeto: rustcapture_core::annotate::Objeto =
            rustcapture_core::annotate::annotations::RectAnnotation {
                rect: Rect::new(60, 90, 80, 60),
                style: rustcapture_core::annotate::Style {
                    color: rustcapture_core::annotate::Color::rgb(255, 0, 0),
                    thickness: 2,
                },
            }
            .into();
        assert!(state.history.apply(
            &mut state.doc,
            rustcapture_core::annotate::Command::add(objeto)
        ));
        state.seleccionado = Some(0);
        let caja = state.doc.get(0).unwrap().bounds(&state.ctx);

        // Imagen mapeada 1:1 en el cliente: vista == píxeles del frame.
        let destino = Rect::new(0, 0, lado, lado);
        pintar_seleccion(mem.0, &state, destino, Escala::nueva(96));
        // SAFETY: GDI debe terminar antes de leer los bits.
        unsafe { _ = GdiFlush() };
        drop(sel);
        (dib.bits().to_vec(), caja, lado as usize)
    }

    #[test]
    fn el_chrome_de_seleccion_pinta_el_asa_de_rotacion() {
        let (bits, caja, lado) = pintar_chrome(240);
        let hay = |x: i32, y: i32| -> bool {
            if x < 0 || y < 0 || x as usize >= lado || y as usize >= lado {
                return false;
            }
            let i = (y as usize * lado + x as usize) * 4;
            bits[i] != 0 || bits[i + 1] != 0 || bits[i + 2] != 0
        };
        // El recuadro se pinta (esto ya funcionaba).
        assert!(
            (caja.x - 2..caja.x + 8).any(|x| hay(x, caja.y)),
            "no se pintó el marco"
        );
        // Y el asa de rotación, FUERA de la caja, también.
        let vista = Rect::new(caja.x, caja.y, caja.width, caja.height);
        let (asa, _) = asa_rotacion_y_talla(vista, Escala::nueva(96));
        let pixeles_asa = (asa.x..asa.right() as i32)
            .flat_map(|x| (asa.y..asa.bottom() as i32).map(move |y| (x, y)))
            .filter(|&(x, y)| hay(x, y))
            .count();
        assert!(
            pixeles_asa > 20,
            "el asa de rotación solo pintó {pixeles_asa} píxeles en {asa:?}"
        );
    }

    /// La banda de propiedades tiene que producir CUATRO zonas clicables con
    /// la herramienta Texto, la primera el chip de fuente con el nombre real
    /// del catálogo. Comprueba lo que `chips` compone Y lo que `pintar` mide:
    /// una etiqueta vacía daría una zona de ancho 0, invisible e impulsable.
    #[test]
    fn la_banda_de_propiedades_pinta_el_chip_de_fuente() {
        let screen = ScreenDc::get().expect("DC de pantalla");
        let mem = MemDc::compatible_with(&screen).expect("DC de memoria");
        let mut dib = Dib::new_32bpp(&mem, 400, 40).expect("DIB");
        dib.bits_mut().fill(0);
        let sel = Selected::bitmap(&mem, &dib).expect("seleccionar DIB");

        let mut state =
            EditorState::con_fuente(Frame::filled(60, 60, [0, 0, 0, 255]), "Segoe UI")
                .expect("estado");
        state.herramienta = math::Herramienta::Texto;
        // El catálogo del sistema tiene que haber dado nombre a la familia 0.
        let nombre = state.ctx.nombre(state.props.familia);
        assert_eq!(nombre, Some("Segoe UI"), "la familia por defecto no cargó");

        let banda = RECT {
            left: 0,
            top: 0,
            right: 400,
            bottom: 26,
        };
        let zonas = props::pintar(mem.0, banda, &state, Escala::nueva(96));
        // SAFETY: GDI debe terminar antes de soltar el DIB.
        unsafe { _ = GdiFlush() };
        drop(sel);

        assert_eq!(zonas.len(), 4, "zonas: {zonas:?}");
        // La primera es la fuente y tiene ancho real (si no, no se ve).
        assert_eq!(zonas[0].1, props::Accion::MenuFuente);
        let ancho = zonas[0].0.right - zonas[0].0.left;
        assert!(ancho > 10, "el chip de fuente mide {ancho} px");
        // Y las zonas no se solapan ni van en desorden.
        for par in zonas.windows(2) {
            assert!(
                par[1].0.left >= par[0].0.right,
                "chips solapados: {:?} y {:?}",
                par[0],
                par[1]
            );
        }
    }

    /// El asa de rotación tiene que quedar POR ENCIMA del recuadro y con el
    /// icono dentro. Si el brazo fuese menor que el radio, el botón pisaría
    /// la caja; si el icono no cupiera, se saldría del círculo.
    #[test]
    fn el_asa_de_rotacion_no_pisa_el_recuadro_y_cabe_el_icono() {
        for dpi in [96u32, 120, 144, 192] {
            let escala = Escala::nueva(dpi);
            let vista = Rect::new(100, 200, 80, 60);
            let (asa, talla) = asa_rotacion_y_talla(vista, escala);
            assert!(
                asa.x >= vista.right() as i32,
                "dpi {dpi}: el asa pisa el recuadro"
            );
            assert!(
                talla as i32 <= asa.width as i32,
                "dpi {dpi}: icono de {talla} no cabe en un asa de {}",
                asa.width
            );
            // A la altura del borde superior.
            assert_eq!(
                asa.y + asa.height as i32 / 2,
                vista.y,
                "dpi {dpi}: no está a la altura del borde superior"
            );
        }
    }

    /// Un asa del tamaño real (6 px lógicos a 100 %) tiene que pintar algo:
    /// si `Ellipse` con una caja tan pequeña no rellenara nada, el asa sería
    /// invisible aunque el código se ejecute.
    #[test]
    fn un_asa_de_seis_pixeles_sigue_pintando() {
        let rojo = COLORREF(0x0000FF);
        let bits = pintar_en_dib(|dc| {
            // SAFETY: DC de memoria vivo con un DIB seleccionado.
            unsafe { circulo_relleno(dc, Rect::new(17, 17, 6, 6), rojo, rojo) };
        });
        let cuenta = (0..40)
            .flat_map(|x| (0..40).map(move |y| (x, y)))
            .filter(|&(x, y)| pintado(&bits, x, y))
            .count();
        assert!(cuenta >= 8, "un asa de 6 px solo pinta {cuenta} píxeles");
    }
}
