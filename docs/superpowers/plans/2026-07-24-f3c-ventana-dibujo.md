# F3/C — Ventana de dibujo — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** La ventana de dibujo (Ventana2): paleta izquierda (7 herramientas + deshacer/rehacer), lienzo con preview en vivo, barra inferior (8 colores + «Más…», grosores, tamaño/negrita de texto, OK/Cancelar), texto in situ con EDIT flotante, y la integración con el editor (botón Draw activo, flag de sucio, avisos). Spec: `2026-07-24-ventana-dibujo-design.md`.

**Architecture:** Patrón modal del editor/overlay. El estado cachea el frame «comprometido» (base + documento) como DIB y solo lo regenera al cambiar el documento; el preview del arrastre renderiza la anotación provisional sobre una copia. Mapeo puro vista↔frame (`draw/math.rs`, TDD) para dibujar exacto sobre la imagen encajada. Ventana de tamaño fijo (sin resize) para no reflow-ear controles.

**Tech Stack:** Sin deps nuevas. APIs: `ChooseColorW` (feature `Win32_UI_Controls_Dialogs` ya presente), `SetWindowSubclass` (`Win32_UI_Shell` ya presente).

## Global Constraints

- Reglas interop: RAII, `// SAFETY:`, nada de `windows` en firmas públicas.
- Colores predefinidos EXACTOS: negro, blanco, rojo, verde, azul, amarillo, naranja RGB(255,140,0), morado RGB(128,0,128). Grosores 1/2/3/5/8. Tamaños de texto 12/16/20/28/36. Resaltador = color actual con alfa 128.
- Texto: clic coloca EDIT multilínea con fuente/tamaño/negrita/color actuales; perder el foco confirma; `Esc` dentro cancela; vacío no crea nada. Sin fuentes del sistema → botón Texto gris.
- `Ctrl+Z`/`Ctrl+Y` = undo/redo. `Esc`/✕/Cancelar con documento no vacío → confirmar descarte.
- OK devuelve el frame horneado; el editor lo adopta, se marca sucio, y cerrar sucio pregunta; Guardar/Copiar con éxito limpian el sucio.
- Comentarios y rustdoc en español; `cargo fmt` antes de verificar.
- **Commits: SOLO con aprobación humana previa.** Único commit: `v0.2.5 — F3/C: ventana de dibujo (Draw activo)`.

---

### Task 1: `draw/math.rs` — mapeo vista↔frame (TDD)

**Files:**
- Create: `crates/platform-win/src/draw/math.rs`
- Create: `crates/platform-win/src/draw/mod.rs` (provisional `pub(crate) mod math;`)
- Modify: `crates/platform-win/src/lib.rs` (`pub mod draw;`)

**Interfaces:**
- Consumes: `Rect`.
- Produces (`pub(crate)`): `view_to_frame(p: (i32,i32), destino: Rect, frame: (u32,u32)) -> Option<(i32,i32)>` (None fuera del destino; escala inversa con clamp al borde del frame); `frame_to_view(p: (i32,i32), destino: Rect, frame: (u32,u32)) -> (i32,i32)`.

- [ ] **Step 1: Tests que fallan** — `draw/math.rs`:

```rust
//! Mapeo puro entre coordenadas de la vista (lienzo encajado) y del
//! frame real (TDD): el dibujo debe caer exacto sobre la imagen.

use rustcapture_core::ports::Rect;

#[cfg(test)]
mod tests {
    use super::*;

    // Imagen 200×100 encajada en destino (10,20,100,50): escala 0.5.
    const DESTINO: Rect = Rect {
        x: 10,
        y: 20,
        width: 100,
        height: 50,
    };
    const FRAME: (u32, u32) = (200, 100);

    #[test]
    fn dentro_del_destino_escala_al_frame() {
        assert_eq!(view_to_frame((10, 20), DESTINO, FRAME), Some((0, 0)));
        assert_eq!(view_to_frame((60, 45), DESTINO, FRAME), Some((100, 50)));
        assert_eq!(view_to_frame((109, 69), DESTINO, FRAME), Some((198, 98)));
    }

    #[test]
    fn fuera_del_destino_es_none() {
        assert_eq!(view_to_frame((9, 20), DESTINO, FRAME), None);
        assert_eq!(view_to_frame((10, 70), DESTINO, FRAME), None);
    }

    #[test]
    fn frame_to_view_es_el_inverso() {
        assert_eq!(frame_to_view((0, 0), DESTINO, FRAME), (10, 20));
        assert_eq!(frame_to_view((100, 50), DESTINO, FRAME), (60, 45));
    }

    #[test]
    fn degenerado_no_divide_por_cero() {
        let destino = Rect::new(0, 0, 0, 0);
        assert_eq!(view_to_frame((5, 5), destino, FRAME), None);
        assert_eq!(frame_to_view((5, 5), destino, FRAME), (0, 0));
    }
}
```

