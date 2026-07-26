//! Barra de propiedades contextual del editor V4: chips según la
//! herramienta activa ("Grosor 2 px · Color ■ …"); clic → menú popup con
//! las opciones o el diálogo de color. Sustituye a la barra inferior de
//! swatches de la antigua ventana de dibujo.

use rustcapture_core::annotate::Color;
use windows::Win32::Foundation::{COLORREF, HWND, POINT, RECT, SIZE};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, CreateSolidBrush, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteObject,
    DrawTextW, FillRect, FrameRect, GetTextExtentPoint32W, HDC, InvalidateRect, SelectObject,
    SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::Controls::Dialogs::{CC_FULLOPEN, CC_RGBINIT, CHOOSECOLORW, ChooseColorW};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, MF_CHECKED, MF_STRING, TPM_LEFTALIGN,
    TPM_RETURNCMD, TPM_TOPALIGN, TrackPopupMenu,
};
use windows::core::PCWSTR;

use crate::dpi::Escala;
use crate::ui::{fuentes, theme};

use super::estado::{EditorState, GROSORES, TAMANOS};
use super::math::Herramienta;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Accion {
    MenuGrosor,
    MenuTamano,
    ToggleNegrita,
    ElegirColor,
}

#[derive(PartialEq, Debug)]
pub(super) struct Chip {
    pub etiqueta: String,
    /// El chip lleva una muestra de color al lado del texto.
    pub muestra_color: bool,
    pub accion: Accion,
}

/// Composición pura de los chips para la herramienta activa.
pub(super) fn chips(
    herramienta: Herramienta,
    grosor: u32,
    tamano_texto: f32,
    negrita: bool,
) -> Vec<Chip> {
    let color = Chip {
        etiqueta: "Color".to_string(),
        muestra_color: true,
        accion: Accion::ElegirColor,
    };
    match herramienta {
        Herramienta::Texto => vec![
            Chip {
                etiqueta: format!("Tamaño {}", tamano_texto as u32),
                muestra_color: false,
                accion: Accion::MenuTamano,
            },
            Chip {
                etiqueta: format!("Negrita: {}", if negrita { "sí" } else { "no" }),
                muestra_color: false,
                accion: Accion::ToggleNegrita,
            },
            color,
        ],
        Herramienta::Resaltador => vec![color],
        _ => vec![
            Chip {
                etiqueta: format!("Grosor {grosor} px"),
                muestra_color: false,
                accion: Accion::MenuGrosor,
            },
            color,
        ],
    }
}

/// Pinta la banda de propiedades en el back buffer y devuelve las zonas
/// clicables (en coordenadas de cliente) para el hit-test.
pub(super) fn pintar(
    dc: HDC,
    banda: RECT,
    state: &EditorState,
    escala: Escala,
) -> Vec<(RECT, Accion)> {
    let paleta = theme::actual().paleta();
    let lista = chips(state.herramienta, state.grosor, state.tamano_texto, state.negrita);
    let mut zonas = Vec::with_capacity(lista.len());
    // SAFETY: DC del back buffer vivo; brochas propias liberadas aquí.
    unsafe {
        SetBkMode(dc, TRANSPARENT);
        let fuente = fuentes::fuente(fuentes::Rol::Denso, escala);
        let fuente_previa = SelectObject(dc, fuente.into());
        let mut x = banda.left + escala.px(12);
        let swatch = escala.px(14);
        let hueco = escala.px(6);
        for chip in &lista {
            let wide: Vec<u16> = chip.etiqueta.encode_utf16().collect();
            let mut medida = SIZE::default();
            _ = GetTextExtentPoint32W(dc, &wide, &mut medida);
            let ancho = medida.cx
                + if chip.muestra_color { hueco + swatch } else { 0 };
            let zona = RECT {
                left: x,
                top: banda.top,
                right: x + ancho,
                bottom: banda.bottom,
            };
            SetTextColor(dc, paleta.texto);
            let mut texto = wide.clone();
            let mut rc = zona;
            DrawTextW(dc, &mut texto, &mut rc, DT_SINGLELINE | DT_VCENTER | DT_NOPREFIX);
            if chip.muestra_color {
                let cy = (banda.top + banda.bottom) / 2;
                let caja = RECT {
                    left: x + medida.cx + hueco,
                    top: cy - swatch / 2,
                    right: x + medida.cx + hueco + swatch,
                    bottom: cy + swatch / 2,
                };
                let relleno = CreateSolidBrush(COLORREF(
                    state.color.r as u32 | (state.color.g as u32) << 8 | (state.color.b as u32) << 16,
                ));
                FillRect(dc, &caja, relleno);
                _ = DeleteObject(relleno.into());
                let borde = CreateSolidBrush(paleta.borde);
                FrameRect(dc, &caja, borde);
                _ = DeleteObject(borde.into());
            }
            zonas.push((zona, chip.accion));
            x += ancho + escala.px(16);
        }
        SelectObject(dc, fuente_previa);
    }
    zonas
}

