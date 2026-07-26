//! Contexto de render: CATÁLOGO de caras tipográficas (f.54). Las fuentes
//! llegan inyectadas como bytes — el core nunca abre archivos ni consulta el
//! registro (D1/D2); eso es trabajo de `platform-win::fuentes_ttf`. Sin
//! ninguna cara cargada, el texto es no-op documentado.

use std::collections::HashMap;

use crate::annotate::canvas::Canvas;
use crate::annotate::giro::Giro;
use crate::annotate::style::{FamiliaId, TextStyle};
use crate::ports::Rect;

pub struct RenderContext {
    /// Nombres por id: el índice ES el `FamiliaId`.
    nombres: Vec<String>,
    /// Caras cargadas por (familia, negrita).
    caras: HashMap<(u16, bool), fontdue::Font>,
}

impl RenderContext {
    pub fn nueva() -> Self {
        Self {
            nombres: Vec::new(),
            caras: HashMap::new(),
        }
    }

    /// Atajo del caso de siempre: una sola familia con sus dos caras, que
    /// queda registrada como la de respaldo.
    pub fn new(font: &[u8], bold: &[u8]) -> Result<Self, String> {
        let mut ctx = Self::nueva();
        let id = ctx.registrar_familia("(por defecto)");
        ctx.cargar_cara(id, false, font)?;
        ctx.cargar_cara(id, true, bold)?;
        Ok(ctx)
    }

    /// Contexto sin tipografía: todo salvo el texto funciona igual.
    pub fn sin_fuente() -> Self {
        Self::nueva()
    }

    /// Registra una familia (o devuelve su id si ya estaba). La PRIMERA que
    /// se registra es la de respaldo: a ella caen las que no tengan caras.
    pub fn registrar_familia(&mut self, nombre: &str) -> FamiliaId {
        if let Some(i) = self.nombres.iter().position(|n| n == nombre) {
            return FamiliaId(i as u16);
        }
        self.nombres.push(nombre.to_string());
        FamiliaId((self.nombres.len() - 1) as u16)
    }

    /// Carga los bytes de una cara. Error si el TTF no se puede parsear; el
    /// catálogo queda intacto en ese caso.
    pub fn cargar_cara(&mut self, id: FamiliaId, bold: bool, ttf: &[u8]) -> Result<(), String> {
        let font = fontdue::Font::from_bytes(ttf, fontdue::FontSettings::default())
            .map_err(String::from)?;
        self.caras.insert((id.0, bold), font);
        Ok(())
    }

    /// `true` si esa familia ya tiene alguna cara cargada (para no releer
    /// del disco al volver a elegirla).
    pub fn tiene_familia(&self, id: FamiliaId) -> bool {
        self.caras.contains_key(&(id.0, false)) || self.caras.contains_key(&(id.0, true))
    }

    pub fn nombre(&self, id: FamiliaId) -> Option<&str> {
        self.nombres.get(id.0 as usize).map(String::as_str)
    }

    pub fn familias(&self) -> Vec<(FamiliaId, &str)> {
        self.nombres
            .iter()
            .enumerate()
            .map(|(i, n)| (FamiliaId(i as u16), n.as_str()))
            .collect()
    }

    pub fn tiene_alguna(&self) -> bool {
        !self.caras.is_empty()
    }

    /// Cara para un estilo, con cadena de respaldo: la pedida → la misma
    /// familia sin negrita → la familia de respaldo con negrita → la de
    /// respaldo normal. Así una fuente ausente o corrupta degrada en vez de
    /// dejar el texto sin pintar.
    pub(crate) fn font(&self, style: TextStyle) -> Option<&fontdue::Font> {
        let f = style.familia.0;
        self.caras
            .get(&(f, style.bold))
            .or_else(|| self.caras.get(&(f, false)))
            .or_else(|| self.caras.get(&(0, style.bold)))
            .or_else(|| self.caras.get(&(0, false)))
    }
}

