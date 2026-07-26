# F3 Slice D — Pixelado y pasos numerados: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** cerrar dos de las cuatro herramientas pendientes de F3 — pixelado/desenfoque (f.25) y pasos numerados (f.23) — en el motor de anotación del core y cableadas en el editor V4.

**Architecture:** las dos herramientas son Strategies nuevas de D5 (`PixelateAnnotation`, `StepAnnotation`) que viven en `crates/core/src/annotate/annotations/` y se apilan en el `Document` como cualquier otra, con undo/redo de D6 gratis. Dos piezas de infraestructura las habilitan: (1) el `Canvas` gana un **lector** de píxeles — la censura tiene que ver lo que hay debajo, y hasta hoy el canvas solo escribía; (2) el módulo de texto gana la **caja de tinta** del texto, que es lo que permite centrar el número dentro del disco de forma óptica y no aproximada. En `platform-win/editor` las propiedades sueltas de la herramienta activa (color, grosor, tamaño, negrita) se agrupan en un struct `Propiedades` antes de añadirle el modo de censura, y la numeración de pasos se sincroniza con las pilas de undo/redo mediante un contador con instantáneas.

**Tech Stack:** Rust 2024, workspace de Cargo; `rustcapture-core` sin dependencias de plataforma (`fontdue` para tipografía); `platform-win` con `windows` 0.62 y GDI.

## Global Constraints

- **TDD obligatorio en `core`** (`skills.md`): test primero, luego implementación. En `platform-win` aplica a la lógica pura (`math.rs`, `props.rs::chips`, contador de pasos); el resto se verifica a mano.
- **Nada de `unsafe` nuevo en `core`.** El core no abre archivos ni toca Win32.
- **Commits:** este proyecto NO hace commit por tarea. `skills.md`: el agente solo commitea automáticamente al cerrar una fase completa del roadmap; cualquier otro commit se pide al humano. Cada tarea termina en «tests verdes», y el commit único (`v0.2.16 — …`) se propone en la Tarea 9.
- **Idioma del código:** comentarios y nombres internos en español, igual que el resto del proyecto; los tipos públicos del core mantienen nombres en inglés (`PixelateAnnotation`, `CensorMode`) como los ya existentes.
- **Comentarios `// SAFETY:`** obligatorios en cada bloque `unsafe` de `platform-win` (convención de `windows-rs-interop`).
- **Unidades lógicas** en todo lo que sea layout de la UI; escalar solo al pintar (D13).
- Comando de test del core: `cargo test -p rustcapture-core`. Del adapter: `cargo test -p platform-win`. Todo: `cargo test`.

---

## Estructura de archivos

| Archivo | Responsabilidad | Acción |
|---|---|---|
| `crates/core/src/annotate/canvas.rs` | puerta única de píxeles: añade el lector `pixel()` | modificar |
| `crates/core/src/annotate/style.rs` | tipos de estilo: añade `CensorMode` y `Color::contraste()` | modificar |
| `crates/core/src/annotate/censor.rs` | rasterizado de censura: mosaico y desenfoque separable | **crear** |
| `crates/core/src/annotate/shapes.rs` | formas: añade `fill_disc_aa` (disco relleno con AA) | modificar |
| `crates/core/src/annotate/text.rs` | tipografía: añade `text_ink_box` (caja de tinta) | modificar |
| `crates/core/src/annotate/annotations/pixelate.rs` | Strategy de censura de una región | **crear** |
| `crates/core/src/annotate/annotations/step.rs` | Strategy de paso numerado (disco + número) | **crear** |
| `crates/core/src/annotate/annotations/mod.rs` | declara y reexporta las dos Strategies nuevas | modificar |
| `crates/core/src/annotate/mod.rs` | declara `censor`, reexporta `CensorMode` | modificar |
| `crates/platform-win/src/editor/estado.rs` | `Propiedades`, `ContadorPasos`, construcción de las anotaciones nuevas | modificar |
| `crates/platform-win/src/editor/math.rs` | `Herramienta::{Pasos, Pixelado}`, toolbar con sus botones habilitados | modificar |
| `crates/platform-win/src/editor/props.rs` | chips de las dos herramientas nuevas + acciones | modificar |
| `crates/platform-win/src/editor/texto.rs` | usa `state.props` en vez de campos sueltos | modificar |
| `crates/platform-win/src/editor/mod.rs` | clic simple del paso, contador en undo/redo | modificar |
| `roadmap.md`, `ideas.md`, `diseno-frontend.md` | estado de f.23/f.25 y de la toolbar V4 | modificar |

---

### Task 1: Lector del canvas y color de contraste

Las dos primitivas que necesitan las tareas siguientes. `Canvas::pixel` es el cambio conceptualmente relevante: hasta hoy el canvas solo escribía, y la censura necesita leer el resultado compuesto (base + anotaciones ya pintadas) para que el pixelado tape lo que hay debajo de él en el z-order.

**Files:**
- Modify: `crates/core/src/annotate/canvas.rs`
- Modify: `crates/core/src/annotate/style.rs`

**Interfaces:**
- Consumes: nada (primera tarea).
- Produces:
  - `Canvas::pixel(&self, x: i32, y: i32) -> Option<Color>`
  - `Color::contraste(&self) -> Color` (devuelve blanco o negro)

- [ ] **Step 1: Escribir los tests que fallan**

En `crates/core/src/annotate/canvas.rs`, dentro de `mod tests`:

```rust
    #[test]
    fn pixel_lee_lo_ya_compuesto_y_fuera_de_rango_es_none() {
        let mut frame = Frame::filled(2, 2, [10, 20, 30, 255]);
        let mut canvas = Canvas::new(&mut frame);
        assert_eq!(canvas.pixel(1, 1), Some(Color::rgba(10, 20, 30, 255)));
        // Lo que se escribe se vuelve a leer: la censura ve el z-order.
        canvas.blend_pixel(0, 0, Color::rgb(200, 0, 0));
        assert_eq!(canvas.pixel(0, 0), Some(Color::rgba(200, 0, 0, 255)));
        assert_eq!(canvas.pixel(-1, 0), None);
        assert_eq!(canvas.pixel(0, 2), None);
    }
```

En `crates/core/src/annotate/style.rs`, dentro de `mod tests`:

```rust
    #[test]
    fn el_contraste_es_negro_sobre_claro_y_blanco_sobre_oscuro() {
        assert_eq!(Color::rgb(255, 255, 255).contraste(), Color::rgb(0, 0, 0));
        assert_eq!(Color::rgb(0, 0, 0).contraste(), Color::rgb(255, 255, 255));
        // Acento del diseño (#D83B01): número blanco encima.
        assert_eq!(
            Color::rgb(0xD8, 0x3B, 0x01).contraste(),
            Color::rgb(255, 255, 255)
        );
        // Amarillo puro es claro: número negro.
        assert_eq!(Color::rgb(255, 255, 0).contraste(), Color::rgb(0, 0, 0));
    }
```

- [ ] **Step 2: Ejecutar los tests y comprobar que fallan**

Run: `cargo test -p rustcapture-core annotate::canvas annotate::style`
Expected: FAIL de compilación — `no method named 'pixel' found` y `no method named 'contraste' found`.

- [ ] **Step 3: Implementar `Canvas::pixel`**

En `crates/core/src/annotate/canvas.rs`, dentro de `impl<'a> Canvas<'a>`, justo antes de `blend_pixel`:

```rust
    /// Lee el píxel YA compuesto (base + anotaciones anteriores); `None`
    /// fuera de rango. Lo necesitan las anotaciones que censuran lo que
    /// tienen debajo (pixelado/desenfoque): ven el z-order, no la base.
    pub fn pixel(&self, x: i32, y: i32) -> Option<Color> {
        if x < 0 || y < 0 || x as u32 >= self.frame.width || y as u32 >= self.frame.height {
            return None;
        }
        let i = (y as usize * self.frame.width as usize + x as usize) * 4;
        let px = &self.frame.pixels[i..i + 4];
        Some(Color::rgba(px[0], px[1], px[2], px[3]))
    }
```

- [ ] **Step 4: Implementar `Color::contraste`**

En `crates/core/src/annotate/style.rs`, dentro de `impl Color`:

```rust
    /// Blanco o negro — el que contraste con este color. Luminancia
    /// percibida ITU-R BT.601 en milésimas para no usar coma flotante.
    /// Sirve para pintar texto legible sobre un relleno (f.23).
    pub const fn contraste(&self) -> Color {
        let luz = 299 * self.r as u32 + 587 * self.g as u32 + 114 * self.b as u32;
        if luz > 140_000 {
            Color::rgb(0, 0, 0)
        } else {
            Color::rgb(255, 255, 255)
        }
    }
```

- [ ] **Step 5: Ejecutar los tests y comprobar que pasan**

Run: `cargo test -p rustcapture-core`
Expected: PASS, incluidos los dos tests nuevos. Sin warnings nuevos.

---

### Task 2: Rasterizado de censura (mosaico y desenfoque)

Módulo interno hermano de `shapes.rs`: funciones puras de rasterizado, sin tipo público. El desenfoque necesita **copia previa** de la zona porque sus vecindades se solapan y escribir en sitio contaminaría las muestras siguientes; el mosaico no la necesita porque sus celdas son disjuntas. El desenfoque es de caja separable con sumas prefijas: coste O(w·h) por pasada, **independiente del radio** — importa porque esto se re-renderiza en cada `WM_MOUSEMOVE` del arrastre.

**Files:**
- Create: `crates/core/src/annotate/censor.rs`
- Modify: `crates/core/src/annotate/mod.rs`

**Interfaces:**
- Consumes: `Canvas::pixel` (Task 1), `Canvas::blend_pixel`, `Rect::intersection`.
- Produces:
  - `pub(crate) fn mosaico(canvas: &mut Canvas, rect: Rect, bloque: u32)`
  - `pub(crate) fn desenfoque(canvas: &mut Canvas, rect: Rect, radio: u32)`

- [ ] **Step 1: Escribir el módulo con sus tests (el archivo entero, tests incluidos)**

Crear `crates/core/src/annotate/censor.rs`:

```rust
//! Censura de regiones (f.25): mosaico y desenfoque. Ambas LEEN el canvas
//! (base + anotaciones ya pintadas) y lo reescriben opaco, así que la
//! región queda censurada tal y como se ve en ese punto del z-order.
//!
//! El desenfoque copia la zona antes de escribir: sus vecindades se
//! solapan y hacerlo en sitio contaminaría las muestras siguientes. El
//! mosaico no lo necesita — sus celdas son disjuntas.
//!
//! Interno, como `shapes`: la API pública es `PixelateAnnotation`.

use crate::annotate::canvas::Canvas;
use crate::annotate::style::Color;
use crate::ports::Rect;

/// Parte del rect que cae dentro del canvas; `None` si no toca nada.
fn recortar(canvas: &Canvas, rect: Rect) -> Option<Rect> {
    Rect::new(0, 0, canvas.width(), canvas.height()).intersection(&rect)
}

/// Media RGB de una celda ya recortada al canvas.
fn media(canvas: &Canvas, x: i32, y: i32, ancho: i32, alto: i32) -> Color {
    let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
    for dy in 0..alto {
        for dx in 0..ancho {
            if let Some(c) = canvas.pixel(x + dx, y + dy) {
                r += u32::from(c.r);
                g += u32::from(c.g);
                b += u32::from(c.b);
                n += 1;
            }
        }
    }
    if n == 0 {
        return Color::rgb(0, 0, 0);
    }
    Color::rgb((r / n) as u8, (g / n) as u8, (b / n) as u8)
}

/// Mosaico: cada celda de `bloque`×`bloque` se aplana a su color medio.
/// La rejilla se ancla al origen del rect (como FastStone), no al frame.
pub(crate) fn mosaico(canvas: &mut Canvas, rect: Rect, bloque: u32) {
    let Some(zona) = recortar(canvas, rect) else {
        return;
    };
    let bloque = bloque.max(1) as i32;
    let (fin_x, fin_y) = (zona.x + zona.width as i32, zona.y + zona.height as i32);
    let mut y = zona.y;
    while y < fin_y {
        let alto = bloque.min(fin_y - y);
        let mut x = zona.x;
        while x < fin_x {
            let ancho = bloque.min(fin_x - x);
            let color = media(canvas, x, y, ancho, alto);
            for dy in 0..alto {
                for dx in 0..ancho {
                    canvas.blend_pixel(x + dx, y + dy, color);
                }
            }
            x += ancho;
        }
        y += alto;
    }
}

/// Desenfoque de caja separable: dos pasadas 1-D con sumas prefijas, de
/// coste independiente del radio. En los bordes se promedian menos
/// muestras en lugar de replicar píxeles: no aparece halo en el contorno.
pub(crate) fn desenfoque(canvas: &mut Canvas, rect: Rect, radio: u32) {
    let Some(zona) = recortar(canvas, rect) else {
        return;
    };
    let (w, h) = (zona.width as usize, zona.height as usize);
    let radio = (radio.max(1) as usize).min(w.max(h));
    let mut buffer = leer_rgb(canvas, zona);
    let mut temp = vec![0u8; buffer.len()];
    // Horizontal: h líneas de w muestras contiguas.
    blur_1d(&buffer, &mut temp, h, w, 3, w * 3, radio);
    // Vertical: w columnas de h muestras separadas por una fila.
    blur_1d(&temp, &mut buffer, w, h, w * 3, 3, radio);
    escribir_rgb(canvas, zona, &buffer);
}

/// Copia la zona a un buffer RGB compacto (el frame es opaco: sin alfa).
fn leer_rgb(canvas: &Canvas, zona: Rect) -> Vec<u8> {
    let mut out = Vec::with_capacity(zona.width as usize * zona.height as usize * 3);
    for fila in 0..zona.height as i32 {
        for col in 0..zona.width as i32 {
            let c = canvas
                .pixel(zona.x + col, zona.y + fila)
                .unwrap_or(Color::rgb(0, 0, 0));
            out.extend_from_slice(&[c.r, c.g, c.b]);
        }
    }
    out
}

fn escribir_rgb(canvas: &mut Canvas, zona: Rect, rgb: &[u8]) {
    for fila in 0..zona.height as usize {
        for col in 0..zona.width as usize {
            let i = (fila * zona.width as usize + col) * 3;
            canvas.blend_pixel(
                zona.x + col as i32,
                zona.y + fila as i32,
                Color::rgb(rgb[i], rgb[i + 1], rgb[i + 2]),
            );
        }
    }
}

/// Una pasada de media móvil 1-D sobre RGB. Hay `lineas` líneas de
/// `largo` muestras; `paso` avanza a la muestra siguiente de la línea y
/// `salto` al inicio de la línea siguiente — así la misma función sirve
/// para filas y para columnas sin transponer el buffer.
fn blur_1d(
    src: &[u8],
    out: &mut [u8],
    lineas: usize,
    largo: usize,
    paso: usize,
    salto: usize,
    radio: usize,
) {
    // prefijo[k] = suma de las k primeras muestras de la línea, por canal.
    let mut prefijo = vec![0u32; (largo + 1) * 3];
    for l in 0..lineas {
        let inicio = l * salto;
        for k in 0..largo {
            let i = inicio + k * paso;
            for c in 0..3 {
                prefijo[(k + 1) * 3 + c] = prefijo[k * 3 + c] + u32::from(src[i + c]);
            }
        }
        for k in 0..largo {
            let desde = k.saturating_sub(radio);
            let hasta = (k + radio + 1).min(largo);
            let n = (hasta - desde) as u32;
            let i = inicio + k * paso;
            for c in 0..3 {
                let suma = prefijo[hasta * 3 + c] - prefijo[desde * 3 + c];
                out[i + c] = ((suma + n / 2) / n) as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::Frame;

    /// Frame 8×8 con mitad izquierda negra y mitad derecha blanca.
    fn mitades() -> Frame {
        let mut frame = Frame::filled(8, 8, [0, 0, 0, 255]);
        for y in 0..8u32 {
            for x in 4..8u32 {
                let i = (y as usize * 8 + x as usize) * 4;
                frame.pixels[i..i + 3].copy_from_slice(&[255, 255, 255]);
            }
        }
        frame
    }

    #[test]
    fn el_mosaico_aplana_cada_celda_a_su_media() {
        let mut frame = mitades();
        // Una celda de 8×8 cubre todo: media = gris medio.
        mosaico(&mut Canvas::new(&mut frame), Rect::new(0, 0, 8, 8), 8);
        let [r, g, b, a] = frame.pixel(0, 0).unwrap();
        assert!((126..=128).contains(&r) && r == g && g == b && a == 255);
        // Todos los píxeles quedan idénticos: es una sola celda.
        assert_eq!(frame.pixel(7, 7), frame.pixel(0, 0));
    }

    #[test]
    fn celdas_de_cuatro_conservan_el_contraste_entre_mitades() {
        let mut frame = mitades();
        mosaico(&mut Canvas::new(&mut frame), Rect::new(0, 0, 8, 8), 4);
        // Celda izquierda toda negra, celda derecha toda blanca.
        assert_eq!(frame.pixel(1, 1), Some([0, 0, 0, 255]));
        assert_eq!(frame.pixel(6, 6), Some([255, 255, 255, 255]));
    }

    #[test]
    fn el_mosaico_solo_toca_su_rect_y_la_ultima_celda_se_recorta() {
        // 10 px de ancho con bloque 4: celdas 4+4+2, la última recortada.
        let mut frame = Frame::filled(10, 4, [0, 0, 0, 255]);
        for x in 8..10u32 {
            for y in 0..4u32 {
                let i = (y as usize * 10 + x as usize) * 4;
                frame.pixels[i..i + 3].copy_from_slice(&[255, 255, 255]);
            }
        }
        mosaico(&mut Canvas::new(&mut frame), Rect::new(0, 0, 10, 4), 4);
        assert_eq!(frame.pixel(0, 0), Some([0, 0, 0, 255])); // celda 0-3
        assert_eq!(frame.pixel(9, 0), Some([255, 255, 255, 255])); // celda 8-9
    }

    #[test]
    fn un_rect_fuera_del_canvas_es_noop() {
        let original = mitades();
        let mut frame = original.clone();
        let mut canvas = Canvas::new(&mut frame);
        mosaico(&mut canvas, Rect::new(50, 50, 10, 10), 4);
        desenfoque(&mut canvas, Rect::new(-30, 0, 10, 10), 4);
        assert_eq!(frame, original);
    }

    #[test]
    fn el_desenfoque_difumina_el_borde_y_conserva_los_extremos() {
        let mut frame = mitades();
        desenfoque(&mut Canvas::new(&mut frame), Rect::new(0, 0, 8, 8), 2);
        // El borde negro/blanco pasa a ser un degradado monótono.
        let fila: Vec<u8> = (0..8).map(|x| frame.pixel(x, 4).unwrap()[0]).collect();
        for par in fila.windows(2) {
            assert!(par[1] >= par[0], "no es monótona: {fila:?}");
        }
        assert!(fila[3] > 0 && fila[4] < 255, "el borde no se difuminó: {fila:?}");
        // Lejos del borde el color se mantiene (radio 2 no alcanza).
        assert_eq!(fila[0], 0);
        assert_eq!(fila[7], 255);
    }

    #[test]
    fn el_desenfoque_de_un_color_plano_no_lo_cambia() {
        let mut frame = Frame::filled(6, 6, [30, 60, 90, 255]);
        desenfoque(&mut Canvas::new(&mut frame), Rect::new(0, 0, 6, 6), 3);
        assert_eq!(frame, Frame::filled(6, 6, [30, 60, 90, 255]));
    }

    #[test]
    fn el_desenfoque_recorta_el_rect_desbordado_sin_panico() {
        let mut frame = mitades();
        // Rect que sale por la derecha y por abajo: se recorta a 6×6.
        desenfoque(&mut Canvas::new(&mut frame), Rect::new(2, 2, 20, 20), 3);
        assert_eq!(frame.pixel(0, 0), Some([0, 0, 0, 255])); // fuera del rect
        let dentro = frame.pixel(4, 4).unwrap();
        assert!(dentro[0] > 0 && dentro[0] < 255);
    }
}
```