/// Clic en la banda de propiedades. Devuelve true si cayó en un chip.
pub(super) fn on_click(hwnd: HWND, state: &mut EditorState, p: (i32, i32)) -> bool {
    let accion = state.chips.iter().find_map(|(zona, accion)| {
        (p.0 >= zona.left && p.0 < zona.right && p.1 >= zona.top && p.1 < zona.bottom)
            .then_some(*accion)
    });
    let Some(accion) = accion else {
        return false;
    };
    match accion {
        Accion::MenuGrosor => {
            let etiquetas: Vec<String> = GROSORES.iter().map(|g| format!("{g} px")).collect();
            let actual = GROSORES.iter().position(|&g| g == state.grosor);
            if let Some(i) = menu_de_opciones(hwnd, p, &etiquetas, actual) {
                state.grosor = GROSORES[i];
            }
        }
        Accion::MenuTamano => {
            let etiquetas: Vec<String> =
                TAMANOS.iter().map(|t| format!("{} px", *t as u32)).collect();
            let actual = TAMANOS.iter().position(|&t| t == state.tamano_texto);
            if let Some(i) = menu_de_opciones(hwnd, p, &etiquetas, actual) {
                state.tamano_texto = TAMANOS[i];
            }
        }
        Accion::ToggleNegrita => state.negrita = !state.negrita,
        Accion::ElegirColor => elegir_color(hwnd, state),
    }
    // SAFETY: invalidación de la propia ventana (repinta los chips).
    unsafe { _ = InvalidateRect(Some(hwnd), None, false) };
    true
}

/// Menú popup en el punto del clic; devuelve el índice elegido.
fn menu_de_opciones(hwnd: HWND, p: (i32, i32), etiquetas: &[String], marcado: Option<usize>) -> Option<usize> {
    // SAFETY: menú efímero crear → mostrar → destruir; los textos viven
    // durante la llamada.
    unsafe {
        let menu = CreatePopupMenu().ok()?;
        let wides: Vec<Vec<u16>> = etiquetas
            .iter()
            .map(|e| e.encode_utf16().chain([0]).collect())
            .collect();
        for (i, wide) in wides.iter().enumerate() {
            let flags = if marcado == Some(i) { MF_STRING | MF_CHECKED } else { MF_STRING };
            _ = AppendMenuW(menu, flags, i + 1, PCWSTR(wide.as_ptr()));
        }
        let mut punto = POINT { x: p.0, y: p.1 };
        _ = ClientToScreen(hwnd, &mut punto);
        // TPM_RETURNCMD: el BOOL devuelto lleva el id elegido (0 = nada).
        let elegido = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD,
            punto.x,
            punto.y,
            None,
            hwnd,
            None,
        );
        _ = DestroyMenu(menu);
        let id = elegido.0 as usize;
        (id >= 1 && id <= etiquetas.len()).then(|| id - 1)
    }
}

/// Diálogo de color estándar de Windows.
fn elegir_color(hwnd: HWND, state: &mut EditorState) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn las_formas_llevan_grosor_y_color() {
        for h in [
            Herramienta::Flecha,
            Herramienta::Linea,
            Herramienta::Rect,
            Herramienta::Elipse,
            Herramienta::Lapiz,
        ] {
            let chips = chips(h, 3, 20.0, false);
            assert_eq!(chips.len(), 2, "{h:?}");
            assert_eq!(chips[0].etiqueta, "Grosor 3 px");
            assert_eq!(chips[0].accion, Accion::MenuGrosor);
            assert!(chips[1].muestra_color);
        }
    }

    #[test]
    fn el_texto_lleva_tamano_negrita_y_color() {
        let chips = chips(Herramienta::Texto, 3, 28.0, true);
        assert_eq!(chips.len(), 3);
        assert_eq!(chips[0].etiqueta, "Tamaño 28");
        assert_eq!(chips[1].etiqueta, "Negrita: sí");
        assert_eq!(chips[1].accion, Accion::ToggleNegrita);
        assert_eq!(chips[2].accion, Accion::ElegirColor);
    }

    #[test]
    fn el_resaltador_solo_lleva_color() {
        let chips = chips(Herramienta::Resaltador, 3, 20.0, false);
        assert_eq!(chips.len(), 1);
        assert!(chips[0].muestra_color);
    }
}
