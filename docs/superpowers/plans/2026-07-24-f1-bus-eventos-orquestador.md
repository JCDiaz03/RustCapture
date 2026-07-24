# F1 — Bus de eventos mpsc + orquestador (D7) — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Un orquestador en `rustcapture-core` que consume eventos (`AppEvent`) de un canal mpsc — peticiones de captura, pulsaciones de hotkey, shutdown — y ejecuta el pipeline capturar → entregar al sink, de forma que hotkeys, barra y CLI sean solo productores del mismo canal (D7).

**Architecture:** El evento transporta datos (`ModeRequest::{Fullscreen, ActiveWindow, Region(Rect)}`), no comportamiento; el orquestador los convierte en estrategias `CaptureMode` (D4) mediante una *mode factory* inyectada — este slice define el trait y la factory como frontera, y el slice de modos (D4) aportará la factory real. Los sinks se registran por su `id()`; los hotkeys se resuelven con un mapa `HotkeyId → CaptureRequest` interno. `handle_event` es síncrono y testeable; `run` es el bucle fino sobre el `Receiver` con un observer para notificar resultados (futuros toasts de la GUI).

**Tech Stack:** Rust edition 2024, `std::sync::mpsc` (sin dependencias nuevas), `thiserror` ya presente.

## Global Constraints

- `rustcapture-core` mantiene cero Win32 y cero UI (D1, D2). Sin dependencias nuevas en este slice.
- TDD obligatorio en `core` (skills.md): test primero, implementación después, en cada tarea.
- Tests unitarios inline (`#[cfg(test)] mod tests`); comando: `cargo test -p rustcapture-core`.
- Comentarios y rustdoc en español.
- Formato: `cargo fmt` antes de cada verificación (rustfmt por defecto expande literales de struct multi-campo).
- **Commits: SOLO con aprobación humana previa** (skills.md). Un único commit propuesto al final: `v0.1.2 — F1: bus de eventos y orquestador`.
- Nombres del evento según D7: `CaptureRequested { mode, destination }` → aquí `CaptureRequest { mode: ModeRequest, destination: &'static str }`.
- Los ids de sink son `&'static str` (la moneda ya usada por `OutputSink::id()`); CLI/config mapean strings de usuario a estos ids conocidos.

---

### Task 1: `CaptureMode` + `CaptureError` en el slice `capture`

**Files:**
- Modify: `crates/core/src/capture/mod.rs`

**Interfaces:**
- Consumes: `crate::ports::{Frame, ScreenSource, ScreenSourceError}` (slice de puertos ya commiteado).
- Produces: trait `CaptureMode { capture(&self, source: &mut dyn ScreenSource) -> Result<Frame, CaptureError> }` y `CaptureError { Source(ScreenSourceError) [#from], NothingToCapture(String) }`. Los consumen la mode factory (Task 3) y las strategies del slice D4.

- [ ] **Step 1: Escribir el test que falla**

Añadir al final de `crates/core/src/capture/mod.rs` (hoy solo tiene el doc-comment; conservarlo):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::mocks::MockScreenSource;
    use crate::ports::{Frame, ScreenSource, ScreenSourceError};

    /// Estrategia mínima para validar el contrato del trait.
    struct DesktopMode;

    impl CaptureMode for DesktopMode {
        fn capture(&self, source: &mut dyn ScreenSource) -> Result<Frame, CaptureError> {
            let rect = source.desktop_rect();
            // `?` prueba la conversión From<ScreenSourceError>.
            Ok(source.capture_region(rect)?)
        }
    }

    #[test]
    fn una_estrategia_captura_a_traves_del_puerto() {
        let mut source = MockScreenSource::new((0, 0), Frame::filled(2, 2, [9, 9, 9, 255]));
        let frame = DesktopMode.capture(&mut source).unwrap();
        assert_eq!((frame.width, frame.height), (2, 2));
        assert_eq!(frame.pixel(1, 1), Some([9, 9, 9, 255]));
    }

    #[test]
    fn los_errores_del_puerto_se_convierten_a_capture_error() {
        let mut source = MockScreenSource::new((0, 0), Frame::filled(1, 1, [0; 4]));
        source.fail_next(ScreenSourceError::Platform("GDI caído".into()));
        let err = DesktopMode.capture(&mut source).unwrap_err();
        assert_eq!(
            err,
            CaptureError::Source(ScreenSourceError::Platform("GDI caído".into()))
        );
    }
}
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — `cannot find trait CaptureMode`.

