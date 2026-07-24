# F3/A — Editor shell (Ventana1 mínima) — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tras cualquier captura de la GUI se abre la ventana del editor (título con dimensiones, imagen encajada en el lienzo, toolbar Guardar como/Copiar/Draw(gris)/Cerrar); la barra se oculta mientras vive el editor. Destino `"editor"` como nuevo default. Spec: `2026-07-24-editor-shell-design.md`.

**Architecture:** `EditorSink` (`OutputSink` id `"editor"`) publica el `Frame` desde el hilo orquestador al hilo de UI vía `PostMessageW` (Box crudo reclamado por el wndproc de la barra); un `AtomicBool` global rechaza capturas con el editor abierto. La ventana del editor reutiliza los patrones del overlay (estado poseído por el llamador, bucle modal, `dib_from_frame` extraído a `gdi`). Encaje de imagen (`fit_rect`) puro con TDD.

**Tech Stack:** Feature nueva de `windows`: `Win32_UI_Controls_Dialogs` (`GetSaveFileNameW`). Sin crates nuevos.

## Global Constraints

- Reglas interop (skill `windows-rs-interop`): RAII, `// SAFETY:` por bloque, nada de `windows` en firmas públicas.
- El editor corre en el hilo de UI con bucle modal; capturas con editor abierto → `OutputError::Failed("editor ocupado")` → beep.
- «Cerrar» sin aviso (aún no hay ediciones posibles; el flag de sucio llega en el Slice C).
- CLI sin cambios (sus defaults no leen `[output].destination`).
- Comentarios y rustdoc en español. `cargo fmt` antes de cada verificación.
- **Commits: SOLO con aprobación humana previa.** Único commit: `v0.2.3 — F3/A: editor shell (captura → editor)`.
- Nota `windows` 0.62: ajustar firmas según compilador (memoria de quirks).

---

### Task 1: `DestinationKind::Editor` como nuevo default

**Files:**
- Modify: `crates/core/src/output/destination.rs`
- Modify: `crates/core/src/config/mod.rs` (default + test)

**Interfaces:**
- Consumes: `DestinationKind` existente.
- Produces: variante `Editor` (serde `"editor"`, `sink_id() == "editor"`), `#[default]` movido a `Editor`; `OutputConfig::default().destination == Editor`.

- [ ] **Step 1: Tests que fallan**

En `destination.rs`, ampliar el test existente:

```rust
    #[test]
    fn los_sink_ids_coinciden_con_los_sinks_reales() {
        assert_eq!(DestinationKind::Clipboard.sink_id(), "clipboard");
        assert_eq!(DestinationKind::File.sink_id(), "file");
        assert_eq!(DestinationKind::Editor.sink_id(), "editor");
    }

    #[test]
    fn el_default_es_editor() {
        assert_eq!(DestinationKind::default(), DestinationKind::Editor);
    }
```

En `config/mod.rs`, SUSTITUIR el test `el_destino_por_defecto_es_clipboard_y_se_puede_cambiar` por:

```rust
    #[test]
    fn el_destino_por_defecto_es_editor_y_se_puede_cambiar() {
        use crate::output::DestinationKind;
        assert_eq!(Config::default().output.destination, DestinationKind::Editor);
        let config = Config::from_toml("[output]\ndestination = \"clipboard\"\n").unwrap();
        assert_eq!(config.output.destination, DestinationKind::Clipboard);
    }
```

- [ ] **Step 2: Rojo** — `cargo test -p rustcapture-core` → FAIL (`Editor` no existe).

- [ ] **Step 3: Implementar** — en `destination.rs`:

```rust
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum DestinationKind {
    Clipboard,
    File,
    /// La captura aterriza en el editor (f.21) — flujo por defecto de la GUI.
    #[default]
    Editor,
}
```

y en `sink_id`: `DestinationKind::Editor => "editor",`. En `config/mod.rs`, `Default for OutputConfig`: `destination: DestinationKind::Editor,`.

- [ ] **Step 4: Verde** — `cargo fmt && cargo test -p rustcapture-core` → PASS (81 + 1 = 82).

- [ ] **Step 5: Staging** — `git add crates/core/`

---

### Task 2: `fit_rect` puro + `dib_from_frame` compartido

