//! Parseo de los nombres de fuente del registro de Windows. Aislado y puro
//! porque es donde están todos los casos raros.

/// Sufijos de tipo que Windows añade al nombre visible.
const TIPOS: [&str; 3] = [" (TrueType)", " (OpenType)", " (VarType)"];

/// Pesos intermedios: no son la familia base ni su negrita, y ofrecerlos
/// llenaría la lista de familias falsas ("Segoe UI Light", "Segoe UI Black").
const PESOS: [&str; 6] = [
    " Light",
    " Semilight",
    " Semibold",
    " Black",
    " Thin",
    " Medium",
];

/// Familia y negrita de un nombre del registro. `None` para lo que no
/// sabemos rasterizar o no queremos ofrecer:
/// - cursivas (aún no hay itálica en `TextStyle`)
/// - pesos que no son ni normal ni negrita
/// - nombres sin sufijo de tipo (las `.fon` bitmap antiguas)
pub(crate) fn analizar(valor: &str) -> Option<(String, bool)> {
    let sin_tipo = TIPOS.iter().find_map(|t| valor.strip_suffix(t))?;
    // Varias familias en un mismo valor van separadas por " & ".
    let primera = sin_tipo.split(" & ").next()?.trim();
    if primera.is_empty() {
        return None;
    }
    let minuscula = primera.to_ascii_lowercase();
    if minuscula.contains("italic") || minuscula.contains("oblique") {
        return None;
    }
    if PESOS.iter().any(|p| primera.ends_with(p)) {
        return None;
    }
    // "Alef Regular" es la cara normal de "Alef", no otra familia: sin esto
    // el catálogo lista las dos por separado.
    let primera = match primera.strip_suffix(" Regular") {
        Some(base) if !base.trim().is_empty() => base.trim(),
        _ => primera,
    };
    match primera.strip_suffix(" Bold") {
        Some(familia) if !familia.trim().is_empty() => Some((familia.trim().to_string(), true)),
        _ => Some((primera.to_string(), false)),
    }
}

/// Familia y negrita del nombre de ARCHIVO de una fuente de la carpeta
/// portable. No se lee la tabla `name` del TTF a propósito: el nombre de
/// archivo es lo que el usuario ve y controla, así que es más predecible
/// (`MiFuente-Bold.ttf` → familia `MiFuente`, negrita).
pub(crate) fn de_archivo(tallo: &str) -> Option<(String, bool)> {
    let limpio = tallo.replace(['-', '_'], " ").trim().to_string();
    if limpio.is_empty() {
        return None;
    }
    // El sufijo de negrita se acepta en cualquier capitalización, que es
    // como los nombran las descargas de fuentes.
    let palabras: Vec<&str> = limpio.rsplitn(2, ' ').collect();
    if palabras.len() == 2 && palabras[0].eq_ignore_ascii_case("bold") {
        let familia = palabras[1].trim();
        if !familia.is_empty() {
            return Some((familia.to_string(), true));
        }
    }
    Some((limpio, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separa_la_familia_de_la_negrita() {
        assert_eq!(
            analizar("Segoe UI (TrueType)"),
            Some(("Segoe UI".to_string(), false))
        );
        assert_eq!(
            analizar("Segoe UI Bold (TrueType)"),
            Some(("Segoe UI".to_string(), true))
        );
        assert_eq!(
            analizar("Consolas (TrueType)"),
            Some(("Consolas".to_string(), false))
        );
    }

    #[test]
    fn descarta_cursivas_y_pesos_intermedios() {
        assert_eq!(analizar("Segoe UI Italic (TrueType)"), None);
        assert_eq!(analizar("Segoe UI Bold Italic (TrueType)"), None);
        assert_eq!(analizar("Segoe UI Semibold (TrueType)"), None);
        assert_eq!(analizar("Segoe UI Black (TrueType)"), None);
        assert_eq!(analizar("Segoe UI Light (TrueType)"), None);
    }

    #[test]
    fn descarta_lo_que_no_lleva_sufijo_de_tipo() {
        // Bitmap antiguas (.fon) y basura.
        assert_eq!(analizar("MS Sans Serif 8,10,12,14,18,24"), None);
        assert_eq!(analizar(""), None);
    }

    #[test]
    fn toma_la_primera_de_un_valor_con_varias_familias() {
        assert_eq!(
            analizar("Cambria & Cambria Math (TrueType)"),
            Some(("Cambria".to_string(), false))
        );
    }

    #[test]
    fn una_familia_que_se_llama_bold_no_se_queda_vacia() {
        assert_eq!(analizar("Bold (TrueType)"), Some(("Bold".to_string(), false)));
    }

    #[test]
    fn regular_es_la_cara_normal_de_su_familia_no_otra_familia() {
        assert_eq!(analizar("Alef Regular (TrueType)"), analizar("Alef (TrueType)"));
        assert_eq!(
            analizar("Amiri Regular (TrueType)"),
            Some(("Amiri".to_string(), false))
        );
        // Pero "Amiri Quran" SÍ es otra familia, no una cara de "Amiri".
        assert_eq!(
            analizar("Amiri Quran Regular (TrueType)"),
            Some(("Amiri Quran".to_string(), false))
        );
        // "Regular" a secas es una familia, no un sufijo sin base.
        assert_eq!(
            analizar("Regular (TrueType)"),
            Some(("Regular".to_string(), false))
        );
    }

    #[test]
    fn el_nombre_de_archivo_separa_guiones_y_negrita() {
        assert_eq!(
            de_archivo("MiFuente"),
            Some(("MiFuente".to_string(), false))
        );
        assert_eq!(
            de_archivo("MiFuente-Bold"),
            Some(("MiFuente".to_string(), true))
        );
        assert_eq!(
            de_archivo("Mi_Fuente_bold"),
            Some(("Mi Fuente".to_string(), true))
        );
        // "Bold" a secas es una familia, no una negrita sin familia.
        assert_eq!(de_archivo("Bold"), Some(("Bold".to_string(), false)));
        assert_eq!(de_archivo(""), None);
        assert_eq!(de_archivo("   "), None);
    }
}
