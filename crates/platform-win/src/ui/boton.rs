//! `IconButton`: BUTTON owner-draw reutilizable con icono tintado y
//! estados normal / hover / pressed / deshabilitado / activo. El padre
//! solo tiene que reenviar su `WM_DRAWITEM` a `pintar_drawitem`.

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateSolidBrush, DeleteObject, GetStockObject, InvalidateRect, NULL_PEN, RoundRect,
    SelectObject,
};
use windows::Win32::UI::Controls::{DRAWITEMSTRUCT, ODS_DISABLED, ODS_SELECTED, WM_MOUSELEAVE};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    BS_OWNERDRAW, CreateWindowExW, GWLP_USERDATA, GetWindowLongPtrW, HMENU, SetWindowLongPtrW,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_MOUSEMOVE, WM_NCDESTROY, WS_CHILD, WS_VISIBLE,
};
use windows::core::{PCWSTR, Result, w};

use crate::dpi::Escala;
use crate::ui::iconos::{self, Icono};
use crate::ui::layout::Caja;
use crate::ui::theme;

const ID_SUBCLASS: usize = 0xB070;

/// Estado propio del botón; vive en un Box cuyo puntero se guarda en el
/// GWLP_USERDATA del control (el BUTTON estándar no lo usa) y se libera
/// en WM_NCDESTROY. No se usa el refdata del subclass a propósito:
/// leerlo exigiría `GetWindowSubclass`, que comctl32 5.82 (la versión
/// sin manifest) no exporta por nombre y rompe el arranque del exe.
struct Estado {
    icono: Icono,
    hover: bool,
    rastreando: bool,
    /// Herramienta seleccionada: fondo acento + icono blanco.
    activo: bool,
    /// Tinte de grabación (#D83B01) en estado normal.
    grabacion: bool,
}

pub(crate) struct Opciones {
    pub icono: Icono,
    pub habilitado: bool,
    pub grabacion: bool,
}

/// Crea el BUTTON owner-draw en la caja física dada.
pub(crate) fn crear(padre: HWND, id: u16, caja: Caja, opciones: Opciones) -> Result<HWND> {
    // SAFETY: creación estándar de un hijo BUTTON; el Box de estado se
    // adopta en el subclass y se libera en WM_NCDESTROY.
    unsafe {
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_OWNERDRAW as u32),
            caja.x,
            caja.y,
            caja.ancho,
            caja.alto,
            Some(padre),
            Some(HMENU(id as usize as *mut _)),
            None,
            None,
        )?;
        let estado = Box::new(Estado {
            icono: opciones.icono,
            hover: false,
            rastreando: false,
            activo: false,
            grabacion: opciones.grabacion,
        });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(estado) as isize);
        _ = SetWindowSubclass(hwnd, Some(subclass), ID_SUBCLASS, 0);
        if !opciones.habilitado {
            _ = EnableWindow(hwnd, false);
        }
        Ok(hwnd)
    }
}

/// Marca/desmarca el estado "herramienta activa" y repinta.
pub(crate) fn set_activo(boton: HWND, activo: bool) {
    if let Some(estado) = estado_de(boton) {
        estado.activo = activo;
        // SAFETY: invalidar un HWND vivo.
        unsafe { _ = InvalidateRect(Some(boton), None, false) };
    }
}

fn estado_de(boton: HWND) -> Option<&'static mut Estado> {
    // SAFETY: el puntero lo puso crear() desde un Box válido y se anula
    // antes de liberarse en WM_NCDESTROY; solo lo usa el hilo de UI.
    // Nota: para un HWND que no sea un IconButton (p. ej. un BUTTON de
    // otra ventana), GWLP_USERDATA vale 0 y devolvemos None.
    unsafe {
        let ptr = GetWindowLongPtrW(boton, GWLP_USERDATA) as *mut Estado;
        ptr.as_mut()
    }
}

