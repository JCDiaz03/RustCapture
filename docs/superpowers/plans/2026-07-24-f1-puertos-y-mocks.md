# F1 — Puertos y mocks (D2) — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Definir en `rustcapture-core` los puertos `ScreenSource`, `OutputSink` y `HotkeyProvider` con sus tipos de frontera (`Rect`, `Frame`, errores) y un mock de test por puerto, dejando la base sobre la que se construyen los modos de captura (D4), el orquestador (D7) y los adapters de `platform-win`.

**Architecture:** Los puertos viven en `core/src/ports/` (un archivo por puerto + tipos compartidos + mocks), como traits puros sin rastro de Win32 (D2). Los mocks son públicos (`ports::mocks`) para que también los usen los tests de `cli`/`gui`. `VideoEncoder` NO se define aquí: pertenece a F5 (D8).

**Tech Stack:** Rust edition 2024, workspace resolver 3. Única dependencia nueva: `thiserror` 2 (solo derive de errores, sin coste en runtime).

## Global Constraints

- `rustcapture-core` mantiene cero Win32 y cero UI (D1, D2). Prohibido `windows-rs` aquí.
- El paquete se llama `rustcapture-core` (el directorio es `crates/core`); no renombrar.
- TDD obligatorio en `core` (skills.md): test primero, implementación después, en cada tarea.
- Tests unitarios inline (`#[cfg(test)] mod tests`) al final de cada archivo, convención Rust.
- Comentarios y rustdoc en español, como el código existente.
- Comando de test: `cargo test -p rustcapture-core`.
- **Commits: SOLO con aprobación humana previa** (skills.md). Formato `vX.Y.Z — <nombre>`. Este plan propone UN commit al final del slice; los pasos de commit de cada tarea son `git add` de staging, sin `git commit`.
- Coordenadas de escritorio en `i32` (el escritorio virtual multi-monitor puede tener origen negativo); dimensiones en `u32`.
- Píxeles siempre RGBA8 (D5: `Canvas` envuelve frame RGBA); los adapters convierten BGRA→RGBA en `platform-win`.

---

### Task 1: Módulo `ports/` y geometría (`Rect`)

**Files:**
- Delete: `crates/core/src/ports.rs`
- Create: `crates/core/src/ports/mod.rs`
- Create: `crates/core/src/ports/geometry.rs`

**Interfaces:**
- Consumes: nada (primer eslabón).
- Produces: `Rect { x: i32, y: i32, width: u32, height: u32 }` con `new`, `right() -> i64`, `bottom() -> i64`, `is_empty() -> bool`, `contains(&Rect) -> bool`, `intersection(&Rect) -> Option<Rect>`. Tareas 2-5 lo consumen.

- [ ] **Step 1: Sustituir `ports.rs` por el directorio con el test que falla**

Borrar `crates/core/src/ports.rs`. Crear `crates/core/src/ports/mod.rs`:

```rust
//! Puertos (D2): traits en las fronteras reales del dominio.
//!
//! Aquí viven `ScreenSource`, `OutputSink` y `HotkeyProvider` con sus
//! tipos de frontera y mocks de test. `VideoEncoder` se define en F5 (D8).

mod geometry;

pub use geometry::Rect;
```

Crear `crates/core/src/ports/geometry.rs` solo con los tests (la impl aún no existe):

