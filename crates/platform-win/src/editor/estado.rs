//! Estado del editor V4 con la anotación in situ (fusión de la antigua
//! ventana de dibujo): frame base + documento de objetos (D5) + Command
//! con undo/redo (D6), buffers de render persistentes y propiedades de
//! la herramienta activa.

use rustcapture_core::annotate::annotations::{
    Annotation, ArrowAnnotation, EllipseAnnotation, HighlightAnnotation, LineAnnotation,
    PenAnnotation, RectAnnotation,
};
use rustcapture_core::annotate::{Color, Document, History, RenderContext, Style};
use rustcapture_core::ports::Frame;
use windows::Win32::Foundation::RECT;

use crate::gdi::raii::Dib;
use crate::gdi::{copy_frame_to_dib, dib_from_frame};
use crate::gdi::raii::{MemDc, ScreenDc};
use crate::ui::tooltip::Tooltips;

use super::math::Herramienta;
use super::props::Accion;
use super::texto::EditBox;

pub(super) const GROSORES: [u32; 5] = [1, 2, 3, 5, 8];
pub(super) const TAMANOS: [f32; 5] = [12.0, 16.0, 20.0, 28.0, 36.0];
/// Color de anotación por defecto: el acento cálido del diseño (#D83B01).
pub(super) const COLOR_DEFECTO: Color = Color::rgb(0xD8, 0x3B, 0x01);

pub(super) struct DragState {
    pub start: (i32, i32),
    pub current: (i32, i32),
    pub points: Vec<(i32, i32)>,
}

pub(super) struct EditorState {
    /// Captura original: el documento se re-renderiza siempre sobre ella.
    pub base: Frame,
    /// base + documento horneado (lo que guardan Copiar/Guardar).
    pub committed: Frame,
    pub committed_dib: Dib,
    /// Buffers persistentes del preview del arrastre: se reescriben sin
    /// asignar memoria (las dimensiones nunca cambian).
    pub preview: Frame,
    pub preview_dib: Dib,
    pub doc: Document,
    pub history: History,
    pub ctx: RenderContext,
    pub tiene_fuente: bool,
    pub herramienta: Herramienta,
    pub color: Color,
    pub grosor: u32,
    pub tamano_texto: f32,
    pub negrita: bool,
    pub drag: Option<DragState>,
    pub edit: Option<EditBox>,
    pub cerrado: bool,
    /// Documento cambiado desde el último guardado/copiado.
    pub dirty: bool,
    /// Nombre del archivo tras Guardar como (título y status bar).
    pub nombre: Option<String>,
    pub tooltips: Option<Tooltips>,
    /// Zonas clicables de la property bar (las rellena el pintado).
    pub chips: Vec<(RECT, Accion)>,
}

impl EditorState {
    pub(super) fn new(base: Frame) -> windows::core::Result<Self> {
        let (ctx, tiene_fuente) = cargar_contexto();
        let screen = ScreenDc::get()?;
        let dc = MemDc::compatible_with(&screen)?;
        let committed = base.clone();
        let committed_dib = dib_from_frame(&dc, &committed)?;
        let preview = committed.clone();
        let preview_dib = dib_from_frame(&dc, &preview)?;
        Ok(Self {
            base,
            committed,
            committed_dib,
            preview,
            preview_dib,
            doc: Document::new(),
            history: History::new(),
            ctx,
            tiene_fuente,
            herramienta: Herramienta::Flecha,
            color: COLOR_DEFECTO,
            grosor: 3,
            tamano_texto: 20.0,
            negrita: false,
            drag: None,
            edit: None,
            cerrado: false,
            dirty: false,
            nombre: None,
            tooltips: None,
            chips: Vec::new(),
        })
    }

    /// Regenera el frame comprometido (base + documento) sobre los
    /// buffers existentes — sin asignar (las dimensiones son fijas).
    pub(super) fn refresh_committed(&mut self) {
        self.committed.pixels.copy_from_slice(&self.base.pixels);
        self.doc.render_onto(&mut self.committed, &self.ctx);
        copy_frame_to_dib(&self.committed, &mut self.committed_dib);
    }

    /// Anotación provisional del arrastre actual (None con Texto o sin drag).
    pub(super) fn anotacion_en_curso(&self) -> Option<Box<dyn Annotation>> {
        let drag = self.drag.as_ref()?;
        let style = Style { color: self.color, thickness: self.grosor };
        let rect = crate::overlay::math::rect_between(drag.start, drag.current);
        Some(match self.herramienta {
            Herramienta::Rect => Box::new(RectAnnotation { rect, style }),
            Herramienta::Elipse => Box::new(EllipseAnnotation { rect, style }),
            Herramienta::Linea => Box::new(LineAnnotation {
                from: drag.start,
                to: drag.current,
                style,
            }),
            Herramienta::Flecha => Box::new(ArrowAnnotation {
                from: drag.start,
                to: drag.current,
                style,
            }),
            // PENDIENTE(rendimiento): clona los puntos en cada repintado
            // del arrastre (O(n) por frame). Solo importa con trazos
            // larguísimos; arreglable pasando el builder a préstamos.
            Herramienta::Lapiz => Box::new(PenAnnotation {
                points: drag.points.clone(),
                style,
            }),
            Herramienta::Resaltador => Box::new(HighlightAnnotation {
                rect,
                color: Color::rgba(self.color.r, self.color.g, self.color.b, 128),
            }),
            Herramienta::Texto => return None,
        })
    }
}

fn cargar_contexto() -> (RenderContext, bool) {
    let normal = std::fs::read("C:/Windows/Fonts/segoeui.ttf");
    let bold = std::fs::read("C:/Windows/Fonts/segoeuib.ttf");
    match (normal, bold) {
        (Ok(n), Ok(b)) => match RenderContext::new(&n, &b) {
            Ok(ctx) => (ctx, true),
            Err(_) => (RenderContext::sin_fuente(), false),
        },
        _ => (RenderContext::sin_fuente(), false),
    }
}