- [ ] **Step 2: Rojo.**

- [ ] **Step 3: Implementar**:

```rust
/// Punto de la vista → píxel del frame; `None` fuera del área encajada.
pub(crate) fn view_to_frame(p: (i32, i32), destino: Rect, frame: (u32, u32)) -> Option<(i32, i32)> {
    if destino.is_empty() || frame.0 == 0 || frame.1 == 0 {
        return None;
    }
    let dentro = p.0 >= destino.x
        && (p.0 as i64) < destino.right()
        && p.1 >= destino.y
        && (p.1 as i64) < destino.bottom();
    if !dentro {
        return None;
    }
    let fx = (p.0 - destino.x) as i64 * frame.0 as i64 / destino.width as i64;
    let fy = (p.1 - destino.y) as i64 * frame.1 as i64 / destino.height as i64;
    Some((
        (fx as i32).clamp(0, frame.0 as i32 - 1),
        (fy as i32).clamp(0, frame.1 as i32 - 1),
    ))
}

/// Píxel del frame → punto de la vista (esquina del píxel escalado).
pub(crate) fn frame_to_view(p: (i32, i32), destino: Rect, frame: (u32, u32)) -> (i32, i32) {
    if destino.is_empty() || frame.0 == 0 || frame.1 == 0 {
        return (0, 0);
    }
    (
        destino.x + (p.0 as i64 * destino.width as i64 / frame.0 as i64) as i32,
        destino.y + (p.1 as i64 * destino.height as i64 / frame.1 as i64) as i32,
    )
}
```

`draw/mod.rs` provisional (doc + `pub(crate) mod math;`); `lib.rs`: `pub mod draw;` (orden alfabético).

- [ ] **Step 4: Verde** — `cargo fmt && cargo test -p platform-win` → PASS (22 + 4 = 26).

- [ ] **Step 5: Staging** — `git add crates/platform-win/`

---

### Task 2: Ventana de dibujo (paleta, lienzo, preview, undo/redo, colores)

**Files:**
- Modify: `crates/platform-win/src/draw/mod.rs` (ventana completa)

**Interfaces:**
- Consumes: `math`, `editor::math::fit_rect` (hacer `pub(crate)` visible entre módulos: ya lo es), `gdi::dib_from_frame`, `annotate::*` del core, `alerts`.
- Produces: `pub fn show_draw(base: Frame) -> Option<Frame>` — modal; `Some(frame_horneado)` con OK, `None` con Cancelar/Esc/✕. El texto in situ llega en la Task 3 (aquí la herramienta Texto no hace nada aún).

- [ ] **Step 1: Implementar la ventana** — `draw/mod.rs` (estructura completa; el detalle del EDIT de texto queda para la Task 3):

Constantes e IDs:

