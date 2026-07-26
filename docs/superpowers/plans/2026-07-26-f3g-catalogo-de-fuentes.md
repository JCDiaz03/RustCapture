# F3 Slice G — Catálogo de fuentes: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** poder elegir la tipografía del texto entre las fuentes del sistema y las que el usuario deje en una carpeta `fonts/` junto al ejecutable, y cambiar fuente, tamaño, negrita y color mientras la caja de edición está abierta.

**Architecture:** el core sigue sin abrir archivos (D1/D2): `RenderContext` pasa de tener dos fuentes fijas a ser un **catálogo** de caras tipográficas indexadas por `(FamiliaId, negrita)`, al que `platform-win` inyecta bytes TTF. `TextStyle` gana un `FamiliaId` — un `u16`, no un `String`, para que siga siendo `Copy` y no se propague un rediseño por todo el motor. `platform-win` gana el módulo `fuentes_ttf`, que construye el catálogo de familias disponibles a partir de tres orígenes con prioridad: la carpeta `fonts/` junto al exe, las fuentes por usuario y las del sistema (ambas vía registro, que mapea nombre → archivo). La caja de edición pasa a reflejar SIEMPRE las propiedades vigentes de la barra: cambiar un chip con la caja abierta la actualiza en vivo, y reeditar un texto carga su estilo en la barra.

**Tech Stack:** Rust 2024; `rustcapture-core` con `fontdue` (sin dependencias de plataforma); `platform-win` con `windows` 0.62 (`Win32_System_Registry` ya está en las features).

## Global Constraints

- **El core no abre archivos ni toca el registro.** Las fuentes entran como `&[u8]`. Lo dice la cabecera de `text.rs` y sigue siendo cierto.
- **`TextStyle` debe seguir siendo `Copy`.** Va por valor en `draw_text`, `text_ink_box`, `draw_text_rotado` y en `StepAnnotation`; convertirlo en no-`Copy` obligaría a tocar todo eso sin necesidad.
- **Cero dependencias nuevas.** El parseo de nombres del registro y el escaneo de la carpeta se hacen a mano.
- **Degradar, nunca fallar:** si una familia o su negrita no está cargada, se cae a la variante disponible y en último término a la familia por defecto. Un texto nunca deja de pintarse por una fuente ausente.
- **TDD en `core`**; en `platform-win`, tests para la lógica pura (parseo de nombres del registro, agrupación en familias) y verificación manual del resto.
- **Commit único al final**, propuesto al humano (`skills.md`).
- Comandos: `cargo test -p rustcapture-core`, `cargo test -p platform-win`, `cargo clippy --all-targets`.

---

## Estructura de archivos

| Archivo | Responsabilidad | Acción |
|---|---|---|
| `crates/core/src/annotate/style.rs` | `FamiliaId` + `TextStyle.familia` | modificar |
| `crates/core/src/annotate/text.rs` | `RenderContext` como catálogo con cadena de respaldo | modificar |
| `crates/platform-win/src/fuentes_ttf/mod.rs` | catálogo de familias: carpeta portable + registro | **crear** |
| `crates/platform-win/src/fuentes_ttf/nombres.rs` | parseo puro de los nombres del registro → familia + negrita | **crear** |
| `crates/platform-win/src/editor/estado.rs` | `Propiedades.familia`, carga del catálogo en el `RenderContext` | modificar |
| `crates/platform-win/src/editor/props.rs` | chip «Fuente» + su menú | modificar |
| `crates/platform-win/src/editor/texto.rs` | la caja refleja la barra; reeditar carga el estilo del objeto | modificar |
| `crates/platform-win/src/editor/mod.rs` | `WM_CTLCOLOREDIT` para el color del texto en la caja | modificar |
| `crates/core/src/config/mod.rs` | `[text] familia` | modificar |
| `ideas.md`, `roadmap.md`, `arquitectura.md`, `diseno-frontend.md` | f.54, D5, estado, chip | modificar |

---

### Task 1: `FamiliaId` y el catálogo del `RenderContext`

**Files:**
- Modify: `crates/core/src/annotate/style.rs`
- Modify: `crates/core/src/annotate/text.rs`

**Interfaces:**
- Produces:
  - `pub struct FamiliaId(pub u16)` con `Default` = 0 (familia por defecto)
  - `TextStyle { color, size, bold, familia: FamiliaId }` (sigue `Copy`)
  - `RenderContext::nueva()`, `registrar_familia(&mut self, nombre: &str) -> FamiliaId`,
    `cargar_cara(&mut self, id: FamiliaId, bold: bool, ttf: &[u8]) -> Result<(), String>`,
    `nombre(&self, id: FamiliaId) -> Option<&str>`, `familias(&self) -> Vec<(FamiliaId, &str)>`,
    `tiene_alguna(&self) -> bool`
  - `RenderContext::sin_fuente()` se conserva (contexto vacío)

