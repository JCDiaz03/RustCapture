//! Tema claro/oscuro (tokens de `diseno-frontend.md` §2). El tema activo
//! es estado global del proceso: se resuelve al arrancar y se refresca
//! cuando Windows emite `WM_SETTINGCHANGE` con "ImmersiveColorSet".

use std::sync::atomic::{AtomicU8, Ordering};

use rustcapture_core::config::ThemeMode;
use windows::Win32::Foundation::{COLORREF, ERROR_SUCCESS, HWND, LPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::core::w;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Tema {
    Claro,
    Oscuro,
}

/// Tokens de color en COLORREF (0x00BBGGRR).
pub(crate) struct Paleta {
    /// Fondo de ventana.
    pub fondo: COLORREF,
    /// Paneles y barras sobre el fondo.
    pub superficie: COLORREF,
    pub texto: COLORREF,
    pub texto_secundario: COLORREF,
    pub borde: COLORREF,
    /// Fondo de botón bajo el cursor.
    pub hover: COLORREF,
    /// Fondo de botón pulsado.
    pub pressed: COLORREF,
    /// Selección y acción primaria (#0067C0).
    pub acento: COLORREF,
    /// Estado de grabación (#D83B01).
    pub grabacion: COLORREF,
    /// Fondo del área de canvas del editor.
    pub canvas: COLORREF,
}

const fn rgb(r: u32, g: u32, b: u32) -> COLORREF {
    COLORREF(r | (g << 8) | (b << 16))
}

pub(crate) const CLARO: Paleta = Paleta {
    fondo: rgb(0xF3, 0xF3, 0xF3),
    superficie: rgb(0xFF, 0xFF, 0xFF),
    texto: rgb(0x1A, 0x1A, 0x1A),
    texto_secundario: rgb(0x5A, 0x5A, 0x5A),
    borde: rgb(0xD0, 0xD0, 0xD0),
    hover: rgb(0xE4, 0xE4, 0xE4),
    pressed: rgb(0xD6, 0xD6, 0xD6),
    acento: rgb(0x00, 0x67, 0xC0),
    grabacion: rgb(0xD8, 0x3B, 0x01),
    canvas: rgb(0xDA, 0xDA, 0xDA),
};

pub(crate) const OSCURO: Paleta = Paleta {
    fondo: rgb(0x20, 0x20, 0x20),
    superficie: rgb(0x2B, 0x2B, 0x2B),
    texto: rgb(0xE8, 0xE8, 0xE8),
    texto_secundario: rgb(0x9A, 0x9A, 0x9A),
    borde: rgb(0x3F, 0x3F, 0x3F),
    hover: rgb(0x3A, 0x3A, 0x3A),
    pressed: rgb(0x45, 0x45, 0x45),
    acento: rgb(0x00, 0x67, 0xC0),
    grabacion: rgb(0xD8, 0x3B, 0x01),
    canvas: rgb(0x19, 0x19, 0x19),
};

impl Tema {
    pub(crate) const fn paleta(self) -> &'static Paleta {
        match self {
            Tema::Claro => &CLARO,
            Tema::Oscuro => &OSCURO,
        }
    }

    pub(crate) const fn es_oscuro(self) -> bool {
        matches!(self, Tema::Oscuro)
    }
}

/// Interpretación del valor `AppsUseLightTheme` del registro: 0 = apps en
/// oscuro; 1, otro valor o clave ausente (Windows sin la opción) = claro.
pub(crate) fn tema_del_sistema(apps_use_light_theme: Option<u32>) -> Tema {
    match apps_use_light_theme {
        Some(0) => Tema::Oscuro,
        _ => Tema::Claro,
    }
}

/// Aplica la preferencia de config sobre lo que diga el sistema.
pub(crate) fn resolver(modo: ThemeMode, sistema: Tema) -> Tema {
    match modo {
        ThemeMode::Auto => sistema,
        ThemeMode::Light => Tema::Claro,
        ThemeMode::Dark => Tema::Oscuro,
    }
}

// 0 = claro, 1 = oscuro; lo escribe refrescar() y lo lee actual().
static ACTUAL: AtomicU8 = AtomicU8::new(0);

/// Tema activo del proceso (lo último que fijó `refrescar`).
pub(crate) fn actual() -> Tema {
    if ACTUAL.load(Ordering::Relaxed) == 1 {
        Tema::Oscuro
    } else {
        Tema::Claro
    }
}

/// Relee el registro y resuelve con la preferencia de config. Llamar al
/// arrancar y en cada `WM_SETTINGCHANGE` de tema; devuelve el tema vigente.
pub(crate) fn refrescar(modo: ThemeMode) -> Tema {
    let tema = resolver(modo, tema_del_sistema(leer_apps_use_light_theme()));
    ACTUAL.store(tema.es_oscuro() as u8, Ordering::Relaxed);
    tema
}

fn leer_apps_use_light_theme() -> Option<u32> {
    let mut valor: u32 = 0;
    let mut tam = size_of::<u32>() as u32;
    // SAFETY: buffers locales del tamaño declarado; RegGetValueW no
    // retiene los punteros tras volver.
    let err = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut valor as *mut u32 as *mut _),
            Some(&mut tam),
        )
    };
    (err == ERROR_SUCCESS).then_some(valor)
}

/// ¿Este `WM_SETTINGCHANGE` anuncia un cambio de tema de apps?
pub(crate) fn es_cambio_de_tema(lparam: LPARAM) -> bool {
    if lparam.0 == 0 {
        return false;
    }
    // SAFETY: WM_SETTINGCHANGE documenta lparam como cadena UTF-16
    // terminada en nulo, propiedad del sistema durante el mensaje.
    match unsafe { windows::core::PCWSTR(lparam.0 as *const u16).to_string() } {
        Ok(s) => s == "ImmersiveColorSet",
        Err(_) => false,
    }
}

/// Barra de título nativa en oscuro (no-op si el sistema no lo soporta).
pub(crate) fn aplicar_titulo_oscuro(hwnd: HWND, oscuro: bool) {
    let valor = windows::core::BOOL::from(oscuro);
    // SAFETY: atributo documentado con puntero y tamaño coherentes; el
    // valor se copia durante la llamada.
    unsafe {
        _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &valor as *const _ as *const _,
            size_of::<windows::core::BOOL>() as u32,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_valor_del_registro_mapea_a_tema() {
        assert_eq!(tema_del_sistema(Some(0)), Tema::Oscuro);
        assert_eq!(tema_del_sistema(Some(1)), Tema::Claro);
        assert_eq!(tema_del_sistema(None), Tema::Claro); // sin clave = claro
    }

    #[test]
    fn la_config_manda_sobre_el_sistema() {
        assert_eq!(resolver(ThemeMode::Auto, Tema::Oscuro), Tema::Oscuro);
        assert_eq!(resolver(ThemeMode::Light, Tema::Oscuro), Tema::Claro);
        assert_eq!(resolver(ThemeMode::Dark, Tema::Claro), Tema::Oscuro);
    }

    #[test]
    fn las_paletas_llevan_los_tokens_del_diseno() {
        // COLORREF es 0x00BBGGRR.
        assert_eq!(CLARO.fondo.0, 0x00F3F3F3);
        assert_eq!(OSCURO.fondo.0, 0x00202020);
        assert_eq!(CLARO.acento.0, 0x00C06700); // #0067C0
        assert_eq!(CLARO.grabacion.0, 0x00013BD8); // #D83B01
        assert_eq!(OSCURO.acento.0, CLARO.acento.0);
    }
}
