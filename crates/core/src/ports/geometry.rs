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
    fn rect_vacio_no_contiene_ni_interseca() {
        let vacio = Rect::new(5, 5, 0, 10);
        assert!(vacio.is_empty());
        assert_eq!(vacio.intersection(&Rect::new(0, 0, 100, 100)), None);
    }
}
