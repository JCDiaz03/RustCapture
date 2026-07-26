//! Instantánea del árbol de ventanas para el picking de objetos y menús
//! (f.11, f.12).
//!
//! Se toma ANTES de mostrar el overlay, y esa es la clave de f.12: un menú
//! desplegado se cierra en cuanto aparece otra ventana, así que cuando el
//! overlay está en pantalla ya no existe como ventana — pero SÍ está en la
//! imagen congelada. Haciendo el hit-test contra la instantánea, capturar un
//! menú es exactamente lo mismo que capturar cualquier otro control.

use rustcapture_core::ports::Rect;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, GetWindowRect, IsWindowVisible,
};
// `BOOL` es de windows::core, no de Win32::Foundation (0.62).
use windows::core::BOOL;

/// Un rect candidato. `profundidad` 0 = ventana de nivel superior, 1 = algún
/// control de su interior; solo se usa para desempatar áreas iguales.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Candidato {
    pub rect: Rect,
    pub profundidad: u16,
}

/// El rect MÁS PEQUEÑO que contiene el punto, o `None` si ninguno lo hace.
///
/// El criterio es el área: así un botón gana a su diálogo y el diálogo gana
/// al escritorio, que es lo que el usuario espera al señalar. A igual área
/// gana el más profundo (el hijo antes que su padre).
pub(crate) fn bajo_el_cursor(candidatos: &[Candidato], p: (i32, i32)) -> Option<Rect> {
    candidatos
        .iter()
        .filter(|c| c.rect.contains_point(p))
        .min_by_key(|c| {
            let area = c.rect.width as u64 * c.rect.height as u64;
            (area, u16::MAX - c.profundidad)
        })
        .map(|c| c.rect)
}

/// Ventanas visibles de nivel superior con sus controles, en coordenadas de
/// escritorio virtual. `excluir` es el HWND propio (la barra), que no debe
/// ofrecerse como candidato aunque esté oculta.
pub(crate) fn instantanea(excluir: HWND) -> Vec<Candidato> {
    let mut acc = Acumulador {
        candidatos: Vec::new(),
        excluir,
    };
    // SAFETY: el lparam lleva un &mut Acumulador que vive toda la
    // enumeración, y EnumWindows es sincrónico.
    unsafe {
        _ = EnumWindows(
            Some(cb_top),
            LPARAM(&mut acc as *mut Acumulador as isize),
        );
    }
    acc.candidatos
}

struct Acumulador {
    candidatos: Vec<Candidato>,
    excluir: HWND,
}

/// `true` si la ventana no debe ofrecerse: invisible o «cloaked» (las apps
/// UWP suspendidas siguen existiendo pero no se ven, y ofrecerlas daría un
/// rect fantasma sobre el que no hay nada).
fn descartable(hwnd: HWND) -> bool {
    // SAFETY: consultas sin precondiciones sobre un HWND de la enumeración.
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return true;
        }
        let mut cloaked = 0u32;
        let consulta = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&raw mut cloaked).cast(),
            size_of::<u32>() as u32,
        );
        consulta.is_ok() && cloaked != 0
    }
}

fn rect_de(hwnd: HWND) -> Option<Rect> {
    let mut rc = RECT::default();
    // SAFETY: GetWindowRect sobre un HWND válido de la enumeración.
    unsafe { GetWindowRect(hwnd, &mut rc) }.ok()?;
    let (w, h) = (rc.right - rc.left, rc.bottom - rc.top);
    (w > 0 && h > 0).then(|| Rect::new(rc.left, rc.top, w as u32, h as u32))
}

unsafe extern "system" fn cb_top(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: el puntero lo puso `instantanea` y vive toda la enumeración.
    let acc = unsafe { &mut *(lparam.0 as *mut Acumulador) };
    if hwnd != acc.excluir
        && !descartable(hwnd)
        && let Some(rect) = rect_de(hwnd)
    {
        acc.candidatos.push(Candidato {
            rect,
            profundidad: 0,
        });
        // Controles del interior (f.11).
        // SAFETY: mismo acumulador; enumeración anidada y sincrónica.
        unsafe {
            _ = EnumChildWindows(Some(hwnd), Some(cb_hijo), lparam);
        }
    }
    BOOL(1)
}