/// Recorre la cobertura de todos los glifos del texto llamando a `emitir`
/// con `(x, y, cobertura)` en coordenadas RELATIVAS a `pos`.
///
/// Es el único sitio que sabe COLOCAR glifos. Lo consumen `draw_text`, la
/// medida `text_ink_box` y el rasterizado a buffer del texto girado: así no
/// pueden divergir en el redondeo, que es lo que descuadraría el centrado
/// del número de un paso o la caja de selección de un texto.
fn recorrer_glifos(
    text: &str,
    style: TextStyle,
    ctx: &RenderContext,
    mut emitir: impl FnMut(i32, i32, u8),
) {
    let Some(font) = ctx.font(style) else {
        return;
    };
    let line_height = (style.size * 1.2).round() as i32;
    for (n, linea) in text.split('\n').enumerate() {
        let base_y = n as i32 * line_height;
        let mut cursor_x = 0.0f32;
        for c in linea.chars() {
            let (metrics, bitmap) = font.rasterize(c, style.size);
            let gx = cursor_x.round() as i32 + metrics.xmin;
            // ymin es respecto a la línea base; colocamos la base a
            // `size` píxeles del tope de la línea.
            let gy = base_y + style.size.round() as i32 - metrics.height as i32 - metrics.ymin;
            for (i, cobertura) in bitmap.iter().enumerate() {
                if *cobertura == 0 {
                    continue;
                }
                emitir(
                    gx + (i % metrics.width) as i32,
                    gy + (i / metrics.width) as i32,
                    *cobertura,
                );
            }
            cursor_x += metrics.advance_width;
        }
    }
}

/// Dibuja texto multilínea; la cobertura del glifo modula el alfa del
/// color. Sin fuente cargada, no hace nada (la GUI siempre la carga).
pub(crate) fn draw_text(
    canvas: &mut Canvas,
    pos: (i32, i32),
    text: &str,
    style: TextStyle,
    ctx: &RenderContext,
) {
    recorrer_glifos(text, style, ctx, |x, y, cobertura| {
        let alfa = (u16::from(style.color.a) * u16::from(cobertura) / 255) as u8;
        canvas.blend_pixel(
            pos.0 + x,
            pos.1 + y,
            crate::annotate::style::Color::rgba(style.color.r, style.color.g, style.color.b, alfa),
        );
    });
}

/// Caja de la TINTA del texto, relativa al `pos` que recibe `draw_text`:
/// `(dx, dy, ancho, alto)`. `None` sin fuente cargada o si nada pinta
/// (p. ej. solo espacios). Replica la colocación de `draw_text` glifo a
/// glifo — si una cambia, la otra tiene que cambiar igual.
///
/// Existe para centrar ópticamente: la caja de línea incluye hueco de
/// ascendente/descendente que las cifras no usan, y centrar por ella
/// deja el número visiblemente bajo dentro del disco (f.23).
pub(crate) fn text_ink_box(
    text: &str,
    style: TextStyle,
    ctx: &RenderContext,
) -> Option<(i32, i32, u32, u32)> {
    let (mut min_x, mut min_y) = (i32::MAX, i32::MAX);
    let (mut max_x, mut max_y) = (i32::MIN, i32::MIN);
    recorrer_glifos(text, style, ctx, |x, y, _| {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    });
    (min_x <= max_x && min_y <= max_y).then(|| {
        (
            min_x,
            min_y,
            (max_x - min_x + 1) as u32,
            (max_y - min_y + 1) as u32,
        )
    })
}

/// Texto girado por mapeo inverso: los glifos se rasterizan una vez a un
/// buffer de cobertura SIN girar (fontdue no sabe rotar) y ese buffer se
/// muestrea con bilineal desde el destino. Rotar hacia delante dejaría
/// huecos entre píxeles; esta es la única forma con glifos ya rasterizados.
///
/// Es la única familia que pierde nitidez al girar, y es inevitable.
pub(crate) fn draw_text_rotado(
    canvas: &mut Canvas,
    pos: (i32, i32),
    text: &str,
    style: TextStyle,
    ctx: &RenderContext,
    giro: Giro,
    centro: (f32, f32),
) {
    if giro.es_nulo() {
        return draw_text(canvas, pos, text, style, ctx);
    }
    let Some((dx, dy, w, h)) = text_ink_box(text, style, ctx) else {
        return;
    };
    // Cobertura del texto en su propio espacio, con la tinta pegada al (0,0)
    // del buffer.
    let (bw, bh) = (w as usize, h as usize);
    let mut cobertura = vec![0u8; bw * bh];
    recorrer_glifos(text, style, ctx, |x, y, c| {
        let (bx, by) = (x - dx, y - dy);
        if bx >= 0 && by >= 0 && (bx as usize) < bw && (by as usize) < bh {
            let i = by as usize * bw + bx as usize;
            cobertura[i] = cobertura[i].max(c);
        }
    });

    let origen = (pos.0 + dx, pos.1 + dy);
    let caja_obj = Rect::new(origen.0, origen.1, w, h);
    let caja = Rect::bounding(&caja_obj.corners().map(|c| giro.aplicar(c, centro)), 1);
    for y in caja.y..caja.bottom() as i32 {
        for x in caja.x..caja.right() as i32 {
            let (ox, oy) = giro.deshacer((x as f32, y as f32), centro);
            let a = muestrear_bilineal(
                &cobertura,
                (bw, bh),
                ox - origen.0 as f32,
                oy - origen.1 as f32,
            );
            if a == 0 {
                continue;
            }
            let alfa = (u16::from(style.color.a) * u16::from(a) / 255) as u8;
            canvas.blend_pixel(
                x,
                y,
                crate::annotate::style::Color::rgba(
                    style.color.r,
                    style.color.g,
                    style.color.b,
                    alfa,
                ),
            );
        }
    }
}