```rust
const PALETTE_W: i32 = 96;
const BOTTOM_H: i32 = 48;
const ID_TOOL_BASE: u16 = 4001; // Rect..Texto = 4001..=4007 (orden del enum)
const ID_UNDO: u16 = 4010;
const ID_REDO: u16 = 4011;
const ID_MAS_COLOR: u16 = 4020;
const ID_COLOR_BASE: u16 = 4030; // 8 swatches 4030..=4037
const ID_GROSOR_BASE: u16 = 4040; // 4040..=4044 → [1,2,3,5,8]
const ID_TAMANO_BASE: u16 = 4050; // 4050..=4054 → [12,16,20,28,36]
const ID_BOLD: u16 = 4060;
const ID_OK: u16 = 4070;
const ID_CANCEL: u16 = 4071;
const GROSORES: [u32; 5] = [1, 2, 3, 5, 8];
const TAMANOS: [f32; 5] = [12.0, 16.0, 20.0, 28.0, 36.0];
const COLORES: [Color; 8] = [
    Color::rgb(0, 0, 0),
    Color::rgb(255, 255, 255),
    Color::rgb(255, 0, 0),
    Color::rgb(0, 200, 0),
    Color::rgb(0, 90, 255),
    Color::rgb(255, 220, 0),
    Color::rgb(255, 140, 0),
    Color::rgb(128, 0, 128),
];
```

Estado y flujo (mismo patrón modal del editor):

```rust
#[derive(Clone, Copy, PartialEq)]
enum Tool { Rect, Ellipse, Line, Arrow, Pen, Highlight, Text }

struct DragState { start: (i32, i32), current: (i32, i32), points: Vec<(i32, i32)> }

struct DrawState {
    base: Frame,
    committed: Frame,
    committed_dib: Dib,
    doc: Document,
    history: History,
    ctx: RenderContext,
    tiene_fuente: bool,
    tool: Tool,
    color: Color,
    thickness: u32,
    text_size: f32,
    bold: bool,
    drag: Option<DragState>,
    edit: Option<EditBox>, // Task 3
    outcome: Option<Option<Frame>>,
}
```

`show_draw`: carga fuentes (`std::fs::read` de segoeui/segoeuib; fallo → `sin_fuente` + `tiene_fuente=false`), construye estado (committed = base.clone(), DIB), crea ventana fija (`WS_CAPTION|WS_SYSMENU|WS_MINIMIZEBOX`, tamaño = imagen clampeada 1280×840 + paleta + barra), bucle modal, devuelve `outcome.flatten()`.

Funciones clave:

- `fn refresh_committed(state)` — `committed = base.clone()`; `doc.render_onto(&mut committed, &ctx)`; regenerar `committed_dib`.
- `fn dest_rect(hwnd, state) -> Rect` — client menos paleta y barra → `fit_rect` + offset `PALETTE_W` en x.
- `fn anotacion_en_curso(state) -> Option<Box<dyn Annotation>>` — construye la anotación provisional del drag según `tool` (Rect/Ellipse/Highlight con `overlay::math::rect_between`; Line/Arrow start→current; Pen con points; Highlight con alfa 128; Text → None).
- WM_CREATE: crear paleta (botones 84×26 en columna: Rect/Elipse/Línea/Flecha/Lápiz/Resalt./Texto/Deshacer/Rehacer), swatches `BS_OWNERDRAW` 24×24, «Más…», grosores, tamaños (deshabilitados salvo con Texto), checkbox «B», OK/Cancelar. Si `!tiene_fuente` → deshabilitar botón Texto.
- WM_DRAWITEM: swatch → `FillRect` con su color (`COLORREF` = BGR) + `FrameRect` negro.
- Ratón (solo si `view_to_frame` da `Some`): LBUTTONDOWN → si `tool==Text` (Task 3) si no `drag=Some` + `SetCapture`; MOUSEMOVE → actualizar `current` (+push en Pen) + invalidar; LBUTTONUP → `ReleaseCapture` + `History::apply(Command::add(anotacion))` + `refresh_committed` + invalidar.
- WM_PAINT: si hay drag → clone committed + render provisional + DIB temporal; si no → `committed_dib`; `StretchBlt` al dest; separadores de paleta/barra con `FillRect`.
- WM_COMMAND: herramientas (cambia `tool` + habilitar tamaños/B si Texto), colores (`state.color = COLORES[i]` conservando... el color pleno; el alfa 128 se aplica solo al resaltador al construir), «Más…» → `ChooseColorW` (CC_FULLOPEN|CC_RGBINIT, custcolors estático local), grosores, tamaños, B (checkbox → `state.bold`), `ID_UNDO`/`ID_REDO` → history + refresh, `ID_OK` → outcome=Some(Some(committed.clone())) + destroy, `ID_CANCEL` → `confirmar_descarte`.
- `fn confirmar_descarte(hwnd, state)` — doc vacío → cancela directo; si no `MessageBoxW MB_YESNO` «¿Descartar las anotaciones?» → IDYES → outcome=Some(None) + destroy.
- WM_KEYDOWN: `Ctrl+Z`/`Ctrl+Y` (GetKeyState(VK_CONTROL)) → undo/redo + refresh; `VK_ESCAPE` → `confirmar_descarte`.
- WM_CLOSE → `confirmar_descarte`; WM_DESTROY → si `outcome.is_none()` → outcome=Some(None) (✕ forzado); patrón de estado idéntico al editor (NCCREATE/GWLP_USERDATA, liberar tras el bucle).

