# F2 (reanudada) — Objeto de ventana, menús y región fija: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** capturar un control concreto o un menú desplegado señalándolo con el cursor (f.11, f.12) y capturar un rectángulo de tamaño predefinido (f.15).

**Architecture:** ninguno de los dos necesita un `CaptureMode` nuevo — los dos terminan en `ModeRequest::Region(rect)`, que ya existe. Son **formas distintas de elegir el rect**, así que todo el trabajo está en el overlay, que pasa de tener una interacción a tener tres modos. El overlay ya congela la pantalla capturándola antes de mostrarse, y eso resuelve el problema de los menús: un menú desplegado se cierra al aparecer cualquier ventana, pero **ya está en la imagen congelada**; lo que hay que hacer es tomar una **instantánea del árbol de ventanas ANTES** de congelar y hacer el hit-test contra esa instantánea en vez de contra las ventanas vivas. Con eso f.11 y f.12 son el mismo mecanismo. La unificación interna es que el overlay siempre tiene una «selección vigente» `Option<Rect>` que pinta sin blanquear; lo único que cambia por modo es cómo se calcula ese rect a partir del ratón.

**Tech Stack:** Rust 2024; `platform-win` con `windows` 0.62 (`EnumWindows`, `EnumChildWindows`, `GetWindowRect`, `DwmGetWindowAttribute`); `rustcapture-core` sin cambios de dominio salvo config.

## Global Constraints

- **Sin `CaptureMode` nuevo:** los dos modos publican `ModeRequest::Region(rect)`. Si algo empuja a añadir una variante, es señal de que el diseño se ha torcido.
- **Coordenadas de escritorio virtual** en todo lo que salga del adapter, con origen posiblemente negativo (multi-monitor). El overlay trabaja en locales y traduce al final, como ya hace.
- **PMv2 ya está activo**, así que `GetWindowRect` devuelve píxeles físicos coherentes con `desktop_rect`. No hay que escalar nada.
- **`// SAFETY:`** en cada bloque `unsafe`; nada de `unsafe` en `core`.
- **La barra no debe salir en la captura ni en los candidatos**: se oculta antes (ya lo hace `flujo_region`) y además se excluye del árbol por su HWND.
- **Compilar con la app CERRADA** antes de probar: con el `.exe` en uso el enlazado falla en silencio y se prueba el binario viejo.
- **Commit único al final**, propuesto al humano (`skills.md`).

---

## Estructura de archivos

| Archivo | Responsabilidad | Acción |
|---|---|---|
| `crates/platform-win/src/ventanas.rs` | instantánea del árbol de ventanas + hit-test | **crear** |
| `crates/platform-win/src/overlay/mod.rs` | tres modos de selección sobre la misma superficie | modificar |
| `crates/platform-win/src/overlay/math.rs` | rect fijo centrado y acotado; ajuste con rueda | modificar |
| `crates/platform-win/src/bar/{mod,math}.rs` | botones Objeto y Región fija habilitados y cableados | modificar |
| `crates/core/src/config/mod.rs` | `[capture] fixed_width/fixed_height` | modificar |
| `crates/platform-win/src/lib.rs` | declarar `mod ventanas;` | modificar |
| `ideas.md`, `roadmap.md`, `arquitectura.md`, `diseno-frontend.md` | f.14 descartada, f.11/f.12/f.15 hechas, D10 | modificar |

---

### Task 1: Instantánea del árbol de ventanas

**Files:**
- Create: `crates/platform-win/src/ventanas.rs`
- Modify: `crates/platform-win/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub(crate) struct Candidato { pub rect: Rect, pub profundidad: u16 }`
  - `pub(crate) fn instantanea(excluir: HWND) -> Vec<Candidato>`
  - `pub(crate) fn bajo_el_cursor(candidatos: &[Candidato], p: (i32, i32)) -> Option<Rect>`

- [ ] **Step 1: Escribir el hit-test con sus tests (lógica pura primero)**

