//! Estado del editor V4 con la anotación in situ (fusión de la antigua
//! ventana de dibujo): frame base + documento de objetos (D5) + Command
//! con undo/redo (D6), buffers de render persistentes y propiedades de
//! la herramienta activa.

use rustcapture_core::annotate::annotations::{
    ArrowAnnotation, EllipseAnnotation, HighlightAnnotation, LineAnnotation, PenAnnotation,
    PixelateAnnotation, RectAnnotation,
};
use rustcapture_core::annotate::{
    CensorMode, Color, Document, FamiliaId, History, Objeto, RenderContext, Style,
};
use rustcapture_core::ports::Frame;
use windows::Win32::Foundation::{COLORREF, RECT};
use windows::Win32::Graphics::Gdi::{CreateSolidBrush, DeleteObject, HBRUSH};

use crate::gdi::raii::Dib;
use crate::gdi::{copy_frame_to_dib, dib_from_frame};
use crate::gdi::raii::{MemDc, ScreenDc};
use crate::ui::tooltip::Tooltips;

use super::math::Herramienta;
use super::props::Accion;
use super::texto::EditBox;

pub(super) const GROSORES: [u32; 5] = [1, 2, 3, 5, 8];
pub(super) const TAMANOS: [f32; 5] = [12.0, 16.0, 20.0, 28.0, 36.0];
/// Bloque del mosaico / radio del desenfoque ofrecidos en la barra (f.25).
pub(super) const CENSURAS: [u32; 5] = [4, 8, 12, 16, 24];
/// Color de anotación por defecto: el acento cálido del diseño (#D83B01).
pub(super) const COLOR_DEFECTO: Color = Color::rgb(0xD8, 0x3B, 0x01);

/// Propiedades de dibujo que edita la barra contextual (f.22-f.25).
/// Agrupadas para que `props::chips` no crezca en parámetros sueltos.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct Propiedades {
    pub color: Color,
    pub grosor: u32,
    pub tamano_texto: f32,
    pub negrita: bool,
    /// Familia tipográfica elegida (f.54).
    pub familia: FamiliaId,
    /// Modo de censura vigente, con su parámetro en px dentro.
    pub censura: CensorMode,
}

impl Default for Propiedades {
    fn default() -> Self {
        Self {
            color: COLOR_DEFECTO,
            grosor: 3,
            tamano_texto: 20.0,
            negrita: false,
            familia: FamiliaId::default(),
            censura: CensorMode::Mosaic { block: 8 },
        }
    }
}

impl Propiedades {
    /// px del modo vigente: bloque del mosaico o radio del desenfoque.
    pub(super) fn censura_px(&self) -> u32 {
        match self.censura {
            CensorMode::Mosaic { block } => block,
            CensorMode::Blur { radius } => radius,
        }
    }

    pub(super) fn con_censura_px(&mut self, px: u32) {
        self.censura = match self.censura {
            CensorMode::Mosaic { .. } => CensorMode::Mosaic { block: px },
            CensorMode::Blur { .. } => CensorMode::Blur { radius: px },
        };
    }

    /// Conmuta mosaico ↔ desenfoque conservando los px elegidos.
    pub(super) fn alternar_censura(&mut self) {
        let px = self.censura_px();
        self.censura = match self.censura {
            CensorMode::Mosaic { .. } => CensorMode::Blur { radius: px },
            CensorMode::Blur { .. } => CensorMode::Mosaic { block: px },
        };
    }
}

impl Drop for EditorState {
    fn drop(&mut self) {
        // SAFETY: la brocha la creó `brocha_caja` y nadie más la posee.
        if let Some((_, brocha)) = self.brocha_caja.take() {
            unsafe { _ = DeleteObject(brocha.into()) };
        }
    }
}

/// Numeración de los pasos (f.23) en paralelo a las pilas de `History`:
/// cada comando aplicado apunta el número vigente ANTES de él, así
/// deshacer un paso devuelve su número al siguiente que se coloque.
/// Sus tres métodos se llaman EXACTAMENTE donde se llama a `History`.
pub(super) struct ContadorPasos {
    siguiente: u32,
    antes: Vec<u32>,
    despues: Vec<u32>,
}

impl ContadorPasos {
    pub(super) fn new() -> Self {
        Self {
            siguiente: 1,
            antes: Vec::new(),
            despues: Vec::new(),
        }
    }