/// Subclass: hover con TrackMouseEvent y liberación del estado.
unsafe extern "system" fn subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _refdata: usize,
) -> LRESULT {
    // SAFETY: el estado vive en GWLP_USERDATA (ver crear()); los
    // mensajes de un HWND llegan siempre por su hilo creador.
    unsafe {
        let Some(estado) = estado_de(hwnd) else {
            return DefSubclassProc(hwnd, msg, wparam, lparam);
        };
        match msg {
            WM_MOUSEMOVE => {
                if !estado.hover {
                    estado.hover = true;
                    _ = InvalidateRect(Some(hwnd), None, false);
                }
                if !estado.rastreando {
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    estado.rastreando = TrackMouseEvent(&mut tme).is_ok();
                }
            }
            WM_MOUSELEAVE => {
                estado.hover = false;
                estado.rastreando = false;
                _ = InvalidateRect(Some(hwnd), None, false);
            }
            WM_NCDESTROY => {
                let ptr = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut Estado;
                let resultado = DefSubclassProc(hwnd, msg, wparam, lparam);
                if !ptr.is_null() {
                    drop(Box::from_raw(ptr));
                }
                return resultado;
            }
            _ => {}
        }
        DefSubclassProc(hwnd, msg, wparam, lparam)
    }
}

/// Pinta un botón desde el `WM_DRAWITEM` del padre. Devuelve `false` si
/// el control no es un IconButton (el padre debe seguir su camino).
pub(crate) fn pintar_drawitem(dis: &DRAWITEMSTRUCT) -> bool {
    let Some(estado) = estado_de(dis.hwndItem) else {
        return false;
    };
    let paleta = theme::actual().paleta();
    let escala = Escala::from_hwnd(dis.hwndItem);
    let rc = dis.rcItem;
    let pressed = (dis.itemState.0 & ODS_SELECTED.0) != 0;
    let deshabilitado = (dis.itemState.0 & ODS_DISABLED.0) != 0;

    // Fondo base = superficie del padre (el botón cubre todo su rect).
    crate::ui::lienzo::rellenar(dis.hDC, &dis.rcItem, paleta.superficie);

    // Fondo de estado con esquinas redondeadas (radio 4 lógico).
    let fondo = if estado.activo {
        Some(paleta.acento)
    } else if pressed {
        Some(paleta.pressed)
    } else if estado.hover && !deshabilitado {
        Some(paleta.hover)
    } else {
        None
    };
    if let Some(color) = fondo {
        // SAFETY: DC del DRAWITEMSTRUCT válido durante el mensaje; brocha
        // propia liberada aquí mismo; el pen NULL es de stock.
        unsafe {
            let brocha = CreateSolidBrush(color);
            let pen_previo = SelectObject(dis.hDC, GetStockObject(NULL_PEN));
            let brocha_previa = SelectObject(dis.hDC, brocha.into());
            let radio = escala.px(8); // diámetro de la elipse de esquina
            // NULL_PEN encoge 1 px el borde derecho/inferior; se compensa.
            _ = RoundRect(dis.hDC, rc.left, rc.top, rc.right + 1, rc.bottom + 1, radio, radio);
            SelectObject(dis.hDC, brocha_previa);
            SelectObject(dis.hDC, pen_previo);
            _ = DeleteObject(brocha.into());
        }
    }

    let (color_icono, opacidad) = if estado.activo {
        (COLORREF(0x00FFFFFF), iconos::OPACO)
    } else if deshabilitado {
        (paleta.texto, iconos::DESHABILITADO)
    } else if estado.grabacion {
        (paleta.grabacion, iconos::OPACO)
    } else {
        (paleta.texto, iconos::OPACO)
    };
    let talla = iconos::talla_para_dpi(escala.dpi());
    let x = rc.left + (rc.right - rc.left - talla as i32) / 2;
    let y = rc.top + (rc.bottom - rc.top - talla as i32) / 2;
    _ = iconos::pintar(dis.hDC, estado.icono, talla, x, y, color_icono, opacidad);
    true
}

