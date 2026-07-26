//! Documento de anotaciones + Commands con undo/redo ilimitado (D6).
//! Los Commands POSEEN los objetos y los mueven con Option::take:
//! nada exige Clone.

use crate::annotate::canvas::Canvas;
use crate::annotate::objeto::Objeto;
use crate::annotate::text::RenderContext;
use crate::ports::Frame;

/// Lista ordenada de objetos: el orden es el orden de pintado (z-order).
#[derive(Default)]
pub struct Document {
    objetos: Vec<Objeto>,
}

impl Document {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.objetos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objetos.is_empty()
    }

    /// Hornea todos los objetos sobre el frame, en orden.
    pub fn render_onto(&self, frame: &mut Frame, ctx: &RenderContext) {
        let mut canvas = Canvas::new(frame);
        for objeto in &self.objetos {
            objeto.render(&mut canvas, ctx);
        }
    }

    /// Igual que `render_onto`, pero pintando el objeto `index` desplazado
    /// `delta`. Es el preview del arrastre de la herramienta Selección: el
    /// documento NO se toca, así el movimiento solo llega a existir como
    /// `Command::Move` cuando el usuario suelta el botón (D6 sigue siendo
    /// la única puerta de edición). Un `index` fuera de rango pinta normal.
    pub fn render_onto_moved(
        &self,
        frame: &mut Frame,
        ctx: &RenderContext,
        index: usize,
        delta: (i32, i32),
    ) {
        let mut canvas = Canvas::new(frame);
        for (i, objeto) in self.objetos.iter().enumerate() {
            if i == index && delta != (0, 0) {
                let mut movido = objeto.clone();
                movido.translate(delta);
                movido.render(&mut canvas, ctx);
            } else {
                objeto.render(&mut canvas, ctx);
            }
        }
    }

    /// Hermano de `render_onto_moved` para el arrastre del asa de rotación:
    /// pinta el objeto `index` con un giro extra sin tocar el documento.
    pub fn render_onto_rotated(
        &self,
        frame: &mut Frame,
        ctx: &RenderContext,
        index: usize,
        delta_rad: f32,
    ) {
        let mut canvas = Canvas::new(frame);
        for (i, objeto) in self.objetos.iter().enumerate() {
            if i == index && delta_rad != 0.0 {
                let mut girado = objeto.clone();
                girado.rotar(delta_rad);
                girado.render(&mut canvas, ctx);
            } else {
                objeto.render(&mut canvas, ctx);
            }
        }
    }

    pub fn get(&self, index: usize) -> Option<&Objeto> {
        self.objetos.get(index)
    }

    /// Índice del objeto bajo el punto, **el de más arriba** en el z-order
    /// (se recorre al revés): si dos se solapan, gana el que se ve encima,
    /// que es el que el usuario cree estar señalando.
    pub fn hit_test(&self, punto: (i32, i32), ctx: &RenderContext) -> Option<usize> {
        self.objetos
            .iter()
            .enumerate()
            .rev()
            .find(|(_, o)| o.bounds(ctx).contains_point(punto))
            .map(|(i, _)| i)
    }
}

/// Comando de edición (D6): posee el objeto que mueve.
pub enum Command {
    Add {
        objeto: Option<Objeto>,
    },
    Remove {
        index: usize,
        removed: Option<Objeto>,
    },
    /// Desplaza un objeto ya colocado. Revertir = aplicar el delta negado,
    /// así que mover sale deshacible sin guardar la posición anterior.
    Move {
        index: usize,
        delta: (i32, i32),
    },
    /// Gira un objeto ya colocado. Como `Move`, revertir es aplicar el
    /// delta negado: no guarda el ángulo anterior.
    Rotate {
        index: usize,
        delta_rad: f32,
    },
    /// Sustituye un objeto conservando su POSICIÓN en el z-order (borrar y
    /// volver a añadir lo mandaría al frente). Lo usa la reedición de un
    /// texto ya colocado.
    Replace {
        index: usize,
        nuevo: Option<Objeto>,
        anterior: Option<Objeto>,
    },
}

impl Command {
    pub fn add(objeto: Objeto) -> Self {
        Command::Add {
            objeto: Some(objeto),
        }
    }

    pub fn remove(index: usize) -> Self {
        Command::Remove {
            index,
            removed: None,
        }
    }

    pub fn move_by(index: usize, delta: (i32, i32)) -> Self {
        Command::Move { index, delta }
    }

    pub fn rotate_by(index: usize, delta_rad: f32) -> Self {
        Command::Rotate { index, delta_rad }
    }

    pub fn replace(index: usize, nuevo: Objeto) -> Self {
        Command::Replace {
            index,
            nuevo: Some(nuevo),
            anterior: None,
        }
    }

