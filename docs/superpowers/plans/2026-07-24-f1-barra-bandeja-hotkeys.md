# F1 — Barra flotante, bandeja y hotkeys (f.1-f.3, D11) — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `rustcapture-gui.exe`: barra flotante de seis botones (pantalla y ventana activos; el resto deshabilitado hasta su fase), icono de bandeja con menú y hotkeys globales estilo FastStone, todo disparando capturas por el bus de eventos. Cierra F1.

**Architecture:** Según spec `docs/superpowers/specs/2026-07-24-barra-bandeja-hotkeys-design.md` y D11: Win32 puro en `platform-win` (módulos `hotkeys`, `bar`, `tray`, `alerts`), `gui` binario fino. Hilo principal = UI + bucle de mensajes (único productor); hilo orquestador construido dentro de sí mismo (nada exige `Send`). `Hotkey::parse` y el schema de config nuevos en `core`.

**Tech Stack:** `windows-rs` 0.62 (features nuevas: `Win32_UI_Input_KeyboardAndMouse`, `Win32_UI_Shell`, `Win32_System_LibraryLoader`). Sin dependencias nuevas de crates.

## Global Constraints

- Reglas interop (skill `windows-rs-interop`): RAII, `// SAFETY:` por bloque, nada de `windows` en firmas públicas, `unwrap` prohibido sobre APIs Win32 fuera de tests.
- TDD donde hay lógica pura: `Hotkey::parse`, config, mapeo `Hotkey`→VK. La UI (barra/bandeja) se verifica compilando + prueba manual guiada con el humano.
- Defaults de fábrica (spec): `printscreen` fullscreen, `alt+printscreen` ventana, `ctrl+printscreen` región (reservado), `ctrl+shift+printscreen` delay (reservado); destino `clipboard`.
- Solo se registran los hotkeys de fullscreen y window en este slice.
- Hotkey no registrable → beep, la app sigue (no bloqueante). Errores de captura → beep. Config rota → MessageBox + exit 2.
- Comentarios y rustdoc en español. `cargo fmt` antes de cada verificación.
- **Commits: SOLO con aprobación humana previa** (skills.md). Único commit al cierre: `v0.2.0 — F1: barra flotante, bandeja y hotkeys (F1 completa)`.
- Nota de versión `windows` 0.62: ajustar firmas según el compilador sin cambiar el diseño (p. ej. `Option<HWND>`, tipos de flags).

---

### Task 1: `Hotkey::parse` en el core

**Files:**
- Modify: `crates/core/src/ports/hotkeys.rs`

**Interfaces:**
- Consumes: `Hotkey`, `Modifiers`, `KeyCode` existentes.
- Produces: `Hotkey::parse(spec: &str) -> Result<Hotkey, String>` — tokens separados por `+`, insensible a mayúsculas/espacios; modificadores `ctrl|alt|shift|win`; tecla final `a..z`/`0..9` (`KeyCode::Char`), `f1..f24` (`KeyCode::F`), `printscreen`/`prtscn` (`KeyCode::PrintScreen`). Tasks 2 y 6 lo consumen.

- [ ] **Step 1: Escribir los tests que fallan**

Añadir a `crates/core/src/ports/hotkeys.rs` al final:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsea_modificadores_y_tecla() {
        assert_eq!(
            Hotkey::parse("ctrl+shift+f").unwrap(),
            Hotkey {
                modifiers: Modifiers {
                    ctrl: true,
                    shift: true,
                    ..Modifiers::default()
                },
                key: KeyCode::Char('f'),
            }
        );
    }

    #[test]
    fn parsea_printscreen_solo_y_con_alias() {
        assert_eq!(Hotkey::parse("printscreen").unwrap().key, KeyCode::PrintScreen);
        assert_eq!(Hotkey::parse("Alt + PrtScn").unwrap().key, KeyCode::PrintScreen);
        assert!(Hotkey::parse("alt+prtscn").unwrap().modifiers.alt);
    }

    #[test]
    fn parsea_teclas_de_funcion_y_digitos() {
        assert_eq!(Hotkey::parse("win+f12").unwrap().key, KeyCode::F(12));
        assert_eq!(Hotkey::parse("ctrl+7").unwrap().key, KeyCode::Char('7'));
    }

    #[test]
    fn sin_tecla_final_es_error() {
        assert!(Hotkey::parse("ctrl+shift").is_err());
        assert!(Hotkey::parse("").is_err());
    }

    #[test]
    fn dos_teclas_finales_es_error() {
        assert!(Hotkey::parse("f1+f2").is_err());
    }

    #[test]
    fn token_desconocido_es_error() {
        assert!(Hotkey::parse("ctrl+ñ").is_err());
        assert!(Hotkey::parse("ctrl+f25").is_err());
    }
}
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — `no function or associated item named parse`.

- [ ] **Step 3: Implementar**

Añadir tras la definición de `Hotkey` (antes de `HotkeyId`):