```rust
//! Instantánea del árbol de ventanas para el picking de objetos y menús
//! (f.11, f.12).
//!
//! Se toma ANTES de mostrar el overlay, y esa es la clave de f.12: un menú
//! desplegado se cierra en cuanto aparece otra ventana, así que cuando el
//! overlay está en pantalla ya no existe como ventana — pero SÍ está en la
//! imagen congelada. Haciendo el hit-test contra la instantánea, capturar un
//! menú es exactamente lo mismo que capturar cualquier otro control.

use rustcapture_core::ports::Rect;
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Dwm::{DWMWA_CLOAKED, DwmGetWindowAttribute};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, EnumWindows, GetWindowRect, IsWindowVisible,
};

/// Un rect candidato. `profundidad` 0 = ventana de nivel superior, 1+ =
/// controles hijos; solo se usa para desempatar áreas iguales.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Candidato {
    pub rect: Rect,
    pub profundidad: u16,
}

/// El rect MÁS PEQUEÑO que contiene el punto, o `None` si ninguno lo hace.
///
/// El criterio es el área: así un botón gana a su diálogo y el diálogo gana
/// al escritorio, que es lo que el usuario espera al señalar. A igual área
/// gana el más profundo (el hijo antes que su padre).
pub(crate) fn bajo_el_cursor(candidatos: &[Candidato], p: (i32, i32)) -> Option<Rect> {
    candidatos
        .iter()
        .filter(|c| c.rect.contains_point(p))
        .min_by_key(|c| {
            let area = c.rect.width as u64 * c.rect.height as u64;
            (area, u16::MAX - c.profundidad)
        })
        .map(|c| c.rect)
}
```

Tests (en el mismo archivo):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn c(x: i32, y: i32, w: u32, h: u32, profundidad: u16) -> Candidato {
        Candidato {
            rect: Rect::new(x, y, w, h),
            profundidad,
        }
    }

    #[test]
    fn gana_el_rect_mas_pequeno_que_contiene_el_punto() {
        // Escritorio, diálogo dentro y botón dentro del diálogo.
        let cs = [
            c(0, 0, 1920, 1080, 0),
            c(400, 300, 400, 200, 0),
            c(420, 320, 80, 24, 1),
        ];
        // Sobre el botón: gana el botón.
        assert_eq!(bajo_el_cursor(&cs, (430, 330)), Some(Rect::new(420, 320, 80, 24)));
        // En el diálogo pero fuera del botón: gana el diálogo.
        assert_eq!(bajo_el_cursor(&cs, (600, 400)), Some(Rect::new(400, 300, 400, 200)));
        // Fuera de todo menos del escritorio.
        assert_eq!(bajo_el_cursor(&cs, (50, 50)), Some(Rect::new(0, 0, 1920, 1080)));
    }

    #[test]
    fn a_igual_area_gana_el_hijo() {
        // Un control que ocupa todo su padre (pasa con los paneles).
        let cs = [c(10, 10, 100, 50, 0), c(10, 10, 100, 50, 2)];
        assert_eq!(bajo_el_cursor(&cs, (20, 20)), Some(Rect::new(10, 10, 100, 50)));
        // Y el elegido es el profundo: se comprueba por el orden del min_by_key.
        let elegido = cs
            .iter()
            .filter(|x| x.rect.contains_point((20, 20)))
            .min_by_key(|x| {
                let area = x.rect.width as u64 * x.rect.height as u64;
                (area, u16::MAX - x.profundidad)
            })
            .unwrap();
        assert_eq!(elegido.profundidad, 2);
    }

    #[test]
    fn sin_candidatos_o_fuera_de_todos_es_none() {
        assert_eq!(bajo_el_cursor(&[], (5, 5)), None);
        assert_eq!(bajo_el_cursor(&[c(0, 0, 10, 10, 0)], (50, 50)), None);
    }

    #[test]
    fn un_monitor_a_la_izquierda_del_primario_funciona_igual() {
        // Origen negativo: es el caso que rompe la aritmética sin signo.
        let cs = [c(-1920, 0, 1920, 1080, 0), c(-800, 100, 300, 200, 0)];
        assert_eq!(bajo_el_cursor(&cs, (-700, 150)), Some(Rect::new(-800, 100, 300, 200)));
        assert_eq!(bajo_el_cursor(&cs, (-1800, 900)), Some(Rect::new(-1920, 0, 1920, 1080)));
    }

    /// El árbol real del sistema: hay ventanas y todas tienen área.
    #[test]
    fn la_instantanea_real_trae_candidatos_usables() {
        let cs = instantanea(HWND::default());
        assert!(cs.len() > 3, "solo {} candidatos", cs.len());
        assert!(cs.iter().all(|c| !c.rect.is_empty()));
        // Y hay al menos un hijo: sin ellos f.11 no tendría sentido.
        assert!(cs.iter().any(|c| c.profundidad > 0), "no se enumeraron hijos");
    }
}
```

- [ ] **Step 2: Ejecutar y ver que falla** (`instantanea` no existe).

- [ ] **Step 3: Implementar la enumeración**

```rust
/// Ventanas visibles de nivel superior con sus controles, en coordenadas de
/// escritorio virtual. `excluir` es el HWND propio (la barra), que no debe
/// aparecer como candidato aunque esté oculta.
pub(crate) fn instantanea(excluir: HWND) -> Vec<Candidato> {
    let mut acc = Acumulador {
        candidatos: Vec::new(),
        excluir,
    };
    // SAFETY: el lparam lleva un &mut Acumulador que vive toda la
    // enumeración; EnumWindows es sincrónico.
    unsafe {
        _ = EnumWindows(
            Some(cb_top),
            LPARAM(&mut acc as *mut Acumulador as isize),
        );
    }
    acc.candidatos
}

