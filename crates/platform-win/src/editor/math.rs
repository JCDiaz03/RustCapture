//! Encaje de la captura en el lienzo del editor y composición del chrome
//! V4 (puro, TDD).

use rustcapture_core::ports::Rect;

use crate::ui::botonera::{Elemento, boton};
use crate::ui::iconos::Icono;

/// Alto LÓGICO de la toolbar (botón 28 + rejilla 4 arriba/abajo).
pub(crate) const TOOLBAR_LOGICO: i32 = 36;
/// Alto LÓGICO de la barra de propiedades contextual.
pub(crate) const PROPS_LOGICO: i32 = 26;
/// Alto LÓGICO de la barra de estado.
pub(crate) const STATUS_LOGICO: i32 = 24;

/// Herramientas de anotación vivas del editor (motor D5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Herramienta {
    Texto,
    Flecha,
    Linea,
    Rect,
    Elipse,
    Lapiz,
    Resaltador,
    Pasos,
    Pixelado,
    /// Elige un objeto ya colocado y lo mueve arrastrando.
    Seleccion,
    /// Borra el objeto que se señala (f.27: objetos, no píxeles).
    Goma,
}

/// Botón de toolbar ↔ herramienta (None = el id no es una herramienta).
pub(crate) fn herramienta_de_id(id: u16) -> Option<Herramienta> {
    match id {
        ID_TEXTO => Some(Herramienta::Texto),
        ID_FLECHA => Some(Herramienta::Flecha),
        ID_LINEA => Some(Herramienta::Linea),
        ID_RECT => Some(Herramienta::Rect),
        ID_ELIPSE => Some(Herramienta::Elipse),
        ID_DRAW => Some(Herramienta::Lapiz),
        ID_RESALTADOR => Some(Herramienta::Resaltador),
        ID_PASOS => Some(Herramienta::Pasos),
        ID_PIXELADO => Some(Herramienta::Pixelado),
        ID_SELECT => Some(Herramienta::Seleccion),
        ID_GOMA => Some(Herramienta::Goma),
        _ => None,
    }
}

pub(crate) fn id_de_herramienta(h: Herramienta) -> u16 {
    match h {
        Herramienta::Texto => ID_TEXTO,
        Herramienta::Flecha => ID_FLECHA,
        Herramienta::Linea => ID_LINEA,
        Herramienta::Rect => ID_RECT,
        Herramienta::Elipse => ID_ELIPSE,
        Herramienta::Lapiz => ID_DRAW,
        Herramienta::Resaltador => ID_RESALTADOR,
        Herramienta::Pasos => ID_PASOS,
        Herramienta::Pixelado => ID_PIXELADO,
        Herramienta::Seleccion => ID_SELECT,
        Herramienta::Goma => ID_GOMA,
    }
}

/// `true` si la herramienta opera sobre objetos ya colocados en vez de
/// crear uno nuevo: no dibuja, y su clic hace hit-test.
pub(crate) fn opera_sobre_objetos(h: Herramienta) -> bool {
    matches!(h, Herramienta::Seleccion | Herramienta::Goma)
}

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

/// Toolbar del editor V4: las herramientas del motor D5 en vivo (la
/// activa se marca con el estado 'activo' del IconButton); leyenda, crop
/// y resize esperan su fase.
pub(crate) fn toolbar() -> Vec<Elemento> {
    use Icono::*;
    vec![
        boton(ID_SELECT, AnnotateSelect, "Selección", true),
        boton(ID_TEXTO, AnnotateText, "Texto", true),
        boton(ID_FLECHA, AnnotateArrow, "Flecha", true),
        boton(ID_LINEA, AnnotateLine, "Línea", true),
        boton(ID_RECT, AnnotateShape, "Rectángulo", true),
        boton(ID_ELIPSE, AnnotateEllipse, "Elipse", true),
        boton(ID_DRAW, AnnotatePencil, "Lápiz", true),
        boton(ID_RESALTADOR, AnnotateHighlight, "Resaltador", true),
        boton(ID_PASOS, AnnotateSteps, "Pasos numerados", true),
        boton(ID_LEYENDA, AnnotateCaption, "Leyenda", false),
        boton(ID_PIXELADO, AnnotatePixelate, "Pixelado", true),
        boton(ID_GOMA, AnnotateEraser, "Goma (Supr)", true),
        Elemento::Separador,
        boton(ID_CROP, AnnotateCrop, "Recortar", false),
        boton(ID_RESIZE, EditResize, "Redimensionar", false),
        Elemento::Separador,
        boton(ID_UNDO, EditUndo, "Deshacer (Ctrl+Z)", true),
        boton(ID_REDO, EditRedo, "Rehacer (Ctrl+Y)", true),
        Elemento::Muelle,
        boton(ID_COPIAR, OutputCopy, "Copiar al portapapeles", true),
        boton(ID_GUARDAR, OutputSaveAs, "Guardar como…", true),
        boton(ID_PRINT, OutputPrint, "Imprimir", false),
        boton(ID_EMAIL, OutputEmail, "Email", false),
    ]
}

