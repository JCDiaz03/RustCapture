# F2 — Captura con retardo y repetir última (f.17, f.18) — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Activar el botón «Delay» de la barra y el hotkey `ctrl+shift+printscreen` (captura de pantalla N segundos después, configurable) y añadir «Repetir última captura» al menú de bandeja — primer slice de F2.

**Architecture:** Fiel a D7: el retardo es un productor que publica tarde. Nuevo evento `AppEvent::DelayedCapture { request, delay_ms }`: el orquestador lo reprograma por un canal *loopback* (un `Sender` propio inyectado con `set_loopback`) desde un hilo temporizador — el bus nunca se bloquea. Los bindings de hotkey se generalizan de `CaptureRequest` a `AppEvent` (un hotkey puede disparar cualquier evento, incluido el retardado). f.18: el orquestador recuerda la última `CaptureRequest` ejecutada con éxito y `AppEvent::RepeatLast` la repite. En la CLI el retardo es un simple `sleep` local antes de publicar (proceso efímero, sin bus que mantener vivo).

**Tech Stack:** Sin dependencias nuevas (`std::thread` + mpsc).

## Global Constraints

- D7: el orquestador nunca duerme; los retardos viven en hilos productores.
- TDD en `core` y en el parsing de la CLI; GUI con verificación manual guiada.
- Retardo por defecto: `[capture] delay_seconds = 5` (configurable, f.17).
- «Repetir última» solo tiene sentido con proceso residente: entra en GUI (menú de bandeja); la CLI no lo ofrece (proceso efímero, documentado).
- Doble beep en el hotkey de delay (al programar y al capturar): feedback deseable, se documenta como comportamiento intencional.
- Comentarios y rustdoc en español. `cargo fmt` antes de cada verificación.
- **Commits: SOLO con aprobación humana previa** (skills.md; no es cierre de fase). Único commit: `v0.2.1 — F2: captura con retardo y repetir última`.

---

### Task 1: Orquestador — `DelayedCapture`, `RepeatLast`, loopback y bindings generalizados

**Files:**
- Modify: `crates/core/src/orchestrator/events.rs`
- Modify: `crates/core/src/orchestrator/mod.rs`

**Interfaces:**
- Consumes: orquestador existente.
- Produces:
  - `AppEvent::DelayedCapture { request: CaptureRequest, delay_ms: u64 }` y `AppEvent::RepeatLast`.
  - `Orchestrator::set_loopback(&mut self, tx: Sender<AppEvent>)` — sin él, `DelayedCapture` falla con `OrchestratorError::DelayUnavailable`.
  - `bind_hotkey(&mut self, id: HotkeyId, event: AppEvent)` y `binding(&self, id) -> Option<&AppEvent>` (antes `CaptureRequest`).
  - `OrchestratorError::{DelayUnavailable, NothingToRepeat}`.
  - `last_request` interno: se actualiza SOLO tras una entrega con éxito.

- [ ] **Step 1: Añadir los eventos**

En `events.rs`, sustituir el enum `AppEvent` por:

```rust
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AppEvent {
    CaptureRequested(CaptureRequest),
    /// Captura programada (f.17): el orquestador lanza un hilo que espera
    /// `delay_ms` y reenvía `CaptureRequested` por su loopback.
    DelayedCapture {
        request: CaptureRequest,
        delay_ms: u64,
    },
    /// Repite la última captura ejecutada con éxito (f.18).
    RepeatLast,
    HotkeyPressed(HotkeyId),
    Shutdown,
}
```

- [ ] **Step 2: Escribir los tests que fallan**

En `orchestrator/mod.rs`, ACTUALIZAR los dos tests de hotkey existentes a bindings por evento:

```rust
    #[test]
    fn hotkey_con_binding_ejecuta_el_evento_asociado() {
        let sink = MockOutputSink::new("clipboard");
        let entregas = sink.delivered_handle();
        let mut orch = orquestador();
        orch.add_sink(Box::new(sink)).unwrap();
        orch.bind_hotkey(HotkeyId(1), peticion("clipboard"));

        let flow = orch
            .handle_event(AppEvent::HotkeyPressed(HotkeyId(1)))
            .unwrap();

        assert_eq!(flow, Flow::Continue);
        assert_eq!(entregas.lock().unwrap().len(), 1);
    }
```