- [ ] **Step 1: Escribir los tests que fallan**

En `text.rs`, dentro de `mod tests`:

```rust
    fn ttf_normal() -> Vec<u8> {
        std::fs::read("C:/Windows/Fonts/segoeui.ttf").expect("fuente del sistema")
    }

    fn ttf_bold() -> Vec<u8> {
        std::fs::read("C:/Windows/Fonts/segoeuib.ttf").expect("fuente del sistema")
    }

    #[test]
    fn el_catalogo_registra_familias_y_devuelve_sus_nombres() {
        let mut ctx = RenderContext::nueva();
        let a = ctx.registrar_familia("Segoe UI");
        let b = ctx.registrar_familia("Consolas");
        assert_ne!(a, b);
        assert_eq!(ctx.nombre(a), Some("Segoe UI"));
        assert_eq!(ctx.nombre(b), Some("Consolas"));
        // Registrar la misma familia dos veces devuelve el mismo id.
        assert_eq!(ctx.registrar_familia("Segoe UI"), a);
        assert_eq!(ctx.familias().len(), 2);
        assert_eq!(ctx.nombre(FamiliaId(77)), None);
    }

    #[test]
    fn una_cara_no_cargada_cae_a_la_que_haya() {
        let mut ctx = RenderContext::nueva();
        let id = ctx.registrar_familia("Segoe UI");
        // Solo la normal cargada: pedir negrita debe caer a la normal.
        ctx.cargar_cara(id, false, &ttf_normal()).unwrap();
        let normal = TextStyle { color: Color::rgb(0, 0, 0), size: 20.0, bold: false, familia: id };
        let negrita = TextStyle { bold: true, ..normal };
        assert!(ctx.font(normal).is_some());
        assert!(ctx.font(negrita).is_some(), "la negrita no cayó a la normal");
    }

    #[test]
    fn una_familia_ausente_cae_a_la_familia_por_defecto() {
        let mut ctx = RenderContext::nueva();
        let defecto = ctx.registrar_familia("Segoe UI");
        assert_eq!(defecto, FamiliaId(0), "la primera registrada es la de respaldo");
        ctx.cargar_cara(defecto, false, &ttf_normal()).unwrap();
        // Familia registrada pero SIN caras cargadas.
        let vacia = ctx.registrar_familia("Fuente Que No Existe");
        let style = TextStyle {
            color: Color::rgb(0, 0, 0),
            size: 20.0,
            bold: false,
            familia: vacia,
        };
        assert!(
            ctx.font(style).is_some(),
            "una familia sin caras debe caer a la por defecto"
        );
    }

    #[test]
    fn un_contexto_vacio_no_tiene_fuentes() {
        let ctx = RenderContext::sin_fuente();
        assert!(!ctx.tiene_alguna());
        let style = TextStyle {
            color: Color::rgb(0, 0, 0),
            size: 20.0,
            bold: false,
            familia: FamiliaId::default(),
        };
        assert!(ctx.font(style).is_none());
    }

    #[test]
    fn un_ttf_invalido_da_error_y_no_ensucia_el_catalogo() {
        let mut ctx = RenderContext::nueva();
        let id = ctx.registrar_familia("Basura");
        assert!(ctx.cargar_cara(id, false, b"no soy un ttf").is_err());
        let style = TextStyle {
            color: Color::rgb(0, 0, 0),
            size: 20.0,
            bold: false,
            familia: id,
        };
        assert!(ctx.font(style).is_none());
    }

    #[test]
    fn dos_familias_distintas_rasterizan_distinto() {
        let mut ctx = RenderContext::nueva();
        let sans = ctx.registrar_familia("Segoe UI");
        ctx.cargar_cara(sans, false, &ttf_normal()).unwrap();
        let mono = ctx.registrar_familia("Consolas");
        ctx.cargar_cara(
            mono,
            false,
            &std::fs::read("C:/Windows/Fonts/consola.ttf").expect("Consolas"),
        )
        .unwrap();
        let base = TextStyle { color: Color::rgb(255, 0, 0), size: 24.0, bold: false, familia: sans };
        let caja_sans = text_ink_box("Hola", base, &ctx).unwrap();
        let caja_mono = text_ink_box("Hola", TextStyle { familia: mono, ..base }, &ctx).unwrap();
        assert_ne!(caja_sans, caja_mono, "las dos familias miden igual");
    }
```

- [ ] **Step 2: Ejecutar y comprobar que fallan**