/// Lado LÓGICO de las asas del objeto seleccionado.
pub(crate) const ASA_LOGICA: i32 = 6;

/// Las 8 asas de un rect (esquinas + puntos medios de cada lado), como
/// rects de lado `asa` centrados en cada punto. En coordenadas de vista.
pub(crate) fn asas(caja: Rect, asa: i32) -> [Rect; 8] {
    let (x0, y0) = (caja.x, caja.y);
    let (x1, y1) = (
        caja.x + caja.width as i32,
        caja.y + caja.height as i32,
    );
    let (xm, ym) = ((x0 + x1) / 2, (y0 + y1) / 2);
    let lado = asa.max(2);
    let centrada = |cx: i32, cy: i32| {
        Rect::new(cx - lado / 2, cy - lado / 2, lado as u32, lado as u32)
    };
    [
        centrada(x0, y0),
        centrada(xm, y0),
        centrada(x1, y0),
        centrada(x1, ym),
        centrada(x1, y1),
        centrada(xm, y1),
        centrada(x0, y1),
        centrada(x0, ym),
    ]
}

/// Franjas verticales del cliente: toolbar y propiedades arriba, status
/// abajo, canvas en medio (nunca negativo).
#[derive(PartialEq, Eq, Debug)]
pub(crate) struct Reparto {
    pub toolbar_fin: i32,
    pub props_fin: i32,
    pub status_inicio: i32,
}

pub(crate) fn reparto(alto_cliente: i32, toolbar: i32, props: i32, status: i32) -> Reparto {
    let toolbar_fin = toolbar.min(alto_cliente.max(0));
    let props_fin = (toolbar_fin + props).min(alto_cliente.max(0));
    let status_inicio = (alto_cliente - status).max(props_fin);
    Reparto { toolbar_fin, props_fin, status_inicio }
}

/// Punto de la vista → píxel del frame; `None` fuera del área encajada.
pub(crate) fn view_to_frame(p: (i32, i32), destino: Rect, frame: (u32, u32)) -> Option<(i32, i32)> {
    if destino.is_empty() || frame.0 == 0 || frame.1 == 0 {
        return None;
    }
    let dentro = p.0 >= destino.x
        && (p.0 as i64) < destino.right()
        && p.1 >= destino.y
        && (p.1 as i64) < destino.bottom();
    if !dentro {
        return None;
    }
    let fx = (p.0 - destino.x) as i64 * frame.0 as i64 / destino.width as i64;
    let fy = (p.1 - destino.y) as i64 * frame.1 as i64 / destino.height as i64;
    Some((
        (fx as i32).clamp(0, frame.0 as i32 - 1),
        (fy as i32).clamp(0, frame.1 as i32 - 1),
    ))
}