    pub(super) fn siguiente(&self) -> u32 {
        self.siguiente
    }

    /// Un comando se aplicó con éxito; `fue_paso` gasta un número.
    pub(super) fn aplicado(&mut self, fue_paso: bool) {
        self.antes.push(self.siguiente);
        if fue_paso {
            self.siguiente += 1;
        }
        self.despues.clear();
    }

    pub(super) fn deshecho(&mut self) {
        if let Some(previo) = self.antes.pop() {
            self.despues.push(self.siguiente);
            self.siguiente = previo;
        }
    }

    pub(super) fn rehecho(&mut self) {
        if let Some(posterior) = self.despues.pop() {
            self.antes.push(self.siguiente);
            self.siguiente = posterior;
        }
    }
}

pub(super) struct DragState {
    pub start: (i32, i32),
    pub current: (i32, i32),
    pub points: Vec<(i32, i32)>,
}

/// Arrastre de un objeto ya colocado (herramienta Selección). El objeto
/// NO se toca mientras dura: el desplazamiento se pinta como preview y
/// solo se convierte en `Command::Move` al soltar (D6).
pub(super) struct MoverDrag {
    pub index: usize,
    pub start: (i32, i32),
    pub current: (i32, i32),
}

impl MoverDrag {
    pub(super) fn delta(&self) -> (i32, i32) {
        (self.current.0 - self.start.0, self.current.1 - self.start.1)
    }
}

/// Arrastre del asa de rotación. Como `MoverDrag`, no toca el documento: el
/// giro se pinta como preview y se convierte en `Command::Rotate` al soltar.
pub(super) struct GirarDrag {
    pub index: usize,
    /// Centro de giro en píxeles del frame (el de la caja sin girar).
    pub centro: (i32, i32),
    /// Ángulo del puntero al empezar y ahora; el delta es la diferencia.
    pub inicial: f32,
    pub actual: f32,
    /// Snap a 15° activo (Shift pulsado al soltar).
    pub snap: bool,
}

impl GirarDrag {
    pub(super) fn delta(&self) -> f32 {
        super::math::ajustar_angulo(self.actual - self.inicial, self.snap)
    }
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
    /// Familias del catálogo (f.54), ordenadas, para el menú del chip. Están
    /// todas REGISTRADAS en `ctx` pero solo la vigente tiene caras cargadas:
    /// parsear 228 TTF al abrir el editor sería absurdo.
    pub familias: Vec<(FamiliaId, String)>,
    pub herramienta: Herramienta,
    pub props: Propiedades,
    /// Numeración de pasos, sincronizada con `history`.
    pub pasos: ContadorPasos,
    pub drag: Option<DragState>,
    /// Índice del objeto seleccionado. Se limpia en cada undo/redo: esas
    /// operaciones reordenan los índices (deshacer un borrado reinserta y
    /// desplaza a los de detrás) y un índice rancio señalaría a otro
    /// objeto. Limpiar es predecible y cuesta nada.
    pub seleccionado: Option<usize>,
    pub mover: Option<MoverDrag>,
    pub girar: Option<GirarDrag>,
    pub edit: Option<EditBox>,
    pub cerrado: bool,
    /// Documento cambiado desde el último guardado/copiado.
    pub dirty: bool,
    /// Nombre del archivo tras Guardar como (título y status bar).
    pub nombre: Option<String>,
    pub tooltips: Option<Tooltips>,
    /// Brocha de fondo de la caja de texto, cacheada: `WM_CTLCOLOREDIT` se
    /// dispara en cada repintado del control y crear una brocha por mensaje
    /// las filtraría.
    brocha_caja: Option<(COLORREF, HBRUSH)>,
    /// Zonas clicables de la property bar (las rellena el pintado).
    pub chips: Vec<(RECT, Accion)>,
}