Run: `cargo test -p rustcapture-core annotate::text`
Expected: FAIL de compilación — `RenderContext::nueva`, `FamiliaId` y `TextStyle.familia` no existen.

- [ ] **Step 3: Añadir `FamiliaId` y el campo a `TextStyle`**

En `style.rs`:

```rust
/// Familia tipográfica, como índice en el catálogo del `RenderContext`.
///
/// Es un `u16` y no un `String` a propósito: `TextStyle` viaja por valor por
/// todo el motor y tiene que seguir siendo `Copy`. El nombre vive en el
/// catálogo, que es también quien lo resolverá al serializar (f.31).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct FamiliaId(pub u16);

/// Estilo del texto (f.22).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TextStyle {
    pub color: Color,
    /// Altura de la fuente en píxeles.
    pub size: f32,
    pub bold: bool,
    /// Familia tipográfica (f.54); `FamiliaId::default()` = la de respaldo.
    pub familia: FamiliaId,
}
```

Reexportar en `annotate/mod.rs`: `pub use style::{CensorMode, Color, FamiliaId, Style, TextStyle};`

- [ ] **Step 4: Convertir `RenderContext` en catálogo**

En `text.rs`, sustituir el struct y su `impl`:

```rust
/// Catálogo de caras tipográficas. Las fuentes llegan inyectadas como bytes:
/// el core nunca abre archivos ni consulta el registro (D1/D2) — eso es
/// trabajo de `platform-win::fuentes_ttf`.
pub struct RenderContext {
    /// Nombres por id; el índice ES el `FamiliaId`.
    nombres: Vec<String>,
    /// Caras cargadas por (familia, negrita).
    caras: std::collections::HashMap<(u16, bool), fontdue::Font>,
}

impl RenderContext {
    pub fn nueva() -> Self {
        Self {
            nombres: Vec::new(),
            caras: std::collections::HashMap::new(),
        }
    }

    /// Contexto sin tipografía: todo salvo el texto funciona igual.
    pub fn sin_fuente() -> Self {
        Self::nueva()
    }

    /// Registra una familia (o devuelve su id si ya estaba). La PRIMERA que
    /// se registra es la de respaldo: a ella caen las que no tengan caras.
    pub fn registrar_familia(&mut self, nombre: &str) -> FamiliaId {
        if let Some(i) = self.nombres.iter().position(|n| n == nombre) {
            return FamiliaId(i as u16);
        }
        self.nombres.push(nombre.to_string());
        FamiliaId((self.nombres.len() - 1) as u16)
    }

    /// Carga los bytes de una cara. Error si el TTF no se puede parsear; el
    /// catálogo queda intacto en ese caso.
    pub fn cargar_cara(&mut self, id: FamiliaId, bold: bool, ttf: &[u8]) -> Result<(), String> {
        let font = fontdue::Font::from_bytes(ttf, fontdue::FontSettings::default())
            .map_err(String::from)?;
        self.caras.insert((id.0, bold), font);
        Ok(())
    }

    pub fn nombre(&self, id: FamiliaId) -> Option<&str> {
        self.nombres.get(id.0 as usize).map(String::as_str)
    }

    pub fn familias(&self) -> Vec<(FamiliaId, &str)> {
        self.nombres
            .iter()
            .enumerate()
            .map(|(i, n)| (FamiliaId(i as u16), n.as_str()))
            .collect()
    }

    pub fn tiene_alguna(&self) -> bool {
        !self.caras.is_empty()
    }

    /// Cara para un estilo, con cadena de respaldo: la pedida → la misma
    /// familia sin negrita → la familia de respaldo con negrita → la de
    /// respaldo normal. Así una fuente ausente degrada en vez de dejar el
    /// texto sin pintar.
    pub(crate) fn font(&self, style: TextStyle) -> Option<&fontdue::Font> {
        let f = style.familia.0;
        self.caras
            .get(&(f, style.bold))
            .or_else(|| self.caras.get(&(f, false)))
            .or_else(|| self.caras.get(&(0, style.bold)))
            .or_else(|| self.caras.get(&(0, false)))
    }
}
```

- [ ] **Step 5: Adaptar los llamadores de `ctx.font`**

`recorrer_glifos` pasa de `ctx.font(style.bold)` a `ctx.font(style)`. Es el único consumidor (por el refactor del slice anterior), así que no hay más sitios.

- [ ] **Step 6: Actualizar las construcciones de `TextStyle`**

El compilador señalará cada `TextStyle { .. }` sin `familia`. En `core` son solo tests y `StepAnnotation::estilo_numero`; en `platform-win`, `editor/texto.rs` y `editor/estado.rs`. En todos, añadir `familia: <la que corresponda>` (en los tests, `FamiliaId::default()`).

