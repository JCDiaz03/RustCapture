# F3 Slice E — Rotación de objetos: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** poder girar cualquier objeto de anotación ya colocado, arrastrando un asa de rotación del recuadro de selección.

**Architecture:** el giro deja de ser asunto de cada tipo y pasa a ser una propiedad del **objeto colocado**: `Objeto` se convierte en un struct `{ forma: Forma, giro: f32 }` donde `Forma` es el enum que hoy se llama `Objeto`. El centro de giro no se guarda — es el centro de la caja SIN girar, que es invariante, así que rotar y desrotar es reversible sin estado extra. El rasterizado se resuelve por familias: las formas hechas de puntos (línea, flecha, lápiz, paso) rotan sus puntos y reutilizan los rasterizadores actuales sin tocarlos; la elipse ya se muestrea paramétricamente y solo añade el giro al muestreo; los rellenos (resaltador) pasan a rasterizar un cuadrilátero; y la censura y el texto se resuelven por **mapeo inverso** (para cada píxel del destino se deshace el giro y se mira qué le corresponde en el espacio del objeto), que es la única forma de rotar glifos rasterizados sin huecos.

**Tech Stack:** Rust 2024, `rustcapture-core` sin dependencias de plataforma (`fontdue`); `platform-win` con `windows` 0.62 y GDI.

## Global Constraints

- **TDD obligatorio en `core`** (`skills.md`): test primero. En `platform-win` aplica a la lógica pura de `math.rs`.
- **Nada de `unsafe` nuevo en `core`**; en `platform-win`, `// SAFETY:` en cada bloque.
- **Cero dependencias nuevas.** Nada de crates de geometría o de imagen: seno, coseno y muestreo a mano.
- **Ángulos en radianes** en el core (`f32`), en grados solo en la UI si hace falta mostrarlos.
- **Un objeto sin girar debe rasterizarse EXACTAMENTE igual que hoy.** `giro == 0.0` toma el camino actual, sin remuestrear: es lo que garantiza que este slice no degrada la calidad de lo que ya funciona. Hay un test por familia que lo comprueba.
- **Commits:** uno al final, propuesto al humano (`skills.md` solo autoriza commit automático al cerrar fase).
- Comandos: `cargo test -p rustcapture-core`, `cargo test -p platform-win`, `cargo clippy --all-targets`.

---

## Estructura de archivos

| Archivo | Responsabilidad | Acción |
|---|---|---|
| `crates/core/src/annotate/objeto.rs` | `struct Objeto { forma, giro }` + `enum Forma`; bounds girado, rotate | modificar |
| `crates/core/src/annotate/giro.rs` | `Giro`: seno/coseno cacheados, rotar y desrotar puntos | **crear** |
| `crates/core/src/annotate/shapes.rs` | `fill_quad_blend` (relleno de cuadrilátero convexo) | modificar |
| `crates/core/src/annotate/censor.rs` | censura de un cuadrilátero girado por mapeo inverso | modificar |
| `crates/core/src/annotate/text.rs` | `draw_text_rotado` por mapeo inverso + coberturas cacheadas | modificar |
| `crates/core/src/annotate/annotations/*.rs` | cada `render` acepta el `Giro` y lo honra | modificar (9) |
| `crates/core/src/annotate/document.rs` | `Command::Rotate` | modificar |
| `crates/core/src/ports/geometry.rs` | `Rect::corners()` | modificar |
| `crates/platform-win/src/editor/math.rs` | asa de rotación: posición, hit-test, ángulo del arrastre, snap | modificar |
| `crates/platform-win/src/editor/estado.rs` | `GirarDrag` | modificar |
| `crates/platform-win/src/editor/mod.rs` | arrastre circular y pintado del asa | modificar |
| `ideas.md`, `roadmap.md`, `arquitectura.md`, `diseno-frontend.md` | f.53, estado, D5, asa | modificar |

---

### Task 1: `Giro` y las esquinas de un rect

La aritmética del giro, aislada y testeada antes de que nadie dependa de ella. `Giro` cachea seno y coseno porque en un arrastre se rotan miles de puntos con el mismo ángulo.

**Files:**
- Create: `crates/core/src/annotate/giro.rs`
- Modify: `crates/core/src/annotate/mod.rs` (añadir `mod giro;` y `pub use giro::Giro;`)
- Modify: `crates/core/src/ports/geometry.rs`

**Interfaces:**
- Produces:
  - `pub struct Giro { rad: f32, sin: f32, cos: f32 }`
  - `Giro::new(rad: f32) -> Giro`, `Giro::nulo() -> Giro`, `Giro::es_nulo(&self) -> bool`, `Giro::rad(&self) -> f32`
  - `Giro::aplicar(&self, p: (i32,i32), centro: (f32,f32)) -> (i32,i32)`
  - `Giro::deshacer(&self, p: (f32,f32), centro: (f32,f32)) -> (f32,f32)`
  - `Rect::corners(&self) -> [(i32,i32); 4]` y `Rect::centro(&self) -> (f32,f32)`

- [ ] **Step 1: Escribir los tests que fallan**

`crates/core/src/annotate/giro.rs`, al final:

```rust
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
```

`crates/core/src/ports/geometry.rs`, en `mod tests`:

```rust
    #[test]
    fn corners_da_las_cuatro_esquinas_inclusivas_en_orden() {
        // Bordes inclusivos: la esquina lejana es el último píxel.
        assert_eq!(
            Rect::new(10, 20, 5, 3).corners(),
            [(10, 20), (14, 20), (14, 22), (10, 22)]
        );
    }

    #[test]
    fn centro_es_el_punto_medio_en_coma_flotante() {
        assert_eq!(Rect::new(10, 20, 5, 3).centro(), (12.0, 21.0));
        assert_eq!(Rect::new(0, 0, 4, 4).centro(), (1.5, 1.5));
    }
```

