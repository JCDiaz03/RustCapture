# F2 — Overlay de selección de región — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Selección interactiva de región (spec `2026-07-24-overlay-seleccion-region-design.md`): máscara blanca 50 % en todo el escritorio, arrastre con la región limpia, crosshair, caja de lupa 300×500 (zoom 5× + coordenadas + ayuda) con salto de esquina, Esc cancela, soltar captura. Activa el botón «Región» y `ctrl+printscreen`.

**Architecture:** Capa de selección de D10: ventana Win32 del tamaño del escritorio virtual que pinta un frame congelado (original + copia blanqueada precalculada) con doble buffer; bucle modal en el hilo de UI, estado poseído por el llamador (`select_region()`); salida = `Rect` en coordenadas de escritorio publicado como `CaptureRequested(Region(rect))` — el mismo camino que `--region` de la CLI. Geometría pura en `overlay/math.rs` con TDD.

**Tech Stack:** Sin dependencias nuevas; GDI ya presente (BitBlt/StretchBlt/DrawText).

## Global Constraints

- Reglas interop (skill `windows-rs-interop`): RAII, `// SAFETY:` por bloque, nada de `windows` en firmas públicas.
- Espec. visual exacta: máscara `#FFFFFF` 50 % (`px=(px+255)/2`); lupa 300×500 = zoom 300×300 a 5× (fuente 60×60) + barra gris 300×30 con `X, Y = <x>,<y>` (coordenadas de escritorio) + bloque azul RGB(30,80,160) 300×170 con ayuda en español y `Selección: W×H px` durante el arrastre; borde de selección rojo 1 px; cruz central 1 px en el zoom; sin tamaño mínimo (clic suelto = 1×1, f.19).
- La barra se auto-oculta durante la selección (~150 ms de pausa antes de congelar) y reaparece siempre.
- Fallo al congelar → beep, overlay no se abre.
- TDD en `pixels::whiten_half` y `overlay/math.rs`; overlay real con verificación manual.
- Comentarios y rustdoc en español. `cargo fmt` antes de cada verificación.
- **Commits: SOLO con aprobación humana previa.** Único commit: `v0.2.2 — F2: overlay de selección de región`.
- Nota `windows` 0.62: ajustar firmas según compilador sin cambiar diseño (ver memoria de quirks).

---

### Task 1: `whiten_half` + acceso de escritura al `Dib`

**Files:**
- Modify: `crates/platform-win/src/pixels.rs`
- Modify: `crates/platform-win/src/gdi/mod.rs` (línea `mod raii;` → `pub(crate) mod raii;`)
- Modify: `crates/platform-win/src/gdi/raii.rs` (`bits` pasa a `*mut u8`; añadir `bits_mut`)

**Interfaces:**
- Consumes: `pixels`, `raii::Dib` existentes.
- Produces: `pixels::whiten_half(pixels: &mut [u8])` (blanquea RGB al 50 %, alfa intacto); `Dib::bits_mut(&mut self) -> &mut [u8]`; `gdi::raii` visible para `overlay` (`pub(crate)`).

- [ ] **Step 1: Tests que fallan** — añadir a los tests de `pixels.rs`:

```rust
    #[test]
    fn whiten_half_mezcla_con_blanco_al_cincuenta() {
        let mut px = vec![0u8, 100, 255, 42];
        whiten_half(&mut px);
        assert_eq!(px, vec![127, 177, 255, 42]); // alfa intacto
    }

    #[test]
    fn whiten_half_es_idempotente_en_blanco_puro() {
        let mut px = vec![255u8, 255, 255, 255];
        whiten_half(&mut px);
        assert_eq!(px, vec![255, 255, 255, 255]);
    }
```

- [ ] **Step 2: Rojo** — `cargo test -p platform-win` → FAIL `whiten_half`.

- [ ] **Step 3: Implementar** — en `pixels.rs`:

```rust
/// Mezcla cada canal RGB con blanco al 50 % (máscara del overlay, D10);
/// el alfa no se toca.
pub fn whiten_half(pixels: &mut [u8]) {
    for px in pixels.chunks_exact_mut(4) {
        for c in &mut px[..3] {
            *c = ((*c as u16 + 255) / 2) as u8;
        }
    }
}
```