    /// Ejecuta sobre el documento; false = inválido (no aplicar).
    fn apply(&mut self, doc: &mut Document) -> bool {
        match self {
            Command::Add { objeto } => match objeto.take() {
                Some(o) => {
                    doc.objetos.push(o);
                    true
                }
                None => false,
            },
            Command::Remove { index, removed } => {
                if *index >= doc.objetos.len() {
                    return false;
                }
                *removed = Some(doc.objetos.remove(*index));
                true
            }
            Command::Move { index, delta } => match doc.objetos.get_mut(*index) {
                // Un delta nulo no es una edición: no se apila.
                Some(_) if *delta == (0, 0) => false,
                Some(o) => {
                    o.translate(*delta);
                    true
                }
                None => false,
            },
            Command::Rotate { index, delta_rad } => match doc.objetos.get_mut(*index) {
                // Un giro nulo no es una edición: no se apila.
                Some(_) if *delta_rad == 0.0 => false,
                Some(o) => {
                    o.rotar(*delta_rad);
                    true
                }
                None => false,
            },
            Command::Replace {
                index,
                nuevo,
                anterior,
            } => match (nuevo.take(), doc.objetos.get_mut(*index)) {
                (Some(n), Some(hueco)) => {
                    *anterior = Some(std::mem::replace(hueco, n));
                    true
                }
                // Índice inválido: se devuelve el objeto a su sitio para
                // que el Command siga siendo reutilizable (redo).
                (n, _) => {
                    *nuevo = n;
                    false
                }
            },
        }
    }

    /// Deshace lo hecho por `apply` (solo se llama tras un apply con éxito).
    fn revert(&mut self, doc: &mut Document) {
        match self {
            Command::Add { objeto } => {
                *objeto = doc.objetos.pop();
            }
            Command::Remove { index, removed } => {
                if let Some(o) = removed.take() {
                    doc.objetos.insert(*index, o);
                }
            }
            Command::Move { index, delta } => {
                if let Some(o) = doc.objetos.get_mut(*index) {
                    o.translate((-delta.0, -delta.1));
                }
            }
            Command::Rotate { index, delta_rad } => {
                if let Some(o) = doc.objetos.get_mut(*index) {
                    o.rotar(-*delta_rad);
                }
            }
            Command::Replace {
                index,
                nuevo,
                anterior,
            } => {
                if let (Some(a), Some(hueco)) = (anterior.take(), doc.objetos.get_mut(*index)) {
                    *nuevo = Some(std::mem::replace(hueco, a));
                }
            }
        }
    }
}

/// Pilas de undo/redo ilimitadas (D6).
#[derive(Default)]
pub struct History {
    undo: Vec<Command>,
    redo: Vec<Command>,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// Aplica y apila; un comando nuevo invalida el redo.
    pub fn apply(&mut self, doc: &mut Document, mut cmd: Command) -> bool {
        if !cmd.apply(doc) {
            return false;
        }
        self.undo.push(cmd);
        self.redo.clear();
        true
    }