- [ ] **Step 2: Ejecutar y comprobar que fallan**

Run: `cargo test -p rustcapture-core giro ports::geometry`
Expected: FAIL de compilación (`Giro` y `corners` no existen).

- [ ] **Step 3: Implementar `Giro`**

```rust
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
#[derive(Clone, Copy, PartialEq, Debug)]
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
```

- [ ] **Step 4: Implementar `Rect::corners` y `Rect::centro`**

En `crates/core/src/ports/geometry.rs`, dentro de `impl Rect`:

```rust
    /// Las cuatro esquinas en orden (sup-izq, sup-der, inf-der, inf-izq),
    /// con bordes INCLUSIVOS: la esquina lejana es el último píxel, no el
    /// borde exclusivo, porque es lo que se rota para obtener la caja.
    pub fn corners(&self) -> [(i32, i32); 4] {
        let x1 = self.x + self.width.max(1) as i32 - 1;
        let y1 = self.y + self.height.max(1) as i32 - 1;
        [(self.x, self.y), (x1, self.y), (x1, y1), (self.x, y1)]
    }

    /// Centro geométrico en coma flotante (el centro de giro).
    pub fn centro(&self) -> (f32, f32) {
        (
            self.x as f32 + (self.width.max(1) as f32 - 1.0) / 2.0,
            self.y as f32 + (self.height.max(1) as f32 - 1.0) / 2.0,
        )
    }
```

- [ ] **Step 5: Declarar el módulo y ejecutar**

En `crates/core/src/annotate/mod.rs`: `mod giro;` y `pub use giro::Giro;`.

Run: `cargo test -p rustcapture-core`
Expected: PASS, con los 7 tests nuevos.

---

### Task 2: `Objeto` pasa a ser `{ forma, giro }`

Refactor sin cambio de comportamiento. El enum actual se renombra a `Forma` y `Objeto` lo envuelve con el giro. Es lo que hace que el giro sea una propiedad del objeto colocado y no un campo repetido nueve veces.

**Files:**
- Modify: `crates/core/src/annotate/objeto.rs`
- Modify: `crates/core/src/annotate/document.rs`
- Modify: `crates/core/src/annotate/mod.rs`
- Modify: `crates/platform-win/src/editor/texto.rs` (el patrón `Objeto::Texto`)

**Interfaces:**
- Consumes: `Giro` (Task 1).
- Produces:
  - `pub enum Forma { Flecha(..), Elipse(..), Resaltador(..), Linea(..), Lapiz(..), Pixelado(..), Rect(..), Paso(..), Texto(..) }`
  - `pub struct Objeto { pub forma: Forma, pub giro: Giro }`
  - `Objeto::render`, `Objeto::bounds(ctx)`, `Objeto::translate(delta)`, `Objeto::rotar(delta_rad)`
  - `Forma::bounds_sin_girar(&self, ctx) -> Rect` (la caja de hoy)
  - `impl From<T> for Objeto` para los 9 tipos (giro nulo)

- [ ] **Step 1: Renombrar el enum y envolverlo**

En `objeto.rs`: renombrar `pub enum Objeto` a `pub enum Forma` (y todos sus `Objeto::` internos a `Forma::`), renombrar `bounds` a `bounds_sin_girar`, y añadir:

```rust
/// Un objeto COLOCADO en el documento: una forma más su giro. El giro vive
/// aquí y no dentro de cada forma porque es propiedad de la colocación, no
/// del tipo: así `rotar` es una línea en vez de nueve, y el formato
/// re-editable (f.31) serializa un solo campo.
#[derive(Clone)]
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
        let girada: Vec<(i32, i32)> = base
            .corners()
            .iter()
            .map(|&c| self.giro.aplicar(c, centro))
            .collect();
        Rect::bounding(&girada, 0)
    }

    pub fn translate(&mut self, delta: (i32, i32)) {
        self.forma.translate(delta);
    }

    /// Suma `delta_rad` al giro. El centro se recalcula de la caja sin
    /// girar, que no cambia al rotar: girar y desgirar es reversible.
    pub fn rotar(&mut self, delta_rad: f32) {
        self.giro = Giro::new(self.giro.rad() + delta_rad);
    }
}
```

Y las conversiones pasan a envolver:

```rust
desde! { ArrowAnnotation => Flecha, /* … los 9 … */ }
```

con la macro construyendo `Objeto::nuevo(Forma::$variante(a))`:

```rust
macro_rules! desde {
    ($($tipo:ty => $variante:ident),* $(,)?) => {
        $(impl From<$tipo> for Objeto {
            fn from(a: $tipo) -> Self {
                Objeto::nuevo(Forma::$variante(a))
            }
        })*
    };
}
```

- [ ] **Step 2: `Forma::render` recibe el giro y lo ignora aún**

```rust
    pub fn render(&self, canvas: &mut Canvas, ctx: &RenderContext, giro: Giro) {
        match self {
            Forma::Flecha(a) => a.render_girado(canvas, ctx, giro),
            // … los 9, todos a un render_girado que en esta tarea delega
            // en el render actual ignorando el giro.
        }
    }
```

Para no romper nada en esta tarea, cada tipo recibe un método puente:

```rust
// En cada annotations/*.rs, junto al impl Annotation:
impl RectAnnotation {
    /// Rasteriza honrando el giro. En esta tarea aún lo ignora; las tareas
    /// 3-6 lo implementan por familias.
    pub(crate) fn render_girado(&self, canvas: &mut Canvas, ctx: &RenderContext, _giro: Giro) {
        self.render(canvas, ctx);
    }
}
```