```rust
//! Geometría de frontera: rectángulos en coordenadas de escritorio virtual.
//! El origen puede ser negativo (monitor a la izquierda del primario).

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
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — `cannot find struct Rect` (error de compilación cuenta como test rojo).

- [ ] **Step 3: Implementar `Rect`**

Añadir encima de los tests en `geometry.rs`:

```rust
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
        Self { x, y, width, height }
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
        Some(Rect::new(x, y, (right - x as i64) as u32, (bottom - y as i64) as u32))
    }
}
```

- [ ] **Step 4: Verificar que pasa**

Run: `cargo test -p rustcapture-core`
Expected: PASS (5 tests de geometría en verde).

- [ ] **Step 5: Staging**

```bash
git add crates/core/src/ports.rs crates/core/src/ports/
```

---

### Task 2: `Frame` RGBA con recorte

**Files:**
- Create: `crates/core/src/ports/frame.rs`
- Modify: `crates/core/src/ports/mod.rs` (añadir `mod frame; pub use frame::{Frame, FrameError};`)
- Modify: `Cargo.toml` (workspace) y `crates/core/Cargo.toml` (añadir `thiserror`)

**Interfaces:**
- Consumes: `Rect` (Task 1).
- Produces: `Frame { width: u32, height: u32, pixels: Vec<u8> }` (RGBA8) con `new(u32, u32, Vec<u8>) -> Result<Frame, FrameError>`, `filled(u32, u32, [u8; 4]) -> Frame`, `pixel(u32, u32) -> Option<[u8; 4]>`, `crop(&Rect) -> Result<Frame, FrameError>` (coordenadas locales al frame). `FrameError { SizeMismatch { expected: usize, got: usize }, OutOfBounds(Rect) }`.

- [ ] **Step 1: Declarar dependencia `thiserror`**

En el `Cargo.toml` del workspace, bajo `[workspace.dependencies]`:

```toml
thiserror = "2"
```

En `crates/core/Cargo.toml`, bajo `[dependencies]`:

```toml
thiserror = { workspace = true }
```

- [ ] **Step 2: Escribir los tests que fallan**

Crear `crates/core/src/ports/frame.rs` con:

```rust
//! Frame RGBA8: la unidad de píxeles que cruza todos los puertos (D4, D5).

use super::Rect;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_valida_la_longitud_del_buffer() {
        assert!(Frame::new(2, 2, vec![0; 16]).is_ok());
        let err = Frame::new(2, 2, vec![0; 15]).unwrap_err();
        assert_eq!(err, FrameError::SizeMismatch { expected: 16, got: 15 });
    }

    #[test]
    fn filled_crea_frame_uniforme_y_pixel_lo_lee() {
        let f = Frame::filled(3, 2, [10, 20, 30, 255]);
        assert_eq!(f.pixel(2, 1), Some([10, 20, 30, 255]));
        assert_eq!(f.pixel(3, 0), None); // fuera de rango
    }

    #[test]
    fn crop_extrae_la_subregion_correcta() {
        // Frame 4x1: píxeles distinguibles por su canal R = columna.
        let pixels: Vec<u8> = (0..4u8).flat_map(|c| [c, 0, 0, 255]).collect();
        let f = Frame::new(4, 1, pixels).unwrap();
        let sub = f.crop(&Rect::new(1, 0, 2, 1)).unwrap();
        assert_eq!((sub.width, sub.height), (2, 1));
        assert_eq!(sub.pixel(0, 0), Some([1, 0, 0, 255]));
        assert_eq!(sub.pixel(1, 0), Some([2, 0, 0, 255]));
    }

    #[test]
    fn crop_fuera_de_limites_falla() {
        let f = Frame::filled(4, 4, [0; 4]);
        let region = Rect::new(2, 2, 4, 4);
        assert_eq!(f.crop(&region).unwrap_err(), FrameError::OutOfBounds(region));
        let negativa = Rect::new(-1, 0, 2, 2);
        assert_eq!(f.crop(&negativa).unwrap_err(), FrameError::OutOfBounds(negativa));
    }
}
```

En `ports/mod.rs` añadir:

```rust
mod frame;

pub use frame::{Frame, FrameError};
```

- [ ] **Step 3: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — `cannot find struct Frame`.

- [ ] **Step 4: Implementar `Frame`**

Encima de los tests en `frame.rs`:

```rust
/// Imagen RGBA8 en memoria. `pixels.len() == width * height * 4`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(thiserror::Error, Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameError {
    #[error("buffer de {got} bytes; se esperaban {expected}")]
    SizeMismatch { expected: usize, got: usize },
    #[error("la región {0:?} se sale del frame")]
    OutOfBounds(Rect),
}

impl Frame {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Result<Self, FrameError> {
        let expected = width as usize * height as usize * 4;
        if pixels.len() != expected {
            return Err(FrameError::SizeMismatch { expected, got: pixels.len() });
        }
        Ok(Self { width, height, pixels })
    }

    /// Frame uniforme del color dado; útil sobre todo en tests y mocks.
    pub fn filled(width: u32, height: u32, rgba: [u8; 4]) -> Self {
        let pixels = rgba.repeat(width as usize * height as usize);
        Self { width, height, pixels }
    }

