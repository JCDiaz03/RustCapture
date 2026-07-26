//! Composición pura de la fila de la barra V4: qué botones hay, en qué
//! orden, cuáles llevan lógica y cuáles esperan su fase. Testeable sin
//! ventanas; `mod.rs` solo materializa lo que se decide aquí.

use crate::ui::iconos::Icono;
use crate::ui::layout::Item;

/// Alto lógico de la barra: botón de 28 + rejilla de 4 arriba y abajo.
pub(crate) const ALTO_LOGICO: i32 = 36;

pub(crate) const ID_FULLSCREEN: u16 = 1001;
pub(crate) const ID_WINDOW: u16 = 1002;
pub(crate) const ID_REGION: u16 = 1003;
pub(crate) const ID_DELAY: u16 = 1004;
pub(crate) const ID_RECORD: u16 = 1005;
pub(crate) const ID_CONFIG: u16 = 1006;
pub(crate) const ID_CLOSE: u16 = 1008;
pub(crate) const ID_OBJECT: u16 = 1009;
pub(crate) const ID_FREEHAND: u16 = 1010;
pub(crate) const ID_FIXED: u16 = 1011;
pub(crate) const ID_SCROLL: u16 = 1012;
pub(crate) const ID_EYEDROPPER: u16 = 1013;
pub(crate) const ID_MAGNIFIER: u16 = 1014;
pub(crate) const ID_RULER: u16 = 1015;
pub(crate) const ID_CROSSHAIR: u16 = 1016;
pub(crate) const ID_PIN: u16 = 1017;
pub(crate) const ID_COLLAPSE: u16 = 1018;

/// Qué hotkey de la config muestra el tooltip de un botón.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum HotkeyTooltip {
    Fullscreen,
    Window,
    Region,
    Delay,
}

pub(crate) struct BotonDef {
    pub id: u16,
    pub icono: Icono,
    pub nombre: &'static str,
    /// false = sin lógica todavía: se muestra deshabilitado (decisión 5).
    pub habilitado: bool,
    /// Tinte #D83B01 (grabación) en reposo.
    pub grabacion: bool,
    pub hotkey: Option<HotkeyTooltip>,
}

pub(crate) enum Elemento {
    Asa,
    Separador,
    Boton(BotonDef),
}

const fn boton(id: u16, icono: Icono, nombre: &'static str, habilitado: bool) -> Elemento {
    Elemento::Boton(BotonDef { id, icono, nombre, habilitado, grabacion: false, hotkey: None })
}

const fn boton_hk(id: u16, icono: Icono, nombre: &'static str, hk: HotkeyTooltip) -> Elemento {
    Elemento::Boton(BotonDef {
        id,
        icono,
        nombre,
        habilitado: true,
        grabacion: false,
        hotkey: Some(hk),
    })
}

/// La fila completa de la barra, en orden visual (mockup V1 + delay f.17).
pub(crate) fn fila() -> Vec<Elemento> {
    use Icono::*;
    vec![
        Elemento::Asa,
        boton_hk(ID_FULLSCREEN, CaptureFullscreen, "Pantalla completa", HotkeyTooltip::Fullscreen),
        boton_hk(ID_WINDOW, CaptureWindow, "Ventana activa", HotkeyTooltip::Window),
        boton(ID_OBJECT, CaptureObject, "Objeto de ventana", false),
        boton_hk(ID_REGION, CaptureRegion, "Región", HotkeyTooltip::Region),
        boton(ID_FREEHAND, CaptureFreehand, "Mano alzada", false),
        boton(ID_FIXED, CaptureFixed, "Región fija", false),
        boton(ID_SCROLL, CaptureScroll, "Captura con scroll", false),
        boton_hk(ID_DELAY, CaptureDelay, "Captura con retardo", HotkeyTooltip::Delay),
        Elemento::Separador,
        Elemento::Boton(BotonDef {
            id: ID_RECORD,
            icono: RecordStart,
            nombre: "Grabar vídeo",
            habilitado: false,
            grabacion: true,
            hotkey: None,
        }),
        Elemento::Separador,
        boton(ID_EYEDROPPER, UtilEyedropper, "Cuentagotas", false),
        boton(ID_MAGNIFIER, UtilMagnifier, "Lupa", false),
        boton(ID_RULER, UtilRuler, "Regla", false),
        boton(ID_CROSSHAIR, UtilCrosshair, "Crosshair", false),
        boton(ID_PIN, UtilPin, "Fijar en pantalla", false),
        Elemento::Separador,
        boton(ID_CONFIG, SysSettings, "Ajustes", false),
        boton(ID_COLLAPSE, SysCollapse, "Colapsar barra", false),
        boton(ID_CLOSE, SysClose, "Salir", true),
    ]
}

/// Proyección de la fila a los items geométricos de `ui::layout`.
pub(crate) fn a_items(fila: &[Elemento]) -> Vec<Item> {
    fila.iter()
        .map(|e| match e {
            Elemento::Asa => Item::Asa,
            Elemento::Separador => Item::Separador,
            Elemento::Boton(_) => Item::Boton,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dpi::Escala;
    use crate::ui::layout;

    fn botones(fila: &[Elemento]) -> Vec<&BotonDef> {
        fila.iter()
            .filter_map(|e| match e {
                Elemento::Boton(b) => Some(b),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn la_fila_tiene_17_botones_con_ids_unicos_y_asa_delante() {
        let fila = fila();
        assert!(matches!(fila[0], Elemento::Asa));
        let botones = botones(&fila);
        assert_eq!(botones.len(), 17);
        let mut ids: Vec<u16> = botones.iter().map(|b| b.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 17, "ids repetidos");
    }

    #[test]
    fn solo_lo_implementado_esta_habilitado() {
        let fila = fila();
        let habilitados: Vec<u16> =
            botones(&fila).iter().filter(|b| b.habilitado).map(|b| b.id).collect();
        assert_eq!(
            habilitados,
            vec![ID_FULLSCREEN, ID_WINDOW, ID_REGION, ID_DELAY, ID_CLOSE]
        );
    }

    #[test]
    fn los_habilitados_de_captura_anuncian_su_hotkey() {
        let fila = fila();
        for b in botones(&fila) {
            if b.hotkey.is_some() {
                assert!(b.habilitado, "{} con tooltip de hotkey pero deshabilitado", b.nombre);
            }
        }
    }

    #[test]
    fn grabar_lleva_el_tinte_de_grabacion_y_salir_cierra_la_fila() {
        let fila = fila();
        let botones = botones(&fila);
        let record = botones.iter().find(|b| b.id == ID_RECORD).unwrap();
        assert!(record.grabacion);
        assert_eq!(botones.last().unwrap().id, ID_CLOSE);
    }

    #[test]
    fn la_fila_cabe_en_una_sola_linea_de_36_logicos() {
        let fila = fila();
        let items = a_items(&fila);
        assert_eq!(items.len(), fila.len());
        let escala = Escala::nueva(96);
        let (cajas, ancho) = layout::distribuir(&items, escala, escala.px(ALTO_LOGICO), None);
        assert_eq!(cajas.len(), fila.len());
        // Todos los botones caben verticalmente y el ancho es coherente.
        for caja in &cajas {
            assert!(caja.y >= 0 && caja.y + caja.alto <= escala.px(ALTO_LOGICO));
        }
        assert!(ancho > 500 && ancho < 700, "ancho natural inesperado: {ancho}");
    }
}
