//! Geometría de frontera: rectángulos en coordenadas de escritorio virtual.
//! El origen puede ser negativo (monitor a la izquierda del primario).

/// Rectángulo en coordenadas de escritorio virtual.
///
/// f.19 permite capturas diminutas: no hay tamaño mínimo, solo el
/// rect de área cero se considera vacío.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Borde derecho exclusivo. `i64` para no desbordar con orígenes extremos.
    pub fn right(&self) -> i64 {
        self.x as i64 + self.width as i64
    }

    /// Borde inferior exclusivo.
    pub fn bottom(&self) -> i64 {
        self.y as i64 + self.height as i64
    }

    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// `true` si `other` cabe entero dentro de `self` (bordes incluidos).
    pub fn contains(&self, other: &Rect) -> bool {
        self.x as i64 <= other.x as i64
            && self.y as i64 <= other.y as i64
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }

    /// `true` si el punto cae dentro (bordes izquierdo/superior incluidos,
    /// derecho/inferior excluidos). Un rect vacío no contiene nada.
    pub fn contains_point(&self, p: (i32, i32)) -> bool {
        !self.is_empty()
            && (p.0 as i64) >= self.x as i64
            && (p.0 as i64) < self.right()
            && (p.1 as i64) >= self.y as i64
            && (p.1 as i64) < self.bottom()
    }

    /// Copia desplazada por `delta`, saturando en los extremos de `i32`.
    pub fn translated(&self, delta: (i32, i32)) -> Rect {
        Rect::new(
            self.x.saturating_add(delta.0),
            self.y.saturating_add(delta.1),
            self.width,
            self.height,
        )
    }

    /// Rect que encierra todos los puntos, ensanchado `margen` px por lado
    /// (el grosor del trazo sobresale del eje geométrico). Vacío sin puntos.
    pub fn bounding(puntos: &[(i32, i32)], margen: u32) -> Rect {
        let Some(&(x0, y0)) = puntos.first() else {
            return Rect::new(0, 0, 0, 0);
        };
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (x0, y0, x0, y0);
        for &(x, y) in &puntos[1..] {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        let m = margen as i64;
        let izq = min_x as i64 - m;
        let arr = min_y as i64 - m;
        // +1 porque los bordes son inclusivos y el ancho es exclusivo.
        let ancho = (max_x as i64 + m + 1 - izq).max(1);
        let alto = (max_y as i64 + m + 1 - arr).max(1);
        Rect::new(
            izq.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            arr.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            ancho.min(u32::MAX as i64) as u32,
            alto.min(u32::MAX as i64) as u32,
        )
    }

    /// Área común, o `None` si no se solapan o alguno es vacío.
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if right <= x as i64 || bottom <= y as i64 {
            return None;
        }
        Some(Rect::new(
            x,
            y,
            (right - x as i64) as u32,
            (bottom - y as i64) as u32,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_acepta_rect_interior_y_rechaza_desbordado() {
        let desktop = Rect::new(-1920, 0, 3840, 1080);
        assert!(desktop.contains(&Rect::new(-100, 10, 50, 50)));
        assert!(!desktop.contains(&Rect::new(1900, 0, 100, 100)));
    }

    #[test]
    fn contains_acepta_el_propio_rect() {
        let r = Rect::new(0, 0, 800, 600);
        assert!(r.contains(&r));
    }

    #[test]
    fn interseccion_de_solapados_devuelve_el_area_comun() {
        let a = Rect::new(0, 0, 100, 100);
        let b = Rect::new(50, 50, 100, 100);
        assert_eq!(a.intersection(&b), Some(Rect::new(50, 50, 50, 50)));
    }

    #[test]
    fn interseccion_de_disjuntos_es_none() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(20, 20, 5, 5);
        assert_eq!(a.intersection(&b), None);
    }

    #[test]
    fn contains_point_incluye_origen_y_excluye_el_borde_lejano() {
        let r = Rect::new(10, 20, 5, 3);
        assert!(r.contains_point((10, 20)));
        assert!(r.contains_point((14, 22)));
        assert!(!r.contains_point((15, 22))); // derecho exclusivo
        assert!(!r.contains_point((14, 23))); // inferior exclusivo
        assert!(!r.contains_point((9, 20)));
        // Origen negativo (monitor a la izquierda del primario).
        let n = Rect::new(-100, -50, 10, 10);
        assert!(n.contains_point((-100, -50)) && n.contains_point((-91, -41)));
        assert!(!n.contains_point((-101, -50)));
        // Vacío: nada dentro.
        assert!(!Rect::new(0, 0, 0, 5).contains_point((0, 0)));
    }

    #[test]
    fn translated_desplaza_sin_cambiar_el_tamano() {
        assert_eq!(
            Rect::new(10, 20, 5, 3).translated((-15, 4)),
            Rect::new(-5, 24, 5, 3)
        );
    }

    #[test]
    fn bounding_encierra_los_puntos_con_su_margen() {
        // Sin margen: un solo punto da un rect de 1×1.
        assert_eq!(Rect::bounding(&[(7, 9)], 0), Rect::new(7, 9, 1, 1));
        // Dos puntos en diagonal, bordes inclusivos.
        assert_eq!(
            Rect::bounding(&[(10, 4), (2, 20)], 0),
            Rect::new(2, 4, 9, 17)
        );
        // Con margen 3 crece 3 px por lado.
        assert_eq!(
            Rect::bounding(&[(10, 10), (10, 10)], 3),
            Rect::new(7, 7, 7, 7)
        );
        // Sin puntos: vacío.
        assert!(Rect::bounding(&[], 5).is_empty());
    }

    #[test]
    fn rect_vacio_no_contiene_ni_interseca() {
        let vacio = Rect::new(5, 5, 0, 10);
        assert!(vacio.is_empty());
        assert_eq!(vacio.intersection(&Rect::new(0, 0, 100, 100)), None);
    }
}
