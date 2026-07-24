//! Ventana de dibujo (Ventana2, Slice C de F3): paleta de herramientas
//! sobre el motor de anotación del core, preview en vivo, texto in situ
//! y OK que devuelve el frame horneado al editor.
//!
//! Hilos: SOLO hilo de UI, bucle modal (patrón del editor). El frame
//! «comprometido» (base + documento) se cachea como DIB y solo se
//! regenera al cambiar el documento.

pub(crate) mod math;

use rustcapture_core::annotate::annotations::{
    Annotation, ArrowAnnotation, EllipseAnnotation, HighlightAnnotation, LineAnnotation,
    PenAnnotation, RectAnnotation, TextAnnotation,
};
use rustcapture_core::annotate::{
    Color, Command, Document, History, RenderContext, Style, TextStyle,
};
use rustcapture_core::ports::{Frame, Rect};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BLACK_BRUSH, BeginPaint, CLIP_DEFAULT_PRECIS, COLOR_APPWORKSPACE, COLOR_BTNFACE, CreateFontW,
    CreateSolidBrush, DEFAULT_CHARSET, DEFAULT_PITCH, DEFAULT_QUALITY, DeleteObject, EndPaint,
    FW_BOLD, FW_NORMAL, FillRect, FrameRect, GetStockObject, GetSysColorBrush, HALFTONE, HBRUSH,
    HDC, HFONT, InvalidateRect, OUT_DEFAULT_PRECIS, PAINTSTRUCT, SRCCOPY, SetStretchBltMode,
    StretchBlt,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::Dialogs::{CC_FULLOPEN, CC_RGBINIT, CHOOSECOLORW, ChooseColorW};
use windows::Win32::UI::Controls::{DRAWITEMSTRUCT, ODS_SELECTED};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetKeyState, ReleaseCapture, SetCapture, SetFocus, VK_CONTROL, VK_ESCAPE,
};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use crate::gdi::dib_from_frame;
use crate::gdi::raii::{Dib, MemDc, ScreenDc, Selected};

const PALETTE_W: i32 = 96;
const BOTTOM_H: i32 = 48;
const ID_TOOL_BASE: u16 = 4001; // Rect..Texto = 4001..=4007 (orden del enum)
const ID_UNDO: u16 = 4010;
const ID_REDO: u16 = 4011;
const ID_MAS_COLOR: u16 = 4020;
const ID_COLOR_BASE: u16 = 4030; // 8 swatches 4030..=4037
const ID_GROSOR_BASE: u16 = 4040; // 4040..=4044 → [1,2,3,5,8]
const ID_TAMANO_BASE: u16 = 4050; // 4050..=4054 → [12,16,20,28,36]
const ID_BOLD: u16 = 4060;
const ID_OK: u16 = 4070;
const ID_CANCEL: u16 = 4071;
const ID_EDIT_TEXT: u16 = 4080;
const WM_APP_CANCEL_TEXT: u32 = WM_APP + 10;

const GROSORES: [u32; 5] = [1, 2, 3, 5, 8];
const TAMANOS: [f32; 5] = [12.0, 16.0, 20.0, 28.0, 36.0];
const COLORES: [Color; 8] = [
    Color::rgb(0, 0, 0),
    Color::rgb(255, 255, 255),
    Color::rgb(255, 0, 0),
    Color::rgb(0, 200, 0),
    Color::rgb(0, 90, 255),
    Color::rgb(255, 220, 0),
    Color::rgb(255, 140, 0),
    Color::rgb(128, 0, 128),
];

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Rect,
    Ellipse,
    Line,
    Arrow,
    Pen,
    Highlight,
    Text,
}

const TOOLS: [(Tool, PCWSTR); 7] = [
    (Tool::Rect, w!("Rect")),
    (Tool::Ellipse, w!("Elipse")),
    (Tool::Line, w!("Línea")),
    (Tool::Arrow, w!("Flecha")),
    (Tool::Pen, w!("Lápiz")),
    (Tool::Highlight, w!("Resalt.")),
    (Tool::Text, w!("Texto")),
];

