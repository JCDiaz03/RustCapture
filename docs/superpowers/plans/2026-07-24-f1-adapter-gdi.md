# F1 — Adapter de captura GDI en `platform-win` — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `GdiScreenSource` — la primera implementación real del puerto `ScreenSource` — capturando el escritorio con GDI (BitBlt) en píxeles físicos (DPI per-monitor, f.6), de modo que el pipeline evento → modo → captura → sink funcione contra la pantalla de verdad.

**Architecture:** Módulo `gdi` en `platform-win` (un módulo por tecnología, skill `windows-rs-interop`): recursos GDI envueltos en RAII (`Drop`), `unsafe` en bloques mínimos con `// SAFETY:`, errores `windows::core::Result` dentro del crate y conversión a `ScreenSourceError::Platform(String)` en la frontera del puerto. La lógica pura (BGRA→RGBA con alfa opaco) vive en `pixels.rs` sin `unsafe` y se testea sin hardware; lo que exige pantalla real son tests `#[ignore]` de humo que SÍ se ejecutan en la verificación final (esta máquina es Windows). WGC queda para más adelante: GDI cubre el MVP y funciona en cualquier Windows 10; se registra la decisión en el commit.

**Tech Stack:** `windows` 0.62 (features mínimas: `Win32_Foundation`, `Win32_Graphics_Gdi`, `Win32_Graphics_Dwm`, `Win32_UI_WindowsAndMessaging`, `Win32_UI_HiDpi`).

## Global Constraints

- Solo `windows-rs` (crate `windows`); nunca `winapi` ni `windows-sys` (skill `windows-rs-interop`).
- Features de `windows` mínimas y justificadas; el interop vive SOLO en `platform-win`; `rustcapture-core` no importa `windows` jamás.
- API pública del adapter 100 % segura: ningún tipo de `windows` ni `unsafe` en firmas públicas.
- Recursos (HDC, HBITMAP) en tipos RAII con `Drop`; prohibido liberar a mano en el flujo normal.
- HRESULT/BOOL: `.ok()?` / `Error::from_win32()`; nunca `unwrap()` sobre APIs de Windows fuera de tests.
- `unsafe` en bloques mínimos, cada uno con `// SAFETY:`.
- TDD en la lógica pura (`pixels.rs`, validación de regiones); interop real en tests de humo `#[ignore]` ejecutados a mano en la verificación.
- Comentarios y rustdoc en español. `cargo fmt` antes de cada verificación.
- **Commits: SOLO con aprobación humana previa** (skills.md). Un único commit propuesto al final: `v0.1.4 — F1: adapter de captura GDI con DPI per-monitor`.
- Los tests de `core` (45) deben seguir en verde: este slice no toca `core`.
- Nota de versión: si alguna API difiere en la versión de `windows` publicada, ajustar imports/firmas según el error del compilador — sin cambiar el diseño.

---

### Task 1: Dependencia `windows` + conversión pura BGRA→RGBA (`pixels.rs`)

**Files:**
- Modify: `crates/platform-win/Cargo.toml` (añadir `windows`)
- Create: `crates/platform-win/src/pixels.rs`
- Modify: `crates/platform-win/src/lib.rs` (añadir `pub mod pixels;`)

**Interfaces:**
- Consumes: nada.
- Produces: `pixels::bgra_to_rgba_opaque(pixels: &mut [u8])` — intercambia B↔R por píxel de 4 bytes y fuerza alfa 255 (BitBlt deja alfa 0). Task 3 la consume tras copiar el DIB.

- [ ] **Step 1: Añadir la dependencia**

En `crates/platform-win/Cargo.toml`, bajo `[dependencies]`:

```toml
windows = { version = "0.62", features = [
    "Win32_Foundation",
    "Win32_Graphics_Gdi",
    "Win32_Graphics_Dwm",
    "Win32_UI_WindowsAndMessaging",
    "Win32_UI_HiDpi",
] }
```

Justificación de features (skill): Gdi = BitBlt/DIB; Dwm = rect real de ventana (`DWMWA_EXTENDED_FRAME_BOUNDS`); WindowsAndMessaging = métricas del escritorio virtual y `GetForegroundWindow`; HiDpi = DPI awareness per-monitor (f.6); Foundation = tipos base (RECT, HWND, BOOL).

