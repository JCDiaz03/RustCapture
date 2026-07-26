//! Texto in situ del editor: un control EDIT hijo en la posición del
//! clic, con subclase para que Esc cancele sin confirmar. Migrado de la
//! antigua ventana de dibujo.

use rustcapture_core::annotate::annotations::TextAnnotation;
use rustcapture_core::annotate::{Command, TextStyle};
use rustcapture_core::ports::Rect;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CLIP_DEFAULT_PRECIS, CreateFontW, DEFAULT_CHARSET, DEFAULT_PITCH, DEFAULT_QUALITY, DeleteObject,
    FW_BOLD, FW_NORMAL, HFONT, InvalidateRect, OUT_DEFAULT_PRECIS,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{SetFocus, VK_ESCAPE};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, ES_AUTOVSCROLL, ES_MULTILINE, GetParent, GetWindowTextW, HMENU,
    PostMessageW, SendMessageW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_KEYDOWN, WM_SETFONT,
    WS_BORDER, WS_CHILD, WS_VISIBLE,
};
use windows::core::{PCWSTR, w};

use super::estado::EditorState;
use super::math;

pub(super) const ID_EDIT_TEXT: u16 = 3080;
pub(super) const WM_APP_CANCEL_TEXT: u32 = WM_APP + 10;

pub(super) struct EditBox {
    pub hwnd: HWND,
    pub pos_frame: (i32, i32),
    pub font: HFONT,
}

pub(super) fn abrir_edit(hwnd: HWND, state: &mut EditorState, pos_frame: (i32, i32), destino: Rect) {
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
            state.tamano_texto.round() as i32,
            0,
            0,
            0,
            if state.negrita {
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

pub(super) fn cerrar_edit(edit: EditBox) {
    // SAFETY: destruye el EDIT y su fuente, creados por abrir_edit.
    unsafe {
        _ = DestroyWindow(edit.hwnd);
        if !edit.font.is_invalid() {
            _ = DeleteObject(edit.font.into());
        }
    }
}

/// Confirma la caja de texto (pérdida de foco, cambio de herramienta u
/// otra acción): texto no vacío → anotación; siempre destruye el EDIT.
pub(super) fn commit_text(hwnd: HWND, state: &mut EditorState) {
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
                    size: state.tamano_texto,
                    bold: state.negrita,
                },
            })),
        );
        state.refresh_committed();
        state.dirty = true;
    }
    // SAFETY: invalidación de la propia ventana.
    unsafe { _ = InvalidateRect(Some(hwnd), None, false) };
}