struct DragState {
    start: (i32, i32),
    current: (i32, i32),
    points: Vec<(i32, i32)>,
}

struct EditBox {
    hwnd: HWND,
    pos_frame: (i32, i32),
    font: HFONT,
}

struct DrawState {
    base: Frame,
    committed: Frame,
    committed_dib: Dib,
    /// Buffers persistentes del preview: se reescriben en cada arrastre
    /// sin asignar memoria (las dimensiones nunca cambian).
    preview: Frame,
    preview_dib: Dib,
    doc: Document,
    history: History,
    ctx: RenderContext,
    tiene_fuente: bool,
    tool: Tool,
    color: Color,
    thickness: u32,
    text_size: f32,
    bold: bool,
    drag: Option<DragState>,
    edit: Option<EditBox>,
    outcome: Option<Option<Frame>>,
}

/// Abre la ventana de dibujo con la captura; bloquea el hilo de UI.
/// `Some(frame)` = OK con las anotaciones horneadas; `None` = cancelado.
pub fn show_draw(base: Frame) -> Option<Frame> {
    match run(base) {
        Ok(resultado) => resultado,
        Err(e) => {
            crate::alerts::error_box("RustCapture Draw", &e.to_string());
            None
        }
    }
}

fn cargar_contexto() -> (RenderContext, bool) {
    let normal = std::fs::read("C:/Windows/Fonts/segoeui.ttf");
    let bold = std::fs::read("C:/Windows/Fonts/segoeuib.ttf");
    match (normal, bold) {
        (Ok(n), Ok(b)) => match RenderContext::new(&n, &b) {
            Ok(ctx) => (ctx, true),
            Err(_) => (RenderContext::sin_fuente(), false),
        },
        _ => (RenderContext::sin_fuente(), false),
    }
}

fn run(base: Frame) -> windows::core::Result<Option<Frame>> {
    let (ctx, tiene_fuente) = cargar_contexto();
    let screen = ScreenDc::get()?;
    let dc = MemDc::compatible_with(&screen)?;
    let committed = base.clone();
    let committed_dib = dib_from_frame(&dc, &committed)?;
    let preview = committed.clone();
    let preview_dib = dib_from_frame(&dc, &preview)?;
    drop(dc);
    drop(screen);

    let win_w = (base.width as i32 + PALETTE_W + 40).clamp(640, 1360);
    let win_h = (base.height as i32 + BOTTOM_H + 80).clamp(420, 900);

    let state_ptr = Box::into_raw(Box::new(DrawState {
        base,
        committed,
        committed_dib,
        preview,
        preview_dib,
        doc: Document::new(),
        history: History::new(),
        ctx,
        tiene_fuente,
        tool: Tool::Rect,
        color: Color::rgb(255, 0, 0),
        thickness: 3,
        text_size: 20.0,
        bold: false,
        drag: None,
        edit: None,
        outcome: None,
    }));

    // SAFETY: patrón del editor: el estado lo posee esta función; la
    // ventana se destruye antes del Box::from_raw.
    let resultado = unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = w!("RustCaptureDraw");
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
            w!("RustCapture Draw"),
            WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_VISIBLE,
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
        while (*state_ptr).outcome.is_none() && GetMessageW(&mut msg, None, 0, 0).as_bool() {
            _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let state = Box::from_raw(state_ptr);
        state.outcome.flatten()
    };
    Ok(resultado)
}

fn state_mut<'a>(hwnd: HWND) -> Option<&'a mut DrawState> {
    // SAFETY: puntero puesto por WM_NCCREATE; liberado tras el bucle.
    unsafe { ((GetWindowLongPtrW(hwnd, GWLP_USERDATA)) as *mut DrawState).as_mut() }
}

// PENDIENTE(limpieza): duplicada en overlay/mod.rs (y `wide` está en
// alerts y editor); extraer a un módulo util interno del crate.
fn punto(lparam: LPARAM) -> (i32, i32) {
    (
        (lparam.0 & 0xFFFF) as i16 as i32,
        ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
    )
}