En `gdi/mod.rs`: `mod raii;` → `pub(crate) mod raii;`. En `raii.rs`, el campo `bits: *const u8` pasa a `bits: *mut u8` (ajustar el cast en `new_32bpp`: `bits as *mut u8`, y en `bits()` castear con `.cast_const()`), y añadir:

```rust
    /// Bits BGRA escribibles (para volcar un frame al bitmap).
    pub(crate) fn bits_mut(&mut self) -> &mut [u8] {
        // SAFETY: mismo buffer y longitud que bits(); acceso exclusivo
        // garantizado por &mut self.
        unsafe { core::slice::from_raw_parts_mut(self.bits, self.len) }
    }
```

- [ ] **Step 4: Verde** — `cargo fmt && cargo test -p platform-win` → PASS (11 normales).

- [ ] **Step 5: Staging** — `git add crates/platform-win/`

---

### Task 2: Geometría pura (`overlay/math.rs`)

**Files:**
- Create: `crates/platform-win/src/overlay/math.rs`
- Create: `crates/platform-win/src/overlay/mod.rs` (solo `pub(crate) mod math;` por ahora — el resto llega en Task 3; declarar `pub mod overlay;` en `lib.rs`)

**Interfaces:**
- Consumes: `rustcapture_core::ports::Rect`.
- Produces (todo `pub(crate)`, coordenadas locales de la ventana del overlay):
  - Consts: `LUPA_SRC=60`, `LUPA_W=300`, `LUPA_ZOOM_H=300`, `LUPA_COORD_H=30`, `LUPA_HELP_H=170`, `LUPA_H=500`.
  - `rect_between(a: (i32,i32), b: (i32,i32)) -> Rect` — normaliza el arrastre; mínimo 1×1.
  - `lupa_source(cursor: (i32,i32), frame_w: u32, frame_h: u32) -> Rect` — 60×60 centrado, clampeado.
  - `lupa_box_pos(monitor: Rect, cursor: (i32,i32)) -> (i32,i32)` — inferior-derecha con margen 20; si el cursor entra en la caja inflada 40 px, superior-izquierda.

- [ ] **Step 1: Tests que fallan** — `overlay/math.rs`:

```rust
//! Geometría pura del overlay (TDD): coordenadas locales de la ventana.

use rustcapture_core::ports::Rect;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_between_normaliza_las_cuatro_direcciones() {
        let esperado = Rect::new(10, 20, 30, 40);
        assert_eq!(rect_between((10, 20), (40, 60)), esperado);
        assert_eq!(rect_between((40, 60), (10, 20)), esperado);
        assert_eq!(rect_between((40, 20), (10, 60)), esperado);
        assert_eq!(rect_between((10, 60), (40, 20)), esperado);
    }

    #[test]
    fn rect_between_de_un_clic_es_un_pixel() {
        assert_eq!(rect_between((5, 5), (5, 5)), Rect::new(5, 5, 1, 1));
    }

    #[test]
    fn lupa_source_centra_sobre_el_cursor() {
        assert_eq!(
            lupa_source((100, 100), 1920, 1080),
            Rect::new(70, 70, 60, 60)
        );
    }

    #[test]
    fn lupa_source_clampa_en_las_esquinas() {
        assert_eq!(lupa_source((0, 0), 1920, 1080), Rect::new(0, 0, 60, 60));
        assert_eq!(
            lupa_source((1919, 1079), 1920, 1080),
            Rect::new(1860, 1020, 60, 60)
        );
    }

    #[test]
    fn lupa_box_va_a_la_esquina_inferior_derecha() {
        let monitor = Rect::new(0, 0, 1920, 1080);
        assert_eq!(
            lupa_box_pos(monitor, (100, 100)),
            (1920 - 300 - 20, 1080 - 500 - 20)
        );
    }

    #[test]
    fn lupa_box_salta_cuando_el_cursor_se_acerca() {
        let monitor = Rect::new(0, 0, 1920, 1080);
        // Cursor dentro de la zona de la caja (esquina inferior derecha).
        assert_eq!(lupa_box_pos(monitor, (1700, 900)), (20, 20));
    }

    #[test]
    fn lupa_box_respeta_monitores_con_origen_negativo() {
        let monitor = Rect::new(-1920, 0, 1920, 1080);
        assert_eq!(
            lupa_box_pos(monitor, (-1800, 100)),
            (-1920 + 1920 - 300 - 20, 1080 - 500 - 20)
        );
    }
}
```