struct Acumulador {
    candidatos: Vec<Candidato>,
    excluir: HWND,
}

/// `true` si la ventana no debe ofrecerse: invisible, sin área o "cloaked"
/// (las apps UWP suspendidas siguen existiendo pero no se ven, y ofrecerlas
/// daría un rect fantasma).
fn descartable(hwnd: HWND) -> bool {
    // SAFETY: consultas sin precondiciones sobre un HWND de la enumeración.
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return true;
        }
        let mut cloaked = 0u32;
        let ok = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            (&raw mut cloaked).cast(),
            size_of::<u32>() as u32,
        );
        ok.is_ok() && cloaked != 0
    }
}

fn rect_de(hwnd: HWND) -> Option<Rect> {
    let mut rc = RECT::default();
    // SAFETY: GetWindowRect sobre un HWND válido.
    unsafe { GetWindowRect(hwnd, &mut rc) }.ok()?;
    let (w, h) = (rc.right - rc.left, rc.bottom - rc.top);
    (w > 0 && h > 0).then(|| Rect::new(rc.left, rc.top, w as u32, h as u32))
}

unsafe extern "system" fn cb_top(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: el puntero lo puso `instantanea` y vive toda la enumeración.
    let acc = unsafe { &mut *(lparam.0 as *mut Acumulador) };
    if hwnd != acc.excluir
        && !descartable(hwnd)
        && let Some(rect) = rect_de(hwnd)
    {
        acc.candidatos.push(Candidato {
            rect,
            profundidad: 0,
        });
        // Controles del interior (f.11).
        // SAFETY: mismo acumulador, enumeración sincrónica anidada.
        unsafe {
            _ = EnumChildWindows(Some(hwnd), Some(cb_hijo), lparam);
        }
    }
    TRUE
}

