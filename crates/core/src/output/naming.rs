//! Generación de nombres automáticos (f.41), pura: la existencia se
//! consulta vía predicado inyectado, sin tocar disco.

/// Devuelve el primer nombre libre según `exists`. El llamador aporta el
/// timestamp ya formateado (el reloj vive en el sink, no aquí).
pub fn auto_name(
    prefix: &str,
    stamp: &str,
    extension: &str,
    exists: impl Fn(&str) -> bool,
) -> String {
    let base = format!("{prefix}_{stamp}.{extension}");
    if !exists(&base) {
        return base;
    }
    let mut n = 2u32;
    loop {
        let candidate = format!("{prefix}_{stamp}_{n}.{extension}");
        if !exists(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_colision_devuelve_el_nombre_base() {
        let name = auto_name("captura", "2026-07-24_120000", "png", |_| false);
        assert_eq!(name, "captura_2026-07-24_120000.png");
    }

    #[test]
    fn con_colision_anade_sufijo_incremental() {
        let ocupados = ["captura_2026-07-24_120000.png"];
        let name = auto_name("captura", "2026-07-24_120000", "png", |n| {
            ocupados.contains(&n)
        });
        assert_eq!(name, "captura_2026-07-24_120000_2.png");
    }

    #[test]
    fn salta_todos_los_sufijos_ocupados() {
        let ocupados = [
            "captura_2026-07-24_120000.png",
            "captura_2026-07-24_120000_2.png",
            "captura_2026-07-24_120000_3.png",
        ];
        let name = auto_name("captura", "2026-07-24_120000", "png", |n| {
            ocupados.contains(&n)
        });
        assert_eq!(name, "captura_2026-07-24_120000_4.png");
    }
}