    /// Píxel en coordenadas locales, `None` fuera de rango.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        Some([self.pixels[i], self.pixels[i + 1], self.pixels[i + 2], self.pixels[i + 3]])
    }

    /// Copia la subregión `region` (coordenadas locales al frame, origen 0,0).
    pub fn crop(&self, region: &Rect) -> Result<Frame, FrameError> {
        let propio = Rect::new(0, 0, self.width, self.height);
        if region.is_empty() || !propio.contains(region) {
            return Err(FrameError::OutOfBounds(*region));
        }
        let mut pixels = Vec::with_capacity(region.width as usize * region.height as usize * 4);
        for fila in 0..region.height {
            let y = (region.y as u32 + fila) as usize;
            let inicio = (y * self.width as usize + region.x as usize) * 4;
            let fin = inicio + region.width as usize * 4;
            pixels.extend_from_slice(&self.pixels[inicio..fin]);
        }
        Ok(Frame { width: region.width, height: region.height, pixels })
    }
}
```

- [ ] **Step 5: Verificar que pasa**

Run: `cargo test -p rustcapture-core`
Expected: PASS (tests de Task 1 + 4 tests de frame).

- [ ] **Step 6: Staging**

```bash
git add Cargo.toml Cargo.lock crates/core/Cargo.toml crates/core/src/ports/
```

---

### Task 3: Puerto `ScreenSource` + `MockScreenSource`

**Files:**
- Create: `crates/core/src/ports/screen_source.rs`
- Create: `crates/core/src/ports/mocks.rs`
- Modify: `crates/core/src/ports/mod.rs` (añadir `mod screen_source; pub mod mocks; pub use screen_source::{ScreenSource, ScreenSourceError};`)

**Interfaces:**
- Consumes: `Rect`, `Frame` (Tasks 1-2).
- Produces: trait `ScreenSource { desktop_rect(&self) -> Rect; active_window_rect(&self) -> Option<Rect>; capture_region(&mut self, region: Rect) -> Result<Frame, ScreenSourceError> }`; `ScreenSourceError { OutOfBounds(Rect), Platform(String) }`; `mocks::MockScreenSource` con `new(origin: (i32, i32), base: Frame)`, `set_active_window(Option<Rect>)`, `fail_next(ScreenSourceError)`, `requests() -> &[Rect]`. Lo consumen las strategies `CaptureMode` (D4) y el orquestador (D7).

- [ ] **Step 1: Escribir los tests que fallan**

Crear `crates/core/src/ports/mocks.rs`:

```rust
//! Mocks de los puertos para tests de `core`, `cli` y `gui`.
//! Públicos a propósito: son parte del contrato de test del workspace (D2).

use super::{Frame, Rect, ScreenSource, ScreenSourceError};

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_2x2() -> MockScreenSource {
        // Canal R = índice del píxel (0..4) para distinguirlos.
        let pixels: Vec<u8> = (0..4u8).flat_map(|i| [i, 0, 0, 255]).collect();
        MockScreenSource::new((-1, -1), Frame::new(2, 2, pixels).unwrap())
    }

    #[test]
    fn desktop_rect_refleja_origen_y_tamano_del_frame_base() {
        let m = mock_2x2();
        assert_eq!(m.desktop_rect(), Rect::new(-1, -1, 2, 2));
    }

    #[test]
    fn capture_region_traduce_coordenadas_de_escritorio_a_frame() {
        let mut m = mock_2x2();
        // El píxel de escritorio (0, 0) es el local (1, 1) → índice 3.
        let f = m.capture_region(Rect::new(0, 0, 1, 1)).unwrap();
        assert_eq!(f.pixel(0, 0), Some([3, 0, 0, 255]));
    }

    #[test]
    fn capture_fuera_del_escritorio_devuelve_out_of_bounds() {
        let mut m = mock_2x2();
        let region = Rect::new(5, 5, 2, 2);
        assert_eq!(
            m.capture_region(region).unwrap_err(),
            ScreenSourceError::OutOfBounds(region)
        );
    }

    #[test]
    fn fail_next_inyecta_el_error_una_sola_vez() {
        let mut m = mock_2x2();
        m.fail_next(ScreenSourceError::Platform("GDI caído".into()));
        assert!(m.capture_region(Rect::new(-1, -1, 1, 1)).is_err());
        assert!(m.capture_region(Rect::new(-1, -1, 1, 1)).is_ok());
    }

    #[test]
    fn registra_las_regiones_solicitadas() {
        let mut m = mock_2x2();
        let r = Rect::new(-1, -1, 2, 2);
        let _ = m.capture_region(r);
        assert_eq!(m.requests(), &[r]);
    }

    #[test]
    fn active_window_es_configurable() {
        let mut m = mock_2x2();
        assert_eq!(m.active_window_rect(), None);
        m.set_active_window(Some(Rect::new(0, 0, 1, 1)));
        assert_eq!(m.active_window_rect(), Some(Rect::new(0, 0, 1, 1)));
    }
}
```

En `ports/mod.rs` añadir:

```rust
mod screen_source;

