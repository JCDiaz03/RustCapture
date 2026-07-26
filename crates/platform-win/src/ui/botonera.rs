//! Botonera compartida: la fila declarativa (`Elemento`/`BotonDef`) y su
//! ciclo crear/recolocar sobre `ui::layout` + `ui::boton`. Barra y editor
//! solo componen su fila (en sus `math.rs`, testeada) y pintan sus
//! extras (asa, separadores) con las cajas que devuelve el layout.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetDlgItem, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos,
};

use crate::dpi::Escala;
use crate::ui::boton;
use crate::ui::iconos::Icono;
use crate::ui::layout::{self, Caja, Item};
use crate::ui::tooltip::Tooltips;

pub(crate) struct BotonDef {
    pub id: u16,
    pub icono: Icono,
    pub nombre: &'static str,
    /// false = sin lógica todavía: se muestra deshabilitado.
    pub habilitado: bool,
    /// Tinte #D83B01 (grabación) en reposo.
    pub grabacion: bool,
}

pub(crate) enum Elemento {
    Asa,
    Separador,
    Muelle,
    Boton(BotonDef),
}

pub(crate) const fn boton(id: u16, icono: Icono, nombre: &'static str, habilitado: bool) -> Elemento {
    Elemento::Boton(BotonDef { id, icono, nombre, habilitado, grabacion: false })
}

pub(crate) fn a_items(fila: &[Elemento]) -> Vec<Item> {
    fila.iter()
        .map(|e| match e {
            Elemento::Asa => Item::Asa,
            Elemento::Separador => Item::Separador,
            Elemento::Muelle => Item::Muelle,
            Elemento::Boton(_) => Item::Boton,
        })
        .collect()
}

/// Cajas físicas de la fila al DPI de la ventana. Con
/// `al_ancho_del_cliente` el muelle absorbe el ancho del client rect;
/// si no, el ancho es el natural (y se devuelve como total).
pub(crate) fn cajas(
    hwnd: HWND,
    fila: &[Elemento],
    alto_logico: i32,
    al_ancho_del_cliente: bool,
) -> (Vec<Caja>, i32) {
    let escala = Escala::from_hwnd(hwnd);
    let ancho_total = al_ancho_del_cliente.then(|| {
        let mut client = windows::Win32::Foundation::RECT::default();
        // SAFETY: consulta del client rect de una ventana viva.
        unsafe { _ = GetClientRect(hwnd, &mut client) };
        client.right - client.left
    });
    layout::distribuir(&a_items(fila), escala, escala.px(alto_logico), ancho_total)
}

/// Crea los IconButton de la fila en sus cajas y registra tooltips para
/// los habilitados con el texto que dé `tooltip_de`.
pub(crate) fn crear(
    hwnd: HWND,
    fila: &[Elemento],
    cajas: &[Caja],
    tooltip_de: impl Fn(&BotonDef) -> String,
) -> Option<Tooltips> {
    let mut tooltips = Tooltips::nuevo(hwnd).ok();
    for (elemento, caja) in fila.iter().zip(cajas) {
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
        if def.habilitado && let Some(tt) = tooltips.as_mut() {
            _ = tt.agregar(control, &tooltip_de(def));
        }
    }
    tooltips
}

/// Recoloca los botones existentes en cajas nuevas (WM_SIZE/WM_DPICHANGED).
pub(crate) fn reposicionar(hwnd: HWND, fila: &[Elemento], cajas: &[Caja]) {
    for (elemento, caja) in fila.iter().zip(cajas) {
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