**Files:**
- Create: `crates/platform-win/src/editor/math.rs`
- Create: `crates/platform-win/src/editor/mod.rs` (provisional: `pub(crate) mod math;`)
- Modify: `crates/platform-win/src/lib.rs` (`pub mod editor;`)
- Modify: `crates/platform-win/src/gdi/mod.rs` (añadir `dib_from_frame` pub(crate))
- Modify: `crates/platform-win/src/overlay/mod.rs` (usar el compartido; borrar el local)

**Interfaces:**
- Consumes: `Rect`, `Frame`, `gdi::raii::{Dib, MemDc}`.
- Produces: `editor::math::fit_rect(imagen: (u32, u32), lienzo: (i32, i32)) -> Rect` — centrado; reduce manteniendo aspecto si no cabe; NUNCA amplía; lienzo degenerado → rect 0. `gdi::dib_from_frame(dc: &MemDc, frame: &Frame) -> windows::core::Result<Dib>` (pub(crate)).

- [ ] **Step 1: Tests que fallan** — `editor/math.rs`:

```rust
//! Encaje de la captura en el lienzo del editor (puro, TDD).

use rustcapture_core::ports::Rect;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imagen_pequena_se_centra_a_tamano_natural() {
        assert_eq!(fit_rect((100, 50), (400, 300)), Rect::new(150, 125, 100, 50));
    }

    #[test]
    fn imagen_ancha_se_reduce_a_lo_ancho() {
        // 2000×1000 en 400×300 → escala 0.2 → 400×200, centrada en Y.
        assert_eq!(fit_rect((2000, 1000), (400, 300)), Rect::new(0, 50, 400, 200));
    }

    #[test]
    fn imagen_alta_se_reduce_a_lo_alto() {
        // 500×1500 en 400×300 → escala 0.2 → 100×300, centrada en X.
        assert_eq!(fit_rect((500, 1500), (400, 300)), Rect::new(150, 0, 100, 300));
    }

    #[test]
    fn lienzo_degenerado_da_rect_vacio() {
        assert_eq!(fit_rect((100, 100), (0, 300)), Rect::new(0, 0, 0, 0));
        assert_eq!(fit_rect((0, 0), (400, 300)), Rect::new(0, 0, 0, 0));
    }
}
```

`editor/mod.rs` provisional: doc-comment + `pub(crate) mod math;`. En `lib.rs`: `pub mod editor;` (orden alfabético).

- [ ] **Step 2: Rojo** — `cargo test -p platform-win` → FAIL.

- [ ] **Step 3: Implementar** — en `math.rs`:

```rust
/// Rect destino de la imagen dentro del lienzo: centrada; si no cabe,
/// reducida manteniendo aspecto. Nunca se amplía.
pub(crate) fn fit_rect(imagen: (u32, u32), lienzo: (i32, i32)) -> Rect {
    let (iw, ih) = (imagen.0 as i64, imagen.1 as i64);
    let (lw, lh) = (lienzo.0 as i64, lienzo.1 as i64);
    if iw == 0 || ih == 0 || lw <= 0 || lh <= 0 {
        return Rect::new(0, 0, 0, 0);
    }
    let (w, h) = if iw <= lw && ih <= lh {
        (iw, ih)
    } else if iw * lh >= ih * lw {
        // Limita el ancho.
        (lw, (ih * lw / iw).max(1))
    } else {
        // Limita el alto.
        ((iw * lh / ih).max(1), lh)
    };
    Rect::new(
        ((lw - w) / 2) as i32,
        ((lh - h) / 2) as i32,
        w as u32,
        h as u32,
    )
}
```

Mover `dib_from_frame` de `overlay/mod.rs` a `gdi/mod.rs` (misma firma, `pub(crate)`, con su import `use rustcapture_core::ports::Frame;` ya presente y `use crate::gdi::raii::{Dib, MemDc};` local); en `overlay/mod.rs` borrar la función y usar `crate::gdi::dib_from_frame`.

- [ ] **Step 4: Verde** — `cargo fmt && cargo test -p platform-win` → PASS (18 + 4 = 22).

- [ ] **Step 5: Staging** — `git add crates/platform-win/`

---

### Task 3: `EditorSink` + ventana del editor

**Files:**
- Modify: `crates/platform-win/Cargo.toml` (feature `Win32_UI_Controls_Dialogs`)
- Modify: `crates/platform-win/src/editor/mod.rs`