/// Regenera el frame comprometido (base + documento) sobre los buffers
/// existentes — sin asignar (las dimensiones son fijas).
fn refresh_committed(state: &mut DrawState) {
    state.committed.pixels.copy_from_slice(&state.base.pixels);
    state.doc.render_onto(&mut state.committed, &state.ctx);
    crate::gdi::copy_frame_to_dib(&state.committed, &mut state.committed_dib);
}

/// Área del lienzo (cliente menos paleta y barra) y rect destino encajado.
fn dest_rect(hwnd: HWND, state: &DrawState) -> Rect {
    let mut client = RECT::default();
    // SAFETY: consulta sin precondiciones.
    unsafe { _ = GetClientRect(hwnd, &mut client) };
    let lienzo = (
        client.right - client.left - PALETTE_W,
        client.bottom - client.top - BOTTOM_H,
    );
    let encajado =
        crate::editor::math::fit_rect((state.committed.width, state.committed.height), lienzo);
    Rect::new(
        encajado.x + PALETTE_W,
        encajado.y,
        encajado.width,
        encajado.height,
    )
}

/// Anotación provisional del arrastre actual (None con Texto o sin drag).
fn anotacion_en_curso(state: &DrawState) -> Option<Box<dyn Annotation>> {
    let drag = state.drag.as_ref()?;
    let style = Style {
        color: state.color,
        thickness: state.thickness,
    };
    let rect = crate::overlay::math::rect_between(drag.start, drag.current);
    Some(match state.tool {
        Tool::Rect => Box::new(RectAnnotation { rect, style }),
        Tool::Ellipse => Box::new(EllipseAnnotation { rect, style }),
        Tool::Line => Box::new(LineAnnotation {
            from: drag.start,
            to: drag.current,
            style,
        }),
        Tool::Arrow => Box::new(ArrowAnnotation {
            from: drag.start,
            to: drag.current,
            style,
        }),
        // PENDIENTE(rendimiento): clona los puntos en cada repintado del
        // arrastre (O(n) por frame). Solo importa con trazos larguísimos;
        // arreglable pasando el builder a préstamos.
        Tool::Pen => Box::new(PenAnnotation {
            points: drag.points.clone(),
            style,
        }),
        Tool::Highlight => Box::new(HighlightAnnotation {
            rect,
            color: Color::rgba(state.color.r, state.color.g, state.color.b, 128),
        }),
        Tool::Text => return None,
    })
}