- [ ] **Step 2: Declarar el módulo**

En `crates/core/src/annotate/mod.rs`, añadir tras la línea `mod canvas;`:

```rust
mod censor;
```

- [ ] **Step 3: Ejecutar los tests y comprobar que pasan**

Run: `cargo test -p rustcapture-core annotate::censor`
Expected: PASS, 7 tests. (Si `blur_1d` fallara por índices, el síntoma es un panic de slice, no un valor incorrecto.)

Nota: en este punto `mosaico`/`desenfoque` no tienen llamadores, así que `cargo build` avisará de `dead_code`. Es esperado y se resuelve en la Task 3; no añadir `#[allow]`.

---

### Task 3: `PixelateAnnotation` y `CensorMode`

**Files:**
- Modify: `crates/core/src/annotate/style.rs`
- Create: `crates/core/src/annotate/annotations/pixelate.rs`
- Modify: `crates/core/src/annotate/annotations/mod.rs`
- Modify: `crates/core/src/annotate/mod.rs`

**Interfaces:**
- Consumes: `censor::mosaico`, `censor::desenfoque` (Task 2).
- Produces:
  - `pub enum CensorMode { Mosaic { block: u32 }, Blur { radius: u32 } }`
  - `pub struct PixelateAnnotation { pub rect: Rect, pub mode: CensorMode }`
  - Reexportados como `rustcapture_core::annotate::CensorMode` y `rustcapture_core::annotate::annotations::PixelateAnnotation`.

- [ ] **Step 1: Escribir el test que falla**

En `crates/core/src/annotate/annotations/mod.rs`, dentro de `mod tests`, añadir:

```rust
    #[test]
    fn el_pixelado_censura_su_rect_en_los_dos_modos() {
        // Frame con un cuadrado blanco de 8×8 dentro de fondo negro.
        let mut frame = Frame::filled(30, 30, [0, 0, 0, 255]);
        for y in 4..12u32 {
            for x in 4..12u32 {
                let i = (y as usize * 30 + x as usize) * 4;
                frame.pixels[i..i + 3].copy_from_slice(&[255, 255, 255]);
            }
        }
        let original = frame.clone();

        // Mosaico con bloque 8 sobre el rect 0..16: el cuadrado blanco se
        // diluye en la media de su celda y deja de ser blanco puro.
        let mut mosaico = original.clone();
        PixelateAnnotation {
            rect: Rect::new(0, 0, 16, 16),
            mode: CensorMode::Mosaic { block: 8 },
        }
        .render(
            &mut Canvas::new(&mut mosaico),
            &RenderContext::sin_fuente(),
        );
        assert_ne!(mosaico.pixel(6, 6), Some([255, 255, 255, 255]));
        // Fuera del rect no se toca nada.
        assert_eq!(mosaico.pixel(20, 20), Some([0, 0, 0, 255]));

        // Desenfoque: el borde del cuadrado deja de ser un salto seco.
        let mut borroso = original.clone();
        PixelateAnnotation {
            rect: Rect::new(0, 0, 16, 16),
            mode: CensorMode::Blur { radius: 3 },
        }
        .render(
            &mut Canvas::new(&mut borroso),
            &RenderContext::sin_fuente(),
        );
        let [r, ..] = borroso.pixel(3, 8).unwrap();
        assert!(r > 0 && r < 255, "el borde no se difuminó (r = {r})");
        assert_eq!(borroso.pixel(20, 20), Some([0, 0, 0, 255]));
    }

    #[test]
    fn el_pixelado_tapa_las_anotaciones_de_debajo() {
        // El z-order manda: una flecha pintada ANTES queda censurada.
        let mut frame = Frame::filled(30, 30, [0, 0, 0, 255]);
        let mut canvas = Canvas::new(&mut frame);
        let ctx = RenderContext::sin_fuente();
        LineAnnotation {
            from: (2, 8),
            to: (14, 8),
            style: ESTILO,
        }
        .render(&mut canvas, &ctx);
        assert!(es_rojo(&frame, 8, 8));
        PixelateAnnotation {
            rect: Rect::new(0, 0, 16, 16),
            mode: CensorMode::Mosaic { block: 16 },
        }
        .render(&mut Canvas::new(&mut frame), &ctx);
        assert!(!es_rojo(&frame, 8, 8), "la línea sobrevivió a la censura");
    }
```

Y ampliar el `use` de ese `mod tests` para que incluya los tipos nuevos:

```rust
    use crate::annotate::style::{CensorMode, Color, Style};
```

- [ ] **Step 2: Ejecutar el test y comprobar que falla**

Run: `cargo test -p rustcapture-core annotate::annotations`
Expected: FAIL de compilación — `cannot find struct 'PixelateAnnotation'` y `unresolved import ... CensorMode`.

- [ ] **Step 3: Añadir `CensorMode` a los estilos**

En `crates/core/src/annotate/style.rs`, tras `struct Style`:

```rust
/// Estilo de censura del pixelado (f.25): las dos variantes recorren el
/// mismo camino (leer el canvas → reescribirlo), solo cambia el filtro.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CensorMode {
    /// Celdas de `block`×`block` aplanadas a su color medio.
    Mosaic { block: u32 },
    /// Desenfoque de caja de radio `radius`.
    Blur { radius: u32 },
}
```

- [ ] **Step 4: Crear la Strategy**

Crear `crates/core/src/annotate/annotations/pixelate.rs`:

```rust
//! Pixelado / desenfoque (f.25): censura una región tal y como se ve en
//! su punto del z-order — lee del canvas y lo reescribe.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::censor;
use crate::annotate::style::CensorMode;
use crate::annotate::text::RenderContext;
use crate::ports::Rect;

pub struct PixelateAnnotation {
    pub rect: Rect,
    pub mode: CensorMode,
}

impl Annotation for PixelateAnnotation {
    fn render(&self, canvas: &mut Canvas, _ctx: &RenderContext) {
        match self.mode {
            CensorMode::Mosaic { block } => censor::mosaico(canvas, self.rect, block),
            CensorMode::Blur { radius } => censor::desenfoque(canvas, self.rect, radius),
        }
    }
}
```

- [ ] **Step 5: Declarar y reexportar**

En `crates/core/src/annotate/annotations/mod.rs`, añadir `mod pixelate;` en orden alfabético (entre `mod pen;` y `mod rect;`) y el reexport correspondiente:

```rust
pub use pixelate::PixelateAnnotation;
```

En `crates/core/src/annotate/mod.rs`, ampliar el reexport de estilos:

```rust
pub use style::{CensorMode, Color, Style, TextStyle};
```

- [ ] **Step 6: Ejecutar los tests y comprobar que pasan**

Run: `cargo test -p rustcapture-core`
Expected: PASS. Ya no debe quedar aviso de `dead_code` en `censor`.

---

### Task 4: Primitivas del paso numerado (disco con AA y caja de tinta)

Dos primitivas para que el número quede bien puesto y el disco no salga con dientes de sierra:

- `fill_disc_aa`: disco relleno con antialiasing por supermuestreo 4×4. Mezcla **una sola vez** por píxel con el alfa de su cobertura, así que no sufre el problema documentado en la cabecera de `shapes.rs` (estampados con solape que se mezclan varias veces).
- `text_ink_box`: caja de la **tinta** del texto respecto al origen que recibe `draw_text`. Centrar por la caja de línea dejaría el número visiblemente bajo, porque esa caja incluye el hueco del ascendente y del descendente que las cifras no usan.

**Files:**
- Modify: `crates/core/src/annotate/shapes.rs`
- Modify: `crates/core/src/annotate/text.rs`

**Interfaces:**
- Consumes: `Canvas::blend_pixel`, `RenderContext::font`.
- Produces:
  - `pub(crate) fn fill_disc_aa(canvas: &mut Canvas, centro: (i32, i32), radio: u32, color: Color)`
  - `pub(crate) fn text_ink_box(text: &str, style: TextStyle, ctx: &RenderContext) -> Option<(i32, i32, u32, u32)>` — `(dx, dy, ancho, alto)` relativos al `pos` de `draw_text`; `None` sin fuente o sin tinta.

- [ ] **Step 1: Escribir los tests que fallan**

En `crates/core/src/annotate/shapes.rs`, dentro de `mod tests`:

```rust
    #[test]
    fn el_disco_aa_rellena_el_centro_y_suaviza_el_borde() {
        let mut frame = Frame::filled(20, 20, NEGRO);
        fill_disc_aa(&mut Canvas::new(&mut frame), (10, 10), 5, ROJO);
        // Centro y radio interior: rojo puro.
        assert!(es_rojo(&frame, 10, 10) && es_rojo(&frame, 10, 7));
        // Justo en el borde: cobertura parcial → rojo a medias.
        let [r, ..] = frame.pixel(10, 5).unwrap();
        assert!(r > 0 && r < 255, "el borde no tiene AA (r = {r})");
        // Fuera del disco: intacto.
        assert_eq!(frame.pixel(10, 3), Some(NEGRO));
        assert_eq!(frame.pixel(3, 3), Some(NEGRO));
    }

    #[test]
    fn el_disco_de_radio_cero_es_noop() {
        let mut frame = Frame::filled(6, 6, NEGRO);
        fill_disc_aa(&mut Canvas::new(&mut frame), (3, 3), 0, ROJO);
        assert_eq!(frame, Frame::filled(6, 6, NEGRO));
    }
```