(y en `bind_hotkey_reemplaza_el_binding_anterior`, los dos bindings pasan a ser `peticion("clipboard")` / `peticion("file")` y el assert compara `Some(&peticion("file"))`.)

Y AÑADIR los tests nuevos:

```rust
    #[test]
    fn delayed_sin_loopback_devuelve_delay_unavailable() {
        let mut orch = orquestador();
        let evento = AppEvent::DelayedCapture {
            request: CaptureRequest {
                mode: ModeRequest::Fullscreen,
                destination: "clipboard",
            },
            delay_ms: 1,
        };
        assert_eq!(
            orch.handle_event(evento).unwrap_err(),
            OrchestratorError::DelayUnavailable
        );
    }

    #[test]
    fn delayed_con_loopback_reenvia_la_peticion_tras_el_retardo() {
        let mut orch = orquestador();
        let (tx, rx) = std::sync::mpsc::channel();
        orch.set_loopback(tx);
        let request = CaptureRequest {
            mode: ModeRequest::Fullscreen,
            destination: "clipboard",
        };
        let flow = orch
            .handle_event(AppEvent::DelayedCapture {
                request: request.clone(),
                delay_ms: 10,
            })
            .unwrap();
        assert_eq!(flow, Flow::Continue);
        // El hilo temporizador reenvía por el loopback.
        let evento = rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("debería llegar la petición reenviada");
        assert_eq!(evento, AppEvent::CaptureRequested(request));
    }

    #[test]
    fn repeat_sin_captura_previa_devuelve_nothing_to_repeat() {
        let mut orch = orquestador();
        assert_eq!(
            orch.handle_event(AppEvent::RepeatLast).unwrap_err(),
            OrchestratorError::NothingToRepeat
        );
    }

    #[test]
    fn repeat_reejecuta_la_ultima_captura_con_exito() {
        let sink = MockOutputSink::new("clipboard");
        let entregas = sink.delivered_handle();
        let mut orch = orquestador();
        orch.add_sink(Box::new(sink)).unwrap();
        orch.handle_event(peticion("clipboard")).unwrap();

        orch.handle_event(AppEvent::RepeatLast).unwrap();

        assert_eq!(entregas.lock().unwrap().len(), 2);
    }

    #[test]
    fn una_captura_fallida_no_se_convierte_en_ultima() {
        let mut orch = orquestador();
        // Sink inexistente: falla antes de capturar.
        let _ = orch.handle_event(peticion("printer"));
        assert_eq!(
            orch.handle_event(AppEvent::RepeatLast).unwrap_err(),
            OrchestratorError::NothingToRepeat
        );
    }
```

- [ ] **Step 3: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — variantes y métodos inexistentes.

- [ ] **Step 4: Implementar**

En `orchestrator/mod.rs`:

Imports: añadir `use std::sync::mpsc::Sender;` junto al `Receiver` y `use std::time::Duration;`.

Errores, añadir variantes:

```rust
    /// `DelayedCapture` exige loopback (`set_loopback`); la CLI no lo usa.
    #[error("captura con retardo no disponible sin loopback")]
    DelayUnavailable,
    #[error("no hay captura previa que repetir")]
    NothingToRepeat,
```

Struct: `bindings: Vec<(HotkeyId, AppEvent)>`, más:

```rust
    loopback: Option<Sender<AppEvent>>,
    last_request: Option<CaptureRequest>,
```

(inicializados a `None` en `new`). Métodos:

```rust
    /// Canal de reentrada para eventos programados (D7): el hilo
    /// temporizador de `DelayedCapture` publica aquí.
    pub fn set_loopback(&mut self, tx: Sender<AppEvent>) {
        self.loopback = Some(tx);
    }

    /// Asocia un hotkey a un evento; rebindear reemplaza (recarga de config).
    pub fn bind_hotkey(&mut self, id: HotkeyId, event: AppEvent) {
        if let Some(entry) = self.bindings.iter_mut().find(|(i, _)| *i == id) {
            entry.1 = event;
        } else {
            self.bindings.push((id, event));
        }
    }

    pub fn binding(&self, id: HotkeyId) -> Option<&AppEvent> {
        self.bindings.iter().find(|(i, _)| *i == id).map(|(_, e)| e)
    }
```

