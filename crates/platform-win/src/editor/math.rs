//! Encaje de la captura en el lienzo del editor y composición del chrome
//! V4 (puro, TDD).

use rustcapture_core::ports::Rect;

use crate::ui::iconos::Icono;
use crate::ui::layout::Item;

/// Alto LÓGICO de la toolbar (botón 28 + rejilla 4 arriba/abajo).
pub(crate) const TOOLBAR_LOGICO: i32 = 36;
/// Alto LÓGICO de la barra de estado.
pub(crate) const STATUS_LOGICO: i32 = 24;

pub(crate) const ID_GUARDAR: u16 = 3001;
pub(crate) const ID_COPIAR: u16 = 3002;
pub(crate) const ID_DRAW: u16 = 3003;
pub(crate) const ID_SELECT: u16 = 3010;
pub(crate) const ID_TEXTO: u16 = 3011;
pub(crate) const ID_FLECHA: u16 = 3012;
pub(crate) const ID_LINEA: u16 = 3013;
pub(crate) const ID_RECT: u16 = 3014;
pub(crate) const ID_ELIPSE: u16 = 3015;
pub(crate) const ID_RESALTADOR: u16 = 3016;
pub(crate) const ID_PASOS: u16 = 3017;
pub(crate) const ID_LEYENDA: u16 = 3018;
pub(crate) const ID_PIXELADO: u16 = 3019;
pub(crate) const ID_GOMA: u16 = 3020;
pub(crate) const ID_CROP: u16 = 3021;
pub(crate) const ID_RESIZE: u16 = 3022;
pub(crate) const ID_UNDO: u16 = 3023;
pub(crate) const ID_REDO: u16 = 3024;
pub(crate) const ID_PRINT: u16 = 3025;
pub(crate) const ID_EMAIL: u16 = 3026;

pub(crate) struct BotonDef {
    pub id: u16,
    pub icono: Icono,
    pub nombre: &'static str,
    pub habilitado: bool,
}

pub(crate) enum Elemento {
    Separador,
    Muelle,
    Boton(BotonDef),
}

const fn boton(id: u16, icono: Icono, nombre: &'static str, habilitado: bool) -> Elemento {
    Elemento::Boton(BotonDef { id, icono, nombre, habilitado })
}

/// Toolbar del editor V4 en su fase de chrome (S5): las herramientas de
/// anotación esperan la fusión con la ventana de dibujo (S6) y solo el
/// acceso a Draw (lápiz) está vivo; a la derecha, las salidas con lógica.
pub(crate) fn toolbar() -> Vec<Elemento> {
    use Icono::*;
    vec![
        boton(ID_SELECT, AnnotateSelect, "Selección", false),
        boton(ID_TEXTO, AnnotateText, "Texto", false),
        boton(ID_FLECHA, AnnotateArrow, "Flecha", false),
        boton(ID_LINEA, AnnotateLine, "Línea", false),
        boton(ID_RECT, AnnotateShape, "Rectángulo", false),
        boton(ID_ELIPSE, AnnotateEllipse, "Elipse", false),
        boton(ID_DRAW, AnnotatePencil, "Dibujar (abre la ventana de dibujo)", true),
        boton(ID_RESALTADOR, AnnotateHighlight, "Resaltador", false),
        boton(ID_PASOS, AnnotateSteps, "Pasos numerados", false),
        boton(ID_LEYENDA, AnnotateCaption, "Leyenda", false),
        boton(ID_PIXELADO, AnnotatePixelate, "Pixelado", false),
        boton(ID_GOMA, AnnotateEraser, "Goma", false),
        Elemento::Separador,
        boton(ID_CROP, AnnotateCrop, "Recortar", false),
        boton(ID_RESIZE, EditResize, "Redimensionar", false),
        Elemento::Separador,
        boton(ID_UNDO, EditUndo, "Deshacer", false),
        boton(ID_REDO, EditRedo, "Rehacer", false),
        Elemento::Muelle,
        boton(ID_COPIAR, OutputCopy, "Copiar al portapapeles", true),
        boton(ID_GUARDAR, OutputSaveAs, "Guardar como…", true),
        boton(ID_PRINT, OutputPrint, "Imprimir", false),
        boton(ID_EMAIL, OutputEmail, "Email", false),
    ]
}

pub(crate) fn a_items(fila: &[Elemento]) -> Vec<Item> {
    fila.iter()
        .map(|e| match e {
            Elemento::Separador => Item::Separador,
            Elemento::Muelle => Item::Muelle,
            Elemento::Boton(_) => Item::Boton,
        })
        .collect()
}