`overlay/mod.rs` provisional:

```rust
//! Overlay de selección de región (capa de selección de D10).

pub(crate) mod math;
```

y en `lib.rs` (orden alfabético): `pub mod overlay;`

- [ ] **Step 2: Rojo** — `cargo test -p platform-win` → FAIL.

- [ ] **Step 3: Implementar** — en `math.rs` sobre los tests:

```rust
pub(crate) const LUPA_SRC: i32 = 60;
pub(crate) const LUPA_W: i32 = 300;
pub(crate) const LUPA_ZOOM_H: i32 = 300;
pub(crate) const LUPA_COORD_H: i32 = 30;
pub(crate) const LUPA_HELP_H: i32 = 170;
pub(crate) const LUPA_H: i32 = LUPA_ZOOM_H + LUPA_COORD_H + LUPA_HELP_H;
const MARGEN: i32 = 20;
const ZONA_SALTO: i32 = 40;

/// Rect normalizado entre dos puntos de arrastre; mínimo 1×1 (f.19).
pub(crate) fn rect_between(a: (i32, i32), b: (i32, i32)) -> Rect {
    Rect::new(
        a.0.min(b.0),
        a.1.min(b.1),
        (a.0 - b.0).unsigned_abs().max(1),
        (a.1 - b.1).unsigned_abs().max(1),
    )
}

/// Fuente del zoom: 60×60 centrado en el cursor, sin salirse del frame.
pub(crate) fn lupa_source(cursor: (i32, i32), frame_w: u32, frame_h: u32) -> Rect {
    let max_x = (frame_w as i32 - LUPA_SRC).max(0);
    let max_y = (frame_h as i32 - LUPA_SRC).max(0);
    Rect::new(
        (cursor.0 - LUPA_SRC / 2).clamp(0, max_x),
        (cursor.1 - LUPA_SRC / 2).clamp(0, max_y),
        LUPA_SRC as u32,
        LUPA_SRC as u32,
    )
}

/// Esquina de la caja de lupa: inferior-derecha del monitor; si el
/// cursor entra en la caja inflada, salta a superior-izquierda.
pub(crate) fn lupa_box_pos(monitor: Rect, cursor: (i32, i32)) -> (i32, i32) {
    let br = (
        monitor.right() as i32 - LUPA_W - MARGEN,
        monitor.bottom() as i32 - LUPA_H - MARGEN,
    );
    let dentro = cursor.0 >= br.0 - ZONA_SALTO
        && cursor.0 < br.0 + LUPA_W + ZONA_SALTO
        && cursor.1 >= br.1 - ZONA_SALTO
        && cursor.1 < br.1 + LUPA_H + ZONA_SALTO;
    if dentro {
        (monitor.x + MARGEN, monitor.y + MARGEN)
    } else {
        br
    }
}
```

- [ ] **Step 4: Verde** — `cargo fmt && cargo test -p platform-win` → PASS (11 + 7 = 18).

- [ ] **Step 5: Staging** — `git add crates/platform-win/`

---

### Task 3: Ventana del overlay (`overlay/mod.rs`)

**Files:**
- Modify: `crates/platform-win/src/overlay/mod.rs`

**Interfaces:**
- Consumes: `math`, `pixels::{rgba_to_bgra, whiten_half}`, `gdi::{GdiScreenSource, raii}`, `alerts::error_beep`.
- Produces: `pub fn select_region() -> Option<Rect>` — bloqueante en el hilo de UI; `None` = cancelado o fallo (con beep). Rect en coordenadas de escritorio.

- [ ] **Step 1: Implementar** — `overlay/mod.rs` completo:

```rust
//! Overlay de selección de región (capa de selección de D10): frame
//! congelado + máscara blanca 50 %, arrastre limpio, crosshair y lupa.
//!
//! Hilos: SOLO desde el hilo de UI (bucle modal anidado, como los
//! menús). El estado lo posee `select_region`; el wndproc lo usa vía
//! puntero crudo y nunca lo libera.

pub(crate) mod math;

use rustcapture_core::ports::{Frame, Rect, ScreenSource};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BitBlt, COLOR_BTNFACE, COLORONCOLOR, CreateSolidBrush, DT_CENTER, DrawTextW, FillRect,
    FrameRect, GetMonitorInfoW, GetSysColorBrush, InvalidateRect, MONITOR_DEFAULTTONEAREST,
    MONITORINFO, MonitorFromPoint, SRCCOPY, SetBkMode, SetStretchBltMode, SetTextColor,
    StretchBlt, TRANSPARENT, BeginPaint, DeleteObject, EndPaint, PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture, VK_ESCAPE};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::w;

use crate::gdi::GdiScreenSource;
use crate::gdi::raii::{Dib, MemDc, ScreenDc, Selected};

/// Estado del overlay; lo posee `select_region`, el wndproc solo lo usa.
struct OverlayState {
    origin: (i32, i32),
    width: i32,
    height: i32,
    original: Dib,
    whitened: Dib,
    back: Dib,
    drag_start: Option<(i32, i32)>,
    cursor: (i32, i32),
    /// None = en curso; Some(None) = cancelado; Some(Some(r)) = elegido
    /// (coordenadas LOCALES; `select_region` las traduce a escritorio).
    outcome: Option<Option<Rect>>,
}

/// Selección interactiva. Bloquea el hilo de UI hasta soltar el botón o
/// Esc. `None` = cancelado o fallo al congelar (con beep).
pub fn select_region() -> Option<Rect> {
    match run() {
        Ok(resultado) => resultado,
        Err(_) => {
            crate::alerts::error_beep();
            None
        }
    }
}

fn dib_from_frame(dc: &MemDc, frame: &Frame) -> windows::core::Result<Dib> {
    let mut dib = Dib::new_32bpp(dc, frame.width, frame.height)?;
    let mut px = frame.pixels.clone();
    crate::pixels::rgba_to_bgra(&mut px);
    dib.bits_mut().copy_from_slice(&px);
    Ok(dib)
}

fn run() -> windows::core::Result<Option<Rect>> {
    // Congelar el escritorio completo.
    let mut source = GdiScreenSource::new();
    let desktop = GdiScreenSource::desktop_rect(&source);
    let frozen = ScreenSource::capture_region(&mut source, desktop).map_err(|e| {
        windows::core::Error::new(windows::Win32::Foundation::E_FAIL, e.to_string())
    })?;
    let mut blanqueado = frozen.clone();
    crate::pixels::whiten_half(&mut blanqueado.pixels);

    // Bitmaps GDI (original, máscara y back buffer del doble buffer).
    let screen = ScreenDc::get()?;
    let dc = MemDc::compatible_with(&screen)?;
    let state = Box::new(OverlayState {
        origin: (desktop.x, desktop.y),
        width: desktop.width as i32,
        height: desktop.height as i32,
        original: dib_from_frame(&dc, &frozen)?,
        whitened: dib_from_frame(&dc, &blanqueado)?,
        back: Dib::new_32bpp(&dc, desktop.width, desktop.height)?,
        drag_start: None,
        cursor: (0, 0),
        outcome: None,
    });
    drop(dc);
    drop(screen);
    let state_ptr = Box::into_raw(state);

    // SAFETY: creación de ventana estándar; state_ptr vive hasta el
    // Box::from_raw del final, siempre después de destruir la ventana.
    let resultado = unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = w!("RustCaptureOverlay");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_CROSS)?,
            ..Default::default()
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST,
            class,
            w!("RustCapture Selección"),
            WS_POPUP | WS_VISIBLE,
            desktop.x,
            desktop.y,
            desktop.width as i32,
            desktop.height as i32,
            None,
            None,
            Some(instance.into()),
            Some(state_ptr.cast()),
        )?;
        _ = SetForegroundWindow(hwnd); // necesario para recibir Esc

        // Bucle modal: consume mensajes hasta que el wndproc resuelva.
        let mut msg = MSG::default();
        while (*state_ptr).outcome.is_none() && GetMessageW(&mut msg, None, 0, 0).as_bool() {
            _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let state = Box::from_raw(state_ptr);
        let origin = state.origin;
        state
            .outcome
            .flatten()
            .map(|local| Rect::new(local.x + origin.0, local.y + origin.1, local.width, local.height))
    };
    Ok(resultado)
}

fn state_mut<'a>(hwnd: HWND) -> Option<&'a mut OverlayState> {
    // SAFETY: puntero puesto por WM_NCCREATE; el dueño (select_region)
    // no lo toca mientras el bucle despacha mensajes.
    unsafe { ((GetWindowLongPtrW(hwnd, GWLP_USERDATA)) as *mut OverlayState).as_mut() }
}

fn punto(lparam: LPARAM) -> (i32, i32) {
    ((lparam.0 & 0xFFFF) as i16 as i32, ((lparam.0 >> 16) & 0xFFFF) as i16 as i32)
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: cada rama documenta su invariante; el estado nunca se
    // libera aquí (lo posee select_region).
    unsafe {
        match msg {
            WM_NCCREATE => {
                let cs = &*(lparam.0 as *const CREATESTRUCTW);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_ERASEBKGND => LRESULT(1), // el doble buffer pinta todo
            WM_LBUTTONDOWN => {
                if let Some(state) = state_mut(hwnd) {
                    state.drag_start = Some(punto(lparam));
                    state.cursor = punto(lparam);
                    SetCapture(hwnd);
                    _ = InvalidateRect(Some(hwnd), None, false);
                }
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                if let Some(state) = state_mut(hwnd) {
                    state.cursor = punto(lparam);
                    _ = InvalidateRect(Some(hwnd), None, false);
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                if let Some(state) = state_mut(hwnd) {
                    if let Some(start) = state.drag_start {
                        _ = ReleaseCapture();
                        state.outcome = Some(Some(math::rect_between(start, punto(lparam))));
                        _ = DestroyWindow(hwnd);
                    }
                }
                LRESULT(0)
            }
            WM_KEYDOWN if wparam.0 as u16 == VK_ESCAPE.0 => {
                if let Some(state) = state_mut(hwnd) {
                    state.outcome = Some(None);
                    _ = DestroyWindow(hwnd);
                }
                LRESULT(0)
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let hdc = BeginPaint(hwnd, &mut ps);
                if let Some(state) = state_mut(hwnd) {
                    _ = pintar(hdc, state);
                }
                _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

/// Compone la escena en el back buffer y la vuelca de un BitBlt.
fn pintar(
    hdc: windows::Win32::Graphics::Gdi::HDC,
    state: &mut OverlayState,
) -> windows::core::Result<()> {
    let screen = ScreenDc::get()?;
    let back_dc = MemDc::compatible_with(&screen)?;
    let src_dc = MemDc::compatible_with(&screen)?;
    let _back = Selected::bitmap(&back_dc, &state.back)?;

    // SAFETY: DCs y bitmaps vivos (RAII); operaciones GDI estándar.
    unsafe {
        // 1. Máscara blanca en todo el escritorio.
        {
            let _s = Selected::bitmap(&src_dc, &state.whitened)?;
            BitBlt(back_dc.0, 0, 0, state.width, state.height, Some(src_dc.0), 0, 0, SRCCOPY)?;
        }
        // 2. Región seleccionada limpia + borde rojo.
        let seleccion = state.drag_start.map(|s| math::rect_between(s, state.cursor));
        if let Some(sel) = seleccion {
            let _o = Selected::bitmap(&src_dc, &state.original)?;
            BitBlt(
                back_dc.0,
                sel.x,
                sel.y,
                sel.width as i32,
                sel.height as i32,
                Some(src_dc.0),
                sel.x,
                sel.y,
                SRCCOPY,
            )?;
            let rojo = CreateSolidBrush(COLORREF(0x0000FF));
            let marco = RECT {
                left: sel.x - 1,
                top: sel.y - 1,
                right: sel.x + sel.width as i32 + 1,
                bottom: sel.y + sel.height as i32 + 1,
            };
            FrameRect(back_dc.0, &marco, rojo);
            _ = DeleteObject(rojo.into());
        }
        // 3. Caja de lupa.
        pintar_lupa(&back_dc, &src_dc, state, seleccion)?;
        // 4. Volcado único a pantalla.
        BitBlt(hdc, 0, 0, state.width, state.height, Some(back_dc.0), 0, 0, SRCCOPY)?;
    }
    Ok(())
}

/// Caja 300×500: zoom 5× (300×300) + coordenadas (300×30) + ayuda (300×170).
fn pintar_lupa(
    back_dc: &MemDc,
    src_dc: &MemDc,
    state: &OverlayState,
    seleccion: Option<Rect>,
) -> windows::core::Result<()> {
    // Monitor del cursor, en coordenadas locales de la ventana.
    let cursor_desktop = POINT {
        x: state.cursor.0 + state.origin.0,
        y: state.cursor.1 + state.origin.1,
    };
    // SAFETY: consultas de monitor sin precondiciones.
    let monitor_local = unsafe {
        let monitor = MonitorFromPoint(cursor_desktop, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            Rect::new(
                info.rcMonitor.left - state.origin.0,
                info.rcMonitor.top - state.origin.1,
                (info.rcMonitor.right - info.rcMonitor.left).max(0) as u32,
                (info.rcMonitor.bottom - info.rcMonitor.top).max(0) as u32,
            )
        } else {
            Rect::new(0, 0, state.width as u32, state.height as u32)
        }
    };
    let (bx, by) = math::lupa_box_pos(monitor_local, state.cursor);
    let fuente = math::lupa_source(state.cursor, state.width as u32, state.height as u32);

    // SAFETY: DCs vivos; dibujo GDI estándar.
    unsafe {
        // Zoom 5× desde el frame original.
        {
            let _o = Selected::bitmap(src_dc, &state.original)?;
            SetStretchBltMode(back_dc.0, COLORONCOLOR);
            StretchBlt(
                back_dc.0,
                bx,
                by,
                math::LUPA_W,
                math::LUPA_ZOOM_H,
                Some(src_dc.0),
                fuente.x,
                fuente.y,
                math::LUPA_SRC,
                math::LUPA_SRC,
                SRCCOPY,
            );
        }
        // Cruz central 1 px (roja) del zoom.
        let rojo = CreateSolidBrush(COLORREF(0x0000FF));
        let cx = bx + math::LUPA_W / 2;
        let cy = by + math::LUPA_ZOOM_H / 2;
        FillRect(
            back_dc.0,
            &RECT { left: bx, top: cy, right: bx + math::LUPA_W, bottom: cy + 1 },
            rojo,
        );
        FillRect(
            back_dc.0,
            &RECT { left: cx, top: by, right: cx + 1, bottom: by + math::LUPA_ZOOM_H },
            rojo,
        );
        _ = DeleteObject(rojo.into());

        // Barra de coordenadas (gris claro).
        let coord_rect = RECT {
            left: bx,
            top: by + math::LUPA_ZOOM_H,
            right: bx + math::LUPA_W,
            bottom: by + math::LUPA_ZOOM_H + math::LUPA_COORD_H,
        };
        FillRect(back_dc.0, &coord_rect, GetSysColorBrush(COLOR_BTNFACE));
        SetBkMode(back_dc.0, TRANSPARENT);
        SetTextColor(back_dc.0, COLORREF(0x000000));
        let mut coords: Vec<u16> = format!("X, Y = {},{}", cursor_desktop.x, cursor_desktop.y)
            .encode_utf16()
            .collect();
        let mut rc = coord_rect;
        rc.top += 6;
        DrawTextW(back_dc.0, &mut coords, &mut rc, DT_CENTER);

        // Bloque de ayuda (azul RGB(30,80,160), texto blanco).
        let ayuda_rect = RECT {
            left: bx,
            top: by + math::LUPA_ZOOM_H + math::LUPA_COORD_H,
            right: bx + math::LUPA_W,
            bottom: by + math::LUPA_H,
        };
        let azul = CreateSolidBrush(COLORREF(0x00A0501E));
        FillRect(back_dc.0, &ayuda_rect, azul);
        _ = DeleteObject(azul.into());
        SetTextColor(back_dc.0, COLORREF(0x00FFFFFF));
        let ayuda = match seleccion {
            Some(sel) => format!(
                "Arrastra para seleccionar\nSuelta para capturar\nESC para cancelar\n\nSelección: {}×{} px",
                sel.width, sel.height
            ),
            None => "Arrastra para seleccionar\nSuelta para capturar\nESC para cancelar".to_string(),
        };
        let mut ayuda: Vec<u16> = ayuda.encode_utf16().collect();
        let mut rc = ayuda_rect;
        rc.top += 24;
        DrawTextW(back_dc.0, &mut ayuda, &mut rc, DT_CENTER);

        // Marco exterior de la caja completa.
        let caja = RECT {
            left: bx,
            top: by,
            right: bx + math::LUPA_W,
            bottom: by + math::LUPA_H,
        };
        let negro = CreateSolidBrush(COLORREF(0x000000));
        FrameRect(back_dc.0, &caja, negro);
        _ = DeleteObject(negro.into());
    }
    Ok(())
}
```