- [ ] **Step 3: Ajustar `Objeto::Texto` en el editor**

En `crates/platform-win/src/editor/texto.rs`, `abrir_reedicion`:

```rust
    let Some(Objeto {
        forma: Forma::Texto(t),
        ..
    }) = state.doc.get(index)
    else {
        return false;
    };
```

con `use rustcapture_core::annotate::{Command, Forma, Objeto, TextStyle};`.

- [ ] **Step 4: Test de que el refactor no cambia nada**

En `objeto.rs`, `mod tests`:

```rust
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
        // El centro se conserva.
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
```

- [ ] **Step 5: Ejecutar**

Run: `cargo test` · Expected: PASS en los cuatro crates. Todos los tests que ya existían siguen verdes sin tocarlos: es la prueba de que el refactor no cambia comportamiento.

---

### Task 3: `Command::Rotate`

**Files:**
- Modify: `crates/core/src/annotate/document.rs`

**Interfaces:**
- Produces: `Command::Rotate { index, delta_rad }` y `Command::rotate_by(index, delta_rad)`.

- [ ] **Step 1: Test que falla**

```rust
    #[test]
    fn rotate_gira_el_objeto_y_undo_lo_devuelve() {
        let ctx = RenderContext::sin_fuente();
        let mut doc = Document::new();
        let mut historia = History::new();
        // Caja alargada: al girar 90° la caja cambia de forma visiblemente.
        historia.apply(
            &mut doc,
            Command::add(
                crate::annotate::annotations::RectAnnotation {
                    rect: Rect::new(0, 0, 12, 4),
                    style: Style {
                        color: Color::rgb(255, 0, 0),
                        thickness: 1,
                    },
                }
                .into(),
            ),
        );
        let antes = doc.get(0).unwrap().bounds(&ctx);
        assert!(historia.apply(&mut doc, Command::rotate_by(0, std::f32::consts::FRAC_PI_2)));
        assert!(doc.get(0).unwrap().bounds(&ctx).height > antes.height);

        assert!(historia.undo(&mut doc));
        assert_eq!(doc.get(0).unwrap().bounds(&ctx), antes);
        assert!(historia.redo(&mut doc));
        assert!(doc.get(0).unwrap().bounds(&ctx).height > antes.height);
    }

    #[test]
    fn un_rotate_invalido_o_nulo_no_se_apila() {
        let mut doc = Document::new();
        let mut historia = History::new();
        historia.apply(&mut doc, Command::add(caja(2)));
        assert!(!historia.apply(&mut doc, Command::rotate_by(9, 0.5)));
        assert!(!historia.apply(&mut doc, Command::rotate_by(0, 0.0)));
        assert!(historia.undo(&mut doc));
        assert!(!historia.can_undo());
    }
```

- [ ] **Step 2: Ejecutar y ver que falla** — Run: `cargo test -p rustcapture-core document`; FAIL: `rotate_by` no existe.

- [ ] **Step 3: Implementar**

Variante nueva en `enum Command`:

```rust
    /// Gira un objeto ya colocado. Como `Move`, revertir es aplicar el
    /// delta negado: no guarda el ángulo anterior.
    Rotate { index: usize, delta_rad: f32 },
```

Constructor:

```rust
    pub fn rotate_by(index: usize, delta_rad: f32) -> Self {
        Command::Rotate { index, delta_rad }
    }
```

En `apply`:

```rust
            Command::Rotate { index, delta_rad } => match doc.objetos.get_mut(*index) {
                Some(_) if *delta_rad == 0.0 => false,
                Some(o) => {
                    o.rotar(*delta_rad);
                    true
                }
                None => false,
            },
```

En `revert`:

```rust
            Command::Rotate { index, delta_rad } => {
                if let Some(o) = doc.objetos.get_mut(*index) {
                    o.rotar(-*delta_rad);
                }
            }
```

- [ ] **Step 4: Ejecutar** — Run: `cargo test -p rustcapture-core`; Expected: PASS.

---

### Task 4: Rotación de las formas hechas de puntos

Familia barata: línea, flecha, lápiz y la posición del disco del paso. Se rotan los puntos y se llama al rasterizador que ya existe — **calidad idéntica a la de un objeto sin girar**, porque no hay remuestreo.

**Files:**
- Modify: `crates/core/src/annotate/annotations/{line,arrow,pen,step}.rs`

**Interfaces:**
- Consumes: `Giro::aplicar`, `Forma::bounds_sin_girar`.

- [ ] **Step 1: Test que falla**

En `annotations/mod.rs`, `mod tests`:

```rust
    /// Una línea girada 90° alrededor de su centro pasa de horizontal a
    /// vertical, y sus píxeles caen donde caería la línea vertical.
    #[test]
    fn la_linea_girada_noventa_grados_queda_vertical() {
        let ctx = RenderContext::sin_fuente();
        let mut o: Objeto = LineAnnotation {
            from: (5, 15),
            to: (25, 15),
            style: ESTILO,
        }
        .into();
        o.rotar(std::f32::consts::FRAC_PI_2);
        let mut frame = Frame::filled(30, 30, [0, 0, 0, 255]);
        o.render(&mut Canvas::new(&mut frame), &ctx);
        // Centro (15,15): la línea ahora va de (15,5) a (15,25).
        assert!(es_rojo(&frame, 15, 8) && es_rojo(&frame, 15, 22));
        assert!(!es_rojo(&frame, 8, 15) && !es_rojo(&frame, 22, 15));
    }

    #[test]
    fn el_lapiz_girado_mantiene_su_longitud_de_trazo() {
        let ctx = RenderContext::sin_fuente();
        let contar = |o: &Objeto| {
            let mut f = Frame::filled(60, 60, [0, 0, 0, 255]);
            o.render(&mut Canvas::new(&mut f), &ctx);
            (0..60)
                .flat_map(|x| (0..60).map(move |y| (x, y)))
                .filter(|&(x, y)| es_rojo(&f, x, y))
                .count()
        };
        let recto: Objeto = PenAnnotation {
            points: vec![(10, 30), (25, 30), (40, 30)],
            style: ESTILO,
        }
        .into();
        let mut girado = recto.clone();
        girado.rotar(std::f32::consts::FRAC_PI_2);
        // Mismo trazo, otra orientación: el número de píxeles apenas cambia.
        let (a, b) = (contar(&recto), contar(&girado));
        assert!(b * 10 > a * 8 && a * 10 > b * 8, "recto {a} vs girado {b}");
    }
```

- [ ] **Step 2: Ejecutar y ver que falla** — los tests fallan porque `render_girado` ignora el giro (la línea sigue horizontal).

- [ ] **Step 3: Implementar en `line.rs`**

```rust
impl LineAnnotation {
    pub(crate) fn render_girado(&self, canvas: &mut Canvas, _ctx: &RenderContext, giro: Giro) {
        if giro.es_nulo() {
            return self.render(canvas, _ctx);
        }
        // Se rotan los extremos y se reutiliza el mismo rasterizado: sin
        // remuestrear, así que la calidad es la de una línea sin girar.
        let centro = self.caja().centro();
        shapes::draw_line(
            canvas,
            giro.aplicar(self.from, centro),
            giro.aplicar(self.to, centro),
            &self.style,
        );
    }

    /// Caja sin girar (la que usa `Forma::bounds_sin_girar`), para que el
    /// centro de giro sea el mismo que ve la selección.
    pub(crate) fn caja(&self) -> Rect {
        Rect::bounding(&[self.from, self.to], self.style.thickness.max(1) / 2)
    }
}
```

**Importante:** `Forma::bounds_sin_girar` pasa a llamar a estos `caja()` en vez de recalcular, para que el centro de giro del render y el de la caja de selección sean el MISMO punto. Si divergen, el objeto se desplaza al girar.

Igual en `arrow.rs` (rota `from` y `to`, la cabeza se recalcula sola desde ellos), `pen.rs` (rota cada punto) y `step.rs` (rota `center`; el disco es invariante al giro, solo el número necesita la Task 6).

- [ ] **Step 4: Ejecutar** — Run: `cargo test -p rustcapture-core`; Expected: PASS.

---

### Task 5: Rectángulo, elipse y relleno girados

**Files:**
- Modify: `crates/core/src/annotate/shapes.rs` (`fill_quad_blend`)
- Modify: `crates/core/src/annotate/annotations/{rect,ellipse,highlight}.rs`

- [ ] **Step 1: Tests que fallan**

```rust
    #[test]
    fn el_rectangulo_girado_cuarenta_y_cinco_grados_es_un_rombo() {
        let ctx = RenderContext::sin_fuente();
        let mut o: Objeto = RectAnnotation {
            rect: Rect::new(10, 10, 20, 20),
            style: ESTILO,
        }
        .into();
        o.rotar(std::f32::consts::FRAC_PI_4);
        let mut frame = Frame::filled(50, 50, [0, 0, 0, 255]);
        o.render(&mut Canvas::new(&mut frame), &ctx);
        // Centro (19.5,19.5): el rombo toca arriba/abajo/izq/der en su
        // punto medio, y las esquinas del cuadrado original quedan vacías.
        assert!(es_rojo(&frame, 19, 5) || es_rojo(&frame, 20, 5), "vértice superior");
        assert!(!es_rojo(&frame, 11, 11), "la esquina original sigue pintada");
    }

    #[test]
    fn el_resaltador_girado_rellena_su_rombo_y_no_la_caja() {
        let ctx = RenderContext::sin_fuente();
        let mut o: Objeto = HighlightAnnotation {
            rect: Rect::new(10, 10, 20, 20),
            color: Color::rgba(255, 255, 0, 128),
        }
        .into();
        o.rotar(std::f32::consts::FRAC_PI_4);
        let mut frame = Frame::filled(50, 50, [0, 0, 0, 255]);
        o.render(&mut Canvas::new(&mut frame), &ctx);
        let amarillo = |x, y| frame.pixel(x, y).is_some_and(|[r, g, b, _]| r > 100 && g > 100 && b == 0);
        assert!(amarillo(19, 19), "el centro debe estar relleno");
        assert!(!amarillo(11, 11), "la esquina de la caja no se rellena");
    }
```

- [ ] **Step 2: Ejecutar y ver que fallan.**

- [ ] **Step 3: `fill_quad_blend` en `shapes.rs`**

