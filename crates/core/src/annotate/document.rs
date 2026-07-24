//! Documento de anotaciones + Commands con undo/redo ilimitado (D6).
//! Los Commands POSEEN las anotaciones y las mueven con Option::take:
//! nada exige Clone.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::text::RenderContext;
use crate::ports::Frame;

/// Lista ordenada de anotaciones: el orden es el orden de pintado.
#[derive(Default)]
pub struct Document {
    annotations: Vec<Box<dyn Annotation>>,
}

impl Document {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.annotations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.annotations.is_empty()
    }

    /// Hornea todas las anotaciones sobre el frame, en orden.
    pub fn render_onto(&self, frame: &mut Frame, ctx: &RenderContext) {
        let mut canvas = Canvas::new(frame);
        for annotation in &self.annotations {
            annotation.render(&mut canvas, ctx);
        }
    }
}

/// Comando de edición (D6): posee la anotación que mueve.
pub enum Command {
    Add {
        annotation: Option<Box<dyn Annotation>>,
    },
    Remove {
        index: usize,
        removed: Option<Box<dyn Annotation>>,
    },
}

impl Command {
    pub fn add(annotation: Box<dyn Annotation>) -> Self {
        Command::Add {
            annotation: Some(annotation),
        }
    }

    pub fn remove(index: usize) -> Self {
        Command::Remove {
            index,
            removed: None,
        }
    }

    /// Ejecuta sobre el documento; false = inválido (no aplicar).
    fn apply(&mut self, doc: &mut Document) -> bool {
        match self {
            Command::Add { annotation } => match annotation.take() {
                Some(a) => {
                    doc.annotations.push(a);
                    true
                }
                None => false,
            },
            Command::Remove { index, removed } => {
                if *index >= doc.annotations.len() {
                    return false;
                }
                *removed = Some(doc.annotations.remove(*index));
                true
            }
        }
    }

    /// Deshace lo hecho por `apply` (solo se llama tras un apply con éxito).
    fn revert(&mut self, doc: &mut Document) {
        match self {
            Command::Add { annotation } => {
                *annotation = doc.annotations.pop();
            }
            Command::Remove { index, removed } => {
                if let Some(a) = removed.take() {
                    doc.annotations.insert(*index, a);
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

    fn caja(x: i32) -> Box<dyn Annotation> {
        Box::new(RectAnnotation {
            rect: Rect::new(x, 1, 3, 3),
            style: Style {
                color: Color::rgb(255, 0, 0),
                thickness: 1,
            },
        })
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
    fn undo_sin_historia_devuelve_false() {
        let mut doc = Document::new();
        let mut historia = History::new();
        assert!(!historia.undo(&mut doc));
        assert!(!historia.redo(&mut doc));
    }
}
