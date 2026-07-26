//! Catálogo de fuentes disponibles para el editor (f.54). Tres orígenes, en
//! orden de prioridad creciente:
//!
//! 1. Fuentes del sistema (`HKLM`), que son la mayoría.
//! 2. Fuentes por usuario (`HKCU`), instaladas sin permisos de admin.
//! 3. `fonts/` junto al ejecutable — lo que el usuario deja ahí manda, y no
//!    exige instalar nada en Windows (portable-first, D9).
//!
//! El registro mapea nombre visible → archivo; los relativos cuelgan de
//! `%SystemRoot%\Fonts`. Solo se aceptan `.ttf`/`.otf`: las colecciones
//! `.ttc` (Cambria, varias asiáticas) necesitan índice de cara dentro del
//! archivo y `fontdue` no lo expone, así que se descartan — es una
//! limitación conocida, no un olvido.

mod nombres;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_SZ, RegCloseKey,
    RegEnumValueW, RegGetValueW, RegOpenKeyExW,
};
use windows::core::{PCWSTR, w};

/// Una familia con sus caras. La negrita puede faltar: `RenderContext::font`
/// cae a la normal en ese caso.
pub(crate) struct Familia {
    pub nombre: String,
    pub normal: PathBuf,
    pub bold: Option<PathBuf>,
}

const CLAVE: PCWSTR = w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts");

/// Extensiones que `fontdue` sabe parsear.
fn extension_valida(p: &Path) -> bool {
    matches!(
        p.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("ttf") | Some("otf")
    )
}

/// `fonts/` junto al ejecutable.
pub(crate) fn carpeta_portable() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .unwrap_or_default()
        .join("fonts")
}

/// Catálogo completo, ordenado por nombre. Una familia solo entra si tiene
/// al menos la cara normal.
pub(crate) fn catalogo() -> Vec<Familia> {
    // (nombre, negrita) -> ruta. BTreeMap para salida ordenada y estable.
    let mut caras: BTreeMap<(String, bool), PathBuf> = BTreeMap::new();
    // De menor a mayor prioridad: lo último sobrescribe.
    leer_registro(HKEY_LOCAL_MACHINE, &mut caras);
    leer_registro(HKEY_CURRENT_USER, &mut caras);
    leer_carpeta(&carpeta_portable(), &mut caras);

    let mut familias: BTreeMap<String, Familia> = BTreeMap::new();
    for ((nombre, bold), ruta) in caras {
        let entrada = familias.entry(nombre.clone()).or_insert_with(|| Familia {
            nombre: nombre.clone(),
            // Provisional: si solo hay negrita, hace de normal (mejor
            // ofrecerla que descartar la familia entera).
            normal: ruta.clone(),
            bold: None,
        });
        if bold {
            entrada.bold = Some(ruta);
        } else {
            entrada.normal = ruta;
        }
    }
    familias.into_values().collect()
}

/// Añade al mapa las fuentes de una raíz del registro.
fn leer_registro(raiz: HKEY, caras: &mut BTreeMap<(String, bool), PathBuf>) {
    let mut clave = HKEY::default();
    // SAFETY: apertura de una clave de solo lectura; se cierra al final.
    let abierta =
        unsafe { RegOpenKeyExW(raiz, CLAVE, Some(0), KEY_READ, &mut clave) }.is_ok();
    if !abierta {
        return;
    }
    let dir_sistema =
        PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string()))
            .join("Fonts");
    let mut i = 0u32;
    loop {
        let mut nombre = [0u16; 256];
        let mut largo = nombre.len() as u32;
        // SAFETY: buffer propio con su longitud; la enumeración acaba cuando
        // RegEnumValueW deja de devolver Ok (ERROR_NO_MORE_ITEMS).
        let ok = unsafe {
            RegEnumValueW(
                clave,
                i,
                Some(windows::core::PWSTR(nombre.as_mut_ptr())),
                &mut largo,
                None,
                None,
                None,
                None,
            )
        }
        .is_ok();
        if !ok {
            break;
        }
        i += 1;
        let visible = String::from_utf16_lossy(&nombre[..largo as usize]);
        let Some((familia, bold)) = nombres::analizar(&visible) else {
            continue;
        };
        let Some(archivo) = valor_texto(clave, &nombre[..largo as usize]) else {
            continue;
        };
        let ruta = PathBuf::from(&archivo);
        let ruta = if ruta.is_absolute() {
            ruta
        } else {
            dir_sistema.join(&archivo)
        };
        if extension_valida(&ruta) && ruta.exists() {
            caras.insert((familia, bold), ruta);
        }
    }
    // SAFETY: la clave la abrió esta función y no se ha cerrado.
    unsafe { _ = RegCloseKey(clave) };
}

/// Lee un valor REG_SZ del que ya se conoce el nombre en UTF-16.
fn valor_texto(clave: HKEY, nombre_utf16: &[u16]) -> Option<String> {
    // El nombre debe llegar terminado en NUL.
    let mut nombre: Vec<u16> = nombre_utf16.to_vec();
    nombre.push(0);
    let mut buffer = [0u16; 512];
    let mut bytes = (buffer.len() * 2) as u32;
    // SAFETY: nombre y buffer son propios; `bytes` entra con la capacidad en
    // bytes y sale con lo escrito.
    let ok = unsafe {
        RegGetValueW(
            clave,
            None,
            PCWSTR(nombre.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
    }
    .is_ok();
    if !ok {
        return None;
    }
    let chars = (bytes as usize / 2).min(buffer.len());
    let fin = buffer[..chars].iter().position(|&c| c == 0).unwrap_or(chars);
    Some(String::from_utf16_lossy(&buffer[..fin]))
}

/// Añade los `.ttf`/`.otf` de una carpeta.
fn leer_carpeta(dir: &Path, caras: &mut BTreeMap<(String, bool), PathBuf>) {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return; // no existe: caso normal, no es un error
    };
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        if !extension_valida(&ruta) {
            continue;
        }
        let Some(tallo) = ruta.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if let Some((familia, bold)) = nombres::de_archivo(tallo) {
            caras.insert((familia, bold), ruta);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// En cualquier Windows hay fuentes; y todas las del catálogo tienen que
    /// existir en disco y ser parseables, porque si no el chip ofrecería
    /// familias que luego no pintan.
    #[test]
    fn el_catalogo_del_sistema_no_esta_vacio_y_es_usable() {
        let familias = catalogo();
        assert!(familias.len() > 10, "solo {} familias", familias.len());
        assert!(
            familias.iter().any(|f| f.nombre == "Segoe UI"),
            "falta Segoe UI"
        );
        for f in familias.iter().take(8) {
            let bytes = std::fs::read(&f.normal)
                .unwrap_or_else(|e| panic!("{}: {e}", f.normal.display()));
            assert!(
                fontdue::Font::from_bytes(&bytes[..], fontdue::FontSettings::default()).is_ok(),
                "{} no es parseable",
                f.nombre
            );
        }
    }

    #[test]
    fn ninguna_familia_del_catalogo_es_una_coleccion_ttc() {
        for f in catalogo() {
            assert!(extension_valida(&f.normal), "{}", f.normal.display());
            if let Some(b) = &f.bold {
                assert!(extension_valida(b), "{}", b.display());
            }
        }
    }

    #[test]
    fn la_carpeta_portable_cuelga_del_ejecutable() {
        let dir = carpeta_portable();
        assert_eq!(dir.file_name().and_then(|s| s.to_str()), Some("fonts"));
    }
}