pub mod mocks;

pub use screen_source::{ScreenSource, ScreenSourceError};
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — `cannot find trait ScreenSource` / `MockScreenSource`.

- [ ] **Step 3: Implementar trait y mock**

Crear `crates/core/src/ports/screen_source.rs`:

```rust
//! Puerto de origen de píxeles (D2): lo implementan GDI/WGC en
//! `platform-win` y `MockScreenSource` en tests.

use super::{Frame, Rect};

#[derive(thiserror::Error, Clone, PartialEq, Eq, Debug)]
pub enum ScreenSourceError {
    #[error("la región {0:?} está fuera del escritorio")]
    OutOfBounds(Rect),
    /// Fallo del adapter (HRESULT, dispositivo perdido...). El texto ya viene
    /// formateado desde `platform-win`; `core` no conoce Win32.
    #[error("fallo de plataforma: {0}")]
    Platform(String),
}

pub trait ScreenSource {
    /// Rect del escritorio virtual completo (multi-monitor: el origen
    /// puede ser negativo).
    fn desktop_rect(&self) -> Rect;

    /// Rect de la ventana activa, si hay alguna (f.10). Vive aquí y no en
    /// un puerto propio mientras sea la única consulta de ventanas (D2:
    /// puertos solo en fronteras reales); si crece, se extrae.
    fn active_window_rect(&self) -> Option<Rect>;

    /// Captura `region` en coordenadas de escritorio virtual.
    fn capture_region(&mut self, region: Rect) -> Result<Frame, ScreenSourceError>;
}
```

En `mocks.rs`, encima de los tests:

```rust
/// `ScreenSource` respaldado por un frame en memoria.
pub struct MockScreenSource {
    origin: (i32, i32),
    base: Frame,
    active_window: Option<Rect>,
    next_error: Option<ScreenSourceError>,
    requests: Vec<Rect>,
}

impl MockScreenSource {
    /// `origin` es la esquina del escritorio virtual que representa `base`.
    pub fn new(origin: (i32, i32), base: Frame) -> Self {
        Self { origin, base, active_window: None, next_error: None, requests: Vec::new() }
    }

    pub fn set_active_window(&mut self, rect: Option<Rect>) {
        self.active_window = rect;
    }

    /// La siguiente llamada a `capture_region` devolverá este error.
    pub fn fail_next(&mut self, error: ScreenSourceError) {
        self.next_error = Some(error);
    }

    /// Regiones solicitadas, en orden.
    pub fn requests(&self) -> &[Rect] {
        &self.requests
    }
}

impl ScreenSource for MockScreenSource {
    fn desktop_rect(&self) -> Rect {
        Rect::new(self.origin.0, self.origin.1, self.base.width, self.base.height)
    }

    fn active_window_rect(&self) -> Option<Rect> {
        self.active_window
    }

    fn capture_region(&mut self, region: Rect) -> Result<Frame, ScreenSourceError> {
        self.requests.push(region);
        if let Some(err) = self.next_error.take() {
            return Err(err);
        }
        if !self.desktop_rect().contains(&region) || region.is_empty() {
            return Err(ScreenSourceError::OutOfBounds(region));
        }
        let local = Rect::new(region.x - self.origin.0, region.y - self.origin.1, region.width, region.height);
        self.base.crop(&local).map_err(|_| ScreenSourceError::OutOfBounds(region))
    }
}
```

- [ ] **Step 4: Verificar que pasa**

Run: `cargo test -p rustcapture-core`
Expected: PASS (todos los anteriores + 6 tests nuevos).

- [ ] **Step 5: Staging**

```bash
git add crates/core/src/ports/
```

---

### Task 4: Puerto `OutputSink` + `MockOutputSink`

**Files:**
- Create: `crates/core/src/ports/output_sink.rs`
- Modify: `crates/core/src/ports/mod.rs` (añadir `mod output_sink; pub use output_sink::{OutputSink, OutputError};`)
- Modify: `crates/core/src/ports/mocks.rs` (añadir `MockOutputSink`)