- [ ] **Step 3: Implementar trait y error**

Entre el doc-comment y los tests de `capture/mod.rs`:

```rust
use crate::ports::{Frame, ScreenSource, ScreenSourceError};

#[derive(thiserror::Error, Clone, PartialEq, Eq, Debug)]
pub enum CaptureError {
    #[error(transparent)]
    Source(#[from] ScreenSourceError),
    /// El modo no tiene nada que capturar (sin ventana activa, etc.).
    #[error("nada que capturar: {0}")]
    NothingToCapture(String),
}

/// Strategy de captura (D4): recibe un `ScreenSource`, devuelve un `Frame`.
/// Las estrategias concretas (pantalla completa, ventana, región...) se
/// construyen desde un `ModeRequest` vía la mode factory del orquestador.
pub trait CaptureMode {
    fn capture(&self, source: &mut dyn ScreenSource) -> Result<Frame, CaptureError>;
}
```

- [ ] **Step 4: Verificar que pasa**

Run: `cargo fmt && cargo test -p rustcapture-core`
Expected: PASS (21 previos + 2 nuevos = 23).

- [ ] **Step 5: Staging**

```bash
git add crates/core/src/capture/mod.rs
```

---

### Task 2: `MockOutputSink` con handle compartido

El orquestador toma posesión de los sinks (`Box<dyn OutputSink>`); los tests necesitan seguir inspeccionando las entregas después de moverlo. Se cambia el almacenamiento interno a `Arc<Mutex<Vec<Frame>>>` y se expone un handle clonable. La API existente `delivered()` pasa de `&[Frame]` a `Vec<Frame>` (copia) — los tests actuales compilan sin cambios porque solo usan `.len()` e indexación.

**Files:**
- Modify: `crates/core/src/ports/mocks.rs`

**Interfaces:**
- Consumes: `MockOutputSink` existente.
- Produces: `MockOutputSink::delivered_handle(&self) -> Arc<Mutex<Vec<Frame>>>`; `delivered(&self) -> Vec<Frame>`. Los consumen los tests del orquestador (Tasks 4-6).

- [ ] **Step 1: Escribir el test que falla**

Añadir al módulo de tests de `mocks.rs`:

```rust
    #[test]
    fn delivered_handle_observa_entregas_tras_mover_el_sink() {
        let sink = MockOutputSink::new("clipboard");
        let handle = sink.delivered_handle();
        let mut boxed: Box<dyn OutputSink> = Box::new(sink);
        boxed.deliver(&Frame::filled(1, 1, [7, 7, 7, 255])).unwrap();
        assert_eq!(handle.lock().unwrap().len(), 1);
        assert_eq!(handle.lock().unwrap()[0].pixel(0, 0), Some([7, 7, 7, 255]));
    }
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — `no method named delivered_handle`.

- [ ] **Step 3: Refactorizar el mock**

En `mocks.rs`, añadir al principio del archivo (tras el doc-comment):

```rust
use std::sync::{Arc, Mutex};
```

Sustituir la definición e impl de `MockOutputSink` por:

```rust
/// `OutputSink` que acumula lo entregado en memoria. El log de entregas
/// vive tras un `Arc<Mutex>` para poder inspeccionarlo aunque el sink se
/// haya movido (p. ej. dentro del orquestador).
pub struct MockOutputSink {
    id: &'static str,
    delivered: Arc<Mutex<Vec<Frame>>>,
    next_error: Option<OutputError>,
}