En `crates/core/src/annotate/text.rs`, crear al final del archivo un `mod tests` (hoy no tiene):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotate::canvas::Canvas;
    use crate::annotate::style::Color;
    use crate::ports::Frame;

    fn ctx_con_fuente() -> RenderContext {
        let normal = std::fs::read("C:/Windows/Fonts/segoeui.ttf").expect("fuente del sistema");
        let bold = std::fs::read("C:/Windows/Fonts/segoeuib.ttf").expect("fuente del sistema");
        RenderContext::new(&normal, &bold).unwrap()
    }

    fn estilo(size: f32) -> TextStyle {
        TextStyle {
            color: Color::rgb(255, 0, 0),
            size,
            bold: true,
        }
    }

    #[test]
    fn sin_fuente_no_hay_caja_de_tinta() {
        assert_eq!(
            text_ink_box("7", estilo(20.0), &RenderContext::sin_fuente()),
            None
        );
    }

    #[test]
    fn el_espacio_no_tiene_tinta() {
        assert_eq!(text_ink_box("   ", estilo(20.0), &ctx_con_fuente()), None);
    }

    #[test]
    fn la_caja_de_tinta_encierra_exactamente_los_pixeles_pintados() {
        // La caja se mide y luego se compara con lo que de verdad se pinta.
        let ctx = ctx_con_fuente();
        let style = estilo(24.0);
        let (dx, dy, w, h) = text_ink_box("12", style, &ctx).expect("hay tinta");
        assert!(w > 0 && h > 0);

        let origen = (10, 10);
        let mut frame = Frame::filled(80, 60, [0, 0, 0, 255]);
        draw_text(&mut Canvas::new(&mut frame), origen, "12", style, &ctx);
        let pintados: Vec<(u32, u32)> = (0..80)
            .flat_map(|x| (0..60).map(move |y| (x, y)))
            .filter(|&(x, y)| frame.pixel(x, y).is_some_and(|[r, ..]| r > 0))
            .collect();
        assert!(!pintados.is_empty());
        let min_x = pintados.iter().map(|p| p.0).min().unwrap() as i32;
        let min_y = pintados.iter().map(|p| p.1).min().unwrap() as i32;
        let max_x = pintados.iter().map(|p| p.0).max().unwrap() as i32;
        let max_y = pintados.iter().map(|p| p.1).max().unwrap() as i32;
        // La caja predicha contiene la tinta real y no se pasa de holgada.
        assert_eq!((min_x, min_y), (origen.0 + dx, origen.1 + dy));
        assert_eq!((max_x + 1, max_y + 1), (origen.0 + dx + w as i32, origen.1 + dy + h as i32));
    }

    #[test]
    fn dos_lineas_dan_una_caja_mas_alta_que_una() {
        let ctx = ctx_con_fuente();
        let (_, _, _, h1) = text_ink_box("A", estilo(20.0), &ctx).unwrap();
        let (_, _, _, h2) = text_ink_box("A\nA", estilo(20.0), &ctx).unwrap();
        assert!(h2 > h1 * 2 - 4, "h1 = {h1}, h2 = {h2}");
    }
}
```

- [ ] **Step 2: Ejecutar los tests y comprobar que fallan**

Run: `cargo test -p rustcapture-core annotate::shapes annotate::text`
Expected: FAIL de compilación — `cannot find function 'fill_disc_aa'` y `cannot find function 'text_ink_box'`.

- [ ] **Step 3: Implementar `fill_disc_aa`**

En `crates/core/src/annotate/shapes.rs`, tras `stamp_disc`:

```rust
/// Disco relleno con antialiasing: cada píxel se mezcla UNA vez con el
/// alfa de su cobertura (supermuestreo 4×4 solo en la corona del borde),
/// así el contorno sale suave y sin las manchas del estampado solapado
/// que describe la cabecera de este módulo.
pub(crate) fn fill_disc_aa(canvas: &mut Canvas, centro: (i32, i32), radio: u32, color: Color) {
    if radio == 0 {
        return;
    }
    let r = f64::from(radio);
    // Centro geométrico = centro del píxel `centro`.
    let (cx, cy) = (f64::from(centro.0) + 0.5, f64::from(centro.1) + 0.5);
    let borde = radio as i32 + 1;
    for py in centro.1 - borde..=centro.1 + borde {
        for px in centro.0 - borde..=centro.0 + borde {
            let dx = f64::from(px) + 0.5 - cx;
            let dy = f64::from(py) + 0.5 - cy;
            let d = (dx * dx + dy * dy).sqrt();
            // Interior y exterior se resuelven sin muestrear.
            let cobertura = if d <= r - 0.75 {
                1.0
            } else if d >= r + 0.75 {
                0.0
            } else {
                cobertura_4x4(px, py, cx, cy, r)
            };
            if cobertura > 0.0 {
                let a = (f64::from(color.a) * cobertura).round() as u8;
                canvas.blend_pixel(px, py, Color::rgba(color.r, color.g, color.b, a));
            }
        }
    }
}

/// Fracción de las 16 submuestras del píxel que caen dentro del disco.
fn cobertura_4x4(px: i32, py: i32, cx: f64, cy: f64, r: f64) -> f64 {
    let mut dentro = 0;
    for sy in 0..4 {
        for sx in 0..4 {
            let x = f64::from(px) + (f64::from(sx) + 0.5) / 4.0;
            let y = f64::from(py) + (f64::from(sy) + 0.5) / 4.0;
            let (dx, dy) = (x - cx, y - cy);
            if dx * dx + dy * dy <= r * r {
                dentro += 1;
            }
        }
    }
    f64::from(dentro) / 16.0
}
```

- [ ] **Step 4: Implementar `text_ink_box`**

En `crates/core/src/annotate/text.rs`, tras `draw_text`:

```rust
/// Caja de la TINTA del texto, relativa al `pos` que recibe `draw_text`:
/// `(dx, dy, ancho, alto)`. `None` sin fuente cargada o si nada pinta
/// (p. ej. solo espacios). Replica la colocación de `draw_text` glifo a
/// glifo — si una cambia, la otra tiene que cambiar igual.
///
/// Existe para centrar ópticamente: la caja de línea incluye hueco de
/// ascendente/descendente que las cifras no usan, y centrar por ella
/// deja el número visiblemente bajo dentro del disco (f.23).
pub(crate) fn text_ink_box(
    text: &str,
    style: TextStyle,
    ctx: &RenderContext,
) -> Option<(i32, i32, u32, u32)> {
    let font = ctx.font(style.bold)?;
    let line_height = (style.size * 1.2).round() as i32;
    let (mut min_x, mut min_y) = (i32::MAX, i32::MAX);
    let (mut max_x, mut max_y) = (i32::MIN, i32::MIN);
    for (n, linea) in text.split('\n').enumerate() {
        let base_y = n as i32 * line_height;
        let mut cursor_x = 0.0f32;
        for c in linea.chars() {
            let metrics = font.metrics(c, style.size);
            if metrics.width > 0 && metrics.height > 0 {
                let gx = cursor_x.round() as i32 + metrics.xmin;
                let gy = base_y + style.size.round() as i32 - metrics.height as i32 - metrics.ymin;
                min_x = min_x.min(gx);
                min_y = min_y.min(gy);
                max_x = max_x.max(gx + metrics.width as i32);
                max_y = max_y.max(gy + metrics.height as i32);
            }
            cursor_x += metrics.advance_width;
        }
    }
    (min_x < max_x && min_y < max_y).then(|| {
        (
            min_x,
            min_y,
            (max_x - min_x) as u32,
            (max_y - min_y) as u32,
        )
    })
}
```

- [ ] **Step 5: Ejecutar los tests y comprobar que pasan**

Run: `cargo test -p rustcapture-core annotate::shapes annotate::text`
Expected: PASS. Si `la_caja_de_tinta_encierra_exactamente_los_pixeles_pintados` falla por un píxel, es que `text_ink_box` y `draw_text` han divergido en el redondeo: comparar las dos expresiones de `gx`/`gy`, deben ser idénticas.

Nota: como en Task 2, `fill_disc_aa` y `text_ink_box` quedan sin llamadores de producción hasta la Task 5; el aviso de `dead_code` es esperado.

---

### Task 5: `StepAnnotation` (paso numerado)

**Files:**
- Create: `crates/core/src/annotate/annotations/step.rs`
- Modify: `crates/core/src/annotate/annotations/mod.rs`

**Interfaces:**
- Consumes: `fill_disc_aa`, `text_ink_box` (Task 4), `Color::contraste` (Task 1), `draw_text`.
- Produces:
  - `pub struct StepAnnotation { pub center: (i32, i32), pub number: u32, pub color: Color, pub font_size: f32 }`
  - `pub fn StepAnnotation::radius(&self) -> u32`
  - Reexportado como `rustcapture_core::annotate::annotations::StepAnnotation`.

- [ ] **Step 1: Escribir el test que falla**

En `crates/core/src/annotate/annotations/mod.rs`, dentro de `mod tests`:

```rust
    #[test]
    fn el_radio_del_paso_crece_con_los_digitos() {
        let paso = |number| StepAnnotation {
            center: (0, 0),
            number,
            color: ROJO,
            font_size: 20.0,
        };
        assert_eq!(paso(1).radius(), paso(9).radius());
        assert!(paso(10).radius() > paso(9).radius());
        assert!(paso(100).radius() > paso(10).radius());
        // Una fuente diminuta no da radio 0 (el disco sigue viéndose).
        assert!(
            StepAnnotation {
                center: (0, 0),
                number: 1,
                color: ROJO,
                font_size: 1.0,
            }
            .radius()
                >= 2
        );
    }

    #[test]
    fn el_paso_pinta_disco_con_el_numero_centrado_y_en_contraste() {
        let mut frame = Frame::filled(60, 60, [0, 0, 0, 255]);
        let paso = StepAnnotation {
            center: (30, 30),
            number: 3,
            color: ROJO,
            font_size: 24.0,
        };
        paso.render(&mut Canvas::new(&mut frame), &ctx_con_fuente());
        let radio = paso.radius() as i32;

        // El disco cubre su radio: un punto a media distancia es rojo.
        assert!(es_rojo(&frame, 30, (30 - radio / 2) as u32));
        // Fuera del disco, intacto.
        assert_eq!(frame.pixel(30, (30 - radio - 3) as u32), Some([0, 0, 0, 255]));

        // El número va en blanco (contraste del rojo) y dentro del disco.
        let blancos: Vec<(u32, u32)> = (0..60)
            .flat_map(|x| (0..60).map(move |y| (x, y)))
            .filter(|&(x, y)| {
                frame
                    .pixel(x, y)
                    .is_some_and(|[r, g, b, _]| r > 200 && g > 200 && b > 200)
            })
            .collect();
        assert!(!blancos.is_empty(), "el número no se pintó");
        // Centrado: el centroide de la tinta cae a ≤2 px del centro.
        let n = blancos.len() as i32;
        let cx = blancos.iter().map(|p| p.0 as i32).sum::<i32>() / n;
        let cy = blancos.iter().map(|p| p.1 as i32).sum::<i32>() / n;
        assert!((cx - 30).abs() <= 2 && (cy - 30).abs() <= 2, "centroide ({cx}, {cy})");
        // Y toda la tinta queda dentro del disco.
        for (x, y) in blancos {
            let (dx, dy) = (x as i32 - 30, y as i32 - 30);
            assert!(dx * dx + dy * dy <= radio * radio, "número fuera en ({x}, {y})");
        }
    }

    #[test]
    fn el_paso_sin_fuente_pinta_solo_el_disco() {
        let mut frame = Frame::filled(40, 40, [0, 0, 0, 255]);
        StepAnnotation {
            center: (20, 20),
            number: 5,
            color: ROJO,
            font_size: 20.0,
        }
        .render(&mut Canvas::new(&mut frame), &RenderContext::sin_fuente());
        assert!(es_rojo(&frame, 20, 20));
    }