```rust
/// Relleno de un cuadrilátero CONVEXO por barrido de filas: para cada y se
/// calculan las intersecciones con los cuatro lados y se rellena entre la
/// menor y la mayor. Lo usan las formas de caja al estar giradas.
pub(crate) fn fill_quad_blend(canvas: &mut Canvas, quad: [(i32, i32); 4], color: Color) {
    let y_min = quad.iter().map(|p| p.1).min().unwrap_or(0);
    let y_max = quad.iter().map(|p| p.1).max().unwrap_or(0);
    for y in y_min..=y_max {
        let mut x_min = i32::MAX;
        let mut x_max = i32::MIN;
        for i in 0..4 {
            let (a, b) = (quad[i], quad[(i + 1) % 4]);
            if a.1 == b.1 {
                // Lado horizontal: aporta sus dos extremos en su propia fila.
                if a.1 == y {
                    x_min = x_min.min(a.0.min(b.0));
                    x_max = x_max.max(a.0.max(b.0));
                }
                continue;
            }
            let (alto, bajo) = if a.1 < b.1 { (a, b) } else { (b, a) };
            if y < alto.1 || y > bajo.1 {
                continue;
            }
            let t = (y - alto.1) as f32 / (bajo.1 - alto.1) as f32;
            let x = (alto.0 as f32 + t * (bajo.0 - alto.0) as f32).round() as i32;
            x_min = x_min.min(x);
            x_max = x_max.max(x);
        }
        if x_min <= x_max {
            for x in x_min..=x_max {
                canvas.blend_pixel(x, y, color);
            }
        }
    }
}
```

Con su test propio en `shapes.rs`:

```rust
    #[test]
    fn fill_quad_rellena_un_rombo_y_deja_las_esquinas() {
        let mut frame = Frame::filled(20, 20, NEGRO);
        // Rombo inscrito en 4..16.
        fill_quad_blend(
            &mut Canvas::new(&mut frame),
            [(10, 4), (16, 10), (10, 16), (4, 10)],
            ROJO,
        );
        assert!(es_rojo(&frame, 10, 10) && es_rojo(&frame, 10, 5));
        assert!(!es_rojo(&frame, 5, 5), "la esquina queda fuera del rombo");
    }
```

- [ ] **Step 4: `render_girado` de los tres tipos**

`rect.rs`: con giro, las 4 esquinas rotadas unidas por `draw_line`:

```rust
    pub(crate) fn render_girado(&self, canvas: &mut Canvas, ctx: &RenderContext, giro: Giro) {
        if giro.es_nulo() {
            return self.render(canvas, ctx);
        }
        let centro = self.rect.centro();
        let q = self.rect.corners().map(|c| giro.aplicar(c, centro));
        for i in 0..4 {
            shapes::draw_line(canvas, q[i], q[(i + 1) % 4], &self.style);
        }
    }
```

`highlight.rs`: `fill_quad_blend` con las esquinas rotadas.

`ellipse.rs`: el muestreo paramétrico ya existe; se rota cada muestra antes de estampar:

```rust
    pub(crate) fn render_girado(&self, canvas: &mut Canvas, ctx: &RenderContext, giro: Giro) {
        if giro.es_nulo() {
            return self.render(canvas, ctx);
        }
        shapes::draw_ellipse_outline_girada(canvas, self.rect, &self.style, giro);
    }
```

y en `shapes.rs`, `draw_ellipse_outline_girada` es el bucle de `draw_ellipse_outline` con `giro.aplicar((x, y), centro)` antes de `stamp_disc` (extraer el cuerpo común para no duplicar el muestreo).

- [ ] **Step 5: Ejecutar** — `cargo test -p rustcapture-core`; PASS.

---

### Task 6: Censura y texto girados (mapeo inverso)

La familia caras. No se puede rotar hacia delante (dejaría huecos entre píxeles): se recorre el **destino** y para cada píxel se deshace el giro para saber qué le corresponde en el espacio del objeto.

**Files:**
- Modify: `crates/core/src/annotate/censor.rs`
- Modify: `crates/core/src/annotate/text.rs`
- Modify: `crates/core/src/annotate/annotations/{pixelate,step,text}.rs`

- [ ] **Step 1: Tests que fallan**

```rust
    #[test]
    fn la_censura_girada_solo_tapa_su_rombo() {
        let ctx = RenderContext::sin_fuente();
        let mut frame = Frame::filled(50, 50, [255, 255, 255, 255]);
        // Se pinta una franja negra que cruza el centro para ver el efecto.
        for x in 0..50u32 {
            let i = (25 * 50 + x as usize) * 4;
            frame.pixels[i..i + 3].copy_from_slice(&[0, 0, 0]);
        }
        let mut o: Objeto = PixelateAnnotation {
            rect: Rect::new(15, 15, 20, 20),
            mode: CensorMode::Mosaic { block: 20 },
        }
        .into();
        o.rotar(std::f32::consts::FRAC_PI_4);
        o.render(&mut Canvas::new(&mut frame), &ctx);
        // El centro se censura (deja de ser negro puro de la franja)...
        assert_ne!(frame.pixel(25, 25), Some([0, 0, 0, 255]));
        // ...y la esquina de la caja, fuera del rombo, sigue blanca.
        assert_eq!(frame.pixel(16, 16), Some([255, 255, 255, 255]));
    }

    #[test]
    fn el_texto_girado_ocupa_otra_orientacion_y_conserva_tinta() {
        let ctx = ctx_con_fuente();
        let contar = |o: &Objeto| {
            let mut f = Frame::filled(120, 120, [0, 0, 0, 255]);
            o.render(&mut Canvas::new(&mut f), &ctx);
            let puntos: Vec<(u32, u32)> = (0..120)
                .flat_map(|x| (0..120).map(move |y| (x, y)))
                .filter(|&(x, y)| f.pixel(x, y).is_some_and(|[r, ..]| r > 60))
                .collect();
            let w = puntos.iter().map(|p| p.0).max().unwrap() - puntos.iter().map(|p| p.0).min().unwrap();
            let h = puntos.iter().map(|p| p.1).max().unwrap() - puntos.iter().map(|p| p.1).min().unwrap();
            (puntos.len(), w, h)
        };
        let recto: Objeto = TextAnnotation {
            pos: (30, 50),
            text: "Hola".to_string(),
            style: crate::annotate::style::TextStyle {
                color: ROJO,
                size: 24.0,
                bold: true,
            },
        }
        .into();
        let mut girado = recto.clone();
        girado.rotar(std::f32::consts::FRAC_PI_2);
        let (n_recto, w_recto, h_recto) = contar(&recto);
        let (n_girado, w_girado, h_girado) = contar(&girado);
        // Girado 90°: ancho y alto se intercambian.
        assert!(w_recto > h_recto && h_girado > w_girado);
        // Y no se pierde ni se duplica tinta (±35 % por el remuestreo).
        assert!(
            n_girado * 100 > n_recto * 65 && n_recto * 100 > n_girado * 65,
            "tinta recto {n_recto} vs girado {n_girado}"
        );
    }
```