**Interfaces:**
- Consumes: `math::fit_rect`, `gdi::dib_from_frame`, `clipboard::ClipboardSink`, `output::{ImageFormat, encode}` del core, `alerts`.
- Produces:
  - `editor::EditorSink::new(bar_hwnd_raw: isize)` con `impl OutputSink` (`id() == "editor"`); `deliver` → Box del frame + `PostMessageW(bar, WM_APP_EDITOR, ptr, 0)`; con editor abierto o post fallido → `Failed`.
  - `editor::WM_APP_EDITOR: u32` (pub(crate), `WM_APP + 3`).
  - `editor::show_editor(frame: Frame)` — modal en el hilo de UI; marca/limpia el `AtomicBool` de ocupado.

- [ ] **Step 1: Feature nueva** — en las features de `windows`:

```toml
    "Win32_UI_Controls_Dialogs",
```

- [ ] **Step 2: Implementar `editor/mod.rs`**

```rust
//! Editor shell (f.21, Slice A de F3): la captura aterriza aquí. Ventana
//! con toolbar mínima (Guardar como / Copiar / Draw / Cerrar) y la
//! imagen encajada en el lienzo. Referencia visual: Ventana1.PNG.
//!
//! Hilos: `EditorSink::deliver` corre en el hilo orquestador y SOLO
//! publica un mensaje; `show_editor` corre en el hilo de UI (bucle
//! modal, patrón del overlay).

pub(crate) mod math;

use std::sync::atomic::{AtomicBool, Ordering};

use rustcapture_core::output::{ImageFormat, encode};
use rustcapture_core::ports::{Frame, OutputError, OutputSink};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, COLOR_APPWORKSPACE, EndPaint, FillRect, GetSysColorBrush, HALFTONE, PAINTSTRUCT,
    SRCCOPY, SetStretchBltMode, StretchBlt,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::Dialogs::{
    GetSaveFileNameW, OFN_OVERWRITEPROMPT, OPENFILENAMEW,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

use crate::gdi::dib_from_frame;
use crate::gdi::raii::{Dib, MemDc, ScreenDc, Selected};

/// Mensaje al wndproc de la barra: wparam = `Box<Frame>` crudo, el
/// receptor toma posesión SIEMPRE (también si decide no abrir).
pub(crate) const WM_APP_EDITOR: u32 = WM_APP + 3;

/// Un editor cada vez (MVP sin tabs). Lo consulta el sink (hilo
/// orquestador) y lo mantiene `show_editor` (hilo UI).
static EDITOR_ABIERTO: AtomicBool = AtomicBool::new(false);

const TOOLBAR_H: i32 = 40;
const ID_GUARDAR: u16 = 3001;
const ID_COPIAR: u16 = 3002;
const ID_DRAW: u16 = 3003;
const ID_CERRAR: u16 = 3004;

/// `OutputSink` que entrega la captura al editor del hilo de UI.
pub struct EditorSink {
    bar_hwnd_raw: isize,
}

impl EditorSink {
    pub fn new(bar_hwnd_raw: isize) -> Self {
        Self { bar_hwnd_raw }
    }
}

impl OutputSink for EditorSink {
    fn id(&self) -> &'static str {
        "editor"
    }

    fn deliver(&mut self, frame: &Frame) -> Result<(), OutputError> {
        if EDITOR_ABIERTO.load(Ordering::SeqCst) {
            return Err(OutputError::Failed("editor ocupado".to_string()));
        }
        let boxed = Box::into_raw(Box::new(frame.clone()));
        // SAFETY: PostMessageW es seguro entre hilos; si falla, se
        // recupera el Box aquí mismo (sin fuga).
        let posted = unsafe {
            PostMessageW(
                Some(HWND(self.bar_hwnd_raw as *mut _)),
                WM_APP_EDITOR,
                WPARAM(boxed as usize),
                LPARAM(0),
            )
        };
        if posted.is_err() {
            // SAFETY: el mensaje no se publicó; el puntero sigue siendo nuestro.
            unsafe { drop(Box::from_raw(boxed)) };
            return Err(OutputError::Failed("la barra no está disponible".to_string()));
        }
        Ok(())
    }
}

struct EditorState {
    frame: Frame,
    dib: Dib,
    cerrado: bool,
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Abre el editor con la captura y bloquea el hilo de UI hasta cerrarlo.
pub fn show_editor(frame: Frame) {
    EDITOR_ABIERTO.store(true, Ordering::SeqCst);
    if let Err(e) = run(frame) {
        crate::alerts::error_box("RustCapture Editor", &e.to_string());
    }
    EDITOR_ABIERTO.store(false, Ordering::SeqCst);
}

fn run(frame: Frame) -> windows::core::Result<()> {
    let screen = ScreenDc::get()?;
    let dc = MemDc::compatible_with(&screen)?;
    let dib = dib_from_frame(&dc, &frame)?;
    drop(dc);
    drop(screen);

    let titulo = wide(&format!(
        "RustCapture Editor — {}×{}",
        frame.width, frame.height
    ));
    // Tamaño inicial: imagen + toolbar, acotado a un máximo razonable.
    let win_w = (frame.width as i32 + 60).clamp(520, 1280);
    let win_h = (frame.height as i32 + TOOLBAR_H + 100).clamp(360, 840);

    let state_ptr = Box::into_raw(Box::new(EditorState {
        frame,
        dib,
        cerrado: false,
    }));

    // SAFETY: patrón del overlay: el estado lo posee esta función; la
    // ventana se destruye antes del Box::from_raw.
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = w!("RustCaptureEditor");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: GetSysColorBrush(COLOR_APPWORKSPACE),
            ..Default::default()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            PCWSTR(titulo.as_ptr()),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            win_w,
            win_h,
            None,
            None,
            Some(instance.into()),
            Some(state_ptr.cast()),
        )?;
        _ = SetForegroundWindow(hwnd);

        let mut msg = MSG::default();
        while !(*state_ptr).cerrado && GetMessageW(&mut msg, None, 0, 0).as_bool() {
            _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        drop(Box::from_raw(state_ptr));
    }
    Ok(())
}

fn state_mut<'a>(hwnd: HWND) -> Option<&'a mut EditorState> {
    // SAFETY: puntero puesto por WM_NCCREATE; liberado solo tras el
    // bucle modal (nunca aquí).
    unsafe { ((GetWindowLongPtrW(hwnd, GWLP_USERDATA)) as *mut EditorState).as_mut() }
}

fn crear_toolbar(hwnd: HWND) {
    let botones: [(u16, PCWSTR, bool, i32); 4] = [
        (ID_GUARDAR, w!("Guardar como…"), true, 10),
        (ID_COPIAR, w!("Copiar"), true, 140),
        (ID_DRAW, w!("Draw"), false, 240),
        (ID_CERRAR, w!("Cerrar"), true, 340),
    ];
    for (id, texto, habilitado, x) in botones {
        // SAFETY: padre válido durante WM_CREATE; el sistema destruye
        // los hijos con la ventana.
        unsafe {
            if let Ok(btn) = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                texto,
                WS_CHILD | WS_VISIBLE,
                x,
                6,
                if id == ID_GUARDAR { 120 } else { 90 },
                26,
                Some(hwnd),
                Some(HMENU(id as usize as *mut _)),
                None,
                None,
            ) {
                _ = windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow(btn, habilitado);
            }
        }
    }
}

/// Diálogo "Guardar como" → codifica y escribe. Errores → MessageBox.
fn guardar_como(hwnd: HWND, frame: &Frame) {
    let mut buffer = [0u16; 260];
    let inicial: Vec<u16> = "captura".encode_utf16().collect();
    buffer[..inicial.len()].copy_from_slice(&inicial);
    let filtro: Vec<u16> = "PNG (*.png)\0*.png\0JPEG (*.jpg)\0*.jpg\0\0"
        .encode_utf16()
        .collect();
    let mut ofn = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: hwnd,
        lpstrFilter: PCWSTR(filtro.as_ptr()),
        nFilterIndex: 1,
        lpstrFile: windows::core::PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        Flags: OFN_OVERWRITEPROMPT,
        ..Default::default()
    };
    // SAFETY: struct completo con punteros a buffers locales vivos.
    let aceptado = unsafe { GetSaveFileNameW(&mut ofn) }.as_bool();
    if !aceptado {
        return; // canceló
    }
    let fin = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    let mut ruta = String::from_utf16_lossy(&buffer[..fin]);
    let format = if ofn.nFilterIndex == 2 {
        ImageFormat::Jpeg
    } else {
        ImageFormat::Png
    };
    let ext = format.extension();
    if !ruta.to_ascii_lowercase().ends_with(&format!(".{ext}"))
        && !(ext == "jpg" && ruta.to_ascii_lowercase().ends_with(".jpeg"))
    {
        ruta.push('.');
        ruta.push_str(ext);
    }
    let resultado = encode(frame, format)
        .map_err(|e| e.to_string())
        .and_then(|bytes| std::fs::write(&ruta, bytes).map_err(|e| e.to_string()));
    match resultado {
        Ok(()) => crate::alerts::capture_beep(),
        Err(e) => crate::alerts::error_box("RustCapture Editor", &format!("{ruta}: {e}")),
    }
}

fn on_command(hwnd: HWND, id: u16) {
    match id {
        ID_GUARDAR => {
            if let Some(state) = state_mut(hwnd) {
                guardar_como(hwnd, &state.frame);
            }
        }
        ID_COPIAR => {
            if let Some(state) = state_mut(hwnd) {
                match crate::clipboard::ClipboardSink::new().deliver(&state.frame) {
                    Ok(()) => crate::alerts::capture_beep(),
                    Err(_) => crate::alerts::error_beep(),
                }
            }
        }
        ID_CERRAR => {
            // Sin ediciones posibles todavía: cierre silencioso (spec).
            // SAFETY: destruir la propia ventana desde su wndproc.
            unsafe { _ = DestroyWindow(hwnd) };
        }
        _ => {}
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: cada rama documenta su invariante; el estado se libera en
    // `run`, nunca aquí.
    unsafe {
        match msg {
            WM_NCCREATE => {
                let cs = &*(lparam.0 as *const CREATESTRUCTW);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_CREATE => {
                crear_toolbar(hwnd);
                LRESULT(0)
            }
            WM_COMMAND => {
                on_command(hwnd, (wparam.0 & 0xFFFF) as u16);
                LRESULT(0)
            }
            WM_SIZE => {
                _ = InvalidateRect(Some(hwnd), None, true);
                LRESULT(0)
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                if let Some(state) = state_mut(hwnd) {
                    _ = pintar(hwnd, hdc, state);
                }
                _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_DESTROY => {
                if let Some(state) = state_mut(hwnd) {
                    state.cerrado = true;
                }
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// Pinta la imagen encajada bajo la toolbar.
fn pintar(
    hwnd: HWND,
    hdc: windows::Win32::Graphics::Gdi::HDC,
    state: &EditorState,
) -> windows::core::Result<()> {
    // SAFETY: hdc de BeginPaint; consultas de rect sin precondiciones.
    unsafe {
        let mut client = windows::Win32::Foundation::RECT::default();
        _ = GetClientRect(hwnd, &mut client);
        let lienzo = (
            client.right - client.left,
            client.bottom - client.top - TOOLBAR_H,
        );
        let destino = math::fit_rect((state.frame.width, state.frame.height), lienzo);
        if destino.is_empty() {
            return Ok(());
        }
        // Separador de la toolbar.
        FillRect(
            hdc,
            &windows::Win32::Foundation::RECT {
                left: 0,
                top: TOOLBAR_H - 2,
                right: client.right,
                bottom: TOOLBAR_H,
            },
            GetSysColorBrush(windows::Win32::Graphics::Gdi::COLOR_BTNSHADOW),
        );
        let screen = ScreenDc::get()?;
        let src_dc = MemDc::compatible_with(&screen)?;
        let _s = Selected::bitmap(&src_dc, &state.dib)?;
        SetStretchBltMode(hdc, HALFTONE);
        _ = StretchBlt(
            hdc,
            destino.x,
            destino.y + TOOLBAR_H,
            destino.width as i32,
            destino.height as i32,
            Some(src_dc.0),
            0,
            0,
            state.frame.width as i32,
            state.frame.height as i32,
            SRCCOPY,
        );
    }
    Ok(())
}
```