```

- [ ] **Step 2: Ejecutar el test y comprobar que falla**

Run: `cargo test -p rustcapture-core annotate::annotations`
Expected: FAIL de compilación — `cannot find struct 'StepAnnotation'`.

- [ ] **Step 3: Crear la Strategy**

Crear `crates/core/src/annotate/annotations/step.rs`:

```rust
//! Paso numerado (f.23): disco relleno con su número centrado dentro. El
//! radio se deriva del tamaño de fuente y de los dígitos, así el número
//! nunca sobresale del disco y la herramienta es un solo clic.

use crate::annotate::annotations::Annotation;
use crate::annotate::canvas::Canvas;
use crate::annotate::shapes;
use crate::annotate::style::{Color, TextStyle};
use crate::annotate::text::{RenderContext, draw_text, text_ink_box};

pub struct StepAnnotation {
    /// Centro del disco, en píxeles del frame.
    pub center: (i32, i32),
    pub number: u32,
    /// Color del disco; el número se pinta en blanco o negro según él.
    pub color: Color,
    /// Altura de la fuente del número (misma escala que `TextAnnotation`).
    pub font_size: f32,
}

impl StepAnnotation {
    /// Radio del disco: cubre el número con margen y crece con los
    /// dígitos, de modo que el 12 no queda apretado donde caía el 1.
    pub fn radius(&self) -> u32 {
        let digitos = self.number.to_string().len() as f32;
        (self.font_size * (0.75 + 0.22 * (digitos - 1.0)))
            .round()
            .max(2.0) as u32
    }
}

impl Annotation for StepAnnotation {
    fn render(&self, canvas: &mut Canvas, ctx: &RenderContext) {
        shapes::fill_disc_aa(canvas, self.center, self.radius(), self.color);
        let style = TextStyle {
            color: self.color.contraste(),
            size: self.font_size,
            bold: true,
        };
        let etiqueta = self.number.to_string();
        // Se centra la caja de TINTA, no la de línea: ver `text_ink_box`.
        if let Some((dx, dy, w, h)) = text_ink_box(&etiqueta, style, ctx) {
            let pos = (
                self.center.0 - dx - w as i32 / 2,
                self.center.1 - dy - h as i32 / 2,
            );
            draw_text(canvas, pos, &etiqueta, style, ctx);
        }
    }
}
```

- [ ] **Step 4: Declarar y reexportar**

En `crates/core/src/annotate/annotations/mod.rs`, añadir `mod step;` (entre `mod rect;` y `mod text;`) y:

```rust
pub use step::StepAnnotation;
```

- [ ] **Step 5: Ejecutar todos los tests del core**

Run: `cargo test -p rustcapture-core`
Expected: PASS, sin avisos de `dead_code`.

- [ ] **Step 6: Comprobar que no hay avisos de clippy**

Run: `cargo clippy -p rustcapture-core --all-targets`
Expected: sin warnings. Si aparece `too_many_arguments` en `blur_1d`, refactorizar agrupando `(paso, salto)` en una tupla `pasos: (usize, usize)` antes que silenciarlo.

---

### Task 6: Agrupar las propiedades del editor en `Propiedades`

Refactor sin cambio de comportamiento, previo al cableado: `chips()` ya recibe cuatro parámetros posicionales y el pixelado le añadiría dos más. Se agrupan en un struct `Copy` que también deja preparada la leyenda del slice siguiente.

**Files:**
- Modify: `crates/platform-win/src/editor/estado.rs`
- Modify: `crates/platform-win/src/editor/props.rs`
- Modify: `crates/platform-win/src/editor/texto.rs`

**Interfaces:**
- Consumes: `CensorMode` (Task 3).
- Produces:
  - `pub(super) struct Propiedades { pub color: Color, pub grosor: u32, pub tamano_texto: f32, pub negrita: bool, pub censura: CensorMode }` con `Default`
  - `Propiedades::censura_px(&self) -> u32`, `con_censura_px(&mut self, px: u32)`, `alternar_censura(&mut self)`
  - `EditorState.props: Propiedades` (sustituye a `color`, `grosor`, `tamano_texto`, `negrita`)
  - `props::chips(herramienta: Herramienta, p: &Propiedades) -> Vec<Chip>`

- [ ] **Step 1: Escribir los tests que fallan**

En `crates/platform-win/src/editor/estado.rs`, crear al final un `mod tests`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn las_propiedades_por_defecto_son_las_del_diseno() {
        let p = Propiedades::default();
        assert_eq!(p.color, COLOR_DEFECTO);
        assert_eq!(p.grosor, 3);
        assert_eq!(p.tamano_texto, 20.0);
        assert!(!p.negrita);
        assert_eq!(p.censura, CensorMode::Mosaic { block: 8 });
        assert_eq!(p.censura_px(), 8);
    }

    #[test]
    fn alternar_la_censura_conserva_los_px() {
        let mut p = Propiedades::default();
        p.con_censura_px(16);
        p.alternar_censura();
        assert_eq!(p.censura, CensorMode::Blur { radius: 16 });
        assert_eq!(p.censura_px(), 16);
        p.alternar_censura();
        assert_eq!(p.censura, CensorMode::Mosaic { block: 16 });
    }
}
```

- [ ] **Step 2: Ejecutar los tests y comprobar que fallan**

Run: `cargo test -p platform-win editor::estado`
Expected: FAIL de compilación — `cannot find type 'Propiedades'`.

- [ ] **Step 3: Añadir `Propiedades` a `estado.rs`**

En `crates/platform-win/src/editor/estado.rs`, ampliar los `use` del core:

```rust
use rustcapture_core::annotate::{CensorMode, Color, Document, History, RenderContext, Style};
```

Añadir la constante de tamaños de censura junto a las otras listas:

```rust
/// Bloque del mosaico / radio del desenfoque ofrecidos en la barra (f.25).
pub(super) const CENSURAS: [u32; 5] = [4, 8, 12, 16, 24];
```

Y el struct, antes de `DragState`:

```rust
/// Propiedades de dibujo que edita la barra contextual (f.22-f.25).
/// Agrupadas para que `props::chips` no crezca en parámetros sueltos.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct Propiedades {
    pub color: Color,
    pub grosor: u32,
    pub tamano_texto: f32,
    pub negrita: bool,
    /// Modo de censura vigente, con su parámetro en px dentro.
    pub censura: CensorMode,
}

impl Default for Propiedades {
    fn default() -> Self {
        Self {
            color: COLOR_DEFECTO,
            grosor: 3,
            tamano_texto: 20.0,
            negrita: false,
            censura: CensorMode::Mosaic { block: 8 },
        }
    }
}

impl Propiedades {
    /// px del modo vigente: bloque del mosaico o radio del desenfoque.
    pub(super) fn censura_px(&self) -> u32 {
        match self.censura {
            CensorMode::Mosaic { block } => block,
            CensorMode::Blur { radius } => radius,
        }
    }

    pub(super) fn con_censura_px(&mut self, px: u32) {
        self.censura = match self.censura {
            CensorMode::Mosaic { .. } => CensorMode::Mosaic { block: px },
            CensorMode::Blur { .. } => CensorMode::Blur { radius: px },
        };
    }

    /// Conmuta mosaico ↔ desenfoque conservando los px elegidos.
    pub(super) fn alternar_censura(&mut self) {
        let px = self.censura_px();
        self.censura = match self.censura {
            CensorMode::Mosaic { .. } => CensorMode::Blur { radius: px },
            CensorMode::Blur { .. } => CensorMode::Mosaic { block: px },
        };
    }
}
```

- [ ] **Step 4: Sustituir los campos sueltos en `EditorState`**

En el struct `EditorState`, quitar las cuatro líneas `pub color`, `pub grosor`, `pub tamano_texto`, `pub negrita` y poner en su lugar:

```rust
    pub props: Propiedades,
```

En `EditorState::new`, quitar las cuatro inicializaciones correspondientes y poner:

```rust
            props: Propiedades::default(),
```

En `anotacion_en_curso`, actualizar las dos referencias:

```rust
        let style = Style {
            color: self.props.color,
            thickness: self.props.grosor,
        };
```

```rust
            Herramienta::Resaltador => Box::new(HighlightAnnotation {
                rect,
                color: Color::rgba(
                    self.props.color.r,
                    self.props.color.g,
                    self.props.color.b,
                    128,
                ),
            }),
```

- [ ] **Step 5: Actualizar `props.rs`**

Cambiar la firma y el cuerpo de `chips`:

```rust
/// Composición pura de los chips para la herramienta activa.
pub(super) fn chips(herramienta: Herramienta, p: &Propiedades) -> Vec<Chip> {
    let color = Chip {
        etiqueta: "Color".to_string(),
        muestra_color: true,
        accion: Accion::ElegirColor,
    };
    match herramienta {
        Herramienta::Texto => vec![
            Chip {
                etiqueta: format!("Tamaño {}", p.tamano_texto as u32),
                muestra_color: false,
                accion: Accion::MenuTamano,
            },
            Chip {
                etiqueta: format!("Negrita: {}", if p.negrita { "sí" } else { "no" }),
                muestra_color: false,
                accion: Accion::ToggleNegrita,
            },
            color,
        ],
        Herramienta::Resaltador => vec![color],
        _ => vec![
            Chip {
                etiqueta: format!("Grosor {} px", p.grosor),
                muestra_color: false,
                accion: Accion::MenuGrosor,
            },
            color,
        ],
    }
}
```