- [ ] **Step 7: Ejecutar**

Run: `cargo test -p rustcapture-core`
Expected: PASS, con los 6 tests nuevos.

---

### Task 2: Parseo de los nombres del registro (lógica pura)

El registro da nombres como `"Segoe UI Bold (TrueType)"` → familia `Segoe UI`, negrita. Es la pieza con más casos raros y se prueba aislada, sin tocar el registro.

**Files:**
- Create: `crates/platform-win/src/fuentes_ttf/nombres.rs`

**Interfaces:**
- Produces: `pub(crate) fn analizar(valor: &str) -> Option<(String, bool)>` — `(familia, negrita)`, `None` si hay que descartarla.

- [ ] **Step 1: Escribir el módulo con sus tests**

```rust
//! Parseo de los nombres de fuente del registro de Windows. Aislado y puro
//! porque es donde están todos los casos raros.

/// Sufijos de tipo que Windows añade al nombre visible.
const TIPOS: [&str; 3] = [" (TrueType)", " (OpenType)", " (VarType)"];

/// Familia y negrita de un nombre del registro. `None` para lo que no
/// sabemos rasterizar o no queremos ofrecer:
/// - cursivas (aún no hay soporte de itálica en `TextStyle`)
/// - pesos que no son ni normal ni negrita (Light, Semibold, Black…), que
///   se ofrecerían como familias falsas
/// - fuentes sin sufijo de tipo (`.fon` bitmap antiguas)
pub(crate) fn analizar(valor: &str) -> Option<(String, bool)> {
    let sin_tipo = TIPOS.iter().find_map(|t| valor.strip_suffix(t))?;
    // Varias familias en un mismo valor van separadas por " & ".
    let primera = sin_tipo.split(" & ").next()?.trim();
    if primera.is_empty() {
        return None;
    }
    if primera.to_ascii_lowercase().contains("italic")
        || primera.to_ascii_lowercase().contains("oblique")
    {
        return None;
    }
    // Pesos intermedios: fuera, no son la familia base ni su negrita.
    const PESOS: [&str; 6] = [" Light", " Semilight", " Semibold", " Black", " Thin", " Medium"];
    if PESOS.iter().any(|p| primera.ends_with(p)) {
        return None;
    }
    match primera.strip_suffix(" Bold") {
        Some(familia) if !familia.is_empty() => Some((familia.trim().to_string(), true)),
        _ => Some((primera.to_string(), false)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separa_la_familia_de_la_negrita() {
        assert_eq!(
            analizar("Segoe UI (TrueType)"),
            Some(("Segoe UI".to_string(), false))
        );
        assert_eq!(
            analizar("Segoe UI Bold (TrueType)"),
            Some(("Segoe UI".to_string(), true))
        );
        assert_eq!(
            analizar("Consolas (TrueType)"),
            Some(("Consolas".to_string(), false))
        );
    }

    #[test]
    fn descarta_cursivas_y_pesos_intermedios() {
        assert_eq!(analizar("Segoe UI Italic (TrueType)"), None);
        assert_eq!(analizar("Segoe UI Bold Italic (TrueType)"), None);
        assert_eq!(analizar("Segoe UI Semibold (TrueType)"), None);
        assert_eq!(analizar("Segoe UI Black (TrueType)"), None);
        assert_eq!(analizar("Segoe UI Light (TrueType)"), None);
    }

    #[test]
    fn descarta_lo_que_no_lleva_sufijo_de_tipo() {
        // Bitmap antiguas (.fon) y basura.
        assert_eq!(analizar("MS Sans Serif 8,10,12,14,18,24"), None);
        assert_eq!(analizar(""), None);
    }

    #[test]
    fn toma_la_primera_de_un_valor_con_varias_familias() {
        assert_eq!(
            analizar("Cambria & Cambria Math (TrueType)"),
            Some(("Cambria".to_string(), false))
        );
    }

    #[test]
    fn una_familia_que_se_llama_bold_no_se_queda_vacia() {
        // " Bold" a secas no debe producir una familia vacía.
        assert_eq!(analizar("Bold (TrueType)"), Some(("Bold".to_string(), false)));
    }
}
```

- [ ] **Step 2: Ejecutar**

Run: `cargo test -p platform-win fuentes_ttf::nombres`
Expected: los 5 tests en verde (el módulo hay que declararlo en la Task 3; hasta entonces `cargo test` no lo compila — hacer las dos tareas seguidas).

---

### Task 3: Catálogo de familias disponibles

**Files:**
- Create: `crates/platform-win/src/fuentes_ttf/mod.rs`
- Modify: `crates/platform-win/src/lib.rs` (declarar `mod fuentes_ttf;`)