**Interfaces:**
- Consumes: `Frame` (Task 2).
- Produces: trait `OutputSink { id(&self) -> &'static str; deliver(&mut self, frame: &Frame) -> Result<(), OutputError> }`; `OutputError { Unavailable(String), Failed(String) }`; `mocks::MockOutputSink` con `new(id: &'static str)`, `fail_next(OutputError)`, `delivered() -> &[Frame]`. Lo consumen el slice `output` (f.40, f.41) y el orquestador (D7).

- [ ] **Step 1: Escribir los tests que fallan**

Añadir al módulo de tests de `mocks.rs`:

```rust
    #[test]
    fn el_sink_registra_los_frames_entregados() {
        let mut sink = MockOutputSink::new("clipboard");
        assert_eq!(sink.id(), "clipboard");
        sink.deliver(&Frame::filled(1, 1, [1, 2, 3, 255])).unwrap();
        assert_eq!(sink.delivered().len(), 1);
        assert_eq!(sink.delivered()[0].pixel(0, 0), Some([1, 2, 3, 255]));
    }

    #[test]
    fn fail_next_del_sink_falla_una_sola_vez_y_no_registra() {
        let mut sink = MockOutputSink::new("file");
        sink.fail_next(OutputError::Failed("disco lleno".into()));
        assert!(sink.deliver(&Frame::filled(1, 1, [0; 4])).is_err());
        assert!(sink.deliver(&Frame::filled(1, 1, [0; 4])).is_ok());
        assert_eq!(sink.delivered().len(), 1);
    }
```

Y en la cabecera de imports de `mocks.rs`, ampliar el `use`:

```rust
use super::{Frame, OutputError, OutputSink, Rect, ScreenSource, ScreenSourceError};
```

En `ports/mod.rs` añadir:

```rust
mod output_sink;

pub use output_sink::{OutputError, OutputSink};
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — `cannot find trait OutputSink`.

- [ ] **Step 3: Implementar trait y mock**

Crear `crates/core/src/ports/output_sink.rs`:

```rust
//! Puerto de salida (D2): portapapeles, archivo, impresora, email...
//! El sink recibe el frame final ya compuesto; codificación de formato y
//! nombres automáticos (f.41, f.45) son responsabilidad del slice `output`.

use super::Frame;

#[derive(thiserror::Error, Clone, PartialEq, Eq, Debug)]
pub enum OutputError {
    /// El destino no está disponible (sin impresora, sin cliente de email...).
    #[error("destino no disponible: {0}")]
    Unavailable(String),
    /// La entrega empezó y falló (disco lleno, portapapeles bloqueado...).
    #[error("entrega fallida: {0}")]
    Failed(String),
}

pub trait OutputSink {
    /// Identificador estable ("clipboard", "file"...) para config y logs.
    fn id(&self) -> &'static str;

    /// Entrega el frame al destino.
    fn deliver(&mut self, frame: &Frame) -> Result<(), OutputError>;
}
```

Añadir a `mocks.rs`:

```rust
/// `OutputSink` que acumula lo entregado en memoria.
pub struct MockOutputSink {
    id: &'static str,
    delivered: Vec<Frame>,
    next_error: Option<OutputError>,
}

impl MockOutputSink {
    pub fn new(id: &'static str) -> Self {
        Self { id, delivered: Vec::new(), next_error: None }
    }

    /// La siguiente llamada a `deliver` devolverá este error.
    pub fn fail_next(&mut self, error: OutputError) {
        self.next_error = Some(error);
    }

    /// Frames entregados con éxito, en orden.
    pub fn delivered(&self) -> &[Frame] {
        &self.delivered
    }
}

