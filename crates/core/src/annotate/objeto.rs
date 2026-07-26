//! Objeto colocado en el documento: una `Forma` más su `Giro`.
//!
//! `Forma` es el enum que CIERRA la jerarquía del trait `Annotation` (D5).
//! Por qué un enum y no `Box<dyn Annotation>`: un `dyn` solo ofrece lo que
//! declara el trait, y el editor necesita tres cosas que el trait no puede
//! dar sin renunciar al objeto — saber dónde está cada objeto para el
//! hit-test (`bounds`), transformarlo (`translate`/`rotar`) y serializarlo
//! para el formato re-editable (f.31, que con `dyn` exigiría `typetag`, es
//! decir una dependencia, contra la prioridad de peso mínimo). Al ser una
//! app cerrada (los plugins están descartados en `ideas.md`), cerrar la
//! lista no cuesta nada: cada tipo sigue en su archivo con su
//! `impl Annotation` y aquí solo se delega con un `match`.
//!
//! El giro vive en `Objeto` y no dentro de cada forma porque es propiedad
//! de la COLOCACIÓN, no del tipo: así `rotar` es una línea en vez de nueve
//! y f.31 serializa un campo en vez de nueve.

use crate::annotate::annotations::{
    ArrowAnnotation, EllipseAnnotation, HighlightAnnotation, LineAnnotation, PenAnnotation,
    PixelateAnnotation, RectAnnotation, StepAnnotation, TextAnnotation,
};
use crate::annotate::canvas::Canvas;
use crate::annotate::giro::Giro;
use crate::annotate::text::RenderContext;
use crate::ports::Rect;

/// Forma de un objeto. Añadir un tipo = un archivo en `annotations/` + una
/// variante aquí; el compilador señala los `match` que faltan.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub enum Forma {
    Flecha(ArrowAnnotation),
    Elipse(EllipseAnnotation),
    Resaltador(HighlightAnnotation),
    Linea(LineAnnotation),
    Lapiz(PenAnnotation),
    Pixelado(PixelateAnnotation),
    Rect(RectAnnotation),
    Paso(StepAnnotation),
    Texto(TextAnnotation),
}

impl Forma {
    /// Delega en la Strategy del tipo concreto (D5 intacto), pasándole el
    /// giro para que lo honre según su familia de rasterizado.
    pub fn render(&self, canvas: &mut Canvas, ctx: &RenderContext, giro: Giro) {
        match self {
            Forma::Flecha(a) => a.render_girado(canvas, ctx, giro),
            Forma::Elipse(a) => a.render_girado(canvas, ctx, giro),
            Forma::Resaltador(a) => a.render_girado(canvas, ctx, giro),
            Forma::Linea(a) => a.render_girado(canvas, ctx, giro),
            Forma::Lapiz(a) => a.render_girado(canvas, ctx, giro),
            Forma::Pixelado(a) => a.render_girado(canvas, ctx, giro),
            Forma::Rect(a) => a.render_girado(canvas, ctx, giro),
            Forma::Paso(a) => a.render_girado(canvas, ctx, giro),
            Forma::Texto(a) => a.render_girado(canvas, ctx, giro),
        }
    }

    /// Caja que encierra lo que la forma pinta SIN girar. Cada tipo la
    /// expone en su propio archivo con `caja()`: es el mismo punto de
    /// verdad que usa su `render_girado` para el centro de giro — si
    /// divergieran, el objeto se desplazaría al rotarlo.
    ///
    /// Necesita el `ctx` porque medir el texto exige la fuente; sin fuente
    /// cargada el texto devuelve un rect vacío y no es seleccionable (la
    /// GUI siempre carga la fuente).
    pub fn bounds_sin_girar(&self, ctx: &RenderContext) -> Rect {
        match self {
            Forma::Flecha(a) => a.caja(),
            Forma::Elipse(a) => a.caja(),
            Forma::Resaltador(a) => a.caja(),
            Forma::Linea(a) => a.caja(),
            Forma::Lapiz(a) => a.caja(),
            Forma::Pixelado(a) => a.caja(),
            Forma::Rect(a) => a.caja(),
            Forma::Paso(a) => a.caja(),
            Forma::Texto(a) => a.caja(ctx),
        }
    }