- [ ] **Step 2: Compilar** — `cargo fmt && cargo test -p platform-win` → PASS (26).

- [ ] **Step 3: Staging** — `git add crates/platform-win/`

---

### Task 3: Texto in situ (EDIT flotante)

**Files:**
- Modify: `crates/platform-win/src/draw/mod.rs`

**Interfaces:**
- Consumes: Task 2.
- Produces: `struct EditBox { hwnd: HWND, pos_frame: (i32,i32), font: HFONT }`; clic con Texto → `abrir_edit` (EDIT multilínea `ES_MULTILINE|ES_AUTOVSCROLL|WS_BORDER` en `frame_to_view(pos)`, 220×70, fuente `CreateFontW` con tamaño/negrita actuales, subclass para Esc); `EN_KILLFOCUS` → `commit_text` (texto no vacío → `Command::add(TextAnnotation)` + refresh; siempre destruye EDIT y fuente); `Esc` dentro → `WM_APP_CANCEL_TEXT (WM_APP+10)` al padre → destruir sin commit.

- [ ] **Step 1: Implementar**

- `abrir_edit(hwnd, state, pos_frame)`: crea el EDIT con id `ID_EDIT_TEXT: u16 = 4080`, `WM_SETFONT`, `SetFocus`, `SetWindowSubclass(edit, Some(edit_subclass), 1, 0)`.
- `edit_subclass` (`SUBCLASSPROC`): `WM_KEYDOWN` + `VK_ESCAPE` → `PostMessageW(GetParent(hwnd), WM_APP_CANCEL_TEXT, 0, 0)` y `LRESULT(0)`; resto → `DefSubclassProc`.
- `commit_text(hwnd, state)`: `let Some(edit) = state.edit.take()` → `GetWindowTextW` (buffer 2048) → trim → si no vacío: `History::apply(Command::add(Box::new(TextAnnotation { pos: edit.pos_frame, text, style: TextStyle { color: state.color, size: state.text_size, bold: state.bold } })))` + `refresh_committed` → `DestroyWindow(edit.hwnd)`, `DeleteObject(edit.font)`, invalidar.
- Ramas nuevas del wndproc: `WM_COMMAND` con `HIWORD(wparam)==EN_KILLFOCUS && LOWORD==ID_EDIT_TEXT` → `commit_text`; `m == WM_APP_CANCEL_TEXT` → tomar `state.edit`, destruir EDIT+fuente SIN commit, invalidar.
- LBUTTONDOWN con `tool==Text`: si ya hay edit → `commit_text` primero; luego `abrir_edit` en el punto.
- `ID_OK`: si hay edit abierto → `commit_text` antes de hornear.

- [ ] **Step 2: Compilar** — `cargo fmt && cargo test -p platform-win` → PASS (26).

- [ ] **Step 3: Staging** — `git add crates/platform-win/`

---

### Task 4: Integración con el editor (Draw activo + sucio)

**Files:**
- Modify: `crates/platform-win/src/editor/mod.rs`

