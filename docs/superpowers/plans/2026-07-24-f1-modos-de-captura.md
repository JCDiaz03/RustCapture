# F1 — Modos de captura (D4) + ModeFactory real — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Las tres strategies `CaptureMode` de F1 — pantalla completa (f.9), ventana activa (f.10) y región rectangular (f.13) — más la función factory `capture::create_mode` que convierte un `ModeRequest` en su strategy, cerrando la frontera que el orquestador dejó inyectable.

**Architecture:** Un archivo por modo bajo `capture/modes/` (D4: añadir un modo = añadir un archivo). `ModeRequest` se muda de `orchestrator::events` a `capture` — es vocabulario del dominio de captura; `events` lo re-exporta para que la API del orquestador no cambie. `create_mode` es una función plana cuya firma coincide con `ModeFactory`, así el wiring es `Box::new(capture::create_mode)`.

**Tech Stack:** Rust edition 2024; sin dependencias nuevas.

## Global Constraints

- `rustcapture-core` mantiene cero Win32 y cero UI (D1, D2). Sin dependencias nuevas.
- TDD obligatorio en `core` (skills.md): test primero, implementación después, en cada tarea.
- Tests unitarios inline (`#[cfg(test)] mod tests`); comando: `cargo test -p rustcapture-core`.
- Comentarios y rustdoc en español. `cargo fmt` antes de cada verificación.
- **Commits: SOLO con aprobación humana previa** (skills.md). Un único commit propuesto al final: `v0.1.3 — F1: modos de captura fullscreen, ventana activa y región`.
- f.19 (capturas diminutas): ningún modo impone tamaño mínimo; los límites los pone el `ScreenSource`.
- La ventana activa se recorta al escritorio visible (una ventana puede asomar fuera de pantalla); si no hay intersección o no hay ventana → `CaptureError::NothingToCapture`.

---

### Task 1: `FullscreenMode`

**Files:**
- Create: `crates/core/src/capture/modes/mod.rs`
- Create: `crates/core/src/capture/modes/fullscreen.rs`
- Modify: `crates/core/src/capture/mod.rs` (añadir `pub mod modes;` tras los `use`)

**Interfaces:**
- Consumes: `capture::{CaptureError, CaptureMode}`, `ports::{Frame, ScreenSource}` (ya commiteados).
- Produces: `modes::FullscreenMode` (struct unitario, `impl CaptureMode`). Task 4 lo consume desde la factory.

- [ ] **Step 1: Escribir los tests que fallan**

Crear `crates/core/src/capture/modes/mod.rs`:

```rust
//! Strategies de captura (D4): un archivo por modo. Añadir un modo nuevo
//! es añadir un archivo aquí y un brazo en `capture::create_mode`.

mod fullscreen;

pub use fullscreen::FullscreenMode;
```

Crear `crates/core/src/capture/modes/fullscreen.rs`:

```rust
//! Pantalla completa (f.9): todo el escritorio virtual.

use crate::capture::{CaptureError, CaptureMode};
use crate::ports::{Frame, ScreenSource};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::Rect;
    use crate::ports::mocks::MockScreenSource;

    #[test]
    fn captura_todo_el_escritorio_virtual() {
        // Origen negativo: monitor a la izquierda del primario.
        let pixels: Vec<u8> = (0..4u8).flat_map(|i| [i, 0, 0, 255]).collect();
        let mut source = MockScreenSource::new((-1, -1), Frame::new(2, 2, pixels).unwrap());

        let frame = FullscreenMode.capture(&mut source).unwrap();

        assert_eq!((frame.width, frame.height), (2, 2));
        assert_eq!(frame.pixel(0, 0), Some([0, 0, 0, 255]));
        assert_eq!(frame.pixel(1, 1), Some([3, 0, 0, 255]));
        // Pidió exactamente el rect del escritorio.
        assert_eq!(source.requests(), &[Rect::new(-1, -1, 2, 2)]);
    }
}
```