/// Píxel del frame → punto de la vista (esquina del píxel escalado).
pub(crate) fn frame_to_view(p: (i32, i32), destino: Rect, frame: (u32, u32)) -> (i32, i32) {
    if destino.is_empty() || frame.0 == 0 || frame.1 == 0 {
        return (0, 0);
    }
    (
        destino.x + (p.0 as i64 * destino.width as i64 / frame.0 as i64) as i32,
        destino.y + (p.1 as i64 * destino.height as i64 / frame.1 as i64) as i32,
    )
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

    fn botones(fila: &[Elemento]) -> Vec<&crate::ui::botonera::BotonDef> {
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
        assert_eq!(crate::ui::botonera::a_items(&fila).len(), fila.len());
    }

    #[test]
    fn las_herramientas_del_motor_y_las_salidas_estan_habilitadas() {
        let fila = toolbar();
        let habilitados: Vec<u16> =
            botones(&fila).iter().filter(|b| b.habilitado).map(|b| b.id).collect();
        assert_eq!(
            habilitados,
            vec![
                ID_SELECT,
                ID_TEXTO,
                ID_FLECHA,
                ID_LINEA,
                ID_RECT,
                ID_ELIPSE,
                ID_DRAW,
                ID_RESALTADOR,
                ID_PASOS,
                ID_PIXELADO,
                ID_GOMA,
                ID_UNDO,
                ID_REDO,
                ID_COPIAR,
                ID_GUARDAR
            ]
        );
    }

    #[test]
    fn cada_herramienta_mapea_a_su_boton_y_vuelta() {
        use Herramienta::*;
        for h in [
            Texto, Flecha, Linea, Rect, Elipse, Lapiz, Resaltador, Pasos, Pixelado, Seleccion,
            Goma,
        ] {
            assert_eq!(herramienta_de_id(id_de_herramienta(h)), Some(h));
        }
        assert_eq!(herramienta_de_id(ID_COPIAR), None);
        assert_eq!(herramienta_de_id(ID_LEYENDA), None); // pendiente
    }

    #[test]
    fn solo_seleccion_y_goma_operan_sobre_objetos_ya_colocados() {
        use Herramienta::*;
        assert!(opera_sobre_objetos(Seleccion) && opera_sobre_objetos(Goma));
        for h in [Texto, Flecha, Linea, Rect, Elipse, Lapiz, Resaltador, Pasos, Pixelado] {
            assert!(!opera_sobre_objetos(h), "{h:?} no crea objetos nuevos");
        }
    }

    #[test]
    fn las_asas_rodean_la_caja_centradas_en_sus_puntos() {
        // Caja 100×50 en (10,20) con asas de 6 → cada asa se centra en su
        // punto, así que sobresale 3 px de la caja.
        let a = asas(Rect::new(10, 20, 100, 50), 6);
        assert_eq!(a[0], Rect::new(7, 17, 6, 6)); // esquina sup. izq.
        assert_eq!(a[1], Rect::new(57, 17, 6, 6)); // medio superior
        assert_eq!(a[2], Rect::new(107, 17, 6, 6)); // esquina sup. der.
        assert_eq!(a[4], Rect::new(107, 67, 6, 6)); // esquina inf. der.
        assert_eq!(a[6], Rect::new(7, 67, 6, 6)); // esquina inf. izq.
        // Las 8 son distintas.
        for i in 0..8 {
            for j in i + 1..8 {
                assert_ne!(a[i], a[j], "asas {i} y {j} coinciden");
            }
        }
    }

    #[test]
    fn el_reparto_apila_toolbar_props_canvas_y_status() {
        assert_eq!(
            reparto(600, 54, 39, 36),
            Reparto { toolbar_fin: 54, props_fin: 93, status_inicio: 564 }
        );
    }

    #[test]
    fn un_cliente_diminuto_no_da_franjas_negativas() {
        let r = reparto(40, 54, 39, 36);
        assert_eq!(r.toolbar_fin, 40);
        assert_eq!(r.props_fin, 40);
        assert_eq!(r.status_inicio, 40); // canvas de alto 0, nunca negativo
        let r = reparto(0, 54, 39, 36);
        assert_eq!(r, Reparto { toolbar_fin: 0, props_fin: 0, status_inicio: 0 });
    }

    // Imagen 200×100 encajada en destino (10,20,100,50): escala 0.5.
    const DESTINO: Rect = Rect { x: 10, y: 20, width: 100, height: 50 };
    const FRAME: (u32, u32) = (200, 100);

    #[test]
    fn dentro_del_destino_escala_al_frame() {
        assert_eq!(view_to_frame((10, 20), DESTINO, FRAME), Some((0, 0)));
        assert_eq!(view_to_frame((60, 45), DESTINO, FRAME), Some((100, 50)));
        assert_eq!(view_to_frame((109, 69), DESTINO, FRAME), Some((198, 98)));
    }

    #[test]
    fn fuera_del_destino_es_none() {
        assert_eq!(view_to_frame((9, 20), DESTINO, FRAME), None);
        assert_eq!(view_to_frame((10, 70), DESTINO, FRAME), None);
    }

    #[test]
    fn frame_to_view_es_el_inverso() {
        assert_eq!(frame_to_view((0, 0), DESTINO, FRAME), (10, 20));
        assert_eq!(frame_to_view((100, 50), DESTINO, FRAME), (60, 45));
    }

    #[test]
    fn degenerado_no_divide_por_cero() {
        let destino = Rect::new(0, 0, 0, 0);
        assert_eq!(view_to_frame((5, 5), destino, FRAME), None);
        assert_eq!(frame_to_view((5, 5), destino, FRAME), (0, 0));
    }
}