- [ ] **Step 2: Escribir los tests que fallan**

Crear `crates/platform-win/src/pixels.rs`:

```rust
//! Conversiones de píxel puras (sin `unsafe`, testeables sin hardware).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intercambia_b_y_r_y_fuerza_alfa_opaco() {
        // Dos píxeles BGRA; BitBlt suele dejar alfa 0.
        let mut px = vec![1u8, 2, 3, 0, 10, 20, 30, 128];
        bgra_to_rgba_opaque(&mut px);
        assert_eq!(px, vec![3, 2, 1, 255, 30, 20, 10, 255]);
    }

    #[test]
    fn buffer_vacio_no_hace_nada() {
        let mut px: Vec<u8> = Vec::new();
        bgra_to_rgba_opaque(&mut px);
        assert!(px.is_empty());
    }
}
```

En `lib.rs`, tras el doc-comment:

```rust
pub mod pixels;
```

- [ ] **Step 3: Verificar que falla**

Run: `cargo test -p platform-win`
Expected: FAIL — `cannot find function bgra_to_rgba_opaque`.

- [ ] **Step 4: Implementar**

En `pixels.rs`, entre el doc-comment y los tests:

```rust
/// Convierte BGRA8 (nativo de GDI/DXGI) a RGBA8 in-place y fuerza alfa
/// opaco: BitBlt no escribe alfa útil y las capturas son siempre opacas.
/// Ignora bytes sobrantes si la longitud no es múltiplo de 4.
pub fn bgra_to_rgba_opaque(pixels: &mut [u8]) {
    for px in pixels.chunks_exact_mut(4) {
        px.swap(0, 2);
        px[3] = 255;
    }
}
```

- [ ] **Step 5: Verificar que pasa**

Run: `cargo fmt && cargo test -p platform-win`
Expected: PASS (2 tests). `cargo test -p rustcapture-core` sigue en 45.

- [ ] **Step 6: Staging**

```bash
git add Cargo.lock crates/platform-win/
```

---

### Task 2: DPI awareness + RAII GDI + métricas del escritorio

**Files:**
- Create: `crates/platform-win/src/dpi.rs`
- Create: `crates/platform-win/src/gdi/raii.rs`
- Create: `crates/platform-win/src/gdi/mod.rs`
- Modify: `crates/platform-win/src/lib.rs` (añadir `pub mod dpi; pub mod gdi;`)

**Interfaces:**
- Consumes: `windows` (Task 1), `rustcapture_core::ports::Rect`.
- Produces:
  - `dpi::ensure_per_monitor_dpi_awareness() -> bool` — activa per-monitor V2 para el proceso; `false` si ya estaba fijado (manifest o llamada previa). Los binarios la llaman al arrancar; los tests de humo también.
  - `gdi::raii` (interno al crate, `pub(crate)`): `ScreenDc` (GetDC/ReleaseDC), `MemDc` (CreateCompatibleDC/DeleteDC), `Dib` (CreateDIBSection/DeleteObject, expone `bits() -> &[u8]`), `Selected` (SelectObject con restauración en `Drop`).
  - `gdi::GdiScreenSource::new() -> GdiScreenSource` y métodos `desktop_rect()`/`active_window_rect()` (aún sin `impl ScreenSource`; llega en Task 3).

- [ ] **Step 1: Escribir el test de humo que falla**

Crear `crates/platform-win/src/gdi/mod.rs`:

```rust
//! Adapter GDI del puerto `ScreenSource` (D2): BitBlt sobre un DIB de
//! 32 bits. Elegido para el MVP frente a WGC por simplicidad y soporte
//! universal en Windows 10; WGC llegará como adapter alternativo.
//!
//! Hilos: `GdiScreenSource` no es `Send` a propósito — los HDC que crea
//! `capture_region` viven y mueren dentro de la llamada, pero el uso
//! previsto es un único hilo orquestador.

mod raii;

#[cfg(test)]
mod tests {
    use super::*;

    /// Humo: exige sesión gráfica real. Ejecutar con
    /// `cargo test -p platform-win -- --ignored`.
    #[test]
    #[ignore = "requiere escritorio real"]
    fn el_escritorio_virtual_tiene_area() {
        crate::dpi::ensure_per_monitor_dpi_awareness();
        let source = GdiScreenSource::new();
        let rect = source.desktop_rect();
        assert!(rect.width > 0 && rect.height > 0);
    }

    /// Humo: en una sesión interactiva casi siempre hay ventana activa;
    /// si la hay, su rect interseca el escritorio.
    #[test]
    #[ignore = "requiere escritorio real"]
    fn la_ventana_activa_si_existe_esta_en_el_escritorio() {
        crate::dpi::ensure_per_monitor_dpi_awareness();
        let source = GdiScreenSource::new();
        if let Some(win) = source.active_window_rect() {
            assert!(source.desktop_rect().intersection(&win).is_some());
        }
    }
}
```

Crear `crates/platform-win/src/dpi.rs`:

```rust
//! DPI awareness per-monitor (f.6): en per-monitor V2 todas las APIs
//! devuelven píxeles físicos, que es lo que captura BitBlt.

use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};

/// Fija el proceso a per-monitor V2. Llamar UNA vez al arrancar cada
/// binario, antes de tocar ninguna ventana o captura. Devuelve `false`
/// si el sistema la rechaza (ya fijada por manifest o llamada previa):
/// no es un error, la awareness ya es definitiva.
pub fn ensure_per_monitor_dpi_awareness() -> bool {
    // SAFETY: cambia estado global del proceso; sin precondiciones de
    // memoria. Idempotente a efectos prácticos (la segunda llamada falla
    // y se ignora).
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_ok() }
}
```

En `lib.rs`, junto a `pub mod pixels;`:

```rust
pub mod dpi;
pub mod gdi;
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p platform-win`
Expected: FAIL — `cannot find struct GdiScreenSource` (los tests `#[ignore]` también deben compilar).

- [ ] **Step 3: Implementar RAII y métricas**

Crear `crates/platform-win/src/gdi/raii.rs`:

```rust
//! Envoltorios RAII de recursos GDI. Internos al adapter: ningún tipo
//! de `windows` sale de este crate.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, GetDC, HBITMAP, HDC, HGDIOBJ, ReleaseDC, SelectObject,
};
use windows::core::{Error, Result};

/// DC de pantalla (`GetDC(None)`); se libera con `ReleaseDC`.
pub(crate) struct ScreenDc(pub(crate) HDC);

impl ScreenDc {
    pub(crate) fn get() -> Result<Self> {
        // SAFETY: GetDC(None) pide el DC del escritorio; no hay
        // precondiciones. NULL indica fallo.
        let dc = unsafe { GetDC(Some(HWND::default())) };
        if dc.is_invalid() {
            return Err(Error::from_win32());
        }
        Ok(Self(dc))
    }
}

impl Drop for ScreenDc {
    fn drop(&mut self) {
        // SAFETY: el HDC fue obtenido con GetDC(None) y no se ha liberado.
        unsafe { ReleaseDC(Some(HWND::default()), self.0) };
    }
}

/// DC de memoria compatible; se libera con `DeleteDC`.
pub(crate) struct MemDc(pub(crate) HDC);

impl MemDc {
    pub(crate) fn compatible_with(screen: &ScreenDc) -> Result<Self> {
        // SAFETY: el HDC de origen es válido mientras viva `screen`.
        let dc = unsafe { CreateCompatibleDC(Some(screen.0)) };
        if dc.is_invalid() {
            return Err(Error::from_win32());
        }
        Ok(Self(dc))
    }
}

impl Drop for MemDc {
    fn drop(&mut self) {
        // SAFETY: el HDC fue creado con CreateCompatibleDC.
        unsafe { _ = DeleteDC(self.0) };
    }
}

/// DIB de 32 bits top-down: los bits viven en memoria del proceso.
pub(crate) struct Dib {
    pub(crate) bitmap: HBITMAP,
    bits: *const u8,
    len: usize,
}

impl Dib {
    pub(crate) fn new_32bpp(dc: &MemDc, width: u32, height: u32) -> Result<Self> {
        let header = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            // Negativo = top-down: la fila 0 es la de arriba.
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        };
        let info = BITMAPINFO {
            bmiHeader: header,
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = core::ptr::null_mut();
        // SAFETY: `info` describe un DIB válido y `bits` recibe el puntero
        // al buffer que posee el propio HBITMAP.
        let bitmap = unsafe { CreateDIBSection(Some(dc.0), &info, DIB_RGB_COLORS, &mut bits, None, 0)? };
        Ok(Self {
            bitmap,
            bits: bits as *const u8,
            len: width as usize * height as usize * 4,
        })
    }

    /// Bits BGRA del bitmap. Llamar tras `GdiFlush` para que GDI haya
    /// terminado de escribir.
    pub(crate) fn bits(&self) -> &[u8] {
        // SAFETY: el buffer pertenece al HBITMAP vivo (self) y mide
        // exactamente `len` bytes (32 bpp * w * h).
        unsafe { core::slice::from_raw_parts(self.bits, self.len) }
    }
}

impl Drop for Dib {
    fn drop(&mut self) {
        // SAFETY: el HBITMAP fue creado con CreateDIBSection.
        unsafe { _ = DeleteObject(self.bitmap.into()) };
    }
}

/// Selección temporal de un objeto en un DC; restaura el anterior en Drop.
pub(crate) struct Selected<'a> {
    dc: &'a MemDc,
    old: HGDIOBJ,
}

impl<'a> Selected<'a> {
    pub(crate) fn bitmap(dc: &'a MemDc, dib: &Dib) -> Result<Self> {
        // SAFETY: DC y bitmap son válidos (RAII vivos).
        let old = unsafe { SelectObject(dc.0, dib.bitmap.into()) };
        if old.is_invalid() {
            return Err(Error::from_win32());
        }
        Ok(Self { dc, old })
    }
}

impl Drop for Selected<'_> {
    fn drop(&mut self) {
        // SAFETY: restaura el objeto que este mismo guard desplazó.
        unsafe { SelectObject(self.dc.0, self.old) };
    }
}
```

En `gdi/mod.rs`, entre `mod raii;` y los tests:

```rust
use rustcapture_core::ports::Rect;
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetSystemMetrics, GetWindowRect, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
};

/// `ScreenSource` real sobre GDI. Sin estado entre capturas: cada
/// `capture_region` crea y destruye sus recursos (RAII).
pub struct GdiScreenSource;

impl GdiScreenSource {
    #[expect(clippy::new_without_default, reason = "constructor con futuro estado (config WGC)")]
    pub fn new() -> Self {
        Self
    }

    /// Rect del escritorio virtual en píxeles físicos (per-monitor V2).
    pub fn desktop_rect(&self) -> Rect {
        // SAFETY: GetSystemMetrics no tiene precondiciones; devuelve 0
        // para métricas desconocidas (escritorio degenerado → rect vacío,
        // que el core rechaza como OutOfBounds).
        unsafe {
            let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let w = GetSystemMetrics(SM_CXVIRTUALSCREEN).max(0) as u32;
            let h = GetSystemMetrics(SM_CYVIRTUALSCREEN).max(0) as u32;
            Rect::new(x, y, w, h)
        }
    }

    /// Rect de la ventana en primer plano. DWM da el marco visible real
    /// (`DWMWA_EXTENDED_FRAME_BOUNDS`); si DWM falla, `GetWindowRect`
    /// (incluye bordes invisibles). Errores → `None`: para el dominio,
    /// "no hay ventana capturable".
    pub fn active_window_rect(&self) -> Option<Rect> {
        // SAFETY: GetForegroundWindow no tiene precondiciones; NULL si
        // no hay ventana en primer plano.
        let hwnd = unsafe { GetForegroundWindow() };
        if hwnd.is_invalid() {
            return None;
        }
        let mut rect = RECT::default();
        // SAFETY: hwnd es una ventana válida ahora mismo (puede morir en
        // paralelo: en ese caso las APIs fallan y devolvemos None).
        let via_dwm = unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut rect as *mut RECT as *mut core::ffi::c_void,
                size_of::<RECT>() as u32,
            )
        }
        .is_ok();
        if !via_dwm {
            // SAFETY: mismas precondiciones que arriba.
            unsafe { GetWindowRect(hwnd, &mut rect) }.ok()?;
        }
        let width = rect.right.saturating_sub(rect.left).max(0) as u32;
        let height = rect.bottom.saturating_sub(rect.top).max(0) as u32;
        Some(Rect::new(rect.left, rect.top, width, height))
    }
}
```