**Interfaces:**
- Consumes: `nombres::analizar` (Task 2).
- Produces:
  - `pub(crate) struct Familia { pub nombre: String, pub normal: PathBuf, pub bold: Option<PathBuf> }`
  - `pub(crate) fn catalogo() -> Vec<Familia>` — ordenado por nombre, sin duplicados, la carpeta portable con prioridad.
  - `pub(crate) fn carpeta_portable() -> PathBuf` — `fonts/` junto al exe.

- [ ] **Step 1: Implementar**

```rust
//! Catálogo de fuentes disponibles para el editor (f.54). Tres orígenes, en
//! orden de prioridad:
//!
//! 1. `fonts/` junto al ejecutable — lo que el usuario deja ahí manda, y no
//!    exige instalar nada en Windows (portable-first, D9).
//! 2. Fuentes por usuario (`HKCU`), instaladas sin permisos de admin.
//! 3. Fuentes del sistema (`HKLM`), que son la mayoría.
//!
//! El registro mapea nombre visible → archivo; los relativos cuelgan de
//! `C:\Windows\Fonts`. Solo se aceptan `.ttf`/`.otf`: las colecciones `.ttc`
//! necesitan índice de cara y `fontdue` no lo expone, así que se descartan
//! (documentado, no olvidado).

mod nombres;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_SZ, RegCloseKey,
    RegEnumValueW, RegGetValueW, RegOpenKeyExW,
};
use windows::core::w;

/// Una familia con sus dos caras (la negrita puede faltar: se sintetiza
/// cayendo a la normal, ver `RenderContext::font`).
pub(crate) struct Familia {
    pub nombre: String,
    pub normal: PathBuf,
    pub bold: Option<PathBuf>,
}

const CLAVE: windows::core::PCWSTR = w!(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts");

/// Extensiones que `fontdue` sabe parsear.
fn extension_valida(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref(),
        Some("ttf") | Some("otf")
    )
}

/// `fonts/` junto al ejecutable.
pub(crate) fn carpeta_portable() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .unwrap_or_default()
        .join("fonts")
}

/// Catálogo completo, ordenado por nombre. Una familia solo entra si tiene
/// al menos la cara normal.
pub(crate) fn catalogo() -> Vec<Familia> {
    // (nombre, negrita) -> ruta. BTreeMap para salida ordenada y estable.
    let mut caras: BTreeMap<(String, bool), PathBuf> = BTreeMap::new();
    // De menor a mayor prioridad: lo último sobrescribe.
    leer_registro(HKEY_LOCAL_MACHINE, &mut caras);
    leer_registro(HKEY_CURRENT_USER, &mut caras);
    leer_carpeta(&carpeta_portable(), &mut caras);

    let mut familias: BTreeMap<String, Familia> = BTreeMap::new();
    for ((nombre, bold), ruta) in caras {
        let entrada = familias.entry(nombre.clone()).or_insert_with(|| Familia {
            nombre: nombre.clone(),
            normal: ruta.clone(),
            bold: None,
        });
        if bold {
            entrada.bold = Some(ruta);
        } else {
            entrada.normal = ruta;
        }
    }
    familias.into_values().collect()
}

/// Añade al mapa las fuentes de una raíz del registro.
fn leer_registro(raiz: HKEY, caras: &mut BTreeMap<(String, bool), PathBuf>) {
    let mut clave = HKEY::default();
    // SAFETY: apertura de una clave de solo lectura; se cierra al salir.
    unsafe {
        if RegOpenKeyExW(raiz, CLAVE, Some(0), KEY_READ, &mut clave).is_err() {
            return;
        }
    }
    let dir_sistema = PathBuf::from(
        std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string()),
    )
    .join("Fonts");
    let mut i = 0u32;
    loop {
        let mut nombre = [0u16; 256];
        let mut largo_nombre = nombre.len() as u32;
        // SAFETY: buffers propios con su longitud; la enumeración termina
        // cuando RegEnumValueW deja de devolver Ok.
        let ok = unsafe {
            RegEnumValueW(
                clave,
                i,
                Some(nombre.as_mut_ptr()),
                &mut largo_nombre,
                None,
                None,
                None,
                None,
            )
            .is_ok()
        };
        if !ok {
            break;
        }
        i += 1;
        let visible = String::from_utf16_lossy(&nombre[..largo_nombre as usize]);
        let Some((familia, bold)) = nombres::analizar(&visible) else {
            continue;
        };
        let Some(archivo) = valor_texto(clave, &nombre[..largo_nombre as usize]) else {
            continue;
        };
        let ruta = PathBuf::from(&archivo);
        let ruta = if ruta.is_absolute() {
            ruta
        } else {
            dir_sistema.join(&archivo)
        };
        if extension_valida(&ruta) && ruta.exists() {
            caras.insert((familia, bold), ruta);
        }
    }
    // SAFETY: la clave la abrió esta función.
    unsafe { _ = RegCloseKey(clave) };
}

/// Lee un valor REG_SZ ya sabiendo su nombre en UTF-16.
fn valor_texto(clave: HKEY, nombre_utf16: &[u16]) -> Option<String> {
    let mut buffer = [0u16; 512];
    let mut bytes = (buffer.len() * 2) as u32;
    // SAFETY: nombre y buffer son propios; `bytes` entra con la capacidad y
    // sale con el tamaño escrito.
    let ok = unsafe {
        RegGetValueW(
            clave,
            None,
            windows::core::PCWSTR(nombre_utf16.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
        .is_ok()
    };
    if !ok {
        return None;
    }
    let chars = (bytes as usize / 2).min(buffer.len());
    let fin = buffer[..chars].iter().position(|&c| c == 0).unwrap_or(chars);
    Some(String::from_utf16_lossy(&buffer[..fin]))
}

/// Añade los `.ttf`/`.otf` de una carpeta. El nombre de familia sale del
/// nombre de archivo: parsear la tabla `name` del TTF exigiría cargar cada
/// archivo, y aquí basta con algo predecible para el usuario.
fn leer_carpeta(dir: &Path, caras: &mut BTreeMap<(String, bool), PathBuf>) {
    let Ok(entradas) = std::fs::read_dir(dir) else {
        return; // no existe: caso normal
    };
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        if !extension_valida(&ruta) {
            continue;
        }
        let Some(tallo) = ruta.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        // "MiFuente-Bold.ttf" / "MiFuente Bold.ttf" → familia + negrita.
        let limpio = tallo.replace(['-', '_'], " ");
        let (familia, bold) = match limpio.strip_suffix(" Bold") {
            Some(f) if !f.trim().is_empty() => (f.trim().to_string(), true),
            _ => (limpio.trim().to_string(), false),
        };
        if !familia.is_empty() {
            caras.insert((familia, bold), ruta);
        }
    }
}
```