`handle_event` completo:

```rust
    pub fn handle_event(&mut self, event: AppEvent) -> Result<Flow, OrchestratorError> {
        match event {
            AppEvent::CaptureRequested(request) => {
                self.capture_and_deliver(&request)?;
                self.last_request = Some(request);
                Ok(Flow::Continue)
            }
            AppEvent::DelayedCapture { request, delay_ms } => {
                let tx = self
                    .loopback
                    .clone()
                    .ok_or(OrchestratorError::DelayUnavailable)?;
                // D7: el orquestador nunca duerme; espera un hilo productor.
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(delay_ms));
                    let _ = tx.send(AppEvent::CaptureRequested(request));
                });
                Ok(Flow::Continue)
            }
            AppEvent::RepeatLast => {
                let request = self
                    .last_request
                    .clone()
                    .ok_or(OrchestratorError::NothingToRepeat)?;
                self.capture_and_deliver(&request)?;
                Ok(Flow::Continue)
            }
            AppEvent::HotkeyPressed(id) => {
                let event = self
                    .binding(id)
                    .cloned()
                    .ok_or(OrchestratorError::UnknownHotkey(id))?;
                // Un binding es cualquier evento (captura, retardada...).
                self.handle_event(event)
            }
            AppEvent::Shutdown => Ok(Flow::Shutdown),
        }
    }
```

- [ ] **Step 5: Verificar que pasa**

Run: `cargo fmt && cargo test -p rustcapture-core`
Expected: PASS (74 + 5 nuevos = 79; los actualizados siguen contando igual).

- [ ] **Step 6: Staging**

```bash
git add crates/core/src/orchestrator/
```

---

### Task 2: Config `[capture] delay_seconds`

**Files:**
- Modify: `crates/core/src/config/mod.rs`

**Interfaces:**
- Consumes: `Config` existente.
- Produces: `CaptureConfig { delay_seconds: u32 }` (default 5) como `Config.capture`; helper `CaptureConfig::delay_ms(&self) -> u64`.

- [ ] **Step 1: Escribir los tests que fallan**

Añadir al módulo de tests de `config/mod.rs`:

```rust
    #[test]
    fn el_retardo_por_defecto_es_cinco_segundos() {
        let config = Config::default();
        assert_eq!(config.capture.delay_seconds, 5);
        assert_eq!(config.capture.delay_ms(), 5_000);
    }

    #[test]
    fn el_retardo_se_configura_en_toml() {
        let config = Config::from_toml("[capture]\ndelay_seconds = 3\n").unwrap();
        assert_eq!(config.capture.delay_ms(), 3_000);
    }
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — `no field capture`.

- [ ] **Step 3: Implementar**

Añadir a `Config` el campo `pub capture: CaptureConfig,` y el struct:

```rust
/// Parámetros de captura (f.17: retardo del botón/hotkey Delay).
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct CaptureConfig {
    pub delay_seconds: u32,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self { delay_seconds: 5 }
    }
}

impl CaptureConfig {
    pub fn delay_ms(&self) -> u64 {
        u64::from(self.delay_seconds) * 1_000
    }
}
```

- [ ] **Step 4: Verificar que pasa**

Run: `cargo fmt && cargo test -p rustcapture-core`
Expected: PASS (79 + 2 = 81).

- [ ] **Step 5: Staging**

```bash
git add crates/core/src/config/
```

---

### Task 3: CLI `--delay N`

**Files:**
- Modify: `crates/cli/src/args.rs`
- Modify: `crates/cli/src/main.rs`

**Interfaces:**
- Consumes: `CliOptions` existente.
- Produces: `CliOptions.delay_seconds: Option<u64>`; `main` duerme ese tiempo ANTES de publicar el evento (proceso efímero: sin loopback ni bus vivo). USAGE ampliado.

- [ ] **Step 1: Escribir los tests que fallan**

Añadir al módulo de tests de `args.rs`:

```rust
    #[test]
    fn delay_parsea_segundos() {
        assert_eq!(p(&["--delay", "3"]).unwrap().delay_seconds, Some(3));
        assert_eq!(p(&[]).unwrap().delay_seconds, None);
    }

    #[test]
    fn delay_no_numerico_es_error() {
        assert!(p(&["--delay", "tres"]).is_err());
    }
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p cli`
Expected: FAIL — `no field delay_seconds`.

- [ ] **Step 3: Implementar**

En `args.rs`: añadir el campo `pub delay_seconds: Option<u64>,` a `CliOptions`; en `parse`, tras el bloque del modo:

```rust
    let delay_seconds: Option<u64> = args
        .opt_value_from_str("--delay")
        .map_err(|e| e.to_string())?;