fn crear_controles(hwnd: HWND, state: &DrawState) {
    let mut client = RECT::default();
    // SAFETY: creación de controles hijos estándar durante WM_CREATE.
    unsafe {
        _ = GetClientRect(hwnd, &mut client);
        let boton =
            |texto: PCWSTR, id: u16, x: i32, y: i32, w: i32, h: i32, style: WINDOW_STYLE| {
                _ = CreateWindowExW(
                    WINDOW_EX_STYLE::default(),
                    w!("BUTTON"),
                    texto,
                    WS_CHILD | WS_VISIBLE | style,
                    x,
                    y,
                    w,
                    h,
                    Some(hwnd),
                    Some(HMENU(id as usize as *mut _)),
                    None,
                    None,
                );
            };
        // Paleta izquierda: herramientas + deshacer/rehacer.
        for (i, (_, texto)) in TOOLS.iter().enumerate() {
            boton(
                *texto,
                ID_TOOL_BASE + i as u16,
                6,
                10 + i as i32 * 32,
                84,
                26,
                WINDOW_STYLE::default(),
            );
        }
        boton(
            w!("Deshacer"),
            ID_UNDO,
            6,
            10 + 7 * 32 + 12,
            84,
            26,
            WINDOW_STYLE::default(),
        );
        boton(
            w!("Rehacer"),
            ID_REDO,
            6,
            10 + 8 * 32 + 12,
            84,
            26,
            WINDOW_STYLE::default(),
        );
        if !state.tiene_fuente
            && let Ok(btn) = GetDlgItem(Some(hwnd), (ID_TOOL_BASE + 6) as i32)
        {
            _ = EnableWindow(btn, false);
        }
        // Barra inferior.
        let y = client.bottom - BOTTOM_H + 10;
        for i in 0..COLORES.len() {
            boton(
                PCWSTR::null(),
                ID_COLOR_BASE + i as u16,
                PALETTE_W + 8 + i as i32 * 28,
                y,
                24,
                24,
                WINDOW_STYLE(BS_OWNERDRAW as u32),
            );
        }
        let x_mas = PALETTE_W + 8 + 8 * 28 + 4;
        boton(
            w!("Más…"),
            ID_MAS_COLOR,
            x_mas,
            y,
            52,
            24,
            WINDOW_STYLE::default(),
        );
        let x_grosor = x_mas + 60;
        for (i, g) in GROSORES.iter().enumerate() {
            let texto = match g {
                1 => w!("1"),
                2 => w!("2"),
                3 => w!("3"),
                5 => w!("5"),
                _ => w!("8"),
            };
            boton(
                texto,
                ID_GROSOR_BASE + i as u16,
                x_grosor + i as i32 * 26,
                y,
                24,
                24,
                WINDOW_STYLE::default(),
            );
        }
        let x_tamano = x_grosor + 5 * 26 + 8;
        for (i, t) in TAMANOS.iter().enumerate() {
            let texto = match *t as u32 {
                12 => w!("12"),
                16 => w!("16"),
                20 => w!("20"),
                28 => w!("28"),
                _ => w!("36"),
            };
            boton(
                texto,
                ID_TAMANO_BASE + i as u16,
                x_tamano + i as i32 * 30,
                y,
                28,
                24,
                WINDOW_STYLE::default(),
            );
        }
        boton(
            w!("B"),
            ID_BOLD,
            x_tamano + 5 * 30 + 4,
            y,
            28,
            24,
            WINDOW_STYLE(BS_AUTOCHECKBOX as u32 | BS_PUSHLIKE as u32),
        );
        boton(
            w!("Cancelar"),
            ID_CANCEL,
            client.right - 90,
            y,
            80,
            26,
            WINDOW_STYLE::default(),
        );
        boton(
            w!("OK"),
            ID_OK,
            client.right - 180,
            y,
            80,
            26,
            WINDOW_STYLE::default(),
        );
        actualizar_controles_texto(hwnd, state);
    }
}

/// Habilita tamaño/negrita solo con la herramienta Texto activa.
fn actualizar_controles_texto(hwnd: HWND, state: &DrawState) {
    let activo = state.tool == Tool::Text && state.tiene_fuente;
    // SAFETY: consultas/habilitación de controles hijos propios.
    unsafe {
        for i in 0..TAMANOS.len() {
            if let Ok(btn) = GetDlgItem(Some(hwnd), (ID_TAMANO_BASE + i as u16) as i32) {
                _ = EnableWindow(btn, activo);
            }
        }
        if let Ok(btn) = GetDlgItem(Some(hwnd), ID_BOLD as i32) {
            _ = EnableWindow(btn, activo);
        }
    }
}

/// Diálogo de color estándar de Windows.
fn elegir_color(hwnd: HWND, state: &mut DrawState) {
    static mut CUSTOM: [COLORREF; 16] = [COLORREF(0x00FFFFFF); 16];
    let actual =
        COLORREF(state.color.r as u32 | (state.color.g as u32) << 8 | (state.color.b as u32) << 16);
    // SAFETY: struct completo; CUSTOM es estático y solo se usa aquí, en
    // el hilo de UI.
    unsafe {
        let mut cc = CHOOSECOLORW {
            lStructSize: size_of::<CHOOSECOLORW>() as u32,
            hwndOwner: hwnd,
            rgbResult: actual,
            lpCustColors: &raw mut CUSTOM as *mut COLORREF,
            Flags: CC_FULLOPEN | CC_RGBINIT,
            ..Default::default()
        };
        if ChooseColorW(&mut cc).as_bool() {
            let v = cc.rgbResult.0;
            state.color = Color::rgb(
                (v & 0xFF) as u8,
                ((v >> 8) & 0xFF) as u8,
                ((v >> 16) & 0xFF) as u8,
            );
        }
    }
}