- [ ] **Step 2: Declarar el módulo**

En `crates/platform-win/src/lib.rs`, junto a los demás: `mod fuentes_ttf;`

- [ ] **Step 3: Test de humo del catálogo real**

En `fuentes_ttf/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// En cualquier Windows hay fuentes; y todas las del catálogo tienen que
    /// existir en disco y ser parseables por fontdue (si no, el chip
    /// ofrecería familias que luego no pintan).
    #[test]
    fn el_catalogo_del_sistema_no_esta_vacio_y_es_usable() {
        let familias = catalogo();
        assert!(familias.len() > 10, "solo {} familias", familias.len());
        assert!(
            familias.iter().any(|f| f.nombre == "Segoe UI"),
            "falta Segoe UI"
        );
        // Muestra: las 5 primeras deben cargar de verdad.
        for f in familias.iter().take(5) {
            let bytes = std::fs::read(&f.normal)
                .unwrap_or_else(|e| panic!("{}: {e}", f.normal.display()));
            assert!(
                fontdue::Font::from_bytes(&bytes[..], fontdue::FontSettings::default()).is_ok(),
                "{} no es parseable",
                f.nombre
            );
        }
    }

    #[test]
    fn la_carpeta_portable_cuelga_del_ejecutable() {
        let dir = carpeta_portable();
        assert_eq!(dir.file_name().and_then(|s| s.to_str()), Some("fonts"));
    }
}
```

Nota: el test necesita `fontdue` como dependencia de dev de `platform-win`. Añadir a su `Cargo.toml`:

```toml
[dev-dependencies]
fontdue = { workspace = true }
```

- [ ] **Step 4: Ejecutar**

Run: `cargo test -p platform-win fuentes_ttf`
Expected: PASS. Si `el_catalogo_del_sistema_no_esta_vacio_y_es_usable` falla por una fuente concreta, el parseo de nombres o el filtro de extensiones está dejando pasar algo que no debería.

---

### Task 4: El editor usa el catálogo

**Files:**
- Modify: `crates/platform-win/src/editor/estado.rs`
- Modify: `crates/core/src/config/mod.rs`

**Interfaces:**
- Produces: `Propiedades.familia: FamiliaId`; `EditorState.familias: Vec<(FamiliaId, String)>` (para el menú del chip); `[text] familia` en config.

- [ ] **Step 1: Cargar el catálogo al construir el estado**