- [ ] **Step 3: Compilar** — `cargo fmt && cargo test -p platform-win` → PASS (22 normales).

- [ ] **Step 4: Staging** — `git add crates/platform-win/`

---

### Task 4: Cableado — barra recibe `WM_APP_EDITOR`, gui registra el sink

**Files:**
- Modify: `crates/platform-win/src/bar.rs`
- Modify: `crates/gui/src/main.rs`

**Interfaces:**
- Consumes: `editor::{EditorSink, WM_APP_EDITOR, show_editor}`.
- Produces: rama en el wndproc de la barra (ocultar → editor modal → mostrar); `gui` crea la barra ANTES de lanzar el hilo orquestador y registra `EditorSink::new(bar.hwnd_raw())`.

- [ ] **Step 1: `bar.rs`** — rama nueva en el wndproc (junto a `WM_APP_REGION`):

```rust
            m if m == crate::editor::WM_APP_EDITOR => {
                // SAFETY: wparam es un Box<Frame> publicado por
                // EditorSink; se toma posesión SIEMPRE.
                let frame = *Box::from_raw(wparam.0 as *mut rustcapture_core::ports::Frame);
                _ = ShowWindow(hwnd, SW_HIDE);
                crate::editor::show_editor(frame);
                _ = ShowWindow(hwnd, SW_SHOW);
                LRESULT(0)
            }
```