**Interfaces:**
- Consumes: `draw::show_draw`.
- Produces: botón Draw habilitado; `EditorState.dirty: bool`; `ID_DRAW` → `show_draw(frame.clone())` → `Some(nuevo)` → sustituir `frame` + `dib` (regenerar con `dib_from_frame`), `dirty=true`, invalidar; `ID_CERRAR` pasa a `PostMessageW(WM_CLOSE)`; `WM_CLOSE` → si `dirty` → `MessageBoxW MB_YESNO` «Hay cambios sin guardar. ¿Descartarlos?» → IDNO aborta; Guardar/Copiar con éxito → `dirty=false`.

- [ ] **Step 1: Implementar**

- `EditorState`: campo `dirty: bool` (false inicial).
- `crear_toolbar`: `(ID_DRAW, w!("Draw"), true, 240, 90)`.
- `on_command`:
  - `ID_DRAW` → tomar clone del frame, `crate::draw::show_draw(clone)`; con `Some(nuevo)`: `state.frame = nuevo`; regenerar `state.dib` (ScreenDc→MemDc→`dib_from_frame`); `state.dirty = true`; `InvalidateRect`.
  - `ID_GUARDAR`/`ID_COPIAR`: en el caso Ok → `state.dirty = false` (además del beep).
  - `ID_CERRAR` → `PostMessageW(hwnd, WM_CLOSE, ...)`.
- wndproc: rama `WM_CLOSE` → si `dirty` y el usuario responde NO al descarte → `LRESULT(0)` (no cerrar); si acepta o no hay sucio → `DestroyWindow`.

- [ ] **Step 2: Compilar y tests** — `cargo fmt && cargo build --workspace && cargo test --workspace`
Expected: 103 core + 26 platform-win + 10 cli = 139 tests.

- [ ] **Step 3: Staging** — `git add crates/platform-win/`

---

### Task 5: Verificación manual guiada con el humano

- [ ] **Step 1: Lanzar** `./target/debug/rustcapture-gui.exe` (background).

- [ ] **Step 2: Checklist**

1. Captura → editor → «Draw» (ya activo) abre la ventana de dibujo con la imagen.
2. Rect/Elipse/Línea/Flecha: arrastrar muestra el preview en vivo y al soltar queda fijado.
3. Lápiz dibuja a mano alzada; Resaltador pinta semitransparente sin tapar.
4. Colores: los 8 swatches cambian el color; «Más…» abre el diálogo de Windows y respeta el color elegido. Grosores 1-8 se notan.
5. Texto: clic coloca la caja de edición ahí mismo; se escribe multilínea (Enter); clic fuera la convierte en texto sobre la imagen con el tamaño/negrita/color elegidos; `Esc` dentro la cancela; tamaño y «B» solo activos con la herramienta Texto.
6. Deshacer/Rehacer (botones y `Ctrl+Z`/`Ctrl+Y`) funcionan en cadena.
7. «Cancelar»/`Esc` con anotaciones → pregunta; sin anotaciones → cierra directo. El editor queda intacto.
8. «OK» → el editor muestra la imagen anotada; «Cerrar» ahora avisa «cambios sin guardar»; tras «Guardar como…» o «Copiar», cerrar ya no avisa.

- [ ] **Step 3: Fallos → `systematic-debugging`; sin OK humano no hay cierre.**

---

### Task 6: Verificación final y propuesta de commit

- [ ] **Step 1:** `cargo build --workspace && cargo test --workspace && cargo test -p platform-win -- --ignored && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check`
Expected: 139 tests + 6 humo; clippy y formato limpios.

- [ ] **Step 2: Roadmap** — Slice C ⏳ → ✅; D5 🔵 → ✅ (la Factory desde toolbar ya existe: la paleta construye las anotaciones).

- [ ] **Step 3: Proponer commit (NO ejecutar sin aprobación)**

```
v0.2.5 — F3/C: ventana de dibujo (Draw activo)

Ventana2 sobre el motor de anotación: paleta de 7 herramientas con
preview en vivo, undo/redo (botones y Ctrl+Z/Y), 8 colores + diálogo
de Windows, grosores, texto in situ con EDIT flotante (tamaño/negrita),
OK hornea al editor con flag de sucio y aviso al cerrar. Mapeo puro
vista↔frame con TDD. Cierra el trío A/B/C del editor adelantado.
```