Ampliar el `use` de `estado`:

```rust
use super::estado::{EditorState, GROSORES, Propiedades, TAMANOS};
```

En `pintar`, la llamada:

```rust
    let lista = chips(state.herramienta, &state.props);
```

y las tres referencias al color del swatch pasan a `state.props.color`.

En `on_click`, los accesos pasan a `state.props.*`:

```rust
        Accion::MenuGrosor => {
            let etiquetas: Vec<String> = GROSORES.iter().map(|g| format!("{g} px")).collect();
            let actual = GROSORES.iter().position(|&g| g == state.props.grosor);
            if let Some(i) = menu_de_opciones(hwnd, p, &etiquetas, actual) {
                state.props.grosor = GROSORES[i];
            }
        }
        Accion::MenuTamano => {
            let etiquetas: Vec<String> =
                TAMANOS.iter().map(|t| format!("{} px", *t as u32)).collect();
            let actual = TAMANOS.iter().position(|&t| t == state.props.tamano_texto);
            if let Some(i) = menu_de_opciones(hwnd, p, &etiquetas, actual) {
                state.props.tamano_texto = TAMANOS[i];
            }
        }
        Accion::ToggleNegrita => state.props.negrita = !state.props.negrita,
```

En `elegir_color`, `state.color` pasa a `state.props.color` (dos sitios: `actual` y la asignación).

Y los tres tests de `props.rs` pasan a construir `Propiedades`:

```rust
    #[test]
    fn las_formas_llevan_grosor_y_color() {
        let p = Propiedades::default();
        for h in [
            Herramienta::Flecha,
            Herramienta::Linea,
            Herramienta::Rect,
            Herramienta::Elipse,
            Herramienta::Lapiz,
        ] {
            let chips = chips(h, &p);
            assert_eq!(chips.len(), 2, "{h:?}");
            assert_eq!(chips[0].etiqueta, "Grosor 3 px");
            assert_eq!(chips[0].accion, Accion::MenuGrosor);
            assert!(chips[1].muestra_color);
        }
    }

    #[test]
    fn el_texto_lleva_tamano_negrita_y_color() {
        let p = Propiedades {
            tamano_texto: 28.0,
            negrita: true,
            ..Propiedades::default()
        };
        let chips = chips(Herramienta::Texto, &p);
        assert_eq!(chips.len(), 3);
        assert_eq!(chips[0].etiqueta, "Tamaño 28");
        assert_eq!(chips[1].etiqueta, "Negrita: sí");
        assert_eq!(chips[1].accion, Accion::ToggleNegrita);
        assert_eq!(chips[2].accion, Accion::ElegirColor);
    }

    #[test]
    fn el_resaltador_solo_lleva_color() {
        let chips = chips(Herramienta::Resaltador, &Propiedades::default());
        assert_eq!(chips.len(), 1);
        assert!(chips[0].muestra_color);
    }
```

- [ ] **Step 6: Actualizar `texto.rs`**

En `abrir_edit`, las dos referencias:

```rust
        let font = CreateFontW(
            state.props.tamano_texto.round() as i32,
```

```rust
            if state.props.negrita {
```

En `commit_text`, el estilo:

```rust
                style: TextStyle {
                    color: state.props.color,
                    size: state.props.tamano_texto,
                    bold: state.props.negrita,
                },
```

- [ ] **Step 7: Ejecutar los tests y comprobar que pasan**

Run: `cargo test -p platform-win`
Expected: PASS, incluidos los dos tests nuevos de `estado`. `cargo build` sin errores: si queda algún `state.color`/`state.grosor` sin migrar, el compilador lo señala.

---

### Task 7: Contador de pasos sincronizado con undo/redo

Si el número viviera en un simple `u32` monótono, colocar el paso 1, deshacerlo y volver a colocarlo daría un 2. El contador guarda el valor vigente ANTES de cada comando aplicado, en pilas paralelas a las de `History`, de modo que deshacer devuelve su número.

**Files:**
- Modify: `crates/platform-win/src/editor/estado.rs`

**Interfaces:**
- Consumes: nada del core.
- Produces: `pub(super) struct ContadorPasos` con `new()`, `siguiente() -> u32`, `aplicado(&mut self, fue_paso: bool)`, `deshecho(&mut self)`, `rehecho(&mut self)`; y el campo `EditorState.pasos: ContadorPasos`.

- [ ] **Step 1: Escribir los tests que fallan**

En el `mod tests` de `crates/platform-win/src/editor/estado.rs`:

```rust
    #[test]
    fn los_pasos_empiezan_en_uno_y_avanzan_al_colocarse() {
        let mut c = ContadorPasos::new();
        assert_eq!(c.siguiente(), 1);
        c.aplicado(true);
        assert_eq!(c.siguiente(), 2);
        c.aplicado(true);
        assert_eq!(c.siguiente(), 3);
    }

    #[test]
    fn otras_herramientas_no_consumen_numero() {
        let mut c = ContadorPasos::new();
        c.aplicado(false); // una flecha
        assert_eq!(c.siguiente(), 1);
    }

    #[test]
    fn deshacer_un_paso_devuelve_su_numero_y_rehacer_lo_vuelve_a_gastar() {
        let mut c = ContadorPasos::new();
        c.aplicado(true);
        assert_eq!(c.siguiente(), 2);
        c.deshecho();
        assert_eq!(c.siguiente(), 1);
        c.rehecho();
        assert_eq!(c.siguiente(), 2);
    }

    #[test]
    fn deshacer_comandos_intercalados_mantiene_la_numeracion() {
        let mut c = ContadorPasos::new();
        c.aplicado(false); // flecha
        c.aplicado(true); // paso 1
        c.aplicado(false); // rectángulo
        assert_eq!(c.siguiente(), 2);
        c.deshecho(); // deshace el rectángulo
        assert_eq!(c.siguiente(), 2);
        c.deshecho(); // deshace el paso 1
        assert_eq!(c.siguiente(), 1);
        c.deshecho(); // deshace la flecha
        assert_eq!(c.siguiente(), 1);
    }

    #[test]
    fn un_comando_nuevo_invalida_el_rehacer_del_contador() {
        let mut c = ContadorPasos::new();
        c.aplicado(true);
        c.deshecho();
        assert_eq!(c.siguiente(), 1);
        c.aplicado(true); // se coloca otro paso: reusa el 1
        assert_eq!(c.siguiente(), 2);
        c.rehecho(); // ya no hay nada que rehacer
        assert_eq!(c.siguiente(), 2);
    }

    #[test]
    fn deshacer_sin_historia_no_rompe_el_contador() {
        let mut c = ContadorPasos::new();
        c.deshecho();
        c.rehecho();
        assert_eq!(c.siguiente(), 1);
    }
```

- [ ] **Step 2: Ejecutar los tests y comprobar que fallan**

Run: `cargo test -p platform-win editor::estado`
Expected: FAIL de compilación — `cannot find type 'ContadorPasos'`.

- [ ] **Step 3: Implementar el contador**

En `crates/platform-win/src/editor/estado.rs`, tras `impl Propiedades`:

```rust
/// Numeración de los pasos (f.23) en paralelo a las pilas de `History`:
/// cada comando aplicado apunta el número vigente ANTES de él, así
/// deshacer un paso devuelve su número al siguiente que se coloque.
/// Sus tres métodos se llaman EXACTAMENTE donde se llama a `History`.
pub(super) struct ContadorPasos {
    siguiente: u32,
    antes: Vec<u32>,
    despues: Vec<u32>,
}

impl ContadorPasos {
    pub(super) fn new() -> Self {
        Self {
            siguiente: 1,
            antes: Vec::new(),
            despues: Vec::new(),
        }
    }

    pub(super) fn siguiente(&self) -> u32 {
        self.siguiente
    }

    /// Un comando se aplicó con éxito; `fue_paso` gasta un número.
    pub(super) fn aplicado(&mut self, fue_paso: bool) {
        self.antes.push(self.siguiente);
        if fue_paso {
            self.siguiente += 1;
        }
        self.despues.clear();
    }

    pub(super) fn deshecho(&mut self) {
        if let Some(previo) = self.antes.pop() {
            self.despues.push(self.siguiente);
            self.siguiente = previo;
        }
    }

    pub(super) fn rehecho(&mut self) {
        if let Some(posterior) = self.despues.pop() {
            self.antes.push(self.siguiente);
            self.siguiente = posterior;
        }
    }
}
```

- [ ] **Step 4: Añadirlo al estado del editor**

En el struct `EditorState`, tras `pub props: Propiedades,`:

```rust
    /// Numeración de pasos, sincronizada con `history`.
    pub pasos: ContadorPasos,
```

Y en `EditorState::new`, tras `props: Propiedades::default(),`:

```rust
            pasos: ContadorPasos::new(),
```

- [ ] **Step 5: Ejecutar los tests y comprobar que pasan**

Run: `cargo test -p platform-win editor::estado`
Expected: PASS, 8 tests en `editor::estado`.

---

### Task 8: Cablear las herramientas Pixelado y Pasos en el editor

**Files:**
- Modify: `crates/platform-win/src/editor/math.rs`
- Modify: `crates/platform-win/src/editor/estado.rs`
- Modify: `crates/platform-win/src/editor/props.rs`
- Modify: `crates/platform-win/src/editor/mod.rs`

**Interfaces:**
- Consumes: `PixelateAnnotation`, `CensorMode` (Task 3), `StepAnnotation` (Task 5), `Propiedades` (Task 6), `ContadorPasos` (Task 7).
- Produces: `Herramienta::Pixelado`, `Herramienta::Pasos`; `Accion::ToggleCensura`, `Accion::MenuCensuraPx`; `mod.rs::colocar_paso`.

- [ ] **Step 1: Escribir los tests que fallan**

En `crates/platform-win/src/editor/math.rs`, actualizar dos tests existentes y añadir uno:

```rust
    #[test]
    fn las_herramientas_del_motor_y_las_salidas_estan_habilitadas() {
        let fila = toolbar();
        let habilitados: Vec<u16> =
            botones(&fila).iter().filter(|b| b.habilitado).map(|b| b.id).collect();
        assert_eq!(
            habilitados,
            vec![
                ID_TEXTO,
                ID_FLECHA,
                ID_LINEA,
                ID_RECT,
                ID_ELIPSE,
                ID_DRAW,
                ID_RESALTADOR,
                ID_PASOS,
                ID_PIXELADO,
                ID_UNDO,
                ID_REDO,
                ID_COPIAR,
                ID_GUARDAR
            ]
        );
    }

    #[test]
    fn cada_herramienta_mapea_a_su_boton_y_vuelta() {
        use Herramienta::*;
        for h in [
            Texto, Flecha, Linea, Rect, Elipse, Lapiz, Resaltador, Pasos, Pixelado,
        ] {
            assert_eq!(herramienta_de_id(id_de_herramienta(h)), Some(h));
        }
        assert_eq!(herramienta_de_id(ID_COPIAR), None);
        assert_eq!(herramienta_de_id(ID_SELECT), None); // sin lógica aún
        assert_eq!(herramienta_de_id(ID_LEYENDA), None); // pendiente
    }

    #[test]
    fn las_herramientas_de_un_clic_no_arrastran() {
        // Texto y Pasos se colocan con un clic simple; el resto arrastra.
        assert!(es_de_un_clic(Herramienta::Texto));
        assert!(es_de_un_clic(Herramienta::Pasos));
        for h in [
            Herramienta::Flecha,
            Herramienta::Linea,
            Herramienta::Rect,
            Herramienta::Elipse,
            Herramienta::Lapiz,
            Herramienta::Resaltador,
            Herramienta::Pixelado,
        ] {
            assert!(!es_de_un_clic(h), "{h:?}");
        }
    }
```

En `crates/platform-win/src/editor/props.rs`, dentro de `mod tests`:

```rust
    #[test]
    fn el_pixelado_lleva_modo_y_px_pero_no_color() {
        let chips = chips(Herramienta::Pixelado, &Propiedades::default());
        assert_eq!(chips.len(), 2);
        assert_eq!(chips[0].etiqueta, "Modo: mosaico");
        assert_eq!(chips[0].accion, Accion::ToggleCensura);
        assert_eq!(chips[1].etiqueta, "Bloque 8 px");
        assert_eq!(chips[1].accion, Accion::MenuCensuraPx);
        assert!(!chips[0].muestra_color && !chips[1].muestra_color);
    }

    #[test]
    fn el_desenfoque_etiqueta_los_px_como_radio() {
        let p = Propiedades {
            censura: CensorMode::Blur { radius: 12 },
            ..Propiedades::default()
        };
        let chips = chips(Herramienta::Pixelado, &p);
        assert_eq!(chips[0].etiqueta, "Modo: desenfoque");
        assert_eq!(chips[1].etiqueta, "Radio 12 px");
    }

    #[test]
    fn los_pasos_llevan_tamano_y_color() {
        let chips = chips(Herramienta::Pasos, &Propiedades::default());
        assert_eq!(chips.len(), 2);
        assert_eq!(chips[0].etiqueta, "Tamaño 20");
        assert_eq!(chips[0].accion, Accion::MenuTamano);
        assert!(chips[1].muestra_color);
    }
```

Y ampliar el `use` de ese `mod tests` con `use rustcapture_core::annotate::CensorMode;` si no está ya visible por el `use` de cabecera del archivo (lo está: se añade en el Step 4).

- [ ] **Step 2: Ejecutar los tests y comprobar que fallan**

Run: `cargo test -p platform-win`
Expected: FAIL de compilación — `no variant named 'Pasos'`, `cannot find function 'es_de_un_clic'`, `no variant named 'ToggleCensura'`.

- [ ] **Step 3: Ampliar `math.rs`**

Añadir las dos variantes al enum:

```rust
/// Herramientas de anotación vivas del editor (motor D5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Herramienta {
    Texto,
    Flecha,
    Linea,
    Rect,
    Elipse,
    Lapiz,
    Resaltador,
    Pasos,
    Pixelado,
}
```

Añadir sus ramas a las dos funciones de mapeo:

```rust
        ID_PASOS => Some(Herramienta::Pasos),
        ID_PIXELADO => Some(Herramienta::Pixelado),
```

```rust
        Herramienta::Pasos => ID_PASOS,
        Herramienta::Pixelado => ID_PIXELADO,
```

Añadir el predicado que usa el wndproc (una sola definición de la regla, en la parte pura y testeada):

```rust
/// `true` si la herramienta se coloca con un clic simple en vez de con
/// arrastre: el texto abre su caja y el paso numerado se estampa.
pub(crate) fn es_de_un_clic(h: Herramienta) -> bool {
    matches!(h, Herramienta::Texto | Herramienta::Pasos)
}
```

Y habilitar los dos botones en `toolbar()` (cambiar el `false` a `true` en esas dos líneas), actualizando el comentario de la función:

```rust
/// Toolbar del editor V4: las herramientas del motor D5 en vivo (la
/// activa se marca con el estado 'activo' del IconButton); leyenda, goma,
/// crop y resize esperan su fase.
```

```rust
        boton(ID_PASOS, AnnotateSteps, "Pasos numerados", true),
        boton(ID_LEYENDA, AnnotateCaption, "Leyenda", false),
        boton(ID_PIXELADO, AnnotatePixelate, "Pixelado", true),
```

- [ ] **Step 4: Ampliar `props.rs`**

Añadir las dos acciones:

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Accion {
    MenuGrosor,
    MenuTamano,
    ToggleNegrita,
    ElegirColor,
    ToggleCensura,
    MenuCensuraPx,
}
```

Ampliar los `use` de cabecera:

```rust
use rustcapture_core::annotate::{CensorMode, Color};
```

```rust
use super::estado::{CENSURAS, EditorState, GROSORES, Propiedades, TAMANOS};
```

Añadir el helper de etiquetas y las dos ramas nuevas de `chips` (antes del brazo `_ =>`):

```rust
/// Etiqueta del chip de modo y nombre del parámetro en px.
fn texto_censura(modo: CensorMode) -> (&'static str, &'static str) {
    match modo {
        CensorMode::Mosaic { .. } => ("Modo: mosaico", "Bloque"),
        CensorMode::Blur { .. } => ("Modo: desenfoque", "Radio"),
    }
}
```

```rust
        Herramienta::Pixelado => {
            let (modo, px) = texto_censura(p.censura);
            vec![
                Chip {
                    etiqueta: modo.to_string(),
                    muestra_color: false,
                    accion: Accion::ToggleCensura,
                },
                Chip {
                    etiqueta: format!("{px} {} px", p.censura_px()),
                    muestra_color: false,
                    accion: Accion::MenuCensuraPx,
                },
            ]
        }
        Herramienta::Pasos => vec![
            Chip {
                etiqueta: format!("Tamaño {}", p.tamano_texto as u32),
                muestra_color: false,
                accion: Accion::MenuTamano,
            },
            color,
        ],
```

Y las dos ramas nuevas en `on_click`:

```rust
        Accion::ToggleCensura => state.props.alternar_censura(),
        Accion::MenuCensuraPx => {
            let etiquetas: Vec<String> = CENSURAS.iter().map(|c| format!("{c} px")).collect();
            let actual = CENSURAS.iter().position(|&c| c == state.props.censura_px());
            if let Some(i) = menu_de_opciones(hwnd, p, &etiquetas, actual) {
                state.props.con_censura_px(CENSURAS[i]);
            }
        }
```

- [ ] **Step 5: Construir las anotaciones nuevas en `estado.rs`**

Ampliar el `use` de anotaciones del core:

```rust
use rustcapture_core::annotate::annotations::{
    Annotation, ArrowAnnotation, EllipseAnnotation, HighlightAnnotation, LineAnnotation,
    PenAnnotation, PixelateAnnotation, RectAnnotation,
};
```

En `anotacion_en_curso`, añadir la rama del pixelado y ampliar el `return None` para que cubra las dos herramientas de un clic:

```rust
            Herramienta::Pixelado => Box::new(PixelateAnnotation {
                rect,
                mode: self.props.censura,
            }),
            // Texto y Pasos se colocan con un clic, no con arrastre.
            Herramienta::Texto | Herramienta::Pasos => return None,
```

- [ ] **Step 6: Cablear el clic y el contador en `mod.rs`**

Ampliar los `use` del core:

```rust
use rustcapture_core::annotate::annotations::StepAnnotation;
```

Añadir la función que estampa el paso, junto a `deshacer`/`rehacer`:

```rust
/// Coloca un paso numerado en el punto del clic (f.23): sin arrastre, con
/// el número que toca según el contador sincronizado con la historia.
fn colocar_paso(hwnd: HWND, state: &mut EditorState, pf: (i32, i32)) {
    let anotacion = Box::new(StepAnnotation {
        center: pf,
        number: state.pasos.siguiente(),
        color: state.props.color,
        font_size: state.props.tamano_texto,
    });
    if state.history.apply(
        &mut state.doc,
        rustcapture_core::annotate::Command::add(anotacion),
    ) {
        state.pasos.aplicado(true);
        state.refresh_committed();
        state.dirty = true;
        // SAFETY: invalidación de la propia ventana.
        unsafe { _ = InvalidateRect(Some(hwnd), None, false) };
    }
}
```

Reemplazar el cuerpo del `if let Some(pf) = math::view_to_frame(...)` de `WM_LBUTTONDOWN` por el reparto entre las tres formas de colocar:

```rust
                    if let Some(pf) = math::view_to_frame(p, destino, tam) {
                        match state.herramienta {
                            math::Herramienta::Texto => {
                                texto::commit_text(hwnd, state);
                                if let Some(state) = state_mut(hwnd) {
                                    texto::abrir_edit(hwnd, state, pf, destino);
                                }
                            }
                            math::Herramienta::Pasos => colocar_paso(hwnd, state, pf),
                            _ => {
                                state.drag = Some(DragState {
                                    start: pf,
                                    current: pf,
                                    points: vec![pf],
                                });
                                SetCapture(hwnd);
                            }
                        }
                    }