impl OutputSink for MockOutputSink {
    fn id(&self) -> &'static str {
        self.id
    }

    fn deliver(&mut self, frame: &Frame) -> Result<(), OutputError> {
        if let Some(err) = self.next_error.take() {
            return Err(err);
        }
        self.delivered.push(frame.clone());
        Ok(())
    }
}
```

- [ ] **Step 4: Verificar que pasa**

Run: `cargo test -p rustcapture-core`
Expected: PASS (todos los anteriores + 2 tests nuevos).

- [ ] **Step 5: Staging**

```bash
git add crates/core/src/ports/
```

---

### Task 5: Puerto `HotkeyProvider` + tipos de tecla + `MockHotkeyProvider`

**Files:**
- Create: `crates/core/src/ports/hotkeys.rs`
- Modify: `crates/core/src/ports/mod.rs` (añadir `mod hotkeys; pub use hotkeys::{Hotkey, HotkeyError, HotkeyId, HotkeyProvider, KeyCode, Modifiers};`)
- Modify: `crates/core/src/ports/mocks.rs` (añadir `MockHotkeyProvider`)

**Interfaces:**
- Consumes: nada de las tareas previas (puerto independiente).
- Produces: `Modifiers { ctrl, alt, shift, win: bool }`, `KeyCode { Char(char), F(u8), PrintScreen }`, `Hotkey { modifiers: Modifiers, key: KeyCode }`, `HotkeyId(u32)`; trait `HotkeyProvider { register(&mut self, hotkey: Hotkey) -> Result<HotkeyId, HotkeyError>; unregister(&mut self, id: HotkeyId) -> Result<(), HotkeyError> }`; `HotkeyError { AlreadyRegistered(Hotkey), UnknownId(HotkeyId), Platform(String) }`; `mocks::MockHotkeyProvider` con `new()`, `registered() -> Vec<(HotkeyId, Hotkey)>`. La entrega de pulsaciones NO es parte del trait: el adapter publica en el canal mpsc con el que se construye (D7, slice del orquestador).

- [ ] **Step 1: Escribir los tests que fallan**

Añadir al módulo de tests de `mocks.rs`:

```rust
    fn ctrl_shift(c: char) -> Hotkey {
        Hotkey {
            modifiers: Modifiers { ctrl: true, shift: true, ..Modifiers::default() },
            key: KeyCode::Char(c),
        }
    }

    #[test]
    fn register_asigna_ids_distintos_y_los_recuerda() {
        let mut hk = MockHotkeyProvider::new();
        let a = hk.register(ctrl_shift('a')).unwrap();
        let b = hk.register(ctrl_shift('b')).unwrap();
        assert_ne!(a, b);
        assert_eq!(hk.registered(), vec![(a, ctrl_shift('a')), (b, ctrl_shift('b'))]);
    }

    #[test]
    fn registrar_el_mismo_atajo_dos_veces_falla() {
        let mut hk = MockHotkeyProvider::new();
        hk.register(ctrl_shift('a')).unwrap();
        assert_eq!(
            hk.register(ctrl_shift('a')).unwrap_err(),
            HotkeyError::AlreadyRegistered(ctrl_shift('a'))
        );
    }

    #[test]
    fn unregister_libera_el_atajo_para_reuso() {
        let mut hk = MockHotkeyProvider::new();
        let id = hk.register(ctrl_shift('a')).unwrap();
        hk.unregister(id).unwrap();
        assert!(hk.registered().is_empty());
        assert!(hk.register(ctrl_shift('a')).is_ok());
    }

    #[test]
    fn unregister_con_id_desconocido_falla() {
        let mut hk = MockHotkeyProvider::new();
        assert_eq!(
            hk.unregister(HotkeyId(99)).unwrap_err(),
            HotkeyError::UnknownId(HotkeyId(99))
        );
    }
```

Ampliar el `use` de la cabecera de `mocks.rs`:

```rust
use super::{
    Frame, Hotkey, HotkeyError, HotkeyId, HotkeyProvider, KeyCode, Modifiers, OutputError,
    OutputSink, Rect, ScreenSource, ScreenSourceError,
};
```

En `ports/mod.rs` añadir:

```rust
mod hotkeys;

pub use hotkeys::{Hotkey, HotkeyError, HotkeyId, HotkeyProvider, KeyCode, Modifiers};
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — `cannot find trait HotkeyProvider`.

- [ ] **Step 3: Implementar tipos, trait y mock**

Crear `crates/core/src/ports/hotkeys.rs`:

```rust
//! Puerto de atajos globales (D2, f.3). El trait cubre solo el registro;
//! las pulsaciones llegan como eventos por el canal mpsc del orquestador
//! (D7), con el que se construye cada adapter.

/// Teclas modificadoras. `win` es la tecla Windows.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

/// Tecla principal del atajo, independiente de códigos VK de Win32;
/// el adapter hace el mapeo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyCode {
    /// Letra o dígito, en minúscula ('a'..'z', '0'..'9').
    Char(char),
    /// Tecla de función F1..F24.
    F(u8),
    PrintScreen,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Hotkey {
    pub modifiers: Modifiers,
    pub key: KeyCode,
}

/// Identificador opaco que asigna el provider al registrar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HotkeyId(pub u32);

#[derive(thiserror::Error, Clone, PartialEq, Eq, Debug)]
pub enum HotkeyError {
    #[error("el atajo {0:?} ya está registrado")]
    AlreadyRegistered(Hotkey),
    #[error("id de atajo desconocido: {0:?}")]
    UnknownId(HotkeyId),
    /// `RegisterHotKey` falló (atajo tomado por otra app, etc.).
    #[error("fallo de plataforma: {0}")]
    Platform(String),
}

pub trait HotkeyProvider {
    fn register(&mut self, hotkey: Hotkey) -> Result<HotkeyId, HotkeyError>;
    fn unregister(&mut self, id: HotkeyId) -> Result<(), HotkeyError>;
}
```

Añadir a `mocks.rs`:

```rust
/// `HotkeyProvider` en memoria con ids incrementales.
pub struct MockHotkeyProvider {
    next_id: u32,
    registered: Vec<(HotkeyId, Hotkey)>,
}

impl MockHotkeyProvider {
    pub fn new() -> Self {
        Self { next_id: 0, registered: Vec::new() }
    }

    /// Atajos vivos (registrados y no liberados), en orden de registro.
    pub fn registered(&self) -> Vec<(HotkeyId, Hotkey)> {
        self.registered.clone()
    }
}

impl Default for MockHotkeyProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl HotkeyProvider for MockHotkeyProvider {
    fn register(&mut self, hotkey: Hotkey) -> Result<HotkeyId, HotkeyError> {
        if self.registered.iter().any(|(_, h)| *h == hotkey) {
            return Err(HotkeyError::AlreadyRegistered(hotkey));
        }
        let id = HotkeyId(self.next_id);
        self.next_id += 1;
        self.registered.push((id, hotkey));
        Ok(id)
    }

    fn unregister(&mut self, id: HotkeyId) -> Result<(), HotkeyError> {
        let pos = self
            .registered
            .iter()
            .position(|(i, _)| *i == id)
            .ok_or(HotkeyError::UnknownId(id))?;
        self.registered.remove(pos);
        Ok(())
    }
}
```

- [ ] **Step 4: Verificar que pasa**

Run: `cargo test -p rustcapture-core`
Expected: PASS (todos los anteriores + 4 tests nuevos).

- [ ] **Step 5: Staging**

```bash
git add crates/core/src/ports/
```

---

### Task 6: Verificación final del slice y cierre

**Files:**
- Modify: `roadmap.md` (marcar ✅ el ítem de puertos de F1, solo tras verificar)

**Interfaces:**
- Consumes: todo lo anterior.
- Produces: slice verificado; propuesta de commit al humano.

- [ ] **Step 1: Verificación completa (skill `verification-before-completion`)**

```bash
cargo build --workspace
cargo test -p rustcapture-core
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

Expected: build limpio, todos los tests PASS, clippy sin warnings, formato correcto. Si `clippy` o `fmt` no están instalados, anotarlo y no bloquear (son deseables, no requisito del slice).

- [ ] **Step 2: Revisión de contrato**

Confirmar que `ports/mod.rs` re-exporta exactamente: `Rect`, `Frame`, `FrameError`, `ScreenSource`, `ScreenSourceError`, `OutputSink`, `OutputError`, `Hotkey`, `HotkeyError`, `HotkeyId`, `HotkeyProvider`, `KeyCode`, `Modifiers`, y el módulo público `mocks`. Ni `windows`, ni `winapi`, ni dependencia alguna salvo `thiserror` en `crates/core/Cargo.toml`.

- [ ] **Step 3: Actualizar roadmap**

En `roadmap.md` §2, cambiar:

```
- ⏳ Puertos `ScreenSource`, `OutputSink`, `HotkeyProvider` + mocks de test (D2).
```

por:

```
- ✅ Puertos `ScreenSource`, `OutputSink`, `HotkeyProvider` + mocks de test (D2).
```

- [ ] **Step 4: Proponer el commit al humano (NO ejecutar sin aprobación)**

Mensaje propuesto:

```
v0.1.1 — F1: puertos ScreenSource, OutputSink y HotkeyProvider

Traits de frontera (D2) con tipos Rect/Frame RGBA, errores por puerto
(thiserror) y mocks públicos en ports::mocks para tests del workspace.
```