- [ ] **Step 2: Ejecutar y ver que fallan.**

- [ ] **Step 3: Censura girada**

En `censor.rs`, las dos funciones públicas ganan el giro y, si no es nulo, recorren la caja girada mapeando cada píxel hacia atrás:

```rust
/// Mosaico girado: las celdas se definen en el espacio SIN girar del rect
/// (así el mosaico gira con el objeto), y se recorre la caja girada
/// deshaciendo el giro píxel a píxel para saber a qué celda pertenece.
pub(crate) fn mosaico_girado(canvas: &mut Canvas, rect: Rect, bloque: u32, giro: Giro) {
    if giro.es_nulo() {
        return mosaico(canvas, rect, bloque);
    }
    let bloque = bloque.max(1) as i32;
    let centro = rect.centro();
    // Primero se promedia cada celda en el espacio del objeto, leyendo del
    // canvas por el punto GIRADO (que es donde están los píxeles reales).
    let (cols, filas) = (
        rect.width.div_ceil(bloque as u32) as i32,
        rect.height.div_ceil(bloque as u32) as i32,
    );
    let mut medias = Vec::with_capacity((cols * filas) as usize);
    for cy in 0..filas {
        for cx in 0..cols {
            let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
            for dy in 0..bloque {
                for dx in 0..bloque {
                    let p = (rect.x + cx * bloque + dx, rect.y + cy * bloque + dy);
                    if p.0 >= rect.x + rect.width as i32 || p.1 >= rect.y + rect.height as i32 {
                        continue;
                    }
                    if let Some(c) = canvas.pixel_en(giro.aplicar(p, centro)) {
                        r += u32::from(c.r);
                        g += u32::from(c.g);
                        b += u32::from(c.b);
                        n += 1;
                    }
                }
            }
            medias.push(if n == 0 {
                None
            } else {
                Some(Color::rgb((r / n) as u8, (g / n) as u8, (b / n) as u8))
            });
        }
    }
    // Y después se escribe recorriendo la caja girada.
    escribir_por_celda(canvas, rect, bloque, giro, &medias, (cols, filas));
}
```

donde `Canvas::pixel_en(p: (i32,i32))` es un alias de `pixel(p.0, p.1)` para leer con tuplas, y `escribir_por_celda` recorre la caja envolvente girada, deshace el giro de cada píxel, comprueba que cae dentro del rect y escribe la media de su celda:

```rust
fn escribir_por_celda(
    canvas: &mut Canvas,
    rect: Rect,
    bloque: i32,
    giro: Giro,
    medias: &[Option<Color>],
    (cols, _filas): (i32, i32),
) {
    let centro = rect.centro();
    let caja = Rect::bounding(&rect.corners().map(|c| giro.aplicar(c, centro)), 1);
    for y in caja.y..caja.bottom() as i32 {
        for x in caja.x..caja.right() as i32 {
            let (ox, oy) = giro.deshacer((x as f32, y as f32), centro);
            let (ox, oy) = (ox.round() as i32, oy.round() as i32);
            if ox < rect.x
                || oy < rect.y
                || ox >= rect.x + rect.width as i32
                || oy >= rect.y + rect.height as i32
            {
                continue;
            }
            let celda = ((oy - rect.y) / bloque) * cols + (ox - rect.x) / bloque;
            if let Some(Some(color)) = medias.get(celda as usize) {
                canvas.blend_pixel(x, y, *color);
            }
        }
    }
}
```

El desenfoque girado sigue el mismo patrón: se desenfoca en el espacio del objeto (leyendo por el punto girado) y se escribe recorriendo la caja girada.

- [ ] **Step 4: Texto girado**

En `text.rs`, `draw_text_rotado` rasteriza los glifos como hoy pero **a un buffer de cobertura** del tamaño de la caja de tinta, y luego recorre la caja girada mapeando hacia atrás con muestreo bilineal:

```rust
/// Texto girado por mapeo inverso: los glifos se rasterizan una vez a un
/// buffer de cobertura sin girar (fontdue no sabe rotar) y ese buffer se
/// muestrea con bilineal desde el destino. Es la única forma de girar
/// glifos ya rasterizados sin dejar huecos entre píxeles.
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
    // Cobertura del texto en su propio espacio.
    let (bw, bh) = (w as usize, h as usize);
    let mut cobertura = vec![0u8; bw * bh];
    rasterizar_cobertura(&mut cobertura, (bw, bh), (dx, dy), text, style, ctx);

    let origen = (pos.0 + dx, pos.1 + dy);
    let caja_obj = Rect::new(origen.0, origen.1, w, h);
    let caja = Rect::bounding(&caja_obj.corners().map(|c| giro.aplicar(c, centro)), 1);
    for y in caja.y..caja.bottom() as i32 {
        for x in caja.x..caja.right() as i32 {
            let (ox, oy) = giro.deshacer((x as f32, y as f32), centro);
            let (fx, fy) = (ox - origen.0 as f32, oy - origen.1 as f32);
            let a = muestrear_bilineal(&cobertura, (bw, bh), fx, fy);
            if a == 0 {
                continue;
            }
            let alfa = (u16::from(style.color.a) * u16::from(a) / 255) as u8;
            canvas.blend_pixel(
                x,
                y,
                Color::rgba(style.color.r, style.color.g, style.color.b, alfa),
            );
        }
    }
}

/// Cobertura bilineal; 0 fuera del buffer.
fn muestrear_bilineal(cob: &[u8], (bw, bh): (usize, usize), fx: f32, fy: f32) -> u8 {
    if fx < -0.5 || fy < -0.5 || fx > bw as f32 - 0.5 || fy > bh as f32 - 0.5 {
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
```