```

En `WM_LBUTTONUP`, apuntar el comando en el contador cuando el `apply` tenga éxito:

```rust
                    if let Some(anotacion) = state.anotacion_en_curso()
                        && state.history.apply(
                            &mut state.doc,
                            rustcapture_core::annotate::Command::add(anotacion),
                        )
                    {
                        state.pasos.aplicado(false);
                        state.refresh_committed();
                        state.dirty = true;
                    }
```

En `deshacer` y `rehacer`, avisar al contador dentro del `if` que ya comprueba el éxito:

```rust
fn deshacer(hwnd: HWND, state: &mut EditorState) {
    texto::commit_text(hwnd, state);
    if state.history.undo(&mut state.doc) {
        state.pasos.deshecho();
        state.refresh_committed();
        state.dirty = true;
        // SAFETY: invalidación de la propia ventana.
        unsafe { _ = InvalidateRect(Some(hwnd), None, false) };
    }
}

fn rehacer(hwnd: HWND, state: &mut EditorState) {
    texto::commit_text(hwnd, state);
    if state.history.redo(&mut state.doc) {
        state.pasos.rehecho();
        state.refresh_committed();
        state.dirty = true;
        // SAFETY: invalidación de la propia ventana.
        unsafe { _ = InvalidateRect(Some(hwnd), None, false) };
    }
}
```

- [ ] **Step 7: Contabilizar también el texto**

En `crates/platform-win/src/editor/texto.rs`, `commit_text`: el `apply` del texto es un comando más y debe apuntarse en el contador, o deshacer un texto desplazaría la numeración de los pasos:

```rust
    if !texto.trim().is_empty() {
        let aplicado = state.history.apply(
            &mut state.doc,
            Command::add(Box::new(TextAnnotation {
                pos,
                text: texto,
                style: TextStyle {
                    color: state.props.color,
                    size: state.props.tamano_texto,
                    bold: state.props.negrita,
                },
            })),
        );
        if aplicado {
            state.pasos.aplicado(false);
            state.refresh_committed();
            state.dirty = true;
        }
    }
```

- [ ] **Step 8: Ejecutar todos los tests**

Run: `cargo test`
Expected: PASS en los cuatro crates.

- [ ] **Step 9: Comprobar clippy y el build de release**

Run: `cargo clippy --all-targets` y `cargo build --release`
Expected: sin warnings; el `gui.exe` de release sigue en el orden de ~1 MB (D13/S2).

---

### Task 9: Verificación manual, documentación y commit

Los tests no cubren lo que hay que mirar con los ojos: nitidez del disco, centrado del número, aspecto de la censura y el rendimiento del preview durante el arrastre.

**Files:**
- Modify: `roadmap.md`
- Modify: `ideas.md`
- Modify: `diseno-frontend.md`

- [ ] **Step 1: Ejecutar la GUI y probar el guion completo**

Run: `cargo run --release -p gui`

Guion de verificación manual (marcar cada punto):

1. `Ctrl+PrtScn` → seleccionar una región con texto legible → se abre el editor.
2. Botón **Pixelado**: la barra de propiedades muestra `Modo: mosaico` y `Bloque 8 px`. Arrastrar sobre el texto → durante el arrastre se ve el mosaico en vivo; al soltar queda fijado y el status bar marca «sin guardar».
3. Clic en `Bloque 8 px` → elegir `24 px` → arrastrar otra zona: celdas visiblemente mayores.
4. Clic en `Modo: mosaico` → pasa a `Modo: desenfoque` y el segundo chip a `Radio 24 px`. Arrastrar → zona borrosa, sin halo ni borde duro en el contorno del rect.
5. Arrastres grandes (media pantalla) → el arrastre sigue fluido, sin tirones perceptibles.
6. `Ctrl+Z` deshace la última censura; `Ctrl+Y` la rehace.
7. Botón **Pasos numerados**: la barra muestra `Tamaño 20` y `Color`. Clic en tres puntos → aparecen los discos 1, 2 y 3, con el borde suave (sin dientes de sierra) y el número centrado y blanco sobre el naranja.
8. `Ctrl+Z` borra el 3 → clic en otro sitio → vuelve a salir un **3** (no un 4).
9. Cambiar el color a amarillo claro → colocar otro paso → el número sale **negro** (contraste).
10. Cambiar `Tamaño` a 36 → el disco crece con el número. Colocar el paso 10 → el disco es algo mayor que el del 9 y el número no sobresale.
11. Combinar: pixelar una zona y colocar un paso ENCIMA → el paso se ve; colocar un paso y pixelar encima → el paso queda censurado (z-order).
12. **Guardar como** PNG → abrir el archivo: contiene exactamente lo que se veía. `Ctrl+Z` después de guardar sigue deshaciendo.
13. Cambiar el tema de Windows a claro/oscuro con el editor abierto → la toolbar y los chips siguen el tema; los dos botones nuevos se pintan con el resto.
14. Repetir 2 y 7 con la ventana en un monitor a 150 % de escala → iconos nítidos y clics en el sitio correcto.

Si algo falla, `systematic-debugging` (hipótesis → experimento mínimo) antes de tocar nada.

- [ ] **Step 2: Actualizar `roadmap.md`**

En §0, el estado de F3 pasa a nombrar lo que queda de verdad:

```markdown
🔵 **Fase actual: F3 — Editor y anotación (adelantada por decisión de producto).** El ciclo capturar → anotar in situ → guardar/copiar funciona en el editor V4 (F3.5 completada: rediseño visual con tema dual, iconos y fusión de la ventana de dibujo en el editor, D12+D13); quedan leyenda y goma/selección, crop/resize, el formato re-editable y el resto de salidas. F1 completada; F2 en pausa (picking de ventana/objeto, mano alzada, región fija, scroll y f.7/f.19).
```

Y en §4 el ítem 🔵 de herramientas:

```markdown
- 🔵 Herramientas (motor en core): texto, flechas, líneas, formas, resaltado, lápiz, pixelado/desenfoque y pasos numerados hechos e integrados en el editor; leyendas pendientes; goma = eliminar objeto (pendiente con la selección).
```

- [ ] **Step 3: Actualizar `ideas.md`**

En §1, marcar el estado de las dos características (el resto del texto se queda igual):

```markdown
23. Herramienta de pasos numerados (1, 2, 3…).
```

```markdown
25. Pixelado / desenfoque para censurar información.
```

Ninguna de las dos lleva marcador de estado ya: quedan completas para la versión 1, así que se retiran los `(parcial)` que pudieran tener. Comprobar con `grep -n "pixelado\|pasos numerados" ideas.md` que no queda ningún marcador obsoleto.

- [ ] **Step 4: Actualizar `diseno-frontend.md`**

En §3, la línea de la toolbar de V4:

```markdown
- Toolbar superior: selección `(D)`, texto, flecha, línea, forma, elipse, lápiz, resaltador, pasos numerados, pixelado `(hechas)`, leyenda, goma `(D)` | recorte, redimensionar `(D)` | deshacer/rehacer `(hechos)`.
```

Y en la línea de la barra de propiedades, añadir los chips nuevos:

```markdown
- Barra de propiedades contextual bajo la toolbar (grosor, color, tamaño de fuente, negrita, modo de censura y bloque/radio) `(hecha, con chips + menú popup)`.
```

- [ ] **Step 5: Comprobar que `arquitectura.md` no necesita cambios**

El slice no introduce ninguna decisión nueva: sigue D5 (Strategy por tipo), D6 (Command) y D12 (anotación dentro del editor). El único matiz que merece una línea es que el `Canvas` ahora también lee. Añadir a D5, al final del párrafo «Hacemos»:

```markdown
El `Canvas` expone lectura además de escritura: la censura (f.25) necesita ver lo que hay debajo, de modo que pixelar tapa también las anotaciones anteriores del z-order, no solo la base.
```

- [ ] **Step 6: Pasar `verification-before-completion`**

Run: `cargo test` · `cargo clippy --all-targets` · `cargo build --release`
Y confirmar que los 14 puntos del guion manual quedaron marcados. Sin las tres salidas verdes y el guion completo no se propone commit.

- [ ] **Step 7: Proponer el commit al humano**

`skills.md` prohíbe el commit automático fuera del cierre de fase. Presentar al humano el mensaje propuesto y esperar su aprobación:

```
v0.2.16 — F3/S-D: pixelado y pasos numerados

Pixelado/desenfoque (f.25) y pasos numerados (f.23) como Strategies del
motor D5, con undo/redo de D6 y cableados en el editor V4.

- core: Canvas gana lector de píxeles (la censura ve el z-order);
  censor.rs con mosaico y desenfoque de caja separable; disco relleno
  con AA y caja de tinta del texto para centrar el número.
- editor: herramientas Pixelado (arrastre) y Pasos (clic simple), chips
  de modo/bloque-radio, propiedades agrupadas en Propiedades y contador
  de pasos sincronizado con las pilas de undo/redo.
- docs: roadmap, ideas y diseño-frontend al día.
```

---

## Autorrevisión

**Cobertura del alcance acordado:**

| Requisito | Tarea |
|---|---|
| Mosaico (f.25) | 2, 3 |
| Desenfoque (f.25) | 2, 3 |
| Chip conmutador de modo + px | 6, 8 |
| Pasos numerados (f.23) | 4, 5 |
| Numeración correcta con undo/redo | 7, 8 |
| Botones `ID_PIXELADO`/`ID_PASOS` habilitados | 8 |
| Documentación al día | 9 |

**Riesgos anotados donde toca:**
- El coste del desenfoque en el preview del arrastre está acotado por diseño (separable con sumas prefijas, independiente del radio) y se verifica a mano en el punto 5 del guion.
- `text_ink_box` duplica la aritmética de colocación de `draw_text`; el test `la_caja_de_tinta_encierra_exactamente_los_pixeles_pintados` es lo que impide que divergan, y el doc comment lo dice explícitamente.
- El contador de pasos solo es correcto si sus tres métodos se llaman en los MISMOS cuatro sitios que `History` (alta por arrastre, alta de texto, alta de paso, undo/redo). Está anotado en su doc comment y cubierto por los tests de la Task 7.