unsafe extern "system" fn cb_hijo(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: igual que en cb_top.
    let acc = unsafe { &mut *(lparam.0 as *mut Acumulador) };
    if !descartable(hwnd)
        && let Some(rect) = rect_de(hwnd)
    {
        // Profundidad 1 para todos los descendientes: solo desempata áreas
        // iguales, y distinguir niveles exactos exigiría recorrer padres.
        acc.candidatos.push(Candidato {
            rect,
            profundidad: 1,
        });
    }
    BOOL(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(x: i32, y: i32, w: u32, h: u32, profundidad: u16) -> Candidato {
        Candidato {
            rect: Rect::new(x, y, w, h),
            profundidad,
        }
    }

    #[test]
    fn gana_el_rect_mas_pequeno_que_contiene_el_punto() {
        // Escritorio, diálogo dentro y botón dentro del diálogo.
        let cs = [
            c(0, 0, 1920, 1080, 0),
            c(400, 300, 400, 200, 0),
            c(420, 320, 80, 24, 1),
        ];
        assert_eq!(
            bajo_el_cursor(&cs, (430, 330)),
            Some(Rect::new(420, 320, 80, 24)),
            "sobre el botón debe ganar el botón"
        );
        assert_eq!(
            bajo_el_cursor(&cs, (600, 400)),
            Some(Rect::new(400, 300, 400, 200)),
            "dentro del diálogo pero fuera del botón"
        );
        assert_eq!(
            bajo_el_cursor(&cs, (50, 50)),
            Some(Rect::new(0, 0, 1920, 1080)),
            "fuera de todo menos del escritorio"
        );
    }

    #[test]
    fn a_igual_area_gana_el_hijo() {
        // Un control que ocupa todo su padre: pasa con los paneles.
        let cs = [c(10, 10, 100, 50, 0), c(10, 10, 100, 50, 1)];
        let elegido = cs
            .iter()
            .filter(|x| x.rect.contains_point((20, 20)))
            .min_by_key(|x| {
                let area = x.rect.width as u64 * x.rect.height as u64;
                (area, u16::MAX - x.profundidad)
            })
            .unwrap();
        assert_eq!(elegido.profundidad, 1);
    }

    #[test]
    fn sin_candidatos_o_fuera_de_todos_es_none() {
        assert_eq!(bajo_el_cursor(&[], (5, 5)), None);
        assert_eq!(bajo_el_cursor(&[c(0, 0, 10, 10, 0)], (50, 50)), None);
    }

    #[test]
    fn un_monitor_a_la_izquierda_del_primario_funciona_igual() {
        // Origen negativo: es el caso que rompe la aritmética sin signo.
        let cs = [c(-1920, 0, 1920, 1080, 0), c(-800, 100, 300, 200, 0)];
        assert_eq!(
            bajo_el_cursor(&cs, (-700, 150)),
            Some(Rect::new(-800, 100, 300, 200))
        );
        assert_eq!(
            bajo_el_cursor(&cs, (-1800, 900)),
            Some(Rect::new(-1920, 0, 1920, 1080))
        );
    }

    /// El árbol real del sistema: tiene que traer ventanas y controles.
    #[test]
    fn la_instantanea_real_trae_candidatos_usables() {
        let cs = instantanea(HWND::default());
        assert!(cs.len() > 3, "solo {} candidatos", cs.len());
        assert!(cs.iter().all(|c| !c.rect.is_empty()));
        assert!(
            cs.iter().any(|c| c.profundidad > 0),
            "no se enumeraron controles hijos: f.11 no funcionaría"
        );
    }

    // La exclusión de la propia barra NO se testea aquí: el `Candidato` no
    // lleva su HWND, así que un test sin crear una ventana real solo podría
    // fingir que la comprueba. Va en el guion de verificación manual («la
    // barra no se ofrece como candidato»).
}
