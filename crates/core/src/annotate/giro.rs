//! Giro de un objeto colocado (f.53): ángulo en radianes alrededor del
//! centro de su caja SIN girar.
//!
//! El centro no se guarda en ninguna parte: se deriva de la caja sin girar,
//! que es invariante al giro. Así rotar y desrotar es exactamente
//! reversible y `Command::Rotate` no necesita guardar estado anterior.
//!
//! Cachea seno y coseno porque un arrastre rota miles de puntos con el
//! mismo ángulo (cada `WM_MOUSEMOVE` re-hornea el documento).

/// Y crece hacia abajo, así que un ángulo positivo gira en el sentido de
/// las agujas del reloj tal y como se ve en pantalla.
///
/// Al serializar (f.31) se guarda SOLO el ángulo: seno y coseno son caché y
/// se reconstruyen al leer, así el archivo no depende de su representación.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(into = "f32", from = "f32")]
pub struct Giro {
    rad: f32,
    sin: f32,
    cos: f32,
}

impl Giro {
    pub fn new(rad: f32) -> Self {
        Self {
            rad,
            sin: rad.sin(),
            cos: rad.cos(),
        }
    }

    pub const fn nulo() -> Self {
        Self {
            rad: 0.0,
            sin: 0.0,
            cos: 1.0,
        }
    }

    pub fn rad(&self) -> f32 {
        self.rad
    }

    /// `true` si no hay giro: quien rasteriza toma el camino directo, sin
    /// remuestrear (es lo que conserva intacta la calidad actual).
    pub fn es_nulo(&self) -> bool {
        self.rad == 0.0
    }

    /// Rota un punto alrededor de `centro`.
    pub fn aplicar(&self, p: (i32, i32), centro: (f32, f32)) -> (i32, i32) {
        if self.es_nulo() {
            return p;
        }
        let (dx, dy) = (p.0 as f32 - centro.0, p.1 as f32 - centro.1);
        (
            (centro.0 + dx * self.cos - dy * self.sin).round() as i32,
            (centro.1 + dx * self.sin + dy * self.cos).round() as i32,
        )
    }

    /// Inverso de `aplicar`, en coma flotante y sin redondear: lo usan el
    /// texto y la censura, que mapean del destino al origen y necesitan la
    /// posición fraccionaria para muestrear.
    pub fn deshacer(&self, p: (f32, f32), centro: (f32, f32)) -> (f32, f32) {
        if self.es_nulo() {
            return p;
        }
        let (dx, dy) = (p.0 - centro.0, p.1 - centro.1);
        (
            centro.0 + dx * self.cos + dy * self.sin,
            centro.1 - dx * self.sin + dy * self.cos,
        )
    }
}

impl Default for Giro {
    fn default() -> Self {
        Self::nulo()
    }
}

impl From<f32> for Giro {
    fn from(rad: f32) -> Self {
        Giro::new(rad)
    }
}

impl From<Giro> for f32 {
    fn from(g: Giro) -> Self {
        g.rad
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CENTRO: (f32, f32) = (10.0, 10.0);

    #[test]
    fn el_giro_nulo_no_mueve_nada() {
        let g = Giro::nulo();
        assert!(g.es_nulo());
        assert_eq!(g.aplicar((3, 7), CENTRO), (3, 7));
    }

    #[test]
    fn noventa_grados_lleva_derecha_a_abajo() {
        // Y crece hacia abajo (coordenadas de pantalla): +90° gira en el
        // sentido de las agujas del reloj tal y como se ve.
        let g = Giro::new(std::f32::consts::FRAC_PI_2);
        assert_eq!(g.aplicar((20, 10), CENTRO), (10, 20));
        assert_eq!(g.aplicar((10, 20), CENTRO), (0, 10));
    }

    #[test]
    fn ciento_ochenta_grados_es_el_punto_opuesto() {
        let g = Giro::new(std::f32::consts::PI);
        assert_eq!(g.aplicar((14, 10), CENTRO), (6, 10));
        assert_eq!(g.aplicar((10, 4), CENTRO), (10, 16));
    }

    #[test]
    fn deshacer_es_el_inverso_de_aplicar() {
        let g = Giro::new(0.7);
        for p in [(0, 0), (25, 3), (-8, 40)] {
            let girado = g.aplicar(p, CENTRO);
            let vuelto = g.deshacer((girado.0 as f32, girado.1 as f32), CENTRO);
            // Ida y vuelta con redondeo intermedio: ±1 px.
            assert!(
                (vuelto.0 - p.0 as f32).abs() <= 1.0 && (vuelto.1 - p.1 as f32).abs() <= 1.0,
                "{p:?} -> {girado:?} -> {vuelto:?}"
            );
        }
    }

    #[test]
    fn el_centro_no_se_mueve() {
        let g = Giro::new(1.234);
        assert_eq!(g.aplicar((10, 10), CENTRO), (10, 10));
    }
}