    fn translate(&mut self, delta: (i32, i32)) {
        let mover = |p: &mut (i32, i32)| {
            p.0 = p.0.saturating_add(delta.0);
            p.1 = p.1.saturating_add(delta.1);
        };
        match self {
            Forma::Elipse(a) => a.rect = a.rect.translated(delta),
            Forma::Rect(a) => a.rect = a.rect.translated(delta),
            Forma::Resaltador(a) => a.rect = a.rect.translated(delta),
            Forma::Pixelado(a) => a.rect = a.rect.translated(delta),
            Forma::Linea(a) => {
                mover(&mut a.from);
                mover(&mut a.to);
            }
            Forma::Flecha(a) => {
                mover(&mut a.from);
                mover(&mut a.to);
            }
            Forma::Lapiz(a) => a.points.iter_mut().for_each(mover),
            Forma::Paso(a) => mover(&mut a.center),
            Forma::Texto(a) => mover(&mut a.pos),
        }
    }
}

/// Un objeto COLOCADO en el documento: una forma más su giro.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct Objeto {
    pub forma: Forma,
    pub giro: Giro,
}

impl Objeto {
    pub fn nuevo(forma: Forma) -> Self {
        Self {
            forma,
            giro: Giro::nulo(),
        }
    }

    pub fn render(&self, canvas: &mut Canvas, ctx: &RenderContext) {
        self.forma.render(canvas, ctx, self.giro);
    }

    /// Caja del objeto TAL Y COMO SE VE: la caja sin girar con sus cuatro
    /// esquinas rotadas. Con giro nulo es exactamente la de antes.
    pub fn bounds(&self, ctx: &RenderContext) -> Rect {
        let base = self.forma.bounds_sin_girar(ctx);
        if self.giro.es_nulo() || base.is_empty() {
            return base;
        }
        let centro = base.centro();
        let girada = base.corners().map(|c| self.giro.aplicar(c, centro));
        Rect::bounding(&girada, 0)
    }

    /// Desplaza el objeto `delta` píxeles. Es la operación que necesita
    /// `Command::Move`: aplicar y revertir es el mismo código con el
    /// delta negado, así que mover queda deshacible sin caso especial.
    pub fn translate(&mut self, delta: (i32, i32)) {
        self.forma.translate(delta);
    }

    /// Suma `delta_rad` al giro. El centro se recalcula de la caja sin
    /// girar, que no cambia al rotar, así que girar y desgirar es
    /// exactamente reversible (lo que necesita `Command::Rotate`).
    pub fn rotar(&mut self, delta_rad: f32) {
        self.giro = Giro::new(self.giro.rad() + delta_rad);
    }
}

/// Conversiones para que los llamadores construyan sin nombrar la variante:
/// `Command::add(RectAnnotation { .. }.into())`.
macro_rules! desde {
    ($($tipo:ty => $variante:ident),* $(,)?) => {
        $(impl From<$tipo> for Objeto {
            fn from(a: $tipo) -> Self {
                Objeto::nuevo(Forma::$variante(a))
            }
        })*
    };
}

