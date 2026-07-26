//! Editor V4 (f.21): la captura aterriza aquí. Toolbar de iconos (las
//! herramientas de anotación esperan la fusión de S6; Draw abre la
//! ventana de dibujo), canvas con la imagen encajada y barra de estado,
//! todo pintado con el tema en un back buffer.
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
    BeginPaint, BitBlt, CreateSolidBrush, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteObject,
    DrawTextW, EndPaint, FillRect, HALFTONE, HBRUSH, HDC, InvalidateRect, PAINTSTRUCT,
    RDW_ALLCHILDREN, RDW_ERASE, RDW_INVALIDATE, RedrawWindow, SRCCOPY, SelectObject, SetBkMode,
    SetStretchBltMode, SetTextColor, StretchBlt, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::DRAWITEMSTRUCT;
use windows::Win32::UI::Controls::Dialogs::{GetSaveFileNameW, OFN_OVERWRITEPROMPT, OPENFILENAMEW};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use crate::dpi::{self, Escala};
use crate::gdi::dib_from_frame;
use crate::gdi::raii::{Dib, MemDc, ScreenDc, Selected};
use crate::ui::{boton, fuentes, layout, theme, tooltip::Tooltips};

use math::Elemento;

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

struct EditorState {
    frame: Frame,
    dib: Dib,
    cerrado: bool,
    /// Hay ediciones (Draw con OK) sin guardar ni copiar.
    dirty: bool,
    /// Nombre del archivo tras Guardar como (título y status bar).
    nombre: Option<String>,
    /// Tooltips de la toolbar; viven lo que la ventana.
    tooltips: Option<Tooltips>,
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
        "Captura {}×{} — RustCapture",
        frame.width, frame.height
    ));
    // Tamaño inicial: imagen + chrome, acotado a un máximo razonable.
    let win_w = (frame.width as i32 + 60).clamp(720, 1280);
    let win_h = (frame.height as i32 + 160).clamp(400, 840);

    let state_ptr = Box::into_raw(Box::new(EditorState {
        frame,
        dib,
        cerrado: false,
        dirty: false,
        nombre: None,
        tooltips: None,
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

fn on_command(hwnd: HWND, id: u16) {
    match id {
        math::ID_GUARDAR => {
            if let Some(state) = state_mut(hwnd)
                && let Some(ruta) = guardar_como(hwnd, &state.frame)
            {
                state.dirty = false;
                // Título y status con el nombre del archivo guardado.
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
            if let Some(state) = state_mut(hwnd) {
                match crate::clipboard::ClipboardSink::new().deliver(&state.frame) {
                    Ok(()) => {
                        state.dirty = false;
                        crate::alerts::capture_beep();
                        // SAFETY: invalidación de la propia ventana.
                        unsafe { _ = InvalidateRect(Some(hwnd), None, false) };
                    }
                    Err(_) => crate::alerts::error_beep(),
                }
            }
        }
        // Draw: ventana de dibujo modal; OK devuelve el frame horneado.
        math::ID_DRAW => {
            if let Some(state) = state_mut(hwnd)
                && let Some(nuevo) = crate::draw::show_draw(state.frame.clone())
                && let Ok(screen) = ScreenDc::get()
                && let Ok(dc) = MemDc::compatible_with(&screen)
                && let Ok(dib) = dib_from_frame(&dc, &nuevo)
            {
                state.frame = nuevo;
                state.dib = dib;
                state.dirty = true;
                // SAFETY: invalidación de la propia ventana.
                unsafe { _ = InvalidateRect(Some(hwnd), None, false) };
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
                on_command(hwnd, (wparam.0 & 0xFFFF) as u16);
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

/// Compone todo el cliente (toolbar, canvas con la imagen encajada y
/// status bar) en un back buffer y lo vuelca de un BitBlt.
fn pintar(hwnd: HWND, hdc: HDC, state: &EditorState) -> windows::core::Result<()> {
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
            escala.px(math::STATUS_LOGICO),
        );

        let screen = ScreenDc::get()?;
        let back_dc = MemDc::compatible_with(&screen)?;
        let back = Dib::new_32bpp(&back_dc, ancho as u32, alto as u32)?;
        let _b = Selected::bitmap(&back_dc, &back)?;

        // Toolbar y status: superficie. Canvas: fondo de canvas del tema.
        let superficie = CreateSolidBrush(paleta.superficie);
        FillRect(
            back_dc.0,
            &RECT { left: 0, top: 0, right: ancho, bottom: reparto.toolbar_fin },
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
                top: reparto.toolbar_fin,
                right: ancho,
                bottom: reparto.status_inicio,
            },
            fondo_canvas,
        );
        _ = DeleteObject(fondo_canvas.into());
        // Separadores de 1 px bajo la toolbar y sobre el status.
        let borde = CreateSolidBrush(paleta.borde);
        let linea = escala.px(1).max(1);
        FillRect(
            back_dc.0,
            &RECT {
                left: 0,
                top: reparto.toolbar_fin - linea,
                right: ancho,
                bottom: reparto.toolbar_fin,
            },
            borde,
        );
        FillRect(
            back_dc.0,
            &RECT {
                left: 0,
                top: reparto.status_inicio,
                right: ancho,
                bottom: reparto.status_inicio + linea,
            },
            borde,
        );
        _ = DeleteObject(borde.into());

        // Imagen encajada en el canvas.
        let lienzo = (ancho, reparto.status_inicio - reparto.toolbar_fin);
        let destino = math::fit_rect((state.frame.width, state.frame.height), lienzo);
        let porcentaje = if state.frame.width > 0 {
            (u64::from(destino.width) * 100 / u64::from(state.frame.width)) as u32
        } else {
            100
        };
        if !destino.is_empty() {
            let src_dc = MemDc::compatible_with(&screen)?;
            let _s = Selected::bitmap(&src_dc, &state.dib)?;
            SetStretchBltMode(back_dc.0, HALFTONE);
            _ = StretchBlt(
                back_dc.0,
                destino.x,
                destino.y + reparto.toolbar_fin,
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

        // Status bar: dimensiones · % de encaje · destino/estado.
        let mut status = format!(
            "{} × {} px · {} %",
            state.frame.width, state.frame.height, porcentaje
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