Sustituir `cargar_contexto` por una versión que registre TODAS las familias del catálogo pero cargue los bytes solo de la de por defecto (registrar es barato; cargar 400 TTF no lo es):

```rust
/// Construye el `RenderContext` con el catálogo del sistema: registra todas
/// las familias (barato, solo nombres) y carga las CARAS únicamente de la
/// familia por defecto. El resto se cargan bajo demanda al elegirlas, para
/// no parsear cientos de TTF al abrir el editor.
fn cargar_contexto(familia_defecto: &str) -> (RenderContext, Vec<(FamiliaId, String)>, bool) {
    let mut ctx = RenderContext::nueva();
    let catalogo = crate::fuentes_ttf::catalogo();
    // La primera registrada es la de respaldo: que sea la pedida si existe.
    let preferida = catalogo
        .iter()
        .find(|f| f.nombre == familia_defecto)
        .or_else(|| catalogo.iter().find(|f| f.nombre == "Segoe UI"))
        .or_else(|| catalogo.first());
    let mut disponibles = Vec::with_capacity(catalogo.len());
    let mut cargada = false;
    if let Some(pref) = preferida {
        let id = ctx.registrar_familia(&pref.nombre);
        cargada = cargar_familia(&mut ctx, id, pref);
    }
    for f in &catalogo {
        let id = ctx.registrar_familia(&f.nombre);
        disponibles.push((id, f.nombre.clone()));
    }
    (ctx, disponibles, cargada)
}

/// Lee del disco las caras de una familia y las mete en el catálogo.
/// Devuelve true si al menos la normal cargó.
pub(super) fn cargar_familia(
    ctx: &mut RenderContext,
    id: FamiliaId,
    familia: &crate::fuentes_ttf::Familia,
) -> bool {
    let normal = std::fs::read(&familia.normal)
        .ok()
        .is_some_and(|b| ctx.cargar_cara(id, false, &b).is_ok());
    if let Some(ruta) = &familia.bold
        && let Ok(b) = std::fs::read(ruta)
    {
        _ = ctx.cargar_cara(id, true, &b);
    }
    normal
}
```

`EditorState` gana `pub familias: Vec<(FamiliaId, String)>` y `Propiedades` gana `pub familia: FamiliaId` (por defecto `FamiliaId::default()`, que es la preferida por ser la primera registrada).

`EditorState::new` pasa a recibir la familia por defecto de la config; si no, `"Segoe UI"`.

- [ ] **Step 2: Sección `[text]` en la config**

En `core/src/config/mod.rs`, añadir al struct de config una sección con `familia: String` (default `"Segoe UI"`), siguiendo el patrón de `[theme]` que ya existe, con su test de round-trip TOML y de default.

- [ ] **Step 3: Ejecutar**

Run: `cargo test`
Expected: PASS.

---

### Task 5: Chip «Fuente» y edición en vivo de la caja

**Files:**
- Modify: `crates/platform-win/src/editor/props.rs`
- Modify: `crates/platform-win/src/editor/texto.rs`
- Modify: `crates/platform-win/src/editor/mod.rs`

**Interfaces:**
- Produces: `Accion::MenuFuente`; la caja de edición refleja `state.props` en vivo.

- [ ] **Step 1: Tests del chip**

En `props.rs`, `mod tests`:

```rust
    #[test]
    fn el_texto_lleva_fuente_tamano_negrita_y_color() {
        let p = Propiedades::default();
        let chips = chips(Herramienta::Texto, &p);
        assert_eq!(chips.len(), 4);
        assert_eq!(chips[0].accion, Accion::MenuFuente);
        assert_eq!(chips[1].accion, Accion::MenuTamano);
        assert_eq!(chips[2].accion, Accion::ToggleNegrita);
        assert_eq!(chips[3].accion, Accion::ElegirColor);
        assert!(chips[3].muestra_color);
    }
```

El chip de fuente muestra el nombre, que sale del catálogo; `chips` recibe además `&str` con el nombre vigente para poder etiquetarlo:
`chips(herramienta: Herramienta, p: &Propiedades, fuente: &str) -> Vec<Chip>`, y el test pasa `"Segoe UI"` comprobando `chips[0].etiqueta == "Segoe UI"`.

- [ ] **Step 2: Implementar el chip y su menú**

`Accion::MenuFuente` abre `menu_de_opciones` con los nombres de `state.familias`. Al elegir: `state.props.familia = id`, y **cargar sus caras si aún no están** (`cargar_familia`), porque el catálogo solo trae los nombres registrados. Si la carga falla, avisar con `alerts::error_beep` y no cambiar la familia.

- [ ] **Step 3: La caja de edición refleja la barra**