```rust
impl Hotkey {
    /// Parsea "ctrl+shift+printscreen" (config f.3). Insensible a
    /// mayúsculas y espacios; exactamente una tecla final.
    pub fn parse(spec: &str) -> Result<Hotkey, String> {
        let mut modifiers = Modifiers::default();
        let mut key: Option<KeyCode> = None;
        let mut poner = |k: KeyCode, key: &mut Option<KeyCode>| -> Result<(), String> {
            if key.replace(k).is_some() {
                return Err(format!("más de una tecla final en \"{spec}\""));
            }
            Ok(())
        };
        for token in spec.split('+') {
            let t = token.trim().to_ascii_lowercase();
            match t.as_str() {
                "ctrl" => modifiers.ctrl = true,
                "alt" => modifiers.alt = true,
                "shift" => modifiers.shift = true,
                "win" => modifiers.win = true,
                "printscreen" | "prtscn" => poner(KeyCode::PrintScreen, &mut key)?,
                t if t.len() == 1 && t.chars().all(|c| c.is_ascii_alphanumeric()) => {
                    poner(KeyCode::Char(t.chars().next().expect("len 1")), &mut key)?
                }
                t if t.starts_with('f') && t[1..].parse::<u8>().is_ok_and(|n| (1..=24).contains(&n)) => {
                    poner(KeyCode::F(t[1..].parse().expect("validado")), &mut key)?
                }
                otro => return Err(format!("token desconocido en \"{spec}\": \"{otro}\"")),
            }
        }
        key.map(|key| Hotkey { modifiers, key })
            .ok_or_else(|| format!("falta la tecla final en \"{spec}\""))
    }
}
```

- [ ] **Step 4: Verificar que pasa**

Run: `cargo fmt && cargo test -p rustcapture-core`
Expected: PASS (63 + 6 = 69).

- [ ] **Step 5: Staging**

```bash
git add crates/core/src/ports/hotkeys.rs
```

---

### Task 2: Config `[hotkeys]` + `[output].destination`

**Files:**
- Modify: `crates/core/src/config/mod.rs`
- Modify: `crates/core/src/output/mod.rs` y crear `crates/core/src/output/destination.rs`

**Interfaces:**
- Consumes: `Config`/`OutputConfig` existentes, `Hotkey::parse` (solo en tests).
- Produces: `output::DestinationKind { Clipboard, File }` (serde lowercase, `Clone + Copy + PartialEq + Eq + Debug`) con `sink_id(&self) -> &'static str` ("clipboard"/"file"); `OutputConfig.destination: DestinationKind` (default Clipboard); `HotkeysConfig { fullscreen, window, region, delay: String }` con los defaults de la spec; `Config.hotkeys: HotkeysConfig`. Task 6 los consume.

- [ ] **Step 1: Escribir los tests que fallan**

Crear `crates/core/src/output/destination.rs`:

```rust
//! Destino por defecto de las capturas de barra/hotkey (spec f.1-f.3).

/// A qué sink va una captura lanzada sin destino explícito.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum DestinationKind {
    #[default]
    Clipboard,
    File,
}

impl DestinationKind {
    /// Id del `OutputSink` registrado en el orquestador.
    pub fn sink_id(&self) -> &'static str {
        match self {
            DestinationKind::Clipboard => "clipboard",
            DestinationKind::File => "file",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_sink_ids_coinciden_con_los_sinks_reales() {
        assert_eq!(DestinationKind::Clipboard.sink_id(), "clipboard");
        assert_eq!(DestinationKind::File.sink_id(), "file");
    }
}
```

En `output/mod.rs` añadir:

```rust
mod destination;

pub use destination::DestinationKind;
```

Añadir al módulo de tests de `config/mod.rs`:

```rust
    #[test]
    fn los_hotkeys_por_defecto_son_estilo_faststone_y_parsean() {
        use crate::ports::Hotkey;
        let config = Config::default();
        assert_eq!(config.hotkeys.fullscreen, "printscreen");
        assert_eq!(config.hotkeys.window, "alt+printscreen");
        assert_eq!(config.hotkeys.region, "ctrl+printscreen");
        assert_eq!(config.hotkeys.delay, "ctrl+shift+printscreen");
        for spec in [
            &config.hotkeys.fullscreen,
            &config.hotkeys.window,
            &config.hotkeys.region,
            &config.hotkeys.delay,
        ] {
            assert!(Hotkey::parse(spec).is_ok(), "default no parseable: {spec}");
        }
    }

    #[test]
    fn el_destino_por_defecto_es_clipboard_y_se_puede_cambiar() {
        use crate::output::DestinationKind;
        assert_eq!(Config::default().output.destination, DestinationKind::Clipboard);
        let config = Config::from_toml("[output]\ndestination = \"file\"\n").unwrap();
        assert_eq!(config.output.destination, DestinationKind::File);
    }

    #[test]
    fn hotkeys_parciales_completan_con_defaults() {
        let config = Config::from_toml("[hotkeys]\nfullscreen = \"ctrl+f1\"\n").unwrap();
        assert_eq!(config.hotkeys.fullscreen, "ctrl+f1");
        assert_eq!(config.hotkeys.window, "alt+printscreen");
    }
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — `no field hotkeys` / `DestinationKind`.

- [ ] **Step 3: Implementar**

En `config/mod.rs`: ampliar el `use` a `use crate::output::{DestinationKind, ImageFormat};`, añadir el campo a `Config`:

```rust
pub struct Config {
    pub output: OutputConfig,
    pub hotkeys: HotkeysConfig,
}
```

añadir a `OutputConfig` el campo y su default:

```rust
    /// Destino de las capturas de barra y hotkeys (f.1, f.3).
    pub destination: DestinationKind,