```

e incluirlo en el `Ok(CliOptions { mode, destination, delay_seconds })`. En `USAGE`, añadir bajo los modos:

```text
OPCIONES:
  --delay N               espera N segundos antes de capturar
```

En `main.rs`, justo antes de crear el canal:

```rust
    // f.17 en CLI: proceso efímero, el retardo es un sleep local.
    if let Some(segundos) = options.delay_seconds {
        std::thread::sleep(std::time::Duration::from_secs(segundos));
    }
```

- [ ] **Step 4: Verificar que pasa**

Run: `cargo fmt && cargo test -p cli`
Expected: PASS (8 + 2 = 10).

Verificación rápida del binario:

```bash
cargo run -q -p cli -- --delay 2 --region 0,0,30x30 --file --dir "<SCRATCH>/delay_test"
```

Expected: tarda ~2 s y deja un PNG.

- [ ] **Step 5: Staging**

```bash
git add crates/cli/
```

---

### Task 4: GUI — botón Delay activo, hotkey y «Repetir última» en bandeja

**Files:**
- Modify: `crates/platform-win/src/bar.rs`
- Modify: `crates/platform-win/src/tray.rs`
- Modify: `crates/gui/src/main.rs`

**Interfaces:**
- Consumes: Tasks 1-2.
- Produces:
  - `Bar::create(tx, destination, delay_ms: u64)` — `BarState` guarda `delay_ms`; botón «Delay» habilitado envía `AppEvent::DelayedCapture` (fullscreen → destino por defecto).
  - `MENU_REPEAT: u16 = 2005` — entrada «Repetir última captura» en el menú de bandeja → `AppEvent::RepeatLast`.
  - `gui/main.rs`: registra también el hotkey `config.hotkeys.delay` bindeado a `DelayedCapture`; bindings ahora `Vec<(HotkeyId, AppEvent)>`; `orch.set_loopback(tx.clone())` dentro del hilo orquestador.

- [ ] **Step 1: Implementar `bar.rs`**

- `BarState`: añadir campo `delay_ms: u64`.
- `Bar::create` y `create_win32`: parámetro nuevo `delay_ms: u64`, incluido en el `BarState`.
- Consts: añadir `pub(crate) const MENU_REPEAT: u16 = 2005;`
- En `crear_botones`, el botón Delay pasa a habilitado: `(ID_DELAY, w!("Delay"), true),`
- En `on_command`, añadir ramas:

```rust
        ID_DELAY => {
            if let Some(state) = state_ref(hwnd) {
                let _ = state.tx.send(AppEvent::DelayedCapture {
                    request: CaptureRequest {
                        mode: ModeRequest::Fullscreen,
                        destination: state.destination,
                    },
                    delay_ms: state.delay_ms,
                });
            }
        }
        MENU_REPEAT => {
            if let Some(state) = state_ref(hwnd) {
                let _ = state.tx.send(AppEvent::RepeatLast);
            }
        }
```

- [ ] **Step 2: Implementar `tray.rs`**

- Import: añadir `MENU_REPEAT` al `use crate::bar::{...}`.
- En `mostrar_menu`, tras «Capturar ventana»:

```rust
        _ = AppendMenuW(
            menu,
            MF_STRING,
            MENU_REPEAT as usize,
            w!("Repetir última captura"),
        );