En `crates/core/src/capture/mod.rs`, tras la línea `use crate::ports::{...};`:

```rust
pub mod modes;
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — `cannot find struct FullscreenMode` (el `pub use` de `modes/mod.rs` no resuelve).

- [ ] **Step 3: Implementar**

En `fullscreen.rs`, entre los `use` y los tests:

```rust
/// Captura el escritorio virtual completo, multi-monitor incluido.
pub struct FullscreenMode;

impl CaptureMode for FullscreenMode {
    fn capture(&self, source: &mut dyn ScreenSource) -> Result<Frame, CaptureError> {
        let rect = source.desktop_rect();
        Ok(source.capture_region(rect)?)
    }
}
```

- [ ] **Step 4: Verificar que pasa**

Run: `cargo fmt && cargo test -p rustcapture-core`
Expected: PASS (35 previos + 1 = 36).

- [ ] **Step 5: Staging**

```bash
git add crates/core/src/capture/
```

---

### Task 2: `ActiveWindowMode`

**Files:**
- Create: `crates/core/src/capture/modes/active_window.rs`
- Modify: `crates/core/src/capture/modes/mod.rs` (añadir `mod active_window; pub use active_window::ActiveWindowMode;`)

**Interfaces:**
- Consumes: `capture::{CaptureError, CaptureMode}`, `ports::{Frame, ScreenSource}`, `Rect::intersection` (slice de puertos).
- Produces: `modes::ActiveWindowMode` (struct unitario, `impl CaptureMode`). Recorta la ventana al escritorio; `NothingToCapture` si no hay ventana o queda fuera.

- [ ] **Step 1: Escribir los tests que fallan**

Crear `crates/core/src/capture/modes/active_window.rs`:

```rust
//! Ventana activa (f.10): captura el rect que reporta el `ScreenSource`,
//! recortado al escritorio visible.

use crate::capture::{CaptureError, CaptureMode};
use crate::ports::{Frame, ScreenSource};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::Rect;
    use crate::ports::mocks::MockScreenSource;

    /// Escritorio 4x4 en (0,0), canal R = índice 0..16.
    fn source_4x4() -> MockScreenSource {
        let pixels: Vec<u8> = (0..16u8).flat_map(|i| [i, 0, 0, 255]).collect();
        MockScreenSource::new((0, 0), Frame::new(4, 4, pixels).unwrap())
    }

    #[test]
    fn captura_el_rect_de_la_ventana_activa() {
        let mut source = source_4x4();
        source.set_active_window(Some(Rect::new(1, 1, 2, 2)));

        let frame = ActiveWindowMode.capture(&mut source).unwrap();

        assert_eq!((frame.width, frame.height), (2, 2));
        // Píxel (0,0) del recorte = escritorio (1,1) = índice 5.
        assert_eq!(frame.pixel(0, 0), Some([5, 0, 0, 255]));
    }

    #[test]
    fn recorta_la_ventana_que_asoma_fuera_del_escritorio() {
        let mut source = source_4x4();
        // Asoma 2 px por la izquierda y 1 por arriba.
        source.set_active_window(Some(Rect::new(-2, -1, 4, 3)));

        let frame = ActiveWindowMode.capture(&mut source).unwrap();

        // Solo la parte visible: (0,0)-(2,2).
        assert_eq!((frame.width, frame.height), (2, 2));
        assert_eq!(source.requests(), &[Rect::new(0, 0, 2, 2)]);
    }

    #[test]
    fn sin_ventana_activa_devuelve_nothing_to_capture() {
        let mut source = source_4x4();
        let err = ActiveWindowMode.capture(&mut source).unwrap_err();
        assert!(matches!(err, CaptureError::NothingToCapture(_)));
    }

    #[test]
    fn ventana_totalmente_fuera_del_escritorio_devuelve_nothing_to_capture() {
        let mut source = source_4x4();
        source.set_active_window(Some(Rect::new(100, 100, 2, 2)));
        let err = ActiveWindowMode.capture(&mut source).unwrap_err();
        assert!(matches!(err, CaptureError::NothingToCapture(_)));
    }
}
```

En `modes/mod.rs`:

```rust
mod active_window;
mod fullscreen;