```

(en `Default for OutputConfig`: `destination: DestinationKind::Clipboard,`)

y añadir el struct nuevo:

```rust
/// Atajos globales (f.3) como strings "ctrl+alt+tecla"; se parsean con
/// `Hotkey::parse` al arrancar. `region` y `delay` están reservados:
/// el schema ya los conoce, se registran cuando llegue su fase (F4, F2).
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct HotkeysConfig {
    pub fullscreen: String,
    pub window: String,
    pub region: String,
    pub delay: String,
}

impl Default for HotkeysConfig {
    fn default() -> Self {
        Self {
            fullscreen: "printscreen".to_string(),
            window: "alt+printscreen".to_string(),
            region: "ctrl+printscreen".to_string(),
            delay: "ctrl+shift+printscreen".to_string(),
        }
    }
}
```

`Config` sigue con `#[derive(Default)]` (ambos campos tienen `Default`).

- [ ] **Step 4: Verificar que pasa**

Run: `cargo fmt && cargo test -p rustcapture-core`
Expected: PASS (69 + 4 = 73).

- [ ] **Step 5: Staging**

```bash
git add crates/core/
```

---

### Task 3: `Win32HotkeyProvider` en `platform-win`

**Files:**
- Modify: `crates/platform-win/Cargo.toml` (feature `Win32_UI_Input_KeyboardAndMouse`)
- Create: `crates/platform-win/src/hotkeys.rs`
- Modify: `crates/platform-win/src/lib.rs` (añadir `pub mod hotkeys;`)

**Interfaces:**
- Consumes: `ports::{Hotkey, HotkeyError, HotkeyId, HotkeyProvider, KeyCode, Modifiers}`.
- Produces: `hotkeys::Win32HotkeyProvider::new()` con `impl HotkeyProvider` (`register` usa `RegisterHotKey(None, ...)`: los `WM_HOTKEY` llegan a la cola del hilo que registra); internas puras `mods_of(&Modifiers) -> u32` y `vk_of(KeyCode) -> Option<u16>` (testeables sin registrar). Task 6 lo consume; Task 4 consume los ids vía `msg.wParam`.

- [ ] **Step 1: Añadir la feature**

En `crates/platform-win/Cargo.toml`, añadir a las features de `windows`:

```toml
    "Win32_UI_Input_KeyboardAndMouse",
```

- [ ] **Step 2: Escribir los tests que fallan (mapeo puro + humo)**

Crear `crates/platform-win/src/hotkeys.rs`:

```rust
//! Adapter del puerto `HotkeyProvider` (f.3): `RegisterHotKey` global.
//!
//! Hilos: registrar y desregistrar SIEMPRE desde el hilo del bucle de
//! mensajes — `RegisterHotKey(None, ...)` entrega los `WM_HOTKEY` a la
//! cola del hilo que registró (los consume `bar::run_message_loop`).

use rustcapture_core::ports::{Hotkey, HotkeyError, HotkeyId, HotkeyProvider, KeyCode, Modifiers};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapea_modificadores_a_flags_win32() {
        let m = Modifiers {
            ctrl: true,
            alt: true,
            ..Modifiers::default()
        };
        assert_eq!(mods_of(&m), MOD_CONTROL.0 | MOD_ALT.0);
        assert_eq!(mods_of(&Modifiers::default()), 0);
    }

    #[test]
    fn mapea_teclas_a_vk() {
        assert_eq!(vk_of(KeyCode::Char('a')), Some(0x41));
        assert_eq!(vk_of(KeyCode::Char('7')), Some(0x37));
        assert_eq!(vk_of(KeyCode::F(12)), Some(VK_F1.0 + 11));
        assert_eq!(vk_of(KeyCode::PrintScreen), Some(VK_SNAPSHOT.0));
    }

    #[test]
    fn teclas_fuera_de_rango_no_mapean() {
        assert_eq!(vk_of(KeyCode::Char('ñ')), None);
        assert_eq!(vk_of(KeyCode::F(25)), None);
    }

    /// Humo: registra un atajo real improbable y lo libera.
    #[test]
    #[ignore = "registra un hotkey global real"]
    fn registrar_y_desregistrar_un_hotkey_real() {
        let mut provider = Win32HotkeyProvider::new();
        let hotkey = Hotkey::parse("ctrl+shift+f9").unwrap();
        let id = provider.register(hotkey).unwrap();
        provider.unregister(id).unwrap();
    }
}
```

- [ ] **Step 3: Verificar que falla**

Run: `cargo test -p platform-win`
Expected: FAIL — `cannot find mods_of` / `Win32HotkeyProvider`.

- [ ] **Step 4: Implementar**

Añadir a `hotkeys.rs` entre los `use` y los tests:

```rust
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey,
    UnregisterHotKey, VK_F1, VK_SNAPSHOT,
};

/// Modificadores del core → flags `MOD_*`.
fn mods_of(m: &Modifiers) -> u32 {
    let mut mods = 0;
    if m.ctrl {
        mods |= MOD_CONTROL.0;
    }
    if m.alt {
        mods |= MOD_ALT.0;
    }
    if m.shift {
        mods |= MOD_SHIFT.0;
    }
    if m.win {
        mods |= MOD_WIN.0;
    }
    mods
}

/// Tecla del core → virtual-key code. `None` si no es representable.
fn vk_of(key: KeyCode) -> Option<u16> {
    match key {
        // Para a-z y 0-9 el VK es el ASCII en mayúscula.
        KeyCode::Char(c) if c.is_ascii_lowercase() => Some(c.to_ascii_uppercase() as u16),
        KeyCode::Char(c) if c.is_ascii_digit() => Some(c as u16),
        KeyCode::Char(_) => None,
        KeyCode::F(n) if (1..=24).contains(&n) => Some(VK_F1.0 + (n as u16 - 1)),
        KeyCode::F(_) => None,
        KeyCode::PrintScreen => Some(VK_SNAPSHOT.0),
    }
}

/// `HotkeyProvider` real. Guarda lo registrado para validar duplicados
/// y liberar por id.
pub struct Win32HotkeyProvider {
    next_id: u32,
    registered: Vec<(HotkeyId, Hotkey)>,
}

impl Win32HotkeyProvider {
    #[expect(clippy::new_without_default, reason = "simetría con el resto de adapters")]
    pub fn new() -> Self {
        Self {
            next_id: 1,
            registered: Vec::new(),
        }
    }
}

impl HotkeyProvider for Win32HotkeyProvider {
    fn register(&mut self, hotkey: Hotkey) -> Result<HotkeyId, HotkeyError> {
        if self.registered.iter().any(|(_, h)| *h == hotkey) {
            return Err(HotkeyError::AlreadyRegistered(hotkey));
        }
        let vk = vk_of(hotkey.key)
            .ok_or_else(|| HotkeyError::Platform(format!("tecla no mapeable: {:?}", hotkey.key)))?;
        let id = HotkeyId(self.next_id);
        // SAFETY: hwnd None → WM_HOTKEY a la cola de este hilo; el id es
        // único dentro del proceso (contador propio).
        unsafe {
            RegisterHotKey(
                None,
                id.0 as i32,
                HOT_KEY_MODIFIERS(mods_of(&hotkey.modifiers) | MOD_NOREPEAT.0),
                vk as u32,
            )
        }
        .map_err(|e| HotkeyError::Platform(e.to_string()))?;
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
        // SAFETY: id registrado por este provider en este hilo.
        unsafe { UnregisterHotKey(None, id.0 as i32) }
            .map_err(|e| HotkeyError::Platform(e.to_string()))?;
        self.registered.remove(pos);
        Ok(())
    }
}

impl Drop for Win32HotkeyProvider {
    fn drop(&mut self) {
        for (id, _) in &self.registered {
            // SAFETY: registrados por este provider; liberar al morir.
            unsafe { _ = UnregisterHotKey(None, id.0 as i32) };
        }
    }
}
```

En `lib.rs`, junto a los otros módulos (orden alfabético):

```rust
pub mod hotkeys;
```

- [ ] **Step 5: Verificar que pasa (incluido humo)**

Run: `cargo fmt && cargo test -p platform-win`
Expected: PASS — 9 normales (6 + 3 nuevos), 5 ignored.

Run: `cargo test -p platform-win -- --ignored`
Expected: PASS — 5 de humo (incluye registrar/desregistrar real).

- [ ] **Step 6: Staging**

```bash
git add crates/platform-win/
```

---

### Task 4: `alerts.rs` + `bar.rs` (ventana, botones, wndproc, bucle)

**Files:**
- Modify: `crates/platform-win/Cargo.toml` (feature `Win32_System_LibraryLoader`)
- Create: `crates/platform-win/src/alerts.rs`
- Create: `crates/platform-win/src/bar.rs`
- Modify: `crates/platform-win/src/lib.rs` (añadir `pub mod alerts; pub mod bar;`)

**Interfaces:**
- Consumes: `orchestrator::{AppEvent, CaptureRequest, ModeRequest}`, `ports::HotkeyId`.
- Produces: `alerts::{error_box(titulo, texto), error_beep()}`; `bar::Bar::create(tx: Sender<AppEvent>, destination: &'static str) -> Result<Bar, String>` (error ya formateado: nada de `windows` en firmas públicas), `Bar::hwnd_raw() -> isize` (para `tray`, opaco), `bar::run_message_loop(tx: &Sender<AppEvent>)`; consts `pub(crate) WM_TRAY` y `pub(crate) MENU_{FULLSCREEN,WINDOW,TOGGLE,QUIT}`. Task 5 cuelga el tray de este wndproc (rama `WM_TRAY` llama a `tray::on_tray_message`); para compilar esta task antes que la 5, la rama se añade EN LA TASK 5.

- [ ] **Step 1: Feature nueva**

En `crates/platform-win/Cargo.toml`, añadir a las features de `windows`:

```toml
    "Win32_System_LibraryLoader",
```

- [ ] **Step 2: Implementar `alerts.rs`**

```rust
//! Avisos mínimos de la GUI: beep no bloqueante y MessageBox para
//! errores fatales de arranque (la config rota, spec §Errores).

use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBeep, MessageBoxW};
use windows::core::PCWSTR;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Beep de error no bloqueante (hotkey no registrable, captura fallida).
pub fn error_beep() {
    // SAFETY: sin precondiciones; el resultado no importa.
    unsafe { _ = MessageBeep(MB_ICONERROR) };
}

/// MessageBox modal de error (solo errores fatales de arranque).
pub fn error_box(titulo: &str, texto: &str) {
    let titulo = wide(titulo);
    let texto = wide(texto);
    // SAFETY: los buffers viven hasta después de la llamada (locals).
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(texto.as_ptr()),
            PCWSTR(titulo.as_ptr()),
            MB_OK | MB_ICONERROR,
        )
    };
}
```