desde! {
    ArrowAnnotation => Flecha,
    EllipseAnnotation => Elipse,
    HighlightAnnotation => Resaltador,
    LineAnnotation => Linea,
    PenAnnotation => Lapiz,
    PixelateAnnotation => Pixelado,
    RectAnnotation => Rect,
    StepAnnotation => Paso,
    TextAnnotation => Texto,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotate::style::{CensorMode, Color, FamiliaId, Style, TextStyle};

    const ESTILO: Style = Style {
        color: Color::rgb(255, 0, 0),
        thickness: 4,
    };

    fn ctx() -> RenderContext {
        let normal = std::fs::read("C:/Windows/Fonts/segoeui.ttf").expect("fuente del sistema");
        let bold = std::fs::read("C:/Windows/Fonts/segoeuib.ttf").expect("fuente del sistema");
        RenderContext::new(&normal, &bold).unwrap()
    }

    /// Un objeto de cada variante, para recorrerlas todas en los tests.
    fn todos() -> Vec<Objeto> {
        vec![
            ArrowAnnotation { from: (10, 10), to: (40, 30), style: ESTILO }.into(),
            EllipseAnnotation { rect: Rect::new(5, 6, 20, 10), style: ESTILO }.into(),
            HighlightAnnotation { rect: Rect::new(1, 2, 30, 8), color: Color::rgba(255, 255, 0, 128) }.into(),
            LineAnnotation { from: (3, 4), to: (50, 40), style: ESTILO }.into(),
            PenAnnotation { points: vec![(2, 2), (9, 20), (30, 5)], style: ESTILO }.into(),
            PixelateAnnotation { rect: Rect::new(7, 8, 12, 12), mode: CensorMode::Mosaic { block: 4 } }.into(),
            RectAnnotation { rect: Rect::new(0, 0, 15, 15), style: ESTILO }.into(),
            StepAnnotation { center: (25, 25), number: 7, color: Color::rgb(1, 2, 3), font_size: 20.0 }.into(),
            TextAnnotation {
                pos: (12, 14),
                text: "Hola".to_string(),
                style: TextStyle { color: Color::rgb(9, 9, 9), size: 18.0, bold: false, familia: FamiliaId::default() },
            }
            .into(),
        ]
    }

    #[test]
    fn todas_las_variantes_tienen_caja_no_vacia() {
        let ctx = ctx();
        for (i, o) in todos().iter().enumerate() {
            assert!(!o.bounds(&ctx).is_empty(), "variante {i} sin caja");
        }
    }

    #[test]
    fn la_caja_contiene_los_extremos_de_la_geometria() {
        let ctx = ctx();
        // Línea de (3,4) a (50,40) con grosor 4 → margen 2.
        let linea: Objeto = LineAnnotation { from: (3, 4), to: (50, 40), style: ESTILO }.into();
        let caja = linea.bounds(&ctx);
        assert!(caja.contains_point((3, 4)) && caja.contains_point((50, 40)));
        assert!(caja.contains_point((1, 2)), "el margen del grosor no está");
        assert!(!caja.contains_point((0, 4)));
        // El paso encierra su disco completo.
        let paso = StepAnnotation { center: (25, 25), number: 7, color: Color::rgb(1, 2, 3), font_size: 20.0 };
        let radio = paso.radius() as i32;
        let caja = Objeto::from(paso).bounds(&ctx);
        assert!(caja.contains_point((25 - radio, 25)) && caja.contains_point((25, 25 + radio)));
    }

    /// La caja tiene que cubrir TODO lo pintado: se comprueba contra el
    /// render real, no contra la fórmula. Es lo que impide que la caja se
    /// quede corta con la cabeza de la flecha o el disco del paso.
    #[test]
    fn la_caja_cubre_todos_los_pixeles_que_pinta_el_objeto() {
        use crate::ports::Frame;
        let ctx = ctx();
        let casos: Vec<Objeto> = vec![
            // Flecha en varias direcciones: la cabeza sobresale del eje.
            ArrowAnnotation { from: (60, 60), to: (110, 60), style: ESTILO }.into(),
            ArrowAnnotation { from: (60, 60), to: (60, 110), style: ESTILO }.into(),
            ArrowAnnotation { from: (110, 110), to: (60, 60), style: ESTILO }.into(),
            LineAnnotation { from: (30, 40), to: (120, 90), style: ESTILO }.into(),
            PenAnnotation { points: vec![(40, 40), (90, 70), (60, 110)], style: ESTILO }.into(),
            StepAnnotation { center: (90, 90), number: 12, color: Color::rgb(255, 0, 0), font_size: 24.0 }.into(),
            EllipseAnnotation { rect: Rect::new(30, 30, 60, 40), style: ESTILO }.into(),
            RectAnnotation { rect: Rect::new(30, 30, 60, 40), style: ESTILO }.into(),
            TextAnnotation {
                pos: (40, 40),
                text: "Ag".to_string(),
                style: TextStyle { color: Color::rgb(255, 0, 0), size: 28.0, bold: true, familia: FamiliaId::default() },
            }
            .into(),
        ];
        for (i, o) in casos.iter().enumerate() {
            let mut frame = Frame::filled(200, 200, [0, 0, 0, 255]);
            o.render(&mut Canvas::new(&mut frame), &ctx);
            let caja = o.bounds(&ctx);
            for y in 0..200u32 {
                for x in 0..200u32 {
                    let pintado = frame.pixel(x, y).is_some_and(|[r, g, b, _]| {
                        r > 0 || g > 0 || b > 0
                    });
                    if pintado {
                        assert!(
                            caja.contains_point((x as i32, y as i32)),
                            "caso {i}: pinta en ({x}, {y}) fuera de la caja {caja:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn sin_giro_la_caja_es_la_de_la_forma_sin_girar() {
        let ctx = ctx();
        for o in todos() {
            assert_eq!(o.bounds(&ctx), o.forma.bounds_sin_girar(&ctx));
        }
    }

    #[test]
    fn un_cuarto_de_vuelta_intercambia_ancho_y_alto() {
        let ctx = ctx();
        let mut o: Objeto = RectAnnotation {
            rect: Rect::new(10, 10, 40, 10),
            style: ESTILO,
        }
        .into();
        let antes = o.bounds(&ctx);
        o.rotar(std::f32::consts::FRAC_PI_2);
        let despues = o.bounds(&ctx);
        // ±1 px por el redondeo de las esquinas rotadas.
        assert!((despues.width as i32 - antes.height as i32).abs() <= 1);
        assert!((despues.height as i32 - antes.width as i32).abs() <= 1);
        // El centro se conserva: girar no desplaza.
        let (ca, cd) = (antes.centro(), despues.centro());
        assert!((ca.0 - cd.0).abs() <= 1.0 && (ca.1 - cd.1).abs() <= 1.0);
    }

    #[test]
    fn girar_y_desgirar_devuelve_la_caja_original() {
        let ctx = ctx();
        for mut o in todos() {
            let original = o.bounds(&ctx);
            o.rotar(0.9);
            o.rotar(-0.9);
            assert_eq!(o.bounds(&ctx), original);
        }
    }

    /// El objeto girado no debe desplazarse respecto a su recuadro: lo que
    /// pinta tiene que seguir cayendo dentro de la caja que devuelve
    /// `bounds`. Es lo que se rompe si el centro de giro del rasterizado y
    /// el de la caja divergen.
    #[test]
    fn lo_que_pinta_un_objeto_girado_cae_dentro_de_su_caja() {
        use crate::ports::Frame;
        let ctx = ctx();
        let casos: Vec<Objeto> = vec![
            LineAnnotation { from: (60, 90), to: (140, 110), style: ESTILO }.into(),
            ArrowAnnotation { from: (60, 90), to: (140, 110), style: ESTILO }.into(),
            PenAnnotation { points: vec![(70, 70), (120, 100), (90, 130)], style: ESTILO }.into(),
            RectAnnotation { rect: Rect::new(60, 70, 70, 50), style: ESTILO }.into(),
            EllipseAnnotation { rect: Rect::new(60, 70, 70, 50), style: ESTILO }.into(),
            HighlightAnnotation {
                rect: Rect::new(60, 70, 70, 50),
                color: Color::rgba(255, 255, 0, 128),
            }
            .into(),
        ];
        for (i, base) in casos.into_iter().enumerate() {
            for &rad in &[0.4, 0.9, std::f32::consts::FRAC_PI_4, 2.1] {
                let mut o = base.clone();
                o.rotar(rad);
                let mut frame = Frame::filled(200, 200, [0, 0, 0, 255]);
                o.render(&mut Canvas::new(&mut frame), &ctx);
                let caja = o.bounds(&ctx);
                // Margen de 1 px: el estampado de discos redondea.
                let holgada = Rect::new(
                    caja.x - 1,
                    caja.y - 1,
                    caja.width + 2,
                    caja.height + 2,
                );
                for y in 0..200u32 {
                    for x in 0..200u32 {
                        let pintado = frame
                            .pixel(x, y)
                            .is_some_and(|[r, g, b, _]| r > 0 || g > 0 || b > 0);
                        if pintado {
                            assert!(
                                holgada.contains_point((x as i32, y as i32)),
                                "caso {i} a {rad} rad: pinta en ({x}, {y}) fuera de {caja:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    /// Round-trip TOML de una variante de CADA forma: si alguna no
    /// serializa, el documento entero se pierde al guardar (f.31).
    #[test]
    fn todas_las_formas_sobreviven_al_toml() {
        let ctx = ctx();
        for (i, original) in todos().into_iter().enumerate() {
            let texto = toml::to_string(&original)
                .unwrap_or_else(|e| panic!("variante {i} no serializa: {e}"));
            let vuelta: Objeto = toml::from_str(&texto)
                .unwrap_or_else(|e| panic!("variante {i} no deserializa: {e}\n{texto}"));
            assert_eq!(
                vuelta.bounds(&ctx),
                original.bounds(&ctx),
                "variante {i} cambió al ir y volver"
            );
        }
    }

    #[test]
    fn el_giro_sobrevive_como_angulo_y_recupera_su_cache() {
        let ctx = ctx();
        let mut o: Objeto = RectAnnotation {
            rect: Rect::new(1, 2, 20, 8),
            style: ESTILO,
        }
        .into();
        o.rotar(0.7);
        let vuelta: Objeto = toml::from_str(&toml::to_string(&o).unwrap()).unwrap();
        assert!((vuelta.giro.rad() - 0.7).abs() < 1e-6);
        // Seno y coseno se reconstruyen: la caja girada sale idéntica.
        assert_eq!(vuelta.bounds(&ctx), o.bounds(&ctx));
    }

    #[test]
    fn el_texto_conserva_contenido_estilo_y_familia() {
        let ctx = ctx();
        let o: Objeto = TextAnnotation {
            pos: (7, 9),
            text: "Hola\nmundo".to_string(),
            style: TextStyle {
                color: Color::rgb(1, 2, 3),
                size: 23.0,
                bold: true,
                familia: crate::annotate::style::FamiliaId(5),
            },
        }
        .into();
        let vuelta: Objeto = toml::from_str(&toml::to_string(&o).unwrap()).unwrap();
        let Forma::Texto(t) = &vuelta.forma else {
            panic!("cambió de variante");
        };
        assert_eq!(t.text, "Hola\nmundo");
        assert_eq!(t.pos, (7, 9));
        assert_eq!(t.style.size, 23.0);
        assert!(t.style.bold);
        assert_eq!(t.style.familia.0, 5);
        _ = ctx;
    }

    #[test]
    fn translate_mueve_la_caja_y_conserva_su_tamano() {
        let ctx = ctx();
        for (i, mut o) in todos().into_iter().enumerate() {
            let antes = o.bounds(&ctx);
            o.translate((7, -3));
            let despues = o.bounds(&ctx);
            assert_eq!(
                (despues.x, despues.y),
                (antes.x + 7, antes.y - 3),
                "variante {i}: la caja no se movió igual"
            );
            assert_eq!(
                (despues.width, despues.height),
                (antes.width, antes.height),
                "variante {i}: cambió de tamaño al moverse"
            );
        }
    }

    #[test]
    fn translate_es_reversible_con_el_delta_negado() {
        let ctx = ctx();
        for mut o in todos() {
            let original = o.bounds(&ctx);
            o.translate((25, 40));
            o.translate((-25, -40));
            assert_eq!(o.bounds(&ctx), original);
        }
    }

    #[test]
    fn mover_un_objeto_mueve_lo_que_pinta() {
        use crate::ports::Frame;
        // Un rectángulo pintado y el mismo rectángulo desplazado deben dar
        // exactamente el mismo frame que dibujarlo ya desplazado.
        let ctx = RenderContext::sin_fuente();
        let pintar = |o: &Objeto| {
            let mut f = Frame::filled(60, 60, [0, 0, 0, 255]);
            o.render(&mut Canvas::new(&mut f), &ctx);
            f
        };
        let mut movido: Objeto = RectAnnotation { rect: Rect::new(5, 5, 10, 10), style: ESTILO }.into();
        movido.translate((20, 12));
        let directo: Objeto = RectAnnotation { rect: Rect::new(25, 17, 10, 10), style: ESTILO }.into();
        assert_eq!(pintar(&movido), pintar(&directo));
    }

    #[test]
    fn el_texto_sin_fuente_no_tiene_caja_y_no_es_seleccionable() {
        let o: Objeto = TextAnnotation {
            pos: (12, 14),
            text: "Hola".to_string(),
            style: TextStyle { color: Color::rgb(9, 9, 9), size: 18.0, bold: false, familia: FamiliaId::default() },
        }
        .into();
        assert!(o.bounds(&RenderContext::sin_fuente()).is_empty());
    }
}