/// Franjas verticales del cliente: toolbar arriba, status abajo, canvas
/// en medio (nunca negativo).
#[derive(PartialEq, Eq, Debug)]
pub(crate) struct Reparto {
    pub toolbar_fin: i32,
    pub status_inicio: i32,
}

pub(crate) fn reparto(alto_cliente: i32, toolbar: i32, status: i32) -> Reparto {
    let toolbar_fin = toolbar.min(alto_cliente.max(0));
    let status_inicio = (alto_cliente - status).max(toolbar_fin);
    Reparto { toolbar_fin, status_inicio }
}

/// Rect destino de la imagen dentro del lienzo: centrada; si no cabe,
/// reducida manteniendo aspecto. Nunca se amplía.
pub(crate) fn fit_rect(imagen: (u32, u32), lienzo: (i32, i32)) -> Rect {
    let (iw, ih) = (imagen.0 as i64, imagen.1 as i64);
    let (lw, lh) = (lienzo.0 as i64, lienzo.1 as i64);
    if iw == 0 || ih == 0 || lw <= 0 || lh <= 0 {
        return Rect::new(0, 0, 0, 0);
    }
    let (w, h) = if iw <= lw && ih <= lh {
        (iw, ih)
    } else if iw * lh >= ih * lw {
        // Limita el ancho.
        (lw, (ih * lw / iw).max(1))
    } else {
        // Limita el alto.
        ((iw * lh / ih).max(1), lh)
    };
    Rect::new(
        ((lw - w) / 2) as i32,
        ((lh - h) / 2) as i32,
        w as u32,
        h as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imagen_pequena_se_centra_a_tamano_natural() {
        assert_eq!(
            fit_rect((100, 50), (400, 300)),
            Rect::new(150, 125, 100, 50)
        );
    }

    #[test]
    fn imagen_ancha_se_reduce_a_lo_ancho() {
        // 2000×1000 en 400×300 → escala 0.2 → 400×200, centrada en Y.
        assert_eq!(
            fit_rect((2000, 1000), (400, 300)),
            Rect::new(0, 50, 400, 200)
        );
    }

    #[test]
    fn imagen_alta_se_reduce_a_lo_alto() {
        // 500×1500 en 400×300 → escala 0.2 → 100×300, centrada en X.
        assert_eq!(
            fit_rect((500, 1500), (400, 300)),
            Rect::new(150, 0, 100, 300)
        );
    }

    #[test]
    fn lienzo_degenerado_da_rect_vacio() {
        assert_eq!(fit_rect((100, 100), (0, 300)), Rect::new(0, 0, 0, 0));
        assert_eq!(fit_rect((0, 0), (400, 300)), Rect::new(0, 0, 0, 0));
    }

    fn botones(fila: &[Elemento]) -> Vec<&BotonDef> {
        fila.iter()
            .filter_map(|e| match e {
                Elemento::Boton(b) => Some(b),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn la_toolbar_tiene_ids_unicos_y_un_muelle() {
        let fila = toolbar();
        let mut ids: Vec<u16> = botones(&fila).iter().map(|b| b.id).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "ids repetidos");
        let muelles = fila.iter().filter(|e| matches!(e, Elemento::Muelle)).count();
        assert_eq!(muelles, 1);
        assert_eq!(a_items(&fila).len(), fila.len());
    }

    #[test]
    fn en_s5_solo_draw_y_las_salidas_con_logica_estan_habilitados() {
        let fila = toolbar();
        let habilitados: Vec<u16> =
            botones(&fila).iter().filter(|b| b.habilitado).map(|b| b.id).collect();
        assert_eq!(habilitados, vec![ID_DRAW, ID_COPIAR, ID_GUARDAR]);
    }

    #[test]
    fn el_reparto_deja_el_canvas_entre_toolbar_y_status() {
        assert_eq!(
            reparto(600, 54, 36),
            Reparto { toolbar_fin: 54, status_inicio: 564 }
        );
    }

    #[test]
    fn un_cliente_diminuto_no_da_franjas_negativas() {
        let r = reparto(40, 54, 36);
        assert_eq!(r.toolbar_fin, 40);
        assert_eq!(r.status_inicio, 40); // canvas de alto 0, nunca negativo
        let r = reparto(0, 54, 36);
        assert_eq!(r.toolbar_fin, 0);
        assert_eq!(r.status_inicio, 0);
    }
}