- [ ] **Step 3: Implementar `bar.rs`**

```rust
//! Barra flotante (f.1, D11): seis botones con el layout definitivo,
//! pantalla y ventana activos en F1. Solo produce eventos (D7).
//! No-activate: no roba el foco, así "ventana activa" es la correcta.
//!
//! Hilos: crear la barra y correr `run_message_loop` en el MISMO hilo
//! (el principal). El estado del wndproc vive en GWLP_USERDATA.

use std::sync::mpsc::Sender;

use rustcapture_core::orchestrator::{AppEvent, CaptureRequest, ModeRequest};
use rustcapture_core::ports::HotkeyId;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_BTNFACE, GetSysColorBrush};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

/// Mensaje de callback del icono de bandeja (Task 5 lo consume).
pub(crate) const WM_TRAY: u32 = WM_APP + 1;

const BTN_W: i32 = 78;
const BTN_H: i32 = 30;
const MARGIN: i32 = 6;

const ID_FULLSCREEN: u16 = 1001;
const ID_WINDOW: u16 = 1002;
const ID_REGION: u16 = 1003;
const ID_DELAY: u16 = 1004;
const ID_RECORD: u16 = 1005;
const ID_CONFIG: u16 = 1006;

pub(crate) const MENU_FULLSCREEN: u16 = 2001;
pub(crate) const MENU_WINDOW: u16 = 2002;
pub(crate) const MENU_TOGGLE: u16 = 2003;
pub(crate) const MENU_QUIT: u16 = 2004;

struct BarState {
    tx: Sender<AppEvent>,
    destination: &'static str,
}

/// Handle de la barra. No expone tipos de `windows` (D2/D11).
pub struct Bar {
    hwnd: HWND,
}

impl Bar {
    /// Crea y muestra la barra. `destination` = sink por defecto de la
    /// config ("clipboard"/"file").
    pub fn create(tx: Sender<AppEvent>, destination: &'static str) -> Result<Self, String> {
        Self::create_win32(tx, destination).map_err(|e| e.to_string())
    }

    fn create_win32(
        tx: Sender<AppEvent>,
        destination: &'static str,
    ) -> windows::core::Result<Self> {
        // SAFETY: registro de clase + creación de ventana estándar; el
        // Box de estado viaja como lpCreateParams y lo adopta WM_NCCREATE.
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class = w!("RustCaptureBar");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                lpszClassName: class,
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                hbrBackground: GetSysColorBrush(COLOR_BTNFACE),
                ..Default::default()
            };
            RegisterClassW(&wc); // 0 si ya estaba registrada: inofensivo
            let state = Box::into_raw(Box::new(BarState { tx, destination }));
            let width = 6 * BTN_W + 7 * MARGIN;
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class,
                w!("RustCapture"),
                WS_POPUP | WS_VISIBLE,
                40,
                40,
                width,
                BTN_H + 2 * MARGIN,
                None,
                None,
                Some(instance.into()),
                Some(state.cast()),
            )?;
            Ok(Self { hwnd })
        }
    }

    /// HWND como entero opaco, para colgar el icono de bandeja (Task 5).
    pub fn hwnd_raw(&self) -> isize {
        self.hwnd.0 as isize
    }
}

/// Bucle de mensajes del hilo UI. Los `WM_HOTKEY` (registrados con hwnd
/// nulo) se traducen aquí a eventos del bus.
pub fn run_message_loop(tx: &Sender<AppEvent>) {
    let mut msg = MSG::default();
    // SAFETY: bucle GetMessage estándar del hilo que posee las ventanas.
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_HOTKEY && msg.hwnd.is_invalid() {
                let _ = tx.send(AppEvent::HotkeyPressed(HotkeyId(msg.wParam.0 as u32)));
                continue;
            }
            _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn state_ref<'a>(hwnd: HWND) -> Option<&'a BarState> {
    // SAFETY: el puntero lo puso WM_NCCREATE desde un Box válido y solo
    // se libera en WM_NCDESTROY (después de todo uso).
    unsafe { ((GetWindowLongPtrW(hwnd, GWLP_USERDATA)) as *const BarState).as_ref() }
}

fn enviar_captura(hwnd: HWND, mode: ModeRequest) {
    if let Some(state) = state_ref(hwnd) {
        let _ = state.tx.send(AppEvent::CaptureRequested(CaptureRequest {
            mode,
            destination: state.destination,
        }));
    }
}

pub(crate) fn on_command(hwnd: HWND, id: u16) {
    match id {
        ID_FULLSCREEN | MENU_FULLSCREEN => enviar_captura(hwnd, ModeRequest::Fullscreen),
        ID_WINDOW | MENU_WINDOW => enviar_captura(hwnd, ModeRequest::ActiveWindow),
        MENU_TOGGLE => {
            // SAFETY: hwnd válido (viene del wndproc de esa ventana).
            unsafe {
                let visible = IsWindowVisible(hwnd).as_bool();
                _ = ShowWindow(hwnd, if visible { SW_HIDE } else { SW_SHOW });
            }
        }
        MENU_QUIT => {
            // SAFETY: destruir la propia ventana dispara WM_DESTROY
            // (Shutdown + PostQuitMessage).
            unsafe { _ = DestroyWindow(hwnd) };
        }
        _ => {}
    }
}

fn crear_botones(hwnd: HWND) {
    // (id, texto, habilitado en F1)
    let botones: [(u16, PCWSTR, bool); 6] = [
        (ID_FULLSCREEN, w!("Pantalla"), true),
        (ID_WINDOW, w!("Ventana"), true),
        (ID_REGION, w!("Región"), false),
        (ID_DELAY, w!("Delay"), false),
        (ID_RECORD, w!("Grabar"), false),
        (ID_CONFIG, w!("Config"), false),
    ];
    for (i, (id, texto, habilitado)) in botones.iter().enumerate() {
        // SAFETY: hwnd padre válido durante WM_CREATE; controles BUTTON
        // estándar, el sistema los destruye con el padre.
        unsafe {
            if let Ok(btn) = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                *texto,
                WS_CHILD | WS_VISIBLE,
                MARGIN + i as i32 * (BTN_W + MARGIN),
                MARGIN,
                BTN_W,
                BTN_H,
                Some(hwnd),
                Some(HMENU(*id as usize as *mut _)),
                None,
                None,
            ) {
                _ = EnableWindow(btn, *habilitado);
            }
        }
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: cada rama documenta su invariante; el estado de
    // GWLP_USERDATA se libera únicamente en WM_NCDESTROY.
    unsafe {
        match msg {
            WM_NCCREATE => {
                let cs = &*(lparam.0 as *const CREATESTRUCTW);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CREATE => {
                crear_botones(hwnd);
                LRESULT(0)
            }
            WM_COMMAND => {
                on_command(hwnd, (wparam.0 & 0xFFFF) as u16);
                LRESULT(0)
            }
            WM_NCHITTEST => {
                // Arrastrable desde cualquier punto del fondo.
                let hit = DefWindowProcW(hwnd, msg, wparam, lparam);
                if hit.0 == HTCLIENT as isize {
                    LRESULT(HTCAPTION as isize)
                } else {
                    hit
                }
            }
            WM_DESTROY => {
                if let Some(state) = state_ref(hwnd) {
                    let _ = state.tx.send(AppEvent::Shutdown);
                }
                PostQuitMessage(0);
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let ptr = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut BarState;
                if !ptr.is_null() {
                    drop(Box::from_raw(ptr));
                }
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
```