- [ ] **Step 2: Compilar** — `cargo fmt && cargo test -p platform-win` → PASS (18 normales; el overlay no tiene tests automáticos).

- [ ] **Step 3: Staging** — `git add crates/platform-win/`

---

### Task 4: Cableado — botón «Región», hotkey y barra auto-oculta

**Files:**
- Modify: `crates/platform-win/src/bar.rs`
- Modify: `crates/gui/src/main.rs`

**Interfaces:**
- Consumes: `overlay::select_region` (Task 3).
- Produces:
  - `bar`: const `WM_APP_REGION: u32 = WM_APP + 2`; botón «Región» habilitado → `PostMessageW(hwnd, WM_APP_REGION, ...)`; rama `WM_APP_REGION` en el wndproc: ocultar barra → 150 ms → `select_region()` → mostrar barra → publicar `CaptureRequested(Region(rect))`.
  - `run_message_loop(tx, region_hotkey: Option<HotkeyId>, bar: &Bar)` — el `WM_HOTKEY` de región se traduce a `WM_APP_REGION` (no pasa por el orquestador).
  - `gui`: registra `config.hotkeys.region` aparte de los bindings y pasa su id al bucle.

- [ ] **Step 1: Implementar `bar.rs`**

Consts: junto a `WM_TRAY`:

```rust
/// Petición de selección de región (botón o hotkey): debe correr en el
/// hilo de UI, nunca en el orquestador.
pub(crate) const WM_APP_REGION: u32 = WM_APP + 2;
```

Botón habilitado: `(ID_REGION, w!("Región"), true),`. En `on_command`:

```rust
        // f.13 interactiva: el overlay corre en el hilo de UI; se
        // despacha como mensaje para salir del contexto del clic.
        ID_REGION => {
            // SAFETY: post a la propia ventana del wndproc.
            unsafe { _ = PostMessageW(Some(hwnd), WM_APP_REGION, WPARAM(0), LPARAM(0)) };
        }
```

Rama nueva del wndproc (antes de `m if m == WM_TRAY`):

```rust
            m if m == WM_APP_REGION => {
                flujo_region(hwnd);
                LRESULT(0)
            }
```

Y la función:

```rust
/// Oculta la barra, abre el overlay y publica la región elegida.
fn flujo_region(hwnd: HWND) {
    // SAFETY: hwnd válido (viene del wndproc); ocultar/mostrar la propia
    // ventana alrededor del overlay para no salir en la captura.
    unsafe {
        _ = ShowWindow(hwnd, SW_HIDE);
        std::thread::sleep(std::time::Duration::from_millis(150));
        let resultado = crate::overlay::select_region();
        _ = ShowWindow(hwnd, SW_SHOW);
        if let (Some(rect), Some(state)) = (resultado, state_ref(hwnd)) {
            let _ = state.tx.send(AppEvent::CaptureRequested(CaptureRequest {
                mode: ModeRequest::Region(rect),
                destination: state.destination,
            }));
        }
    }
}
```