- [ ] **Step 4: Verificar que compila y pasa**

Run: `cargo fmt && cargo test -p platform-win`
Expected: PASS (2 tests de pixels; los 2 de humo aparecen como `ignored`).

Run: `cargo test -p platform-win -- --ignored`
Expected: PASS — los 2 tests de humo contra el escritorio real de esta máquina.

- [ ] **Step 5: Staging**

```bash
git add crates/platform-win/
```

---

### Task 3: `capture_region` + `impl ScreenSource`

**Files:**
- Modify: `crates/platform-win/src/gdi/mod.rs`

**Interfaces:**
- Consumes: `raii::{ScreenDc, MemDc, Dib, Selected}`, `pixels::bgra_to_rgba_opaque`, `rustcapture_core::ports::{Frame, Rect, ScreenSource, ScreenSourceError}`.
- Produces: `impl ScreenSource for GdiScreenSource` — `desktop_rect`/`active_window_rect` delegan en los métodos de Task 2; `capture_region(&mut self, region: Rect) -> Result<Frame, ScreenSourceError>` valida contra el escritorio (OutOfBounds) y captura vía `grab` (privada, `windows::core::Result<Frame>`, convertida a `Platform(String)` en la frontera).

- [ ] **Step 1: Escribir los tests que fallan**

Añadir al módulo de tests de `gdi/mod.rs`:

```rust
    use rustcapture_core::ports::{ScreenSource, ScreenSourceError};

    /// No toca GDI: la validación de límites es previa a toda captura.
    /// Ejecutable sin `--ignored` (no exige sesión gráfica, solo métricas).
    #[test]
    fn una_region_absurda_devuelve_out_of_bounds_sin_capturar() {
        let mut source = GdiScreenSource::new();
        let region = Rect::new(i32::MIN, i32::MIN, 1, 1);
        assert_eq!(
            source.capture_region(region).unwrap_err(),
            ScreenSourceError::OutOfBounds(region)
        );
    }

    /// Humo: captura real de la esquina del escritorio.
    #[test]
    #[ignore = "requiere escritorio real"]
    fn captura_una_region_pequena_con_dimensiones_y_alfa_correctos() {
        crate::dpi::ensure_per_monitor_dpi_awareness();
        let mut source = GdiScreenSource::new();
        let desktop = source.desktop_rect();
        let region = Rect::new(desktop.x, desktop.y, 8, 8);
        let frame = source.capture_region(region).unwrap();
        assert_eq!((frame.width, frame.height), (8, 8));
        // Alfa forzado a opaco en la conversión BGRA→RGBA.
        assert!(frame.pixels.chunks_exact(4).all(|px| px[3] == 255));
    }
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p platform-win`
Expected: FAIL — `GdiScreenSource` no implementa `ScreenSource` (`no method named capture_region`).

- [ ] **Step 3: Implementar**

Añadir a `gdi/mod.rs` (los `use` nuevos se integran con los existentes):