`rasterizar_cobertura` es el bucle de `draw_text` escribiendo en el buffer en vez de en el canvas (extraer el recorrido de glifos a una función común que reciba un `&mut impl FnMut(i32, i32, u8)`, para que `draw_text` y este compartan la colocación y no puedan divergir).

`text.rs` (anotación) y `step.rs` (el número) llaman a `draw_text_rotado` con el centro de su caja sin girar.

- [ ] **Step 5: Ejecutar** — `cargo test -p rustcapture-core`; PASS. Comprobar además que los tests de calidad sin girar de las tareas anteriores siguen verdes: el camino `giro.es_nulo()` no debe haberse tocado.

---

### Task 7: El asa de rotación en el editor

**Files:**
- Modify: `crates/platform-win/src/editor/math.rs`
- Modify: `crates/platform-win/src/editor/estado.rs`
- Modify: `crates/platform-win/src/editor/mod.rs`

**Interfaces:**
- Produces:
  - `math::asa_rotacion(caja: Rect, asa: i32, brazo: i32) -> Rect`
  - `math::angulo_hacia(centro: (i32,i32), p: (i32,i32)) -> f32`
  - `math::ajustar_angulo(rad: f32, snap: bool) -> f32`
  - `estado::GirarDrag { index, centro, angulo_inicial, angulo_actual }`

- [ ] **Step 1: Tests puros que fallan**

```rust
    #[test]
    fn el_asa_de_rotacion_va_sobre_el_borde_superior() {
        let caja = Rect::new(10, 20, 100, 50);
        let asa = asa_rotacion(caja, 6, 18);
        // Centrada en X, por encima de la caja a la distancia del brazo.
        assert_eq!(asa.x + asa.width as i32 / 2, 60);
        assert!(asa.bottom() as i32 <= 20, "el asa pisa la caja");
        assert_eq!(asa.y + asa.height as i32 / 2, 2); // 20 - 18
    }

    #[test]
    fn el_angulo_se_mide_desde_arriba_en_el_sentido_del_reloj() {
        use std::f32::consts::{FRAC_PI_2, PI};
        let c = (10, 10);
        assert!((angulo_hacia(c, (10, 0))).abs() < 0.01); // arriba = 0
        assert!((angulo_hacia(c, (20, 10)) - FRAC_PI_2).abs() < 0.01); // derecha
        assert!((angulo_hacia(c, (10, 20)).abs() - PI).abs() < 0.01); // abajo
    }

    #[test]
    fn el_snap_redondea_a_quince_grados() {
        let grados = |r: f32| r.to_degrees();
        assert!((grados(ajustar_angulo(0.30, true)) - 15.0).abs() < 0.01);
        assert!((grados(ajustar_angulo(0.20, true)) - 15.0).abs() < 0.01);
        assert!((grados(ajustar_angulo(0.05, true))).abs() < 0.01);
        // Sin snap, el ángulo pasa tal cual.
        assert_eq!(ajustar_angulo(0.3, false), 0.3);
    }
```

- [ ] **Step 2: Ejecutar y ver que fallan.**

- [ ] **Step 3: Implementar en `math.rs`**

```rust
/// Distancia LÓGICA del asa de rotación por encima del borde superior.
pub(crate) const BRAZO_LOGICO: i32 = 18;
/// Paso del snap de rotación con Shift.
const SNAP_GRADOS: f32 = 15.0;

/// Asa de rotación: centrada sobre el borde superior, separada `brazo`.
pub(crate) fn asa_rotacion(caja: Rect, asa: i32, brazo: i32) -> Rect {
    let lado = asa.max(2);
    let cx = caja.x + caja.width as i32 / 2;
    let cy = caja.y - brazo;
    Rect::new(cx - lado / 2, cy - lado / 2, lado as u32, lado as u32)
}

/// Ángulo del vector centro→p medido desde ARRIBA y creciendo en el
/// sentido de las agujas del reloj, que es como gira `Giro`.
pub(crate) fn angulo_hacia(centro: (i32, i32), p: (i32, i32)) -> f32 {
    let (dx, dy) = ((p.0 - centro.0) as f32, (p.1 - centro.1) as f32);
    dx.atan2(-dy)
}

/// Redondea a múltiplos de 15° cuando `snap` (Shift pulsado).
pub(crate) fn ajustar_angulo(rad: f32, snap: bool) -> f32 {
    if !snap {
        return rad;
    }
    let paso = SNAP_GRADOS.to_radians();
    (rad / paso).round() * paso
}
```

- [ ] **Step 4: Estado del arrastre**

En `estado.rs`:

```rust
/// Arrastre del asa de rotación. Como `MoverDrag`, no toca el documento:
/// el giro se pinta como preview y se convierte en `Command::Rotate` al
/// soltar.
pub(super) struct GirarDrag {
    pub index: usize,
    /// Centro de giro en píxeles del frame.
    pub centro: (i32, i32),
    /// Ángulo del puntero al empezar y ahora; el delta es la diferencia.
    pub inicial: f32,
    pub actual: f32,
}

impl GirarDrag {
    pub(super) fn delta(&self) -> f32 {
        self.actual - self.inicial
    }
}
```

y el campo `pub girar: Option<GirarDrag>` en `EditorState` (inicializado a `None`, limpiado igual que `mover` al cambiar de herramienta).

- [ ] **Step 5: Preview del giro**

`Document` gana el hermano de `render_onto_moved`:

```rust
    /// Como `render_onto`, pintando el objeto `index` con un giro extra.
    /// Preview del arrastre del asa: el documento no se toca.
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
```

con su test (mismo patrón que `el_preview_pinta_movido_sin_tocar_el_documento`).

- [ ] **Step 6: Cablear el wndproc**

En `WM_LBUTTONDOWN`, rama `Seleccion`, ANTES del hit-test de objetos: si hay algo seleccionado y el clic cae en el asa de rotación (en coordenadas de vista), arrancar `GirarDrag` en vez de seleccionar. En `WM_MOUSEMOVE`, actualizar `actual` con `angulo_hacia` e invalidar solo el canvas. En `WM_LBUTTONUP`, aplicar `Command::rotate_by(index, ajustar_angulo(delta, shift))` — con `GetKeyState(VK_SHIFT)` para el snap — y apuntarlo en el contador de pasos como cualquier comando.

En `pintar_seleccion`, añadir el asa: una línea de 1 px del borde superior al asa y el asa como círculo relleno en el acento (distinguible de las 8 asas cuadradas). Durante el giro, el marco se dibuja girado (las 4 esquinas rotadas), no como caja envolvente.

- [ ] **Step 7: Ejecutar** — `cargo test` y `cargo clippy --all-targets`; PASS sin warnings nuevos.

---

### Task 8: Verificación manual y documentación

- [ ] **Step 1: Guion manual**

Run: `cargo run --release -p gui`

1. Coloca una flecha, un rectángulo, un texto, un resaltador y un pixelado.
2. Selección → elige la flecha: aparece el asa redonda sobre el borde superior.
3. Arrastra el asa: la flecha gira siguiendo el puntero, con el marco girado.
4. Suelta → `Ctrl+Z` deshace el giro, `Ctrl+Y` lo rehace.
5. Con **Shift** pulsado el giro salta de 15 en 15.
6. Gira el texto 90°: legible en vertical, sin huecos ni dientes de sierra graves.
7. Gira el pixelado 45°: censura el rombo, **no** la caja envolvente.
8. Gira el resaltador 30°: relleno sin bandas ni filas vacías.
9. Gira un paso numerado: el disco no cambia (es redondo) y el número gira con él.
10. Mover un objeto ya girado: se desplaza sin cambiar de ángulo.
11. Guardar como PNG: lo guardado coincide con lo que se ve.
12. Repetir 3 y 6 a 150 % de escala.
13. Un objeto sin girar debe verse **exactamente** como antes de este slice (comparar con una captura previa si hay dudas).

- [ ] **Step 2: Documentación**

- `ideas.md`: **f.53** «Rotación de objetos ya colocados: asa en el recuadro de selección, con salto de 15° manteniendo Shift.» (numeración append-only).
- `arquitectura.md` D5: el giro es propiedad del objeto colocado, no del tipo; centro derivado de la caja sin girar; familias de rasterizado (puntos rotados / paramétrico / cuadrilátero / mapeo inverso) y por qué el texto necesita remuestreo.
- `arquitectura.md` D6: `Rotate` es hermano de `Move` — revertir es el delta negado.
- `roadmap.md`: ítem de rotación en F3.
- `diseno-frontend.md` V4: el asa redonda de rotación y el snap con Shift.

- [ ] **Step 3: `verification-before-completion`**

`cargo test` · `cargo clippy --all-targets` · `cargo build --release` + los 13 puntos del guion. Sin eso no se propone commit.

- [ ] **Step 4: Proponer commit al humano** (no automático, `skills.md`).

---

## Autorrevisión

| Requisito | Tarea |
|---|---|
| Asa de rotación en el recuadro | 7 |
| Arrastre circular + snap 15° con Shift | 7 |
| Giro en línea, flecha, lápiz, paso | 4 |
| Giro en rectángulo, elipse, resaltador | 5 |
| Giro en pixelado y texto | 6 |
| `bounds` del objeto girado (hit-test) | 2 |
| Deshacer/rehacer el giro | 3 |
| No degradar la calidad sin girar | Constraint global + camino `es_nulo()` en cada familia |

**Riesgos anotados:**
- **El centro de giro tiene que ser el MISMO** en el rasterizado y en la caja de selección, o el objeto se desplaza al girar. Por eso la Task 4 obliga a que `bounds_sin_girar` consuma los mismos `caja()` que usa cada `render_girado`, en vez de recalcular la caja por su cuenta.
- **El texto pierde nitidez al girar** y es inevitable con glifos ya rasterizados; el bilineal lo suaviza pero no lo evita. Si molesta, la alternativa es rasterizar el glifo a 4× y reducir, a costa de memoria y tiempo.
- **El hit-test de un objeto girado usa la caja envolvente**, que es más generosa que el objeto (un texto girado 45° tiene una caja bastante mayor que su tinta). Es aceptable para seleccionar; si molesta, el paso siguiente es probar el punto contra el cuadrilátero girado en vez de contra la caja.