`run_message_loop` pasa a:

```rust
/// Bucle de mensajes del hilo UI. Los `WM_HOTKEY` (hwnd nulo) van al bus
/// salvo el de región, que se traduce a `WM_APP_REGION` (UI).
pub fn run_message_loop(tx: &Sender<AppEvent>, region_hotkey: Option<HotkeyId>, bar: &Bar) {
    let mut msg = MSG::default();
    // SAFETY: bucle GetMessage estándar del hilo que posee las ventanas.
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_HOTKEY && msg.hwnd.is_invalid() {
                let id = HotkeyId(msg.wParam.0 as u32);
                if Some(id) == region_hotkey {
                    _ = PostMessageW(Some(bar.hwnd), WM_APP_REGION, WPARAM(0), LPARAM(0));
                } else {
                    let _ = tx.send(AppEvent::HotkeyPressed(id));
                }
                continue;
            }
            _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
```

- [ ] **Step 2: Implementar `gui/main.rs`**

Tras el bucle de registro de hotkeys existente, registrar el de región (no va a bindings):

```rust
    // Hotkey de región: se resuelve en el hilo de UI (overlay), no en el
    // orquestador; run_message_loop lo traduce a WM_APP_REGION.
    let region_hotkey = Hotkey::parse(&config.hotkeys.region)
        .ok()
        .and_then(|hk| hotkeys.register(hk).ok());
    if region_hotkey.is_none() {
        platform_win::alerts::error_beep();
    }
```