impl MockOutputSink {
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            delivered: Arc::new(Mutex::new(Vec::new())),
            next_error: None,
        }
    }

    /// La siguiente llamada a `deliver` devolverá este error.
    pub fn fail_next(&mut self, error: OutputError) {
        self.next_error = Some(error);
    }

    /// Copia de los frames entregados con éxito, en orden.
    pub fn delivered(&self) -> Vec<Frame> {
        self.delivered.lock().unwrap().clone()
    }

    /// Handle compartido al log de entregas.
    pub fn delivered_handle(&self) -> Arc<Mutex<Vec<Frame>>> {
        Arc::clone(&self.delivered)
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
        self.delivered.lock().unwrap().push(frame.clone());
        Ok(())
    }
}
```

- [ ] **Step 4: Verificar que pasa**

Run: `cargo fmt && cargo test -p rustcapture-core`
Expected: PASS (23 + 1 = 24; los dos tests previos del sink siguen en verde sin tocarlos).

- [ ] **Step 5: Staging**

```bash
git add crates/core/src/ports/mocks.rs
```

---

### Task 3: Módulo `orchestrator`: eventos, registro de sinks y bindings

**Files:**
- Create: `crates/core/src/orchestrator/mod.rs`
- Create: `crates/core/src/orchestrator/events.rs`
- Modify: `crates/core/src/lib.rs` (añadir `pub mod orchestrator;` en orden alfabético, entre `config` y `output`)

**Interfaces:**
- Consumes: `ports::{HotkeyId, OutputSink, Rect, ScreenSource}`, `capture::{CaptureError, CaptureMode}` (Task 1).
- Produces:
  - `events`: `ModeRequest { Fullscreen, ActiveWindow, Region(Rect) }`, `CaptureRequest { mode: ModeRequest, destination: &'static str }`, `AppEvent { CaptureRequested(CaptureRequest), HotkeyPressed(HotkeyId), Shutdown }` — todos `Clone + PartialEq + Debug`.
  - `ModeFactory = Box<dyn Fn(&ModeRequest) -> Result<Box<dyn CaptureMode>, CaptureError>>`.
  - `Orchestrator::new(source: Box<dyn ScreenSource>, mode_factory: ModeFactory) -> Orchestrator`; `add_sink(&mut self, Box<dyn OutputSink>) -> Result<(), OrchestratorError>` (falla con `DuplicateSink`); `bind_hotkey(&mut self, HotkeyId, CaptureRequest)` (rebind reemplaza); `binding(&self, HotkeyId) -> Option<&CaptureRequest>`.
  - `OrchestratorError { DuplicateSink(&'static str), UnknownSink(&'static str), UnknownHotkey(HotkeyId), Capture(CaptureError) [#from], Output(OutputError) [#from] }`.
- Tasks 4-6 añaden `handle_event`/`run` a este mismo struct.

- [ ] **Step 1: Escribir los tests que fallan**

Crear `crates/core/src/orchestrator/events.rs`:

```rust
//! Eventos del bus (D7): datos, no comportamiento. Cualquier productor
//! (hotkey, barra, CLI, auto-captura futura) publica estos valores en el
//! canal mpsc que consume el orquestador.

use crate::ports::{HotkeyId, Rect};

/// Qué capturar. La mode factory lo convierte en una strategy `CaptureMode`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModeRequest {
    Fullscreen,
    ActiveWindow,
    Region(Rect),
}

/// `CaptureRequested { mode, destination }` de D7.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CaptureRequest {
    pub mode: ModeRequest,
    /// Id del sink registrado ("clipboard", "file"...).
    pub destination: &'static str,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AppEvent {
    CaptureRequested(CaptureRequest),
    HotkeyPressed(HotkeyId),
    Shutdown,
}
```

Crear `crates/core/src/orchestrator/mod.rs`:

```rust
//! Orquestador (D7): consume `AppEvent` del canal mpsc y ejecuta el
//! pipeline capturar → entregar. Hotkeys, barra y CLI son solo
//! productores; este módulo es el único consumidor.

mod events;

pub use events::{AppEvent, CaptureRequest, ModeRequest};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::{CaptureError, CaptureMode};
    use crate::ports::mocks::{MockOutputSink, MockScreenSource};
    use crate::ports::{Frame, HotkeyId, ScreenSource};

    /// Fuente 2x2 con canal R = índice del píxel, origen (0, 0).
    fn source_2x2() -> Box<dyn ScreenSource> {
        let pixels: Vec<u8> = (0..4u8).flat_map(|i| [i, 0, 0, 255]).collect();
        Box::new(MockScreenSource::new((0, 0), Frame::new(2, 2, pixels).unwrap()))
    }

    struct DesktopMode;

    impl CaptureMode for DesktopMode {
        fn capture(
            &self,
            source: &mut dyn ScreenSource,
        ) -> Result<Frame, CaptureError> {
            let rect = source.desktop_rect();
            Ok(source.capture_region(rect)?)
        }
    }

    /// Factory de test: solo soporta `Fullscreen`; el resto simula un
    /// modo aún no implementado.
    fn test_factory() -> ModeFactory {
        Box::new(|req| match req {
            ModeRequest::Fullscreen => Ok(Box::new(DesktopMode)),
            _ => Err(CaptureError::NothingToCapture("modo no soportado en test".into())),
        })
    }

    fn orquestador() -> Orchestrator {
        Orchestrator::new(source_2x2(), test_factory())
    }

    #[test]
    fn add_sink_rechaza_ids_duplicados() {
        let mut orch = orquestador();
        orch.add_sink(Box::new(MockOutputSink::new("clipboard"))).unwrap();
        let err = orch.add_sink(Box::new(MockOutputSink::new("clipboard"))).unwrap_err();
        assert_eq!(err, OrchestratorError::DuplicateSink("clipboard"));
    }

    #[test]
    fn bind_hotkey_reemplaza_el_binding_anterior() {
        let mut orch = orquestador();
        let a_region = CaptureRequest {
            mode: ModeRequest::Fullscreen,
            destination: "clipboard",
        };
        let a_archivo = CaptureRequest {
            mode: ModeRequest::Fullscreen,
            destination: "file",
        };
        orch.bind_hotkey(HotkeyId(1), a_region.clone());
        orch.bind_hotkey(HotkeyId(1), a_archivo.clone());
        assert_eq!(orch.binding(HotkeyId(1)), Some(&a_archivo));
        assert_eq!(orch.binding(HotkeyId(2)), None);
    }
}
```

En `crates/core/src/lib.rs`, añadir entre `config` y `output`:

```rust
pub mod orchestrator;
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — `cannot find type ModeFactory` / `Orchestrator`.

- [ ] **Step 3: Implementar `Orchestrator` (construcción y registro)**

En `orchestrator/mod.rs`, entre los `pub use` y los tests:

```rust
use crate::capture::{CaptureError, CaptureMode};
use crate::ports::{HotkeyId, OutputError, OutputSink, ScreenSource};

/// Convierte la petición (datos) en una strategy (comportamiento).
/// El slice de modos (D4) aporta la factory real; los tests, una fake.
pub type ModeFactory =
    Box<dyn Fn(&ModeRequest) -> Result<Box<dyn CaptureMode>, CaptureError>>;

#[derive(thiserror::Error, Clone, PartialEq, Eq, Debug)]
pub enum OrchestratorError {
    #[error("sink duplicado: {0}")]
    DuplicateSink(&'static str),
    #[error("sink desconocido: {0}")]
    UnknownSink(&'static str),
    #[error("atajo sin binding: {0:?}")]
    UnknownHotkey(HotkeyId),
    #[error(transparent)]
    Capture(#[from] CaptureError),
    #[error(transparent)]
    Output(#[from] OutputError),
}

pub struct Orchestrator {
    source: Box<dyn ScreenSource>,
    mode_factory: ModeFactory,
    sinks: Vec<Box<dyn OutputSink>>,
    bindings: Vec<(HotkeyId, CaptureRequest)>,
}

impl Orchestrator {
    pub fn new(source: Box<dyn ScreenSource>, mode_factory: ModeFactory) -> Self {
        Self {
            source,
            mode_factory,
            sinks: Vec::new(),
            bindings: Vec::new(),
        }
    }

    pub fn add_sink(&mut self, sink: Box<dyn OutputSink>) -> Result<(), OrchestratorError> {
        if self.sinks.iter().any(|s| s.id() == sink.id()) {
            return Err(OrchestratorError::DuplicateSink(sink.id()));
        }
        self.sinks.push(sink);
        Ok(())
    }

    /// Asocia un hotkey a una petición; rebindear reemplaza (recarga de config).
    pub fn bind_hotkey(&mut self, id: HotkeyId, request: CaptureRequest) {
        if let Some(entry) = self.bindings.iter_mut().find(|(i, _)| *i == id) {
            entry.1 = request;
        } else {
            self.bindings.push((id, request));
        }
    }

    pub fn binding(&self, id: HotkeyId) -> Option<&CaptureRequest> {
        self.bindings.iter().find(|(i, _)| *i == id).map(|(_, r)| r)
    }
}
```

- [ ] **Step 4: Verificar que pasa**

Run: `cargo fmt && cargo test -p rustcapture-core`
Expected: PASS (24 + 2 = 26). Nota: `binding`/`bindings` aún sin más uso — si clippy avisa de dead code en algún helper, es porque Tasks 4-5 aún no llegaron; no suprimir avisos, continuar (la verificación final con `-D warnings` se hace en Task 7 con todo cableado).

- [ ] **Step 5: Staging**

```bash
git add crates/core/src/lib.rs crates/core/src/orchestrator/
```

---

### Task 4: `handle_event` — ruta de captura

**Files:**
- Modify: `crates/core/src/orchestrator/mod.rs`

**Interfaces:**
- Consumes: todo lo de Task 3.
- Produces: `Flow { Continue, Shutdown }` (`Clone + Copy + PartialEq + Eq + Debug`); `Orchestrator::handle_event(&mut self, event: AppEvent) -> Result<Flow, OrchestratorError>` — en esta task solo la rama `CaptureRequested` (y `Shutdown` trivial); Task 5 añade `HotkeyPressed`.

- [ ] **Step 1: Escribir los tests que fallan**

Añadir al módulo de tests de `orchestrator/mod.rs`:

```rust
    fn peticion(destination: &'static str) -> AppEvent {
        AppEvent::CaptureRequested(CaptureRequest {
            mode: ModeRequest::Fullscreen,
            destination,
        })
    }

    #[test]
    fn capture_requested_entrega_el_frame_al_sink_destino() {
        let sink = MockOutputSink::new("clipboard");
        let entregas = sink.delivered_handle();
        let otro = MockOutputSink::new("file");
        let otras_entregas = otro.delivered_handle();
        let mut orch = orquestador();
        orch.add_sink(Box::new(sink)).unwrap();
        orch.add_sink(Box::new(otro)).unwrap();

        let flow = orch.handle_event(peticion("clipboard")).unwrap();

        assert_eq!(flow, Flow::Continue);
        let frames = entregas.lock().unwrap();
        assert_eq!(frames.len(), 1);
        // Fullscreen del mock 2x2: el píxel (1,1) es el índice 3.
        assert_eq!(frames[0].pixel(1, 1), Some([3, 0, 0, 255]));
        assert!(otras_entregas.lock().unwrap().is_empty());
    }

    #[test]
    fn destino_no_registrado_devuelve_unknown_sink() {
        let mut orch = orquestador();
        assert_eq!(
            orch.handle_event(peticion("printer")).unwrap_err(),
            OrchestratorError::UnknownSink("printer")
        );
    }

    #[test]
    fn un_fallo_de_la_factory_se_propaga_como_capture() {
        let mut orch = orquestador();
        orch.add_sink(Box::new(MockOutputSink::new("clipboard"))).unwrap();
        let evento = AppEvent::CaptureRequested(CaptureRequest {
            mode: ModeRequest::ActiveWindow,
            destination: "clipboard",
        });
        assert_eq!(
            orch.handle_event(evento).unwrap_err(),
            OrchestratorError::Capture(CaptureError::NothingToCapture(
                "modo no soportado en test".into()
            ))
        );
    }

    #[test]
    fn un_fallo_del_sink_se_propaga_como_output() {
        use crate::ports::OutputError;
        let mut sink = MockOutputSink::new("clipboard");
        sink.fail_next(OutputError::Failed("portapapeles bloqueado".into()));
        let entregas = sink.delivered_handle();
        let mut orch = orquestador();
        orch.add_sink(Box::new(sink)).unwrap();
        assert_eq!(
            orch.handle_event(peticion("clipboard")).unwrap_err(),
            OrchestratorError::Output(OutputError::Failed("portapapeles bloqueado".into()))
        );
        assert!(entregas.lock().unwrap().is_empty());
    }

    #[test]
    fn shutdown_devuelve_flow_shutdown() {
        let mut orch = orquestador();
        assert_eq!(orch.handle_event(AppEvent::Shutdown).unwrap(), Flow::Shutdown);
    }
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — `no method named handle_event` / `cannot find type Flow`.

- [ ] **Step 3: Implementar la ruta de captura**

Añadir en `orchestrator/mod.rs` (fuera del impl existente, junto a `OrchestratorError`):

```rust
/// Qué hacer tras procesar un evento: seguir consumiendo o parar el bucle.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flow {
    Continue,
    Shutdown,
}
```

Y dentro de `impl Orchestrator`:

```rust
    /// Procesa un evento de forma síncrona. `run` lo llama en bucle; los
    /// tests lo llaman directamente.
    pub fn handle_event(&mut self, event: AppEvent) -> Result<Flow, OrchestratorError> {
        match event {
            AppEvent::CaptureRequested(request) => {
                self.capture_and_deliver(&request)?;
                Ok(Flow::Continue)
            }
            AppEvent::HotkeyPressed(id) => Err(OrchestratorError::UnknownHotkey(id)),
            AppEvent::Shutdown => Ok(Flow::Shutdown),
        }
    }

    fn capture_and_deliver(&mut self, request: &CaptureRequest) -> Result<(), OrchestratorError> {
        // Resolver el sink primero: no capturamos si el destino no existe.
        let sink = self
            .sinks
            .iter_mut()
            .find(|s| s.id() == request.destination)
            .ok_or(OrchestratorError::UnknownSink(request.destination))?;
        let mode = (self.mode_factory)(&request.mode)?;
        let frame = mode.capture(self.source.as_mut())?;
        sink.deliver(&frame)?;
        Ok(())
    }
```

(La rama `HotkeyPressed` queda provisional a propósito: Task 5 la sustituye por la resolución del binding; con el mapa vacío, `UnknownHotkey` ya es el comportamiento correcto.)

- [ ] **Step 4: Verificar que pasa**

Run: `cargo fmt && cargo test -p rustcapture-core`
Expected: PASS (26 + 5 = 31).

- [ ] **Step 5: Staging**

```bash
git add crates/core/src/orchestrator/
```

---

### Task 5: `handle_event` — resolución de hotkeys

**Files:**
- Modify: `crates/core/src/orchestrator/mod.rs`

**Interfaces:**
- Consumes: `handle_event` y `bind_hotkey`/`binding` (Tasks 3-4).
- Produces: la rama `HotkeyPressed(id)` resuelve el binding y ejecuta la petición asociada; sin binding → `UnknownHotkey(id)`.

- [ ] **Step 1: Escribir los tests que fallan**

Añadir al módulo de tests:

```rust
    #[test]
    fn hotkey_con_binding_ejecuta_la_peticion_asociada() {
        let sink = MockOutputSink::new("clipboard");
        let entregas = sink.delivered_handle();
        let mut orch = orquestador();
        orch.add_sink(Box::new(sink)).unwrap();
        orch.bind_hotkey(
            HotkeyId(1),
            CaptureRequest {
                mode: ModeRequest::Fullscreen,
                destination: "clipboard",
            },
        );

        let flow = orch.handle_event(AppEvent::HotkeyPressed(HotkeyId(1))).unwrap();

        assert_eq!(flow, Flow::Continue);
        assert_eq!(entregas.lock().unwrap().len(), 1);
    }

    #[test]
    fn hotkey_sin_binding_devuelve_unknown_hotkey() {
        let mut orch = orquestador();
        assert_eq!(
            orch.handle_event(AppEvent::HotkeyPressed(HotkeyId(7))).unwrap_err(),
            OrchestratorError::UnknownHotkey(HotkeyId(7))
        );
    }
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — `hotkey_con_binding_ejecuta_la_peticion_asociada` falla con `UnknownHotkey` (la rama provisional de Task 4 ignora los bindings).

- [ ] **Step 3: Implementar la resolución**

En `handle_event`, sustituir la rama:

```rust
            AppEvent::HotkeyPressed(id) => Err(OrchestratorError::UnknownHotkey(id)),
```

por:

```rust
            AppEvent::HotkeyPressed(id) => {
                let request = self
                    .binding(id)
                    .cloned()
                    .ok_or(OrchestratorError::UnknownHotkey(id))?;
                self.capture_and_deliver(&request)?;
                Ok(Flow::Continue)
            }
```

- [ ] **Step 4: Verificar que pasa**

Run: `cargo fmt && cargo test -p rustcapture-core`
Expected: PASS (31 + 2 = 33).

- [ ] **Step 5: Staging**

```bash
git add crates/core/src/orchestrator/
```

---

### Task 6: `run` — el bucle sobre el canal mpsc

**Files:**
- Modify: `crates/core/src/orchestrator/mod.rs`

**Interfaces:**
- Consumes: `handle_event` completo (Tasks 4-5), `std::sync::mpsc::Receiver`.
- Produces: `Orchestrator::run(&mut self, events: Receiver<AppEvent>, observer: impl FnMut(&AppEvent, &Result<Flow, OrchestratorError>))` — consume hasta `Shutdown` o desconexión del canal; los errores no rompen el bucle, se notifican al observer.

- [ ] **Step 1: Escribir los tests que fallan**

Añadir al módulo de tests:

```rust
    #[test]
    fn run_procesa_hasta_shutdown_y_no_muere_por_errores() {
        let sink = MockOutputSink::new("clipboard");
        let entregas = sink.delivered_handle();
        let mut orch = orquestador();
        orch.add_sink(Box::new(sink)).unwrap();

        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(peticion("clipboard")).unwrap(); // ok
        tx.send(peticion("printer")).unwrap(); // error: sigue vivo
        tx.send(AppEvent::Shutdown).unwrap();
        tx.send(peticion("clipboard")).unwrap(); // tras shutdown: ignorado
        drop(tx);

        let mut log = Vec::new();
        orch.run(rx, |event, result| log.push((event.clone(), result.clone())));

        assert_eq!(log.len(), 3);
        assert_eq!(log[0].1, Ok(Flow::Continue));
        assert_eq!(log[1].1, Err(OrchestratorError::UnknownSink("printer")));
        assert_eq!(log[2].1, Ok(Flow::Shutdown));
        assert_eq!(entregas.lock().unwrap().len(), 1);
    }

    #[test]
    fn run_termina_al_desconectarse_todos_los_productores() {
        let mut orch = orquestador();
        orch.add_sink(Box::new(MockOutputSink::new("clipboard"))).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(peticion("clipboard")).unwrap();
        drop(tx);

        let mut procesados = 0;
        orch.run(rx, |_, _| procesados += 1);

        assert_eq!(procesados, 1);
    }
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — `no method named run`.

- [ ] **Step 3: Implementar el bucle**

Añadir a `impl Orchestrator` (y `use std::sync::mpsc::Receiver;` en los imports del módulo):

```rust
    /// Bucle consumidor (D7): un evento cada vez, hasta `Shutdown` o hasta
    /// que todos los productores suelten su `Sender`. Los errores de un
    /// evento no tumban el bucle: se notifican al observer (futuros toasts
    /// de la GUI, stderr en la CLI).
    pub fn run<F>(&mut self, events: Receiver<AppEvent>, mut observer: F)
    where
        F: FnMut(&AppEvent, &Result<Flow, OrchestratorError>),
    {
        for event in events {
            let result = self.handle_event(event.clone());
            observer(&event, &result);
            if matches!(result, Ok(Flow::Shutdown)) {
                break;
            }
        }
    }
```

- [ ] **Step 4: Verificar que pasa**

Run: `cargo fmt && cargo test -p rustcapture-core`
Expected: PASS (33 + 2 = 35).

- [ ] **Step 5: Staging**

```bash
git add crates/core/src/orchestrator/
```

---

### Task 7: Verificación final del slice y cierre

**Files:**
- Modify: `roadmap.md` (marcar ✅ el ítem del bus, solo tras verificar)

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

Expected: build limpio, 35 tests PASS, clippy sin warnings, formato correcto.

- [ ] **Step 2: Revisión de contrato**

Confirmar que `orchestrator` exporta: `AppEvent`, `CaptureRequest`, `ModeRequest`, `ModeFactory`, `Orchestrator`, `OrchestratorError`, `Flow`; que `capture` exporta `CaptureMode` y `CaptureError`; y que `core` sigue sin dependencias nuevas (`thiserror` únicamente).

- [ ] **Step 3: Actualizar roadmap**

En `roadmap.md` §2, cambiar:

```
- ⏳ Bus de eventos mpsc + orquestador (D7).
```

por:

```
- ✅ Bus de eventos mpsc + orquestador (D7).
```

- [ ] **Step 4: Proponer el commit al humano (NO ejecutar sin aprobación)**

Mensaje propuesto:

```
v0.1.2 — F1: bus de eventos y orquestador

AppEvent/CaptureRequest por canal mpsc y Orchestrator (D7): registro de
sinks, bindings de hotkeys, pipeline capturar→entregar vía ModeFactory
(frontera con las strategies de D4) y bucle run con observer.
```