fn deshacer(hwnd: HWND, state: &mut DrawState) {
    if state.history.undo(&mut state.doc) {
        refresh_committed(state);
        // SAFETY: invalidación de la propia ventana.
        unsafe { _ = InvalidateRect(Some(hwnd), None, true) };
    }
}

fn rehacer(hwnd: HWND, state: &mut DrawState) {
    if state.history.redo(&mut state.doc) {
        refresh_committed(state);
        // SAFETY: invalidación de la propia ventana.
        unsafe { _ = InvalidateRect(Some(hwnd), None, true) };
    }
}

/// Cancelación con confirmación si hay anotaciones.
fn confirmar_descarte(hwnd: HWND, state: &mut DrawState) {
    let descartar = state.doc.is_empty() || {
        // SAFETY: MessageBox modal sobre la propia ventana.
        unsafe {
            MessageBoxW(
                Some(hwnd),
                w!("¿Descartar las anotaciones?"),
                w!("RustCapture Draw"),
                MB_YESNO | MB_ICONQUESTION,
            ) == IDYES
        }
    };
    if descartar {
        state.outcome = Some(None);
        // SAFETY: destruir la propia ventana.
        unsafe { _ = DestroyWindow(hwnd) };
    }
}

// ------------------------- texto in situ (Task 3) -------------------------

fn abrir_edit(hwnd: HWND, state: &mut DrawState, pos_frame: (i32, i32)) {
    let destino = dest_rect(hwnd, state);
    let (vx, vy) = math::frame_to_view(
        pos_frame,
        destino,
        (state.committed.width, state.committed.height),
    );
    // SAFETY: creación de un EDIT hijo + fuente propia; ambos se
    // destruyen en commit/cancel.
    unsafe {
        let Ok(edit) = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("EDIT"),
            PCWSTR::null(),
            WS_CHILD
                | WS_VISIBLE
                | WS_BORDER
                | WINDOW_STYLE((ES_MULTILINE | ES_AUTOVSCROLL) as u32),
            vx,
            vy,
            220,
            70,
            Some(hwnd),
            Some(HMENU(ID_EDIT_TEXT as usize as *mut _)),
            None,
            None,
        ) else {
            return;
        };
        let font = CreateFontW(
            state.text_size.round() as i32,
            0,
            0,
            0,
            if state.bold {
                FW_BOLD.0 as i32
            } else {
                FW_NORMAL.0 as i32
            },
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
        if !font.is_invalid() {
            SendMessageW(
                edit,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );
        }
        _ = SetWindowSubclass(edit, Some(edit_subclass), 1, 0);
        _ = SetFocus(Some(edit));
        state.edit = Some(EditBox {
            hwnd: edit,
            pos_frame,
            font,
        });
    }
}

/// Subclass del EDIT: Esc cancela la caja sin confirmar.
unsafe extern "system" fn edit_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    // SAFETY: reenvío estándar de subclass.
    unsafe {
        if msg == WM_KEYDOWN && wparam.0 as u16 == VK_ESCAPE.0 {
            _ = PostMessageW(
                Some(GetParent(hwnd).unwrap_or_default()),
                WM_APP_CANCEL_TEXT,
                WPARAM(0),
                LPARAM(0),
            );
            return LRESULT(0);
        }
        DefSubclassProc(hwnd, msg, wparam, lparam)
    }
}

fn cerrar_edit(edit: EditBox) {
    // SAFETY: destruye el EDIT y su fuente, creados por abrir_edit.
    unsafe {
        _ = DestroyWindow(edit.hwnd);
        if !edit.font.is_invalid() {
            _ = DeleteObject(edit.font.into());
        }
    }
}