```rust
use rustcapture_core::ports::{Frame, ScreenSource, ScreenSourceError};
use windows::Win32::Graphics::Gdi::{BitBlt, CAPTUREBLT, GdiFlush, SRCCOPY};
use windows::core::Result as WinResult;

impl GdiScreenSource {
    /// Captura real. Errores Win32 quedan en `windows::core::Result`;
    /// la frontera del puerto los aplana a `Platform(String)`.
    fn grab(&self, region: Rect) -> WinResult<Frame> {
        let screen = raii::ScreenDc::get()?;
        let mem = raii::MemDc::compatible_with(&screen)?;
        let dib = raii::Dib::new_32bpp(&mem, region.width, region.height)?;
        let _selected = raii::Selected::bitmap(&mem, &dib)?;
        // SAFETY: ambos DC son válidos (RAII vivos); CAPTUREBLT incluye
        // ventanas por capas (tooltips, popups).
        unsafe {
            BitBlt(
                mem.0,
                0,
                0,
                region.width as i32,
                region.height as i32,
                Some(screen.0),
                region.x,
                region.y,
                SRCCOPY | CAPTUREBLT,
            )?;
            // SAFETY: fuerza a GDI a terminar antes de leer los bits.
            _ = GdiFlush();
        }
        let mut pixels = dib.bits().to_vec();
        crate::pixels::bgra_to_rgba_opaque(&mut pixels);
        Frame::new(region.width, region.height, pixels)
            .map_err(|e| windows::core::Error::new(windows::Win32::Foundation::E_FAIL, e.to_string()))
    }
}

impl ScreenSource for GdiScreenSource {
    fn desktop_rect(&self) -> Rect {
        GdiScreenSource::desktop_rect(self)
    }

    fn active_window_rect(&self) -> Option<Rect> {
        GdiScreenSource::active_window_rect(self)
    }

    fn capture_region(&mut self, region: Rect) -> Result<Frame, ScreenSourceError> {
        if region.is_empty() || !GdiScreenSource::desktop_rect(self).contains(&region) {
            return Err(ScreenSourceError::OutOfBounds(region));
        }
        self.grab(region)
            .map_err(|e| ScreenSourceError::Platform(e.to_string()))
    }
}
```

Nota: los métodos inherentes y los del trait comparten nombre; las llamadas
internas usan la forma cualificada `GdiScreenSource::desktop_rect(self)`
para no recursar por el trait.

- [ ] **Step 4: Verificar que pasa**

Run: `cargo fmt && cargo test -p platform-win`
Expected: PASS — 3 tests normales (2 pixels + out_of_bounds), 3 ignored.

Run: `cargo test -p platform-win -- --ignored`
Expected: PASS — 3 tests de humo contra el escritorio real.

- [ ] **Step 5: Staging**

```bash
git add crates/platform-win/
```

---

### Task 4: Verificación final del slice y cierre

**Files:**
- Modify: `roadmap.md` (marcar ✅ el ítem del adapter, solo tras verificar)

**Interfaces:**
- Consumes: todo lo anterior.
- Produces: slice verificado; propuesta de commit al humano.

- [ ] **Step 1: Verificación completa (skill `verification-before-completion`)**

```bash
cargo build --workspace
cargo test --workspace
cargo test -p platform-win -- --ignored
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

Expected: build limpio; 45 tests de core + 3 de platform-win en verde; 3 tests de humo en verde contra el escritorio real; clippy y formato limpios.

- [ ] **Step 2: Revisión de contrato (skill `windows-rs-interop`)**

- Ningún tipo de `windows` ni `unsafe` en firmas públicas de `platform-win`.
- Cada bloque `unsafe` tiene su `// SAFETY:`.
- `rustcapture-core` sigue sin depender de `windows` (revisar `crates/core/Cargo.toml`).
- Features de `windows` usadas todas (quitar las sobrantes si el compilador no las exige).

- [ ] **Step 3: Actualizar roadmap**

En `roadmap.md` §2, cambiar:

```
- ⏳ Adapter de captura GDI/WGC en `platform-win` con DPI per-monitor (f.6).
```

por:

```
- ✅ Adapter de captura GDI en `platform-win` con DPI per-monitor (f.6) — WGC diferido como adapter alternativo.
```

- [ ] **Step 4: Proponer el commit al humano (NO ejecutar sin aprobación)**

Mensaje propuesto:

```
v0.1.4 — F1: adapter de captura GDI con DPI per-monitor

GdiScreenSource implementa ScreenSource con BitBlt sobre DIB 32 bpp
top-down, RAII para HDC/HBITMAP, rect de ventana vía DWM y conversión
BGRA→RGBA pura testeada. WGC se difiere: GDI cubre el MVP en cualquier
Windows 10. Tests de humo #[ignore] verificados contra el escritorio.
```