unsafe extern "system" fn cb_hijo(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: igual que en cb_top.
    let acc = unsafe { &mut *(lparam.0 as *mut Acumulador) };
    if !descartable(hwnd)
        && let Some(rect) = rect_de(hwnd)
    {
        // Profundidad 1 para todos los descendientes: solo desempata áreas
        // iguales, y distinguir niveles exactos exigiría recorrer padres.
        acc.candidatos.push(Candidato {
            rect,
            profundidad: 1,
        });
    }
    TRUE
}
```

En `lib.rs`: `mod ventanas;`

- [ ] **Step 4: Ejecutar** — `cargo test -p platform-win ventanas`; los 5 tests en verde.

---

### Task 2: Rect de tamaño fijo (lógica pura)

**Files:**
- Modify: `crates/platform-win/src/overlay/math.rs`

**Interfaces:**
- Produces:
  - `pub(crate) fn rect_fijo(cursor: (i32,i32), tam: (u32,u32), limite: Rect) -> Rect`
  - `pub(crate) fn ajustar_tam(tam: (u32,u32), pasos: i32, solo_ancho: bool) -> (u32,u32)`
  - `pub(crate) const PASO_FIJO: i32 = 10;`

- [ ] **Step 1: Tests que fallan**

```rust
    #[test]
    fn el_rect_fijo_se_centra_en_el_cursor() {
        let limite = Rect::new(0, 0, 1000, 800);
        assert_eq!(rect_fijo((500, 400), (200, 100), limite), Rect::new(400, 350, 200, 100));
    }

    #[test]
    fn el_rect_fijo_no_se_sale_del_limite() {
        let limite = Rect::new(0, 0, 1000, 800);
        // Pegado a la esquina superior izquierda.
        assert_eq!(rect_fijo((10, 10), (200, 100), limite), Rect::new(0, 0, 200, 100));
        // Y a la inferior derecha.
        assert_eq!(rect_fijo((995, 795), (200, 100), limite), Rect::new(800, 700, 200, 100));
    }

    #[test]
    fn un_rect_fijo_mayor_que_el_limite_se_recorta_al_limite() {
        let limite = Rect::new(0, 0, 100, 80);
        assert_eq!(rect_fijo((50, 40), (400, 300), limite), Rect::new(0, 0, 100, 80));
    }

    #[test]
    fn el_limite_con_origen_negativo_funciona() {
        let limite = Rect::new(-1920, -100, 3840, 1180);
        let r = rect_fijo((-1900, -90), (200, 100), limite);
        assert_eq!(r, Rect::new(-1920, -100, 200, 100));
    }

    #[test]
    fn la_rueda_ajusta_el_tamano_con_minimo() {
        assert_eq!(ajustar_tam((200, 100), 1, false), (210, 110));
        assert_eq!(ajustar_tam((200, 100), -1, false), (190, 90));
        // Shift: solo el ancho.
        assert_eq!(ajustar_tam((200, 100), 2, true), (220, 100));
        // Nunca baja de un mínimo usable.
        assert_eq!(ajustar_tam((10, 10), -5, false), (8, 8));
    }
```

- [ ] **Step 2: Ejecutar y ver que falla.**

- [ ] **Step 3: Implementar**

```rust
/// Píxeles que mueve cada paso de rueda en la región fija (f.15).
pub(crate) const PASO_FIJO: i32 = 10;
/// Lado mínimo, para que la rueda no lo deje en nada.
const MINIMO_FIJO: u32 = 8;

/// Rect de `tam` centrado en el cursor y empujado dentro de `limite`. Si no
/// cabe, se recorta al límite (mejor eso que devolver algo fuera de pantalla).
pub(crate) fn rect_fijo(cursor: (i32, i32), tam: (u32, u32), limite: Rect) -> Rect {
    let w = tam.0.min(limite.width).max(1);
    let h = tam.1.min(limite.height).max(1);
    let max_x = limite.right() as i32 - w as i32;
    let max_y = limite.bottom() as i32 - h as i32;
    Rect::new(
        (cursor.0 - w as i32 / 2).clamp(limite.x, max_x.max(limite.x)),
        (cursor.1 - h as i32 / 2).clamp(limite.y, max_y.max(limite.y)),
        w,
        h,
    )
}