En `texto.rs`:
- `EditBox` pierde el campo `estilo`: el estilo de confirmación se lee de `state.props` al confirmar, porque ahora los chips mandan sobre la caja abierta.
- `abrir_reedicion` copia el estilo del objeto a `state.props` antes de crear la caja (así los chips muestran lo que estás editando y no se pierde su estilo).
- Nueva `pub(super) fn refrescar_fuente(hwnd: HWND, state: &mut EditorState)`: si hay caja abierta, destruye su `HFONT`, crea otro con `state.props` (familia real vía `ctx.nombre(props.familia)`, tamaño y negrita), lo asigna con `WM_SETFONT` e invalida el EDIT. La llama `props::on_click` tras cambiar cualquier chip.

- [ ] **Step 4: Color del texto en la caja**

En el wndproc, atender `WM_CTLCOLOREDIT`: si el control es el EDIT del texto, `SetTextColor` con `state.props.color`, `SetBkColor` con el fondo del tema y devolver una brocha del tema (guardada en el estado para no filtrarla). Sin esto el color del chip no se vería hasta confirmar.

- [ ] **Step 5: Ejecutar**

Run: `cargo test` y `cargo clippy --all-targets`
Expected: PASS sin warnings nuevos.

---

### Task 6: Verificación manual y documentación

- [ ] **Step 1: Guion manual**

Run: `cargo build --release` y `cargo run --release -p gui`

1. Editor → herramienta Texto: la barra muestra `Segoe UI`, `Tamaño 20`, `Negrita: no`, `Color`.
2. Clic en el chip de fuente: aparece la lista de familias del sistema, ordenada.
3. Elegir otra familia (p. ej. `Consolas`) y escribir: la caja de edición ya se ve en esa fuente.
4. Con la caja abierta, cambiar tamaño → la caja crece; negrita → engorda; color → **el texto de la caja cambia de color**.
5. Confirmar: la anotación sale con la fuente, tamaño, negrita y color elegidos.
6. Doble clic sobre ese texto: la barra pasa a mostrar SU estilo; cambiar la fuente y confirmar → se reestila.
7. Crear `fonts/` junto al exe con un `.ttf` cualquiera (probar también `MiFuente-Bold.ttf`) → reabrir el editor: la familia aparece en la lista y funciona.
8. Poner en `fonts/` un archivo `.ttf` corrupto → la app no debe caerse; esa familia simplemente no pinta o no aparece.
9. Borrar la carpeta `fonts/` → todo sigue funcionando con las del sistema.
10. Repetir 3 y 5 a 150 % de escala.

- [ ] **Step 2: Documentación**

- `ideas.md`: **f.54** «Tipografía elegible para el texto: familias del sistema más las que el usuario deje en una carpeta `fonts/` junto al ejecutable, sin instalarlas en Windows.»
- `arquitectura.md` D5: `RenderContext` es un catálogo; `TextStyle.familia` es un id `u16` para no perder `Copy`; la cadena de respaldo; el core sigue sin abrir archivos y el catálogo vive en `platform-win`.
- `arquitectura.md` D9: la carpeta `fonts/` junto al exe es otra pieza portable-first.
- `roadmap.md`: ítem de fuentes en F3.6.
- `diseno-frontend.md` V4: el chip de fuente y que los chips afectan a la caja abierta.

- [ ] **Step 3: `verification-before-completion`** y **Step 4: proponer commit** (no automático).

---

## Autorrevisión

| Requisito | Tarea |
|---|---|
| Elegir fuente del texto | 1, 5 |
| Cambiar fuente/tamaño/negrita/color con la caja abierta | 5 |
| Carpeta `fonts/` junto al exe, con prioridad | 3 |
| Fuentes del sistema y por usuario | 3 |
| No caerse con un TTF corrupto | 1 (error de `cargar_cara`), 3 (filtro), guion 8 |

**Riesgos anotados:**
- **El menú con 400 familias** es un `TrackPopupMenu` largo. Windows le pone flechas de scroll solo, así que funciona, pero no es cómodo. Si molesta, el paso siguiente es una ventana de lista con filtro por escritura.
- **Las `.ttc` quedan fuera** (Cambria, algunas asiáticas): `fontdue` no expone el índice de cara dentro de la colección. Documentado en la cabecera del módulo, no olvidado.
- **La negrita puede faltar** en familias que solo traen una cara; la cadena de respaldo cae a la normal, así que se verá igual que la normal en vez de no verse.
- **El nombre de familia de la carpeta portable sale del nombre de archivo**, no de la tabla `name` del TTF. Es predecible para el usuario (`MiFuente-Bold.ttf` → familia `MiFuente`, negrita) pero puede no coincidir con el nombre "oficial" de la fuente.