impl EditorState {
    /// `preferida` es la familia por defecto (`[text].familia` de la config);
    /// si no existe en el sistema se cae a Segoe UI y, en último término, a la
    /// primera del catálogo.
    pub(super) fn con_fuente(base: Frame, preferida: &str) -> windows::core::Result<Self> {
        let (ctx, familias, tiene_fuente) = cargar_contexto(preferida);
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
            familias,
            herramienta: Herramienta::Flecha,
            props: Propiedades::default(),
            pasos: ContadorPasos::new(),
            drag: None,
            seleccionado: None,
            mover: None,
            girar: None,
            edit: None,
            cerrado: false,
            dirty: false,
            nombre: None,
            tooltips: None,
            brocha_caja: None,
            chips: Vec::new(),
        })
    }

    /// Brocha del fondo de la caja de texto para `WM_CTLCOLOREDIT`. Se
    /// reutiliza mientras el color no cambie (el tema puede cambiar en vivo).
    pub(super) fn brocha_caja(&mut self, color: COLORREF) -> HBRUSH {
        if let Some((c, b)) = self.brocha_caja
            && c == color
        {
            return b;
        }
        // SAFETY: se libera la anterior antes de sustituirla; la última vive
        // hasta el Drop del estado.
        unsafe {
            if let Some((_, vieja)) = self.brocha_caja.take() {
                _ = DeleteObject(vieja.into());
            }
            let nueva = CreateSolidBrush(color);
            self.brocha_caja = Some((color, nueva));
            nueva
        }
    }

    /// Regenera el frame comprometido (base + documento) sobre los
    /// buffers existentes — sin asignar (las dimensiones son fijas).
    pub(super) fn refresh_committed(&mut self) {
        self.committed.pixels.copy_from_slice(&self.base.pixels);
        self.doc.render_onto(&mut self.committed, &self.ctx);
        copy_frame_to_dib(&self.committed, &mut self.committed_dib);
    }

    /// Objeto provisional del arrastre actual (None con las herramientas
    /// de un clic o sin arrastre).
    pub(super) fn anotacion_en_curso(&self) -> Option<Objeto> {
        let drag = self.drag.as_ref()?;
        let style = Style {
            color: self.props.color,
            thickness: self.props.grosor,
        };
        let rect = crate::overlay::math::rect_between(drag.start, drag.current);
        Some(match self.herramienta {
            Herramienta::Rect => RectAnnotation { rect, style }.into(),
            Herramienta::Elipse => EllipseAnnotation { rect, style }.into(),
            Herramienta::Linea => LineAnnotation {
                from: drag.start,
                to: drag.current,
                style,
            }
            .into(),
            Herramienta::Flecha => ArrowAnnotation {
                from: drag.start,
                to: drag.current,
                style,
            }
            .into(),
            // PENDIENTE(rendimiento): clona los puntos en cada repintado
            // del arrastre (O(n) por frame). Solo importa con trazos
            // larguísimos; arreglable pasando el builder a préstamos.
            Herramienta::Lapiz => PenAnnotation {
                points: drag.points.clone(),
                style,
            }
            .into(),
            Herramienta::Resaltador => HighlightAnnotation {
                rect,
                color: Color::rgba(
                    self.props.color.r,
                    self.props.color.g,
                    self.props.color.b,
                    128,
                ),
            }
            .into(),
            Herramienta::Pixelado => PixelateAnnotation {
                rect,
                mode: self.props.censura,
            }
            .into(),
            // Texto y Pasos se colocan con un clic, no con arrastre; la
            // selección y la goma no crean objetos, operan sobre los que ya
            // hay (su arrastre lo lleva `mover`, no este).
            Herramienta::Texto
            | Herramienta::Pasos
            | Herramienta::Seleccion
            | Herramienta::Goma => return None,
        })
    }
}

/// Construye el `RenderContext` con el catálogo del sistema (f.54):
/// REGISTRA todas las familias (barato, solo nombres) y carga las CARAS
/// únicamente de la preferida. El resto se cargan al elegirlas, para no
/// parsear cientos de TTF al abrir el editor.
///
/// La preferida se registra PRIMERA a propósito: es la `FamiliaId(0)`, la de
/// respaldo a la que cae `RenderContext::font` cuando algo falta.
fn cargar_contexto(preferida: &str) -> (RenderContext, Vec<(FamiliaId, String)>, bool) {
    let mut ctx = RenderContext::nueva();
    let catalogo = crate::fuentes_ttf::catalogo();
    let elegida = catalogo
        .iter()
        .find(|f| f.nombre == preferida)
        .or_else(|| catalogo.iter().find(|f| f.nombre == "Segoe UI"))
        .or_else(|| catalogo.first());
    let mut tiene_fuente = false;
    if let Some(f) = elegida {
        let id = ctx.registrar_familia(&f.nombre);
        tiene_fuente = cargar_familia(&mut ctx, id, f);
    }
    let familias = catalogo
        .iter()
        .map(|f| (ctx.registrar_familia(&f.nombre), f.nombre.clone()))
        .collect();
    (ctx, familias, tiene_fuente)
}