/// Ajusta el tamaño con la rueda; `solo_ancho` = Shift pulsado.
pub(crate) fn ajustar_tam(tam: (u32, u32), pasos: i32, solo_ancho: bool) -> (u32, u32) {
    let mover = |v: u32| -> u32 {
        (v as i64 + (pasos * PASO_FIJO) as i64).clamp(MINIMO_FIJO as i64, u32::MAX as i64) as u32
    };
    if solo_ancho {
        (mover(tam.0), tam.1)
    } else {
        (mover(tam.0), mover(tam.1))
    }
}
```

- [ ] **Step 4: Ejecutar** — `cargo test -p platform-win overlay::math`; PASS.

---

### Task 3: El overlay gana tres modos

**Files:**
- Modify: `crates/platform-win/src/overlay/mod.rs`

**Interfaces:**
- Produces:
  - `pub enum ModoOverlay { Rectangulo, Objeto, Fija { ancho: u32, alto: u32 } }`
  - `pub fn select_region(modo: ModoOverlay, excluir_raw: isize) -> Option<Rect>`

- [ ] **Step 1: Unificar la «selección vigente»**

`OverlayState` gana:
- `modo: ModoOverlay`
- `candidatos: Vec<Candidato>` (vacío salvo en modo Objeto)
- `tam_fijo: (u32, u32)` (solo modo Fija; se ajusta con la rueda)

Y se extrae una función que es la que unifica los tres modos:

```rust
/// Rect que el overlay pinta SIN blanquear, en coordenadas locales. Es la
/// única diferencia real entre los tres modos: uno lo saca del arrastre,
/// otro del árbol de ventanas y otro del tamaño fijo.
fn seleccion_vigente(state: &OverlayState) -> Option<Rect> {
    match &state.modo {
        ModoOverlay::Rectangulo => state
            .drag_start
            .map(|inicio| math::rect_between(inicio, state.cursor)),
        ModoOverlay::Objeto => {
            // Los candidatos están en coordenadas de ESCRITORIO; el overlay
            // pinta en locales.
            let global = (
                state.cursor.0 + state.origin.0,
                state.cursor.1 + state.origin.1,
            );
            crate::ventanas::bajo_el_cursor(&state.candidatos, global)
                .map(|r| Rect::new(r.x - state.origin.0, r.y - state.origin.1, r.width, r.height))
        }
        ModoOverlay::Fija { .. } => Some(math::rect_fijo(
            state.cursor,
            state.tam_fijo,
            Rect::new(0, 0, state.width as u32, state.height as u32),
        )),
    }
}
```

El pintado y la invalidación mínima pasan a consumir `seleccion_vigente` en vez de `drag_start`+`cursor`, y la caja de información muestra el tamaño de ese rect en los tres modos.

- [ ] **Step 2: Ratón por modo**

- `WM_LBUTTONDOWN`: en `Rectangulo` arranca el arrastre (como hoy); en `Objeto` y `Fija` **confirma directamente** la selección vigente (son de un clic).
- `WM_LBUTTONUP`: solo cierra en `Rectangulo`.
- `WM_MOUSEWHEEL`: solo en `Fija`, ajusta `tam_fijo` con `ajustar_tam` (Shift = solo ancho) e invalida.
- `Esc`: cancela en los tres.

- [ ] **Step 3: Cursor por modo**

En `Objeto` y `Fija` el crosshair propio no aporta (no se está apuntando a un píxel exacto): usar `IDC_ARROW` del sistema. En `Rectangulo` se mantiene el crosshair actual.

- [ ] **Step 4: La instantánea se toma ANTES de congelar**

En `run()`, si el modo es `Objeto`, `crate::ventanas::instantanea(excluir)` **antes** de `capture_region`. El orden importa: es lo que hace que un menú desplegado siga estando en el árbol.

- [ ] **Step 5: Ejecutar** — `cargo test -p platform-win` y `cargo clippy --all-targets`; PASS.

---

### Task 4: Config y barra

**Files:**
- Modify: `crates/core/src/config/mod.rs`, `crates/platform-win/src/bar/{mod,math}.rs`

- [ ] **Step 1: `[capture] fixed_width/fixed_height`**

En `CaptureConfig`, dos campos con default 800×600 y su test de round-trip TOML y de default (patrón de `delay_seconds`).

- [ ] **Step 2: Botones habilitados**

`ID_OBJECT` y `ID_FIXED` pasan a `true` en `bar/math.rs::toolbar()`. Actualizar el test de habilitados. **Quitar** el botón `ID_FREEHAND` de la fila: la feature está descartada, y un botón que nunca se va a encender es ruido (la constante y el icono del atlas se quedan; el atlas es append-only).

- [ ] **Step 3: Cableado**

Dos mensajes `WM_APP_OBJETO` y `WM_APP_FIJA` hermanos de `WM_APP_REGION`, y `flujo_region` se generaliza a `flujo_seleccion(hwnd, modo)`: oculta la barra, espera los 150 ms, llama a `select_region(modo, hwnd_raw)` y publica `ModeRequest::Region(rect)`. `BarState` gana el tamaño fijo de la config.

- [ ] **Step 4: Ejecutar** — `cargo test` y `cargo clippy --all-targets`; PASS.

---

### Task 5: Verificación manual y documentación

- [ ] **Step 1: Guion manual** (compilar con la app CERRADA)

1. Botón **Objeto de ventana**: al mover el cursor se resalta la ventana bajo él; sobre un botón o un panel del Explorador se resalta **ese control**, no la ventana entera.
2. La caja de información muestra el tamaño del objeto resaltado.
3. Clic → la captura es exactamente ese control.
4. **Menú desplegado (f.12):** abre un menú (por ejemplo el de Inicio o el contextual del escritorio), pulsa el hotkey/botón de Objeto y, aunque el menú se cierre al aparecer el overlay, **debe seguir visible en la imagen congelada y ser señalable**. Es el punto que justifica todo el diseño.
5. `Esc` cancela sin capturar.
6. Botón **Región fija**: rect de 800×600 siguiendo al cursor, con su tamaño a la vista.
7. **Rueda** arriba/abajo: crece y decrece de 10 en 10. **Shift+rueda**: solo el ancho.
8. Pegado a los bordes de la pantalla: el rect se empuja dentro, no se sale.
9. Con el rect más grande que el monitor: se recorta al monitor sin fallar.
10. Cambiar `fixed_width/fixed_height` en `config.toml` y reabrir: arranca con ese tamaño.
11. Todo lo anterior en un **segundo monitor** y a **150 %** de escala.
12. Comprobar que la **barra no sale** en ninguna captura ni se ofrece como candidato.

- [ ] **Step 2: Documentación**

- `ideas.md`: f.14 ya está en §Descartado con su porqué (hecho al planificar).
- `roadmap.md`: f.11/f.12/f.15 hechas; f.14 marcada 🚫 con enlace al descarte; F2 queda solo con scroll capture y f.7/f.19.
- `arquitectura.md` D10: la capa de selección tiene tres modos sobre la misma superficie, y la instantánea del árbol se toma antes de congelar (el porqué de los menús).
- `diseno-frontend.md`: V1 sin el botón de mano alzada; V3 con el resaltado de objeto y el rect fijo con rueda.

- [ ] **Step 3: `verification-before-completion`** y **Step 4: proponer commit**.

---

## Autorrevisión

| Requisito | Tarea |
|---|---|
| Señalar y capturar un control (f.11) | 1, 3 |
| Capturar un menú desplegado (f.12) | 1 (instantánea antes de congelar), 3 |
| Región de tamaño predefinido (f.15) | 2, 3, 4 |
| Ajustar el tamaño sin diálogo | 2, 3 |
| Sin `CaptureMode` nuevo | Constraint global |

**Riesgos anotados:**
- **El orden importa y no se ve en el código a simple vista:** la instantánea DEBE tomarse antes de `capture_region`. Si alguien la mueve después, f.12 deja de funcionar y f.11 sigue pareciendo correcta — un fallo silencioso. Va comentado en `run()` y es el punto 4 del guion.
- **`EnumChildWindows` puede ser lento** en ventanas con cientos de controles (el Explorador, navegadores). Se toma una sola vez por invocación, no por movimiento del ratón, así que el coste es del arranque del overlay; si se nota, el paso siguiente es enumerar hijos en diferido solo para la ventana bajo el cursor.
- **Las apps con DPI virtualizado** (procesos no-PMv2) devuelven rects que Windows escala; el resaltado puede quedar desplazado unos píxeles respecto a lo que se ve. No se corrige aquí; queda anotado.
- **El escritorio como candidato** es un rect del tamaño de la pantalla: aparecerá seleccionado cuando el cursor no esté sobre nada, lo que es razonable pero conviene verlo en el punto 1 del guion.