En `lib.rs` (orden alfabético):

```rust
pub mod alerts;
pub mod bar;
```

- [ ] **Step 4: Verificar que compila**

Run: `cargo fmt && cargo test -p platform-win`
Expected: PASS — mismos 9 normales (la barra no tiene tests automáticos; la prueba real es la Task 7).

- [ ] **Step 5: Staging**

```bash
git add crates/platform-win/
```

---

### Task 5: `tray.rs` + rama `WM_TRAY` en el wndproc

**Files:**
- Modify: `crates/platform-win/Cargo.toml` (feature `Win32_UI_Shell`)
- Create: `crates/platform-win/src/tray.rs`
- Modify: `crates/platform-win/src/bar.rs` (rama `WM_TRAY` en el wndproc)
- Modify: `crates/platform-win/src/lib.rs` (añadir `pub mod tray;`)

**Interfaces:**
- Consumes: `bar::{WM_TRAY, MENU_*, on_command, hwnd_raw}`.
- Produces: `tray::Tray::new(hwnd_raw: isize) -> Result<Tray, String>` (icono en bandeja; `Drop` lo quita); `tray::on_tray_message(hwnd, lparam)` — clic izquierdo = mostrar/ocultar barra, derecho = menú contextual (los comandos del menú llegan como `WM_COMMAND` y los resuelve `bar::on_command`).

- [ ] **Step 1: Feature nueva**

```toml
    "Win32_UI_Shell",
```

- [ ] **Step 2: Implementar `tray.rs`**