pub use active_window::ActiveWindowMode;
pub use fullscreen::FullscreenMode;
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — `cannot find struct ActiveWindowMode`.

- [ ] **Step 3: Implementar**

En `active_window.rs`, entre los `use` y los tests:

```rust
/// Captura la ventana activa (f.10), recortada al escritorio visible.
pub struct ActiveWindowMode;

impl CaptureMode for ActiveWindowMode {
    fn capture(&self, source: &mut dyn ScreenSource) -> Result<Frame, CaptureError> {
        let window = source
            .active_window_rect()
            .ok_or_else(|| CaptureError::NothingToCapture("no hay ventana activa".into()))?;
        let visible = source.desktop_rect().intersection(&window).ok_or_else(|| {
            CaptureError::NothingToCapture("la ventana activa está fuera de la pantalla".into())
        })?;
        Ok(source.capture_region(visible)?)
    }
}
```

- [ ] **Step 4: Verificar que pasa**

Run: `cargo fmt && cargo test -p rustcapture-core`
Expected: PASS (36 + 4 = 40).

- [ ] **Step 5: Staging**

```bash
git add crates/core/src/capture/
```

---

### Task 3: `RegionMode`

**Files:**
- Create: `crates/core/src/capture/modes/region.rs`
- Modify: `crates/core/src/capture/modes/mod.rs` (añadir `mod region; pub use region::RegionMode;`)

**Interfaces:**
- Consumes: `capture::{CaptureError, CaptureMode}`, `ports::{Frame, Rect, ScreenSource}`.
- Produces: `modes::RegionMode` con `RegionMode::new(region: Rect) -> RegionMode`, `impl CaptureMode`. Los errores de región inválida vienen del puerto (`Source(OutOfBounds)`), sin duplicar validación.

- [ ] **Step 1: Escribir los tests que fallan**

Crear `crates/core/src/capture/modes/region.rs`:

```rust
//! Región rectangular (f.13). El rect llega elegido por el usuario
//! (overlay, CLI); el modo solo lo ejecuta. Sin tamaño mínimo (f.19).

use crate::capture::{CaptureError, CaptureMode};
use crate::ports::{Frame, Rect, ScreenSource};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::ScreenSourceError;
    use crate::ports::mocks::MockScreenSource;

    fn source_4x4() -> MockScreenSource {
        let pixels: Vec<u8> = (0..16u8).flat_map(|i| [i, 0, 0, 255]).collect();
        MockScreenSource::new((0, 0), Frame::new(4, 4, pixels).unwrap())
    }

    #[test]
    fn captura_exactamente_la_region_pedida() {
        let mut source = source_4x4();
        let frame = RegionMode::new(Rect::new(2, 3, 1, 1)).capture(&mut source).unwrap();
        assert_eq!((frame.width, frame.height), (1, 1));
        // Escritorio (2,3) = índice 14.
        assert_eq!(frame.pixel(0, 0), Some([14, 0, 0, 255]));
    }

    #[test]
    fn una_region_fuera_del_escritorio_propaga_el_error_del_puerto() {
        let mut source = source_4x4();
        let region = Rect::new(3, 3, 5, 5);
        let err = RegionMode::new(region).capture(&mut source).unwrap_err();
        assert_eq!(err, CaptureError::Source(ScreenSourceError::OutOfBounds(region)));
    }
}
```

En `modes/mod.rs`:

```rust
mod active_window;
mod fullscreen;
mod region;

pub use active_window::ActiveWindowMode;
pub use fullscreen::FullscreenMode;
pub use region::RegionMode;
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — `cannot find struct RegionMode`.

- [ ] **Step 3: Implementar**

En `region.rs`, entre los `use` y los tests:

```rust
/// Captura un rect fijo en coordenadas de escritorio virtual.
pub struct RegionMode {
    region: Rect,
}

impl RegionMode {
    pub fn new(region: Rect) -> Self {
        Self { region }
    }
}

impl CaptureMode for RegionMode {
    fn capture(&self, source: &mut dyn ScreenSource) -> Result<Frame, CaptureError> {
        Ok(source.capture_region(self.region)?)
    }
}
```

- [ ] **Step 4: Verificar que pasa**

Run: `cargo fmt && cargo test -p rustcapture-core`
Expected: PASS (40 + 2 = 42).

- [ ] **Step 5: Staging**

```bash
git add crates/core/src/capture/
```

---

### Task 4: `ModeRequest` a `capture` + factory `create_mode`

**Files:**
- Modify: `crates/core/src/capture/mod.rs` (añadir `ModeRequest` y `create_mode`)
- Modify: `crates/core/src/orchestrator/events.rs` (sustituir la definición de `ModeRequest` por un re-export)

**Interfaces:**
- Consumes: los tres modos (Tasks 1-3), `orchestrator::{ModeFactory, Orchestrator}` (solo en tests).
- Produces: `capture::ModeRequest` (mismo enum `{ Fullscreen, ActiveWindow, Region(Rect) }`, mismos derives); `capture::create_mode(request: &ModeRequest) -> Result<Box<dyn CaptureMode>, CaptureError>` — firma compatible con `ModeFactory` vía `Box::new(create_mode)`. `orchestrator::ModeRequest` sigue existiendo como re-export: ningún consumidor externo cambia.

- [ ] **Step 1: Escribir los tests que fallan**

Añadir al módulo de tests de `crates/core/src/capture/mod.rs` (dentro del `mod tests` existente):

```rust
    #[test]
    fn create_mode_region_captura_la_region_pedida() {
        let pixels: Vec<u8> = (0..16u8).flat_map(|i| [i, 0, 0, 255]).collect();
        let mut source = MockScreenSource::new((0, 0), Frame::new(4, 4, pixels).unwrap());
        let mode = create_mode(&ModeRequest::Region(crate::ports::Rect::new(1, 1, 2, 2))).unwrap();
        let frame = mode.capture(&mut source).unwrap();
        assert_eq!((frame.width, frame.height), (2, 2));
        assert_eq!(frame.pixel(0, 0), Some([5, 0, 0, 255]));
    }

    #[test]
    fn create_mode_active_window_sin_ventana_falla_al_capturar() {
        let mut source = MockScreenSource::new((0, 0), Frame::filled(2, 2, [0; 4]));
        let mode = create_mode(&ModeRequest::ActiveWindow).unwrap();
        assert!(matches!(
            mode.capture(&mut source).unwrap_err(),
            CaptureError::NothingToCapture(_)
        ));
    }

    #[test]
    fn el_orquestador_funciona_con_la_factory_real() {
        use crate::orchestrator::{AppEvent, CaptureRequest, Flow, Orchestrator};
        use crate::ports::mocks::MockOutputSink;

        let sink = MockOutputSink::new("clipboard");
        let entregas = sink.delivered_handle();
        let source = MockScreenSource::new((0, 0), Frame::filled(3, 3, [8, 8, 8, 255]));
        let mut orch = Orchestrator::new(Box::new(source), Box::new(create_mode));
        orch.add_sink(Box::new(sink)).unwrap();

        let flow = orch
            .handle_event(AppEvent::CaptureRequested(CaptureRequest {
                mode: ModeRequest::Fullscreen,
                destination: "clipboard",
            }))
            .unwrap();

        assert_eq!(flow, Flow::Continue);
        let frames = entregas.lock().unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!((frames[0].width, frames[0].height), (3, 3));
    }
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — `cannot find function create_mode` / `cannot find type ModeRequest` en `capture`.