- [ ] **Step 2: `gui/main.rs`** — reordenar: crear `bar` y `_tray` ANTES del `thread::spawn` del orquestador (mover esos dos bloques `let bar = …` / `let _tray = …` arriba, justo tras el registro de hotkeys), y dentro del spawn añadir el sink (necesita el hwnd como `isize`, capturado antes del closure):

```rust
    let bar_raw = bar.hwnd_raw();
```

y en el closure del orquestador, junto a los otros `add_sink`:

```rust
        orch.add_sink(Box::new(platform_win::editor::EditorSink::new(bar_raw)))
            .expect("sink único");
```

- [ ] **Step 3: Compilar y tests** — `cargo fmt && cargo build --workspace && cargo test --workspace`
Expected: 82 core + 22 platform-win + 10 cli = 114 tests.

- [ ] **Step 4: Staging** — `git add crates/platform-win/ crates/gui/`

---

### Task 5: Verificación manual guiada con el humano

- [ ] **Step 1: Lanzar** `./target/debug/rustcapture-gui.exe` (background).

- [ ] **Step 2: Checklist**

1. Botón «Pantalla» → NO va al portapapeles: se abre el editor con la captura, título `RustCapture Editor — W×H`, y la barra desaparece.
2. La imagen se ve encajada y centrada bajo la toolbar; al redimensionar la ventana se reencaja.
3. «Copiar» → beep y la imagen queda en el portapapeles (pegar en Paint).
4. «Guardar como…» → diálogo estándar; guardar como PNG y como JPEG funciona (extensión añadida sola si no la escribes); cancelar no hace nada.
5. «Draw» está visible pero gris.
6. «Cerrar» (o la ✕ de la ventana) → el editor se va y la barra vuelve.
7. Región con overlay y `PrtScn` → también abren el editor.
8. Con el editor abierto, `PrtScn` → beep de error (editor ocupado), nada se rompe.
9. `config.toml` con `destination = "clipboard"` → el flujo antiguo directo sigue funcionando (probar y retirar la config).

- [ ] **Step 3: Fallos → `systematic-debugging`; sin OK humano no hay cierre.**

---

### Task 6: Verificación final y propuesta de commit

- [ ] **Step 1:** `cargo build --workspace && cargo test --workspace && cargo test -p platform-win -- --ignored && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check`
Expected: 114 tests + 6 humo; clippy y formato limpios.

- [ ] **Step 2: Roadmap** — Slice A de F3: `- ⏳ Slice A — Editor shell…` → `- ✅ …`

- [ ] **Step 3: Proponer commit (NO ejecutar sin aprobación)**

```
v0.2.3 — F3/A: editor shell (captura → editor)

Nuevo flujo por defecto de la GUI: EditorSink ("editor", nuevo default
de [output].destination) publica el frame al hilo de UI y la captura
aterriza en la ventana del editor (título con dimensiones, imagen
encajada con fit_rect, toolbar Guardar como/Copiar/Draw gris/Cerrar);
la barra se oculta mientras el editor vive. CLI sin cambios. Un editor
cada vez (sin tabs); capturas con editor abierto se rechazan con beep.
```