/// Confirma la caja de texto (pérdida de foco u OK): texto no vacío →
/// anotación; siempre destruye el EDIT.
fn commit_text(hwnd: HWND, state: &mut DrawState) {
    let Some(edit) = state.edit.take() else {
        return;
    };
    let mut buffer = [0u16; 2048];
    // SAFETY: lectura del texto del EDIT vivo.
    let len = unsafe { GetWindowTextW(edit.hwnd, &mut buffer) } as usize;
    let texto = String::from_utf16_lossy(&buffer[..len]);
    let texto = texto.replace("\r\n", "\n");
    let pos = edit.pos_frame;
    cerrar_edit(edit);
    if !texto.trim().is_empty() {
        state.history.apply(
            &mut state.doc,
            Command::add(Box::new(TextAnnotation {
                pos,
                text: texto,
                style: TextStyle {
                    color: state.color,
                    size: state.text_size,
                    bold: state.bold,
                },
            })),
        );
        refresh_committed(state);
    }
    // SAFETY: invalidación de la propia ventana.
    unsafe { _ = InvalidateRect(Some(hwnd), None, true) };
}

// --------------------------------------------------------------------------

fn on_command(hwnd: HWND, id: u16, state: &mut DrawState) {
    match id {
        _ if (ID_TOOL_BASE..ID_TOOL_BASE + 7).contains(&id) => {
            state.tool = TOOLS[(id - ID_TOOL_BASE) as usize].0;
            actualizar_controles_texto(hwnd, state);
        }
        _ if (ID_COLOR_BASE..ID_COLOR_BASE + 8).contains(&id) => {
            state.color = COLORES[(id - ID_COLOR_BASE) as usize];
        }
        _ if (ID_GROSOR_BASE..ID_GROSOR_BASE + 5).contains(&id) => {
            state.thickness = GROSORES[(id - ID_GROSOR_BASE) as usize];
        }
        _ if (ID_TAMANO_BASE..ID_TAMANO_BASE + 5).contains(&id) => {
            state.text_size = TAMANOS[(id - ID_TAMANO_BASE) as usize];
        }
        ID_BOLD => state.bold = !state.bold,
        ID_MAS_COLOR => elegir_color(hwnd, state),
        ID_UNDO => deshacer(hwnd, state),
        ID_REDO => rehacer(hwnd, state),
        ID_OK => {
            commit_text(hwnd, state);
            state.outcome = Some(Some(state.committed.clone()));
            // SAFETY: destruir la propia ventana.
            unsafe { _ = DestroyWindow(hwnd) };
        }
        ID_CANCEL => confirmar_descarte(hwnd, state),
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
                if let Some(state) = state_mut(hwnd) {
                    crear_controles(hwnd, state);
                }
                LRESULT(0)
            }
            WM_DRAWITEM => {
                let dis = &*(lparam.0 as *const DRAWITEMSTRUCT);
                let idx = dis.CtlID as u16;
                if (ID_COLOR_BASE..ID_COLOR_BASE + 8).contains(&idx) {
                    let c = COLORES[(idx - ID_COLOR_BASE) as usize];
                    let brush: HBRUSH = CreateSolidBrush(COLORREF(
                        c.r as u32 | (c.g as u32) << 8 | (c.b as u32) << 16,
                    ));
                    FillRect(dis.hDC, &dis.rcItem, brush);
                    _ = DeleteObject(brush.into());
                    let borde = if (dis.itemState.0 & ODS_SELECTED.0) != 0 {
                        GetSysColorBrush(COLOR_BTNFACE)
                    } else {
                        HBRUSH(GetStockObject(BLACK_BRUSH).0)
                    };
                    FrameRect(dis.hDC, &dis.rcItem, borde);
                    LRESULT(1)
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
            WM_LBUTTONDOWN => {
                if let Some(state) = state_mut(hwnd) {
                    let destino = dest_rect(hwnd, state);
                    let tam = (state.committed.width, state.committed.height);
                    if let Some(pf) = math::view_to_frame(punto(lparam), destino, tam) {
                        if state.tool == Tool::Text {
                            commit_text(hwnd, state);
                            if let Some(state) = state_mut(hwnd) {
                                abrir_edit(hwnd, state, pf);
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
                        if state.tool == Tool::Pen {
                            drag.points.push(pf);
                        }
                        _ = InvalidateRect(Some(hwnd), None, false);
                    }
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                if let Some(state) = state_mut(hwnd)
                    && state.drag.is_some()
                {
                    _ = ReleaseCapture();
                    if let Some(anotacion) = anotacion_en_curso(state) {
                        state.history.apply(&mut state.doc, Command::add(anotacion));
                        refresh_committed(state);
                    }
                    state.drag = None;
                    _ = InvalidateRect(Some(hwnd), None, true);
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if let Some(state) = state_mut(hwnd) {
                    let ctrl = GetKeyState(VK_CONTROL.0 as i32) < 0;
                    match wparam.0 as u16 {
                        k if k == VK_ESCAPE.0 => confirmar_descarte(hwnd, state),
                        k if ctrl && k == b'Z' as u16 => deshacer(hwnd, state),
                        k if ctrl && k == b'Y' as u16 => rehacer(hwnd, state),
                        _ => {}
                    }
                }
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = (wparam.0 & 0xFFFF) as u16;
                let code = ((wparam.0 >> 16) & 0xFFFF) as u32;
                if let Some(state) = state_mut(hwnd) {
                    if id == ID_EDIT_TEXT {
                        if code == EN_KILLFOCUS {
                            commit_text(hwnd, state);
                        }
                    } else {
                        on_command(hwnd, id, state);
                    }
                }
                LRESULT(0)
            }
            m if m == WM_APP_CANCEL_TEXT => {
                if let Some(state) = state_mut(hwnd)
                    && let Some(edit) = state.edit.take()
                {
                    cerrar_edit(edit);
                    _ = InvalidateRect(Some(hwnd), None, true);
                }
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
            WM_CLOSE => {
                if let Some(state) = state_mut(hwnd) {
                    confirmar_descarte(hwnd, state);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                if let Some(state) = state_mut(hwnd)
                    && state.outcome.is_none()
                {
                    state.outcome = Some(None);
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// Pinta el lienzo: comprometido, o comprometido + provisional en drag.
fn pintar(hwnd: HWND, hdc: HDC, state: &mut DrawState) -> windows::core::Result<()> {
    let destino = dest_rect(hwnd, state);
    if destino.is_empty() {
        return Ok(());
    }
    let screen = ScreenDc::get()?;
    let src_dc = MemDc::compatible_with(&screen)?;

    // Preview del arrastre sobre los buffers persistentes: dos memcpy y
    // cero asignaciones por repintado (camino caliente del mousemove).
    let dib = if let Some(anotacion) = anotacion_en_curso(state) {
        state
            .preview
            .pixels
            .copy_from_slice(&state.committed.pixels);
        {
            let mut canvas = rustcapture_core::annotate::Canvas::new(&mut state.preview);
            anotacion.render(&mut canvas, &state.ctx);
        }
        crate::gdi::copy_frame_to_dib(&state.preview, &mut state.preview_dib);
        &state.preview_dib
    } else {
        &state.committed_dib
    };
    let _s = Selected::bitmap(&src_dc, dib)?;
    // SAFETY: DCs vivos; blits estándar.
    unsafe {
        SetStretchBltMode(hdc, HALFTONE);
        _ = StretchBlt(
            hdc,
            destino.x,
            destino.y,
            destino.width as i32,
            destino.height as i32,
            Some(src_dc.0),
            0,
            0,
            state.committed.width as i32,
            state.committed.height as i32,
            SRCCOPY,
        );
    }
    Ok(())
}