    pub fn undo(&mut self, doc: &mut Document) -> bool {
        match self.undo.pop() {
            Some(mut cmd) => {
                cmd.revert(doc);
                self.redo.push(cmd);
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self, doc: &mut Document) -> bool {
        match self.redo.pop() {
            Some(mut cmd) => {
                if !cmd.apply(doc) {
                    return false;
                }
                self.undo.push(cmd);
                true
            }
            None => false,
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotate::annotations::RectAnnotation;
    use crate::annotate::style::{Color, Style};
    use crate::ports::Rect;

    fn caja(x: i32) -> Objeto {
        RectAnnotation {
            rect: Rect::new(x, 1, 3, 3),
            style: Style {
                color: Color::rgb(255, 0, 0),
                thickness: 1,
            },
        }
        .into()
    }

    fn render(doc: &Document) -> Frame {
        let mut frame = Frame::filled(20, 10, [0, 0, 0, 255]);
        doc.render_onto(&mut frame, &RenderContext::sin_fuente());
        frame
    }

    fn es_rojo(frame: &Frame, x: u32, y: u32) -> bool {
        frame.pixel(x, y) == Some([255, 0, 0, 255])
    }

    #[test]
    fn add_pinta_y_undo_lo_quita() {
        let mut doc = Document::new();
        let mut historia = History::new();
        assert!(historia.apply(&mut doc, Command::add(caja(2))));
        assert_eq!(doc.len(), 1);
        assert!(es_rojo(&render(&doc), 2, 1));

        assert!(historia.undo(&mut doc));
        assert!(doc.is_empty());
        assert!(!es_rojo(&render(&doc), 2, 1));

        assert!(historia.redo(&mut doc));
        assert!(es_rojo(&render(&doc), 2, 1));
    }

    #[test]
    fn remove_borra_el_objeto_y_undo_lo_restaura_en_su_sitio() {
        let mut doc = Document::new();
        let mut historia = History::new();
        historia.apply(&mut doc, Command::add(caja(2)));
        historia.apply(&mut doc, Command::add(caja(10)));

        assert!(historia.apply(&mut doc, Command::remove(0)));
        let frame = render(&doc);
        assert!(!es_rojo(&frame, 2, 1) && es_rojo(&frame, 10, 1));

        assert!(historia.undo(&mut doc));
        let frame = render(&doc);
        assert!(es_rojo(&frame, 2, 1) && es_rojo(&frame, 10, 1));
        assert_eq!(doc.len(), 2);
    }

    #[test]
    fn remove_invalido_no_se_apila() {
        let mut doc = Document::new();
        let mut historia = History::new();
        assert!(!historia.apply(&mut doc, Command::remove(5)));
        assert!(!historia.can_undo());
    }

    #[test]
    fn un_comando_nuevo_vacia_el_redo() {
        let mut doc = Document::new();
        let mut historia = History::new();
        historia.apply(&mut doc, Command::add(caja(2)));
        historia.undo(&mut doc);
        assert!(historia.can_redo());
        historia.apply(&mut doc, Command::add(caja(10)));
        assert!(!historia.can_redo());
        assert!(!historia.redo(&mut doc));
    }

    #[test]
    fn el_hit_test_devuelve_el_objeto_de_encima() {
        let ctx = RenderContext::sin_fuente();
        let mut doc = Document::new();
        let mut historia = History::new();
        // Dos cajas solapadas: la segunda se pinta encima.
        historia.apply(&mut doc, Command::add(caja(2)));
        historia.apply(&mut doc, Command::add(caja(3)));
        // En la zona solapada gana la de arriba (índice 1).
        assert_eq!(doc.hit_test((3, 1), &ctx), Some(1));
        // Donde solo llega la de abajo, gana esa.
        assert_eq!(doc.hit_test((2, 1), &ctx), Some(0));
        // Fuera de todo: nada.
        assert_eq!(doc.hit_test((50, 50), &ctx), None);
        assert_eq!(Document::new().hit_test((2, 1), &ctx), None);
    }

    #[test]
    fn move_desplaza_el_objeto_y_undo_lo_devuelve() {
        let ctx = RenderContext::sin_fuente();
        let mut doc = Document::new();
        let mut historia = History::new();
        historia.apply(&mut doc, Command::add(caja(2)));
        let antes = doc.get(0).unwrap().bounds(&ctx);

        assert!(historia.apply(&mut doc, Command::move_by(0, (10, 5))));
        let movido = doc.get(0).unwrap().bounds(&ctx);
        assert_eq!((movido.x, movido.y), (antes.x + 10, antes.y + 5));
        // Y lo pintado se ha movido con él.
        let frame = render(&doc);
        assert!(!es_rojo(&frame, 2, 1) && es_rojo(&frame, 12, 6));

        assert!(historia.undo(&mut doc));
        assert_eq!(doc.get(0).unwrap().bounds(&ctx), antes);
        assert!(es_rojo(&render(&doc), 2, 1));

        assert!(historia.redo(&mut doc));
        assert_eq!(doc.get(0).unwrap().bounds(&ctx), movido);
    }

    /// Caja alargada: al girar 90° la caja pasa de ancha a alta.
    fn alargada() -> Objeto {
        crate::annotate::annotations::RectAnnotation {
            rect: Rect::new(4, 4, 12, 2),
            style: Style {
                color: Color::rgb(255, 0, 0),
                thickness: 1,
            },
        }
        .into()
    }

    #[test]
    fn rotate_gira_el_objeto_y_undo_lo_devuelve() {
        let ctx = RenderContext::sin_fuente();
        let mut doc = Document::new();
        let mut historia = History::new();
        historia.apply(&mut doc, Command::add(alargada()));
        let antes = doc.get(0).unwrap().bounds(&ctx);
        assert!(antes.width > antes.height);

        assert!(historia.apply(&mut doc, Command::rotate_by(0, std::f32::consts::FRAC_PI_2)));
        let girado = doc.get(0).unwrap().bounds(&ctx);
        assert!(girado.height > girado.width, "no giró: {girado:?}");

        assert!(historia.undo(&mut doc));
        assert_eq!(doc.get(0).unwrap().bounds(&ctx), antes);
        assert!(historia.redo(&mut doc));
        assert_eq!(doc.get(0).unwrap().bounds(&ctx), girado);
    }

    #[test]
    fn un_rotate_invalido_o_nulo_no_se_apila() {
        let mut doc = Document::new();
        let mut historia = History::new();
        historia.apply(&mut doc, Command::add(caja(2)));
        assert!(!historia.apply(&mut doc, Command::rotate_by(9, 0.5)));
        // Soltar el asa sin haber girado no debe gastar un undo.
        assert!(!historia.apply(&mut doc, Command::rotate_by(0, 0.0)));
        assert!(historia.undo(&mut doc));
        assert!(!historia.can_undo());
    }

    #[test]
    fn el_preview_pinta_girado_sin_tocar_el_documento() {
        let ctx = RenderContext::sin_fuente();
        let mut doc = Document::new();
        let mut historia = History::new();
        historia.apply(&mut doc, Command::add(alargada()));
        let antes = doc.get(0).unwrap().bounds(&ctx);

        let mut frame = Frame::filled(24, 24, [0, 0, 0, 255]);
        doc.render_onto_rotated(&mut frame, &ctx, 0, std::f32::consts::FRAC_PI_2);
        // Girado 90° alrededor de (9.5, 4.5): pinta por encima y por debajo
        // de la franja original, no en sus extremos horizontales.
        assert!(!es_rojo(&frame, 4, 4) && !es_rojo(&frame, 15, 4));
        assert!((0..24).any(|y| es_rojo(&frame, 9, y) || es_rojo(&frame, 10, y)));
        // El documento sigue intacto: el giro aún no existe como Command.
        assert_eq!(doc.get(0).unwrap().bounds(&ctx), antes);
    }

    #[test]
    fn replace_sustituye_en_su_sitio_del_z_order() {
        let ctx = RenderContext::sin_fuente();
        let mut doc = Document::new();
        let mut historia = History::new();
        historia.apply(&mut doc, Command::add(caja(2)));
        historia.apply(&mut doc, Command::add(caja(10)));
        // Se sustituye el PRIMERO: debe seguir siendo el primero.
        assert!(historia.apply(&mut doc, Command::replace(0, caja(16))));
        assert_eq!(doc.len(), 2);
        assert_eq!(doc.get(0).unwrap().bounds(&ctx).x, 16);
        assert_eq!(doc.get(1).unwrap().bounds(&ctx).x, 10);

        assert!(historia.undo(&mut doc));
        assert_eq!(doc.get(0).unwrap().bounds(&ctx).x, 2);
        assert_eq!(doc.get(1).unwrap().bounds(&ctx).x, 10);

        assert!(historia.redo(&mut doc));
        assert_eq!(doc.get(0).unwrap().bounds(&ctx).x, 16);
    }

    #[test]
    fn un_replace_con_indice_invalido_no_se_apila() {
        let mut doc = Document::new();
        let mut historia = History::new();
        assert!(!historia.apply(&mut doc, Command::replace(3, caja(2))));
        assert!(!historia.can_undo());
        assert!(doc.is_empty());
    }

    #[test]
    fn el_preview_pinta_movido_sin_tocar_el_documento() {
        let ctx = RenderContext::sin_fuente();
        let mut doc = Document::new();
        let mut historia = History::new();
        historia.apply(&mut doc, Command::add(caja(2)));
        let antes = doc.get(0).unwrap().bounds(&ctx);

        let mut frame = Frame::filled(20, 10, [0, 0, 0, 255]);
        doc.render_onto_moved(&mut frame, &ctx, 0, (10, 5));
        // Se pinta en el destino, no en el origen...
        assert!(!es_rojo(&frame, 2, 1) && es_rojo(&frame, 12, 6));
        // ...y el documento sigue intacto (el Command aún no existe).
        assert_eq!(doc.get(0).unwrap().bounds(&ctx), antes);
        assert!(!historia.can_undo() || doc.len() == 1);

        // Índice fuera de rango: pinta como si nada.
        let mut frame = Frame::filled(20, 10, [0, 0, 0, 255]);
        doc.render_onto_moved(&mut frame, &ctx, 9, (10, 5));
        assert!(es_rojo(&frame, 2, 1));
    }

    #[test]
    fn un_move_invalido_o_nulo_no_se_apila() {
        let mut doc = Document::new();
        let mut historia = History::new();
        historia.apply(&mut doc, Command::add(caja(2)));
        // Índice que no existe.
        assert!(!historia.apply(&mut doc, Command::move_by(9, (1, 1))));
        // Delta nulo: arrastrar sin mover no debe gastar un undo.
        assert!(!historia.apply(&mut doc, Command::move_by(0, (0, 0))));
        // Solo queda en la pila el Add inicial.
        assert!(historia.undo(&mut doc));
        assert!(!historia.can_undo());
    }

    #[test]
    fn undo_sin_historia_devuelve_false() {
        let mut doc = Document::new();
        let mut historia = History::new();
        assert!(!historia.undo(&mut doc));
        assert!(!historia.redo(&mut doc));
    }
}