y la llamada final: `run_message_loop(&tx, region_hotkey, &bar);`

- [ ] **Step 3: Compilar y tests** — `cargo fmt && cargo build --workspace && cargo test --workspace`
Expected: build limpio; 81 core + 18 platform-win + 10 cli = 109 tests.

- [ ] **Step 4: Staging** — `git add crates/platform-win/ crates/gui/`

---

### Task 5: Verificación manual guiada con el humano

- [ ] **Step 1: Lanzar** `./target/debug/rustcapture-gui.exe` (background).

- [ ] **Step 2: Checklist**

1. Botón «Región» (ya activo) → la barra desaparece y TODAS las pantallas quedan veladas en blanco al 50 % con cursor crosshair.
2. Arrastrar en cualquier dirección: la zona seleccionada se ve limpia con borde rojo; al soltar → beep y la región está en el portapapeles; la barra vuelve.
3. La caja 300×500: zoom 5× siguiendo al cursor, `X, Y = …` correcto (esquina superior izquierda del monitor primario ≈ 0,0), ayuda en azul con `Selección: W×H` durante el arrastre.
4. Acercar el cursor a la caja → salta a la esquina superior izquierda del monitor.
5. `Esc` → todo vuelve sin capturar; la barra reaparece.
6. `Ctrl+PrtScn` desde cualquier app = mismo flujo que el botón.
7. Con dos monitores: la máscara cubre ambos y se puede seleccionar en cualquiera.
8. Clic sin arrastre → captura 1×1 (sin crash).

- [ ] **Step 3: Fallos → `systematic-debugging`; sin OK humano no hay cierre.**

---

### Task 6: Verificación final y propuesta de commit

- [ ] **Step 1:** `cargo build --workspace && cargo test --workspace && cargo test -p platform-win -- --ignored && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check`
Expected: 109 tests + 6 humo; clippy y formato limpios.

- [ ] **Step 2: Roadmap** — `- ⏳ Overlay de selección de región (capa de selección de D10)…` → `- ✅ …`

- [ ] **Step 3: Proponer commit (NO ejecutar sin aprobación)**

```
v0.2.2 — F2: overlay de selección de región

Capa de selección de D10: frame congelado con máscara blanca 50 %,
arrastre limpio con borde rojo, crosshair y caja de lupa 300×500 (zoom
5×, coordenadas y ayuda) con salto de esquina. Activa el botón Región y
ctrl+printscreen; publica CaptureRequested(Region) — el mismo camino que
--region en la CLI. La barra se auto-oculta durante la selección.
```