/// Cobertura interpolada bilinealmente; 0 fuera del buffer.
fn muestrear_bilineal(cob: &[u8], (bw, bh): (usize, usize), fx: f32, fy: f32) -> u8 {
    if bw == 0 || bh == 0 || fx < -1.0 || fy < -1.0 || fx > bw as f32 || fy > bh as f32 {
        return 0;
    }
    let (x0, y0) = (fx.floor() as i32, fy.floor() as i32);
    let (tx, ty) = (fx - x0 as f32, fy - y0 as f32);
    let en = |x: i32, y: i32| -> f32 {
        if x < 0 || y < 0 || x as usize >= bw || y as usize >= bh {
            0.0
        } else {
            f32::from(cob[y as usize * bw + x as usize])
        }
    };
    let arriba = en(x0, y0) * (1.0 - tx) + en(x0 + 1, y0) * tx;
    let abajo = en(x0, y0 + 1) * (1.0 - tx) + en(x0 + 1, y0 + 1) * tx;
    (arriba * (1.0 - ty) + abajo * ty).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotate::canvas::Canvas;
    use crate::annotate::style::Color;
    use crate::ports::Frame;

    fn ttf_normal() -> Vec<u8> {
        std::fs::read("C:/Windows/Fonts/segoeui.ttf").expect("fuente del sistema")
    }

    fn ttf_bold() -> Vec<u8> {
        std::fs::read("C:/Windows/Fonts/segoeuib.ttf").expect("fuente del sistema")
    }

    fn ctx_con_fuente() -> RenderContext {
        RenderContext::new(&ttf_normal(), &ttf_bold()).unwrap()
    }

    fn estilo(size: f32) -> TextStyle {
        TextStyle {
            color: Color::rgb(255, 0, 0),
            size,
            bold: true,
            familia: FamiliaId::default(),
        }
    }

    #[test]
    fn el_catalogo_registra_familias_y_devuelve_sus_nombres() {
        let mut ctx = RenderContext::nueva();
        let a = ctx.registrar_familia("Segoe UI");
        let b = ctx.registrar_familia("Consolas");
        assert_ne!(a, b);
        assert_eq!(ctx.nombre(a), Some("Segoe UI"));
        assert_eq!(ctx.nombre(b), Some("Consolas"));
        // Registrar la misma familia dos veces devuelve el mismo id.
        assert_eq!(ctx.registrar_familia("Segoe UI"), a);
        assert_eq!(ctx.familias().len(), 2);
        assert_eq!(ctx.nombre(FamiliaId(77)), None);
    }

    #[test]
    fn una_cara_no_cargada_cae_a_la_que_haya() {
        let mut ctx = RenderContext::nueva();
        let id = ctx.registrar_familia("Segoe UI");
        // Solo la normal cargada: pedir negrita debe caer a la normal.
        ctx.cargar_cara(id, false, &ttf_normal()).unwrap();
        let normal = TextStyle {
            color: Color::rgb(0, 0, 0),
            size: 20.0,
            bold: false,
            familia: id,
        };
        let negrita = TextStyle { bold: true, ..normal };
        assert!(ctx.font(normal).is_some());
        assert!(ctx.font(negrita).is_some(), "la negrita no cayó a la normal");
    }

    #[test]
    fn una_familia_ausente_cae_a_la_familia_por_defecto() {
        let mut ctx = RenderContext::nueva();
        let defecto = ctx.registrar_familia("Segoe UI");
        assert_eq!(defecto, FamiliaId(0), "la primera registrada es la de respaldo");
        ctx.cargar_cara(defecto, false, &ttf_normal()).unwrap();
        // Familia registrada pero SIN caras cargadas.
        let vacia = ctx.registrar_familia("Fuente Que No Existe");
        let style = TextStyle {
            color: Color::rgb(0, 0, 0),
            size: 20.0,
            bold: false,
            familia: vacia,
        };
        assert!(
            ctx.font(style).is_some(),
            "una familia sin caras debe caer a la por defecto"
        );
    }

    #[test]
    fn un_contexto_vacio_no_tiene_fuentes() {
        let ctx = RenderContext::sin_fuente();
        assert!(!ctx.tiene_alguna());
        assert!(ctx.font(estilo(20.0)).is_none());
    }

    #[test]
    fn un_ttf_invalido_da_error_y_no_ensucia_el_catalogo() {
        let mut ctx = RenderContext::nueva();
        let id = ctx.registrar_familia("Basura");
        assert!(ctx.cargar_cara(id, false, b"no soy un ttf").is_err());
        let style = TextStyle {
            color: Color::rgb(0, 0, 0),
            size: 20.0,
            bold: false,
            familia: id,
        };
        assert!(ctx.font(style).is_none());
    }

    #[test]
    fn dos_familias_distintas_rasterizan_distinto() {
        let mut ctx = RenderContext::nueva();
        let sans = ctx.registrar_familia("Segoe UI");
        ctx.cargar_cara(sans, false, &ttf_normal()).unwrap();
        let mono = ctx.registrar_familia("Consolas");
        ctx.cargar_cara(
            mono,
            false,
            &std::fs::read("C:/Windows/Fonts/consola.ttf").expect("Consolas"),
        )
        .unwrap();
        let base = TextStyle {
            color: Color::rgb(255, 0, 0),
            size: 24.0,
            bold: false,
            familia: sans,
        };
        let caja_sans = text_ink_box("Hola", base, &ctx).unwrap();
        let caja_mono = text_ink_box("Hola", TextStyle { familia: mono, ..base }, &ctx).unwrap();
        assert_ne!(caja_sans, caja_mono, "las dos familias miden igual");
    }

    #[test]
    fn sin_fuente_no_hay_caja_de_tinta() {
        assert_eq!(
            text_ink_box("7", estilo(20.0), &RenderContext::sin_fuente()),
            None
        );
    }

    #[test]
    fn el_espacio_no_tiene_tinta() {
        assert_eq!(text_ink_box("   ", estilo(20.0), &ctx_con_fuente()), None);
    }

    #[test]
    fn la_caja_de_tinta_encierra_exactamente_los_pixeles_pintados() {
        // La caja se mide y luego se compara con lo que de verdad se pinta.
        let ctx = ctx_con_fuente();
        let style = estilo(24.0);
        let (dx, dy, w, h) = text_ink_box("12", style, &ctx).expect("hay tinta");
        assert!(w > 0 && h > 0);

        let origen = (10, 10);
        let mut frame = Frame::filled(80, 60, [0, 0, 0, 255]);
        draw_text(&mut Canvas::new(&mut frame), origen, "12", style, &ctx);
        let pintados: Vec<(u32, u32)> = (0..80)
            .flat_map(|x| (0..60).map(move |y| (x, y)))
            .filter(|&(x, y)| frame.pixel(x, y).is_some_and(|[r, ..]| r > 0))
            .collect();
        assert!(!pintados.is_empty());
        let min_x = pintados.iter().map(|p| p.0).min().unwrap() as i32;
        let min_y = pintados.iter().map(|p| p.1).min().unwrap() as i32;
        let max_x = pintados.iter().map(|p| p.0).max().unwrap() as i32;
        let max_y = pintados.iter().map(|p| p.1).max().unwrap() as i32;
        // La caja predicha contiene la tinta real y no se pasa de holgada.
        assert_eq!((min_x, min_y), (origen.0 + dx, origen.1 + dy));
        assert_eq!(
            (max_x + 1, max_y + 1),
            (origen.0 + dx + w as i32, origen.1 + dy + h as i32)
        );
    }

    #[test]
    fn dos_lineas_dan_una_caja_mas_alta_que_una() {
        let ctx = ctx_con_fuente();
        let (_, _, _, h1) = text_ink_box("A", estilo(20.0), &ctx).unwrap();
        let (_, _, _, h2) = text_ink_box("A\nA", estilo(20.0), &ctx).unwrap();
        assert!(h2 > h1 * 2 - 4, "h1 = {h1}, h2 = {h2}");
    }
}