```

- [ ] **Step 3: Implementar `gui/main.rs`**

- El bucle de registro de hotkeys pasa a construir eventos:

```rust
    let delay_ms = config.capture.delay_ms();
    let mut hotkeys = Win32HotkeyProvider::new();
    let mut bindings = Vec::new();
    let objetivo = |mode: ModeRequest| {
        AppEvent::CaptureRequested(CaptureRequest {
            mode,
            destination,
        })
    };
    let eventos = [
        (&config.hotkeys.fullscreen, objetivo(ModeRequest::Fullscreen)),
        (&config.hotkeys.window, objetivo(ModeRequest::ActiveWindow)),
        (
            &config.hotkeys.delay,
            AppEvent::DelayedCapture {
                request: CaptureRequest {
                    mode: ModeRequest::Fullscreen,
                    destination,
                },
                delay_ms,
            },
        ),
    ];
    for (spec, event) in eventos {
        let registrado =
            Hotkey::parse(spec).and_then(|hk| hotkeys.register(hk).map_err(|e| e.to_string()));
        match registrado {
            Ok(id) => bindings.push((id, event)),
            Err(_) => platform_win::alerts::error_beep(),
        }
    }
```

- Hilo orquestador: añadir el loopback (clonar ANTES del spawn):

```rust
    let loopback = tx.clone();
    let out = config.output.clone();
    let orch_thread = thread::spawn(move || {
        let mut orch = Orchestrator::new(Box::new(GdiScreenSource::new()), Box::new(create_mode));
        orch.set_loopback(loopback);
        // ... resto igual (sinks, bindings, run)
    });
```

- `Bar::create(tx.clone(), destination, delay_ms)`.

- [ ] **Step 4: Verificar que compila**

Run: `cargo fmt && cargo build --workspace 2>&1 | tail -1 && cargo test --workspace`
Expected: build limpio; 81 core + 9 platform-win + 10 cli = 100 tests.

- [ ] **Step 5: Staging**

```bash
git add crates/platform-win/ crates/gui/
```

---

### Task 5: Verificación manual guiada con el humano

**Files:** ninguno.

**Interfaces:**
- Consumes: `rustcapture-gui.exe`.
- Produces: confirmación humana (bloqueante).

- [ ] **Step 1: Lanzar la GUI** (`./target/debug/rustcapture-gui.exe` en background)

- [ ] **Step 2: Checklist para el humano**

1. Botón «Delay» ya no está gris; al pulsarlo, ~5 s después suena el beep y la captura del monitor activo llega al portapapeles.
2. `Ctrl+Shift+PrtScn` hace lo mismo desde cualquier app (beep al programar + beep al capturar).
3. Menú de bandeja: nueva entrada «Repetir última captura»; tras cualquier captura, repite la misma (mismo modo) — pegable en Paint.
4. «Repetir última» recién arrancada la app (sin captura previa) → beep de error, la app sigue viva.
5. Todo lo de F1 sigue funcionando (Pantalla, Ventana, arrastre, salir limpio).

- [ ] **Step 3: Fallos → `systematic-debugging`; sin OK humano no hay cierre.**

---

### Task 6: Verificación final y propuesta de commit

**Files:**
- Modify: `roadmap.md`

**Interfaces:**
- Consumes: OK humano de Task 5.
- Produces: ítem de F2 ✅; propuesta de commit.

- [ ] **Step 1: Verificación completa (skill `verification-before-completion`)**

```bash
cargo build --workspace
cargo test --workspace
cargo test -p platform-win -- --ignored
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

Expected: 100 tests + 6 humo; clippy y formato limpios.

- [ ] **Step 2: Actualizar roadmap**

`- ⏳ Retardo/temporizador y repetir última captura (f.17, f.18).` → `- ✅ …`

- [ ] **Step 3: Proponer el commit al humano (NO ejecutar sin aprobación)**

```
v0.2.1 — F2: captura con retardo y repetir última

AppEvent::DelayedCapture con loopback del orquestador (D7: el retardo
es un productor que publica tarde), bindings de hotkey generalizados a
eventos, RepeatLast sobre la última captura exitosa. Botón Delay y
ctrl+shift+printscreen activos, «Repetir última» en bandeja, --delay
en la CLI. Verificación manual completa.
```