- [ ] **Step 3: Implementar la mudanza y la factory**

En `crates/core/src/capture/mod.rs`, tras la definición de `CaptureMode`:

```rust
/// Qué capturar (datos del evento, D7). La factory `create_mode` lo
/// convierte en su strategy. Vive aquí porque es vocabulario del dominio
/// de captura; `orchestrator::events` lo re-exporta.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModeRequest {
    Fullscreen,
    ActiveWindow,
    Region(Rect),
}

/// Factory real de strategies (D4). Firma compatible con
/// `orchestrator::ModeFactory`: el wiring es `Box::new(create_mode)`.
pub fn create_mode(request: &ModeRequest) -> Result<Box<dyn CaptureMode>, CaptureError> {
    Ok(match request {
        ModeRequest::Fullscreen => Box::new(modes::FullscreenMode),
        ModeRequest::ActiveWindow => Box::new(modes::ActiveWindowMode),
        ModeRequest::Region(rect) => Box::new(modes::RegionMode::new(*rect)),
    })
}
```

Ampliar el `use` del módulo para incluir `Rect`:

```rust
use crate::ports::{Frame, Rect, ScreenSource, ScreenSourceError};
```

En `crates/core/src/orchestrator/events.rs`, eliminar el enum `ModeRequest` y su doc-comment, y sustituir por un re-export (el `use` de `Rect` deja de hacer falta):

```rust
pub use crate::capture::ModeRequest;
```

quedando la cabecera del archivo:

```rust
use crate::ports::HotkeyId;

pub use crate::capture::ModeRequest;
```

- [ ] **Step 4: Verificar que pasa**

Run: `cargo fmt && cargo test -p rustcapture-core`
Expected: PASS (42 + 3 = 45; los tests del orquestador siguen en verde — el re-export mantiene la ruta `orchestrator::ModeRequest`).

- [ ] **Step 5: Staging**

```bash
git add crates/core/src/capture/ crates/core/src/orchestrator/
```

---

### Task 5: Verificación final del slice y cierre

**Files:**
- Modify: `roadmap.md` (marcar ✅ el ítem de modos, solo tras verificar)

**Interfaces:**
- Consumes: todo lo anterior.
- Produces: slice verificado; propuesta de commit al humano.

- [ ] **Step 1: Verificación completa (skill `verification-before-completion`)**

```bash
cargo build --workspace
cargo test -p rustcapture-core
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

Expected: build limpio, 45 tests PASS, clippy sin warnings, formato correcto.

- [ ] **Step 2: Revisión de contrato**

Confirmar: `capture` exporta `CaptureMode`, `CaptureError`, `ModeRequest`, `create_mode` y `modes::{FullscreenMode, ActiveWindowMode, RegionMode}`; `orchestrator::ModeRequest` sigue resolviendo (re-export); sin dependencias nuevas ni ciclos (`capture` NO importa `orchestrator` fuera de tests).

- [ ] **Step 3: Actualizar roadmap**

En `roadmap.md` §2, cambiar:

```
- ⏳ Modos: pantalla completa, ventana activa, región rectangular (f.9, f.10, f.13) como strategies `CaptureMode` (D4).
```

por:

```
- ✅ Modos: pantalla completa, ventana activa, región rectangular (f.9, f.10, f.13) como strategies `CaptureMode` (D4).
```

- [ ] **Step 4: Proponer el commit al humano (NO ejecutar sin aprobación)**

Mensaje propuesto:

```
v0.1.3 — F1: modos de captura fullscreen, ventana activa y región

Strategies CaptureMode (D4) en capture/modes/ — un archivo por modo — y
factory create_mode compatible con ModeFactory; ModeRequest se muda a
capture y orchestrator::events lo re-exporta sin romper la API.
```