/// Lee del disco las caras de una familia y las mete en el catálogo.
/// Devuelve true si al menos la normal cargó.
pub(super) fn cargar_familia(
    ctx: &mut RenderContext,
    id: FamiliaId,
    familia: &crate::fuentes_ttf::Familia,
) -> bool {
    let normal = std::fs::read(&familia.normal)
        .ok()
        .is_some_and(|b| ctx.cargar_cara(id, false, &b).is_ok());
    if let Some(ruta) = &familia.bold
        && let Ok(b) = std::fs::read(ruta)
    {
        // Si la negrita falla, la cadena de respaldo cae a la normal.
        _ = ctx.cargar_cara(id, true, &b);
    }
    normal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn las_propiedades_por_defecto_son_las_del_diseno() {
        let p = Propiedades::default();
        assert_eq!(p.color, COLOR_DEFECTO);
        assert_eq!(p.grosor, 3);
        assert_eq!(p.tamano_texto, 20.0);
        assert!(!p.negrita);
        assert_eq!(p.censura, CensorMode::Mosaic { block: 8 });
        assert_eq!(p.censura_px(), 8);
    }

    #[test]
    fn alternar_la_censura_conserva_los_px() {
        let mut p = Propiedades::default();
        p.con_censura_px(16);
        p.alternar_censura();
        assert_eq!(p.censura, CensorMode::Blur { radius: 16 });
        assert_eq!(p.censura_px(), 16);
        p.alternar_censura();
        assert_eq!(p.censura, CensorMode::Mosaic { block: 16 });
    }

    #[test]
    fn los_pasos_empiezan_en_uno_y_avanzan_al_colocarse() {
        let mut c = ContadorPasos::new();
        assert_eq!(c.siguiente(), 1);
        c.aplicado(true);
        assert_eq!(c.siguiente(), 2);
        c.aplicado(true);
        assert_eq!(c.siguiente(), 3);
    }

    #[test]
    fn otras_herramientas_no_consumen_numero() {
        let mut c = ContadorPasos::new();
        c.aplicado(false); // una flecha
        assert_eq!(c.siguiente(), 1);
    }

    #[test]
    fn deshacer_un_paso_devuelve_su_numero_y_rehacer_lo_vuelve_a_gastar() {
        let mut c = ContadorPasos::new();
        c.aplicado(true);
        assert_eq!(c.siguiente(), 2);
        c.deshecho();
        assert_eq!(c.siguiente(), 1);
        c.rehecho();
        assert_eq!(c.siguiente(), 2);
    }

    #[test]
    fn deshacer_comandos_intercalados_mantiene_la_numeracion() {
        let mut c = ContadorPasos::new();
        c.aplicado(false); // flecha
        c.aplicado(true); // paso 1
        c.aplicado(false); // rectángulo
        assert_eq!(c.siguiente(), 2);
        c.deshecho(); // deshace el rectángulo
        assert_eq!(c.siguiente(), 2);
        c.deshecho(); // deshace el paso 1
        assert_eq!(c.siguiente(), 1);
        c.deshecho(); // deshace la flecha
        assert_eq!(c.siguiente(), 1);
    }

    #[test]
    fn un_comando_nuevo_invalida_el_rehacer_del_contador() {
        let mut c = ContadorPasos::new();
        c.aplicado(true);
        c.deshecho();
        assert_eq!(c.siguiente(), 1);
        c.aplicado(true); // se coloca otro paso: reusa el 1
        assert_eq!(c.siguiente(), 2);
        c.rehecho(); // ya no hay nada que rehacer
        assert_eq!(c.siguiente(), 2);
    }

    #[test]
    fn deshacer_sin_historia_no_rompe_el_contador() {
        let mut c = ContadorPasos::new();
        c.deshecho();
        c.rehecho();
        assert_eq!(c.siguiente(), 1);
    }
}