```rust
//! Icono en la bandeja del sistema (f.2). El callback llega al wndproc
//! de la barra como `bar::WM_TRAY`; aquí vive el icono (RAII) y el menú.

use windows::Win32::Foundation::{HWND, LPARAM, POINT};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, IDI_APPLICATION, LoadIconW,
    MF_SEPARATOR, MF_STRING, PostMessageW, SetForegroundWindow, TPM_BOTTOMALIGN, TrackPopupMenu,
    WM_COMMAND, WM_CONTEXTMENU, WM_LBUTTONUP, WM_RBUTTONUP, WPARAM,
};
use windows::core::w;

use crate::bar::{MENU_FULLSCREEN, MENU_QUIT, MENU_TOGGLE, MENU_WINDOW, WM_TRAY};

/// Icono de bandeja con quita-y-pon RAII.
pub struct Tray {
    data: NOTIFYICONDATAW,
}

impl Tray {
    pub fn new(hwnd_raw: isize) -> Result<Self, String> {
        Self::new_win32(hwnd_raw).map_err(|e| e.to_string())
    }

    fn new_win32(hwnd_raw: isize) -> windows::core::Result<Self> {
        let hwnd = HWND(hwnd_raw as *mut _);
        let mut data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: WM_TRAY,
            // SAFETY: icono de stock del sistema; no se libera.
            hIcon: unsafe { LoadIconW(None, IDI_APPLICATION)? },
            ..Default::default()
        };
        let tip: Vec<u16> = "RustCapture".encode_utf16().collect();
        data.szTip[..tip.len()].copy_from_slice(&tip);
        // SAFETY: data completa y con cbSize correcto.
        unsafe { Shell_NotifyIconW(NIM_ADD, &data).ok()? };
        Ok(Self { data })
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        // SAFETY: quita el icono añadido por new(); mismo uID/hWnd.
        unsafe { _ = Shell_NotifyIconW(NIM_DELETE, &self.data) };
    }
}

/// Rama `WM_TRAY` del wndproc de la barra.
pub(crate) fn on_tray_message(hwnd: HWND, lparam: LPARAM) {
    match (lparam.0 & 0xFFFF) as u32 {
        // Izquierdo: mostrar/ocultar la barra (mismo comando del menú).
        WM_LBUTTONUP => {
            // SAFETY: post a la propia ventana del wndproc.
            unsafe {
                _ = PostMessageW(
                    Some(hwnd),
                    WM_COMMAND,
                    WPARAM(MENU_TOGGLE as usize),
                    LPARAM(0),
                )
            };
        }
        WM_RBUTTONUP | WM_CONTEXTMENU => mostrar_menu(hwnd),
        _ => {}
    }
}

fn mostrar_menu(hwnd: HWND) {
    // SAFETY: menú efímero: crear → mostrar → destruir en esta función.
    // SetForegroundWindow es el requisito clásico para que el menú se
    // cierre al clicar fuera.
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        _ = AppendMenuW(menu, MF_STRING, MENU_FULLSCREEN as usize, w!("Capturar pantalla"));
        _ = AppendMenuW(menu, MF_STRING, MENU_WINDOW as usize, w!("Capturar ventana"));
        _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        _ = AppendMenuW(menu, MF_STRING, MENU_TOGGLE as usize, w!("Mostrar/ocultar barra"));
        _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        _ = AppendMenuW(menu, MF_STRING, MENU_QUIT as usize, w!("Salir"));
        let mut pt = POINT::default();
        _ = GetCursorPos(&mut pt);
        _ = SetForegroundWindow(hwnd);
        _ = TrackPopupMenu(menu, TPM_BOTTOMALIGN, pt.x, pt.y, None, hwnd, None);
        _ = DestroyMenu(menu);
    }
}
```

- [ ] **Step 3: Colgar la rama en el wndproc**

En `bar.rs`, añadir al `match` del wndproc (antes de la rama `_`):

```rust
            WM_TRAY => {
                crate::tray::on_tray_message(hwnd, lparam);
                LRESULT(0)
            }
```

(`WM_TRAY` no es constante de patrón directa por ser `u32` calculada: si el compilador exige, usar `m if m == WM_TRAY =>`.)

En `lib.rs`:

```rust
pub mod tray;
```

- [ ] **Step 4: Verificar que compila**

Run: `cargo fmt && cargo test -p platform-win`
Expected: PASS — mismos tests; tray se prueba en la Task 7.

- [ ] **Step 5: Staging**

```bash
git add crates/platform-win/
```

---

### Task 6: `gui/main.rs` — cableado del binario

**Files:**
- Modify: `crates/gui/Cargo.toml` (`[[bin]] name = "rustcapture-gui"`)
- Modify: `crates/gui/src/main.rs`

**Interfaces:**
- Consumes: todo lo anterior + `config::{Config, default_location}`, `DestinationKind::sink_id`.
- Produces: binario `rustcapture-gui.exe` residente.

- [ ] **Step 1: Cargo del binario**

En `crates/gui/Cargo.toml`, añadir tras `[package]`:

```toml
[[bin]]
name = "rustcapture-gui"
path = "src/main.rs"
```

- [ ] **Step 2: Implementar `main.rs`**

Sustituir el contenido por:

```rust
//! Binario GUI fino (D1, D11): barra + bandeja + hotkeys. El hilo
//! principal es la UI y único productor de eventos; el orquestador vive
//! en su propio hilo y se construye dentro de él (nada exige `Send`).

#![windows_subsystem = "windows"]

use std::process::ExitCode;
use std::sync::mpsc;
use std::thread;

use platform_win::bar::{Bar, run_message_loop};
use platform_win::clipboard::ClipboardSink;
use platform_win::gdi::GdiScreenSource;
use platform_win::hotkeys::Win32HotkeyProvider;
use platform_win::tray::Tray;
use rustcapture_core::capture::create_mode;
use rustcapture_core::config::Config;
use rustcapture_core::orchestrator::{CaptureRequest, ModeRequest, Orchestrator};
use rustcapture_core::output::FileSink;
use rustcapture_core::ports::{Hotkey, HotkeyProvider};

fn main() -> ExitCode {
    platform_win::dpi::ensure_per_monitor_dpi_awareness();

    let (config_path, _storage) = rustcapture_core::config::default_location();
    let config = match Config::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            platform_win::alerts::error_box("RustCapture", &e.to_string());
            return ExitCode::from(2);
        }
    };
    let destination = config.output.destination.sink_id();

    let (tx, rx) = mpsc::channel();

    // Hotkeys: registrar en ESTE hilo — WM_HOTKEY llega a su cola y lo
    // traduce run_message_loop. Fallos: beep y seguimos (spec §Errores).
    let mut hotkeys = Win32HotkeyProvider::new();
    let mut bindings = Vec::new();
    for (spec, mode) in [
        (&config.hotkeys.fullscreen, ModeRequest::Fullscreen),
        (&config.hotkeys.window, ModeRequest::ActiveWindow),
    ] {
        let registrado = Hotkey::parse(spec)
            .and_then(|hk| hotkeys.register(hk).map_err(|e| e.to_string()));
        match registrado {
            Ok(id) => bindings.push((id, CaptureRequest { mode, destination })),
            Err(_) => platform_win::alerts::error_beep(),
        }
    }

    // Hilo orquestador: construido dentro para no exigir Send a los
    // trait objects; solo cruzan Receiver y bindings.
    let out = config.output.clone();
    let orch_thread = thread::spawn(move || {
        let mut orch = Orchestrator::new(Box::new(GdiScreenSource::new()), Box::new(create_mode));
        orch.add_sink(Box::new(ClipboardSink::new()))
            .expect("sink único");
        orch.add_sink(Box::new(
            FileSink::new(out.dir, out.format).with_prefix(out.prefix),
        ))
        .expect("sink único");
        for (id, request) in bindings {
            orch.bind_hotkey(id, request);
        }
        orch.run(rx, |_, result| {
            if result.is_err() {
                platform_win::alerts::error_beep();
            }
        });
    });

    let bar = match Bar::create(tx.clone(), destination) {
        Ok(b) => b,
        Err(e) => {
            platform_win::alerts::error_box("RustCapture", &e);
            return ExitCode::FAILURE;
        }
    };
    let _tray = match Tray::new(bar.hwnd_raw()) {
        Ok(t) => t,
        Err(e) => {
            platform_win::alerts::error_box("RustCapture", &e);
            return ExitCode::FAILURE;
        }
    };

    run_message_loop(&tx);

    // WM_DESTROY ya envió Shutdown; soltar nuestro Sender y esperar.
    drop(tx);
    drop(hotkeys); // desregistra los hotkeys globales
    let _ = orch_thread.join();
    ExitCode::SUCCESS
}
```

- [ ] **Step 3: Verificar que compila**

Run: `cargo fmt && cargo build -p gui`
Expected: build limpio de `rustcapture-gui.exe`.

- [ ] **Step 4: Staging**

```bash
git add crates/gui/
```

---

### Task 7: Verificación manual guiada con el humano

**Files:** ninguno (verificación).

**Interfaces:**
- Consumes: `rustcapture-gui.exe`.
- Produces: confirmación humana del checklist (bloqueante: sin ella no hay cierre).

- [ ] **Step 1: Lanzar la GUI**

Run: `cargo run -p gui` (en background; la ventana debe aparecer).

- [ ] **Step 2: Pedir al humano el checklist**

1. La barra aparece flotante, arriba-izquierda, con 6 botones (Región/Delay/Grabar/Config en gris).
2. Botón «Pantalla» → captura al portapapeles (pegar en Paint/Word para comprobar).
3. Botón «Ventana» → captura de la ventana activa al portapapeles.
4. `PrtScn` y `Alt+PrtScn` hacen lo mismo desde cualquier app.
5. La barra se arrastra desde el fondo y no roba el foco al clicar.
6. Icono en bandeja: clic izquierdo oculta/muestra la barra; derecho abre el menú y sus 4 entradas funcionan.
7. «Salir» del menú cierra la app del todo (el proceso muere, el icono desaparece, los hotkeys quedan liberados).

- [ ] **Step 3: Anotar resultados**

Cualquier fallo → `systematic-debugging` antes de continuar. Sin OK humano no se pasa a Task 8.

---

### Task 8: Verificación final, cierre de F1 y propuesta de commit

**Files:**
- Modify: `roadmap.md` (ítem de barra ✅ y §0 Estado general → F2)

**Interfaces:**
- Consumes: OK humano de Task 7.
- Produces: F1 completa; propuesta de commit `v0.2.0`.

- [ ] **Step 1: Verificación completa (skill `verification-before-completion`)**

```bash
cargo build --workspace
cargo test --workspace
cargo test -p platform-win -- --ignored
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

Expected: 73 core + 9 platform-win + 8 cli = 90 tests; 5 humo; clippy y formato limpios.

- [ ] **Step 2: Actualizar roadmap (cierre de fase)**

- Ítem: `- ⏳ Barra flotante mínima + icono en bandeja + hotkeys globales (f.1-f.3).` → `- ✅ …`
- §0: `🔵 **Fase actual: F1 — MVP de captura.** …` → `🔵 **Fase actual: F2 — Resto de modos de captura.** F1 completada: el MVP captura a diario desde barra, bandeja, hotkeys y CLI.`

- [ ] **Step 3: Proponer el commit al humano (NO ejecutar sin aprobación)**

Mensaje propuesto:

```
v0.2.0 — F1: barra flotante, bandeja y hotkeys (F1 completa)

Win32 puro (D11): barra de 6 botones no-activate, tray con menú,
Win32HotkeyProvider (RegisterHotKey) con Hotkey::parse y defaults
FastStone en config, gui como binario fino con hilo orquestador.
Incluye spec del diseño y D11 en arquitectura.md. Verificación manual
completa por el humano. Cierra la fase F1 del roadmap.
```
