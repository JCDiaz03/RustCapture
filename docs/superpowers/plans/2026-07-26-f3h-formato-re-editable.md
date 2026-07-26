# F3 Slice H — Formato re-editable `.rcap`: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** guardar la captura con sus anotaciones **editables** en un archivo `.rcap` y volver a abrirlo para seguir trabajando (f.31).

**Architecture:** el `.rcap` es un ZIP **sin comprimir** con dos miembros: `imagen.png` (el frame base, sin anotar) y `documento.toml` (versión, familias tipográficas usadas y la lista de objetos). Ambas piezas se generan en el core, que no toca el disco: `empaquetar` devuelve `Vec<u8>` y `desempaquetar` lo consume. Serializar es `derive` sobre el enum `Forma` — la razón de fondo por la que se cerró la jerarquía en el slice E. Cero dependencias nuevas: `toml` ya es dependencia directa, y `crc32fast` y `flate2` ya están en el árbol porque los arrastra `png`, así que el contenedor no añade nada al binario. El `FamiliaId` NO se guarda como número del catálogo (cambia entre máquinas): el documento lleva la lista de **nombres** y los objetos indexan en ella; al abrir, el editor registra esos nombres en su catálogo y remapea.

**Tech Stack:** Rust 2024; `rustcapture-core` con `serde`, `toml`, `png`, `crc32fast`, `flate2`; `platform-win` con `windows` 0.62.

## Global Constraints

- **Cero dependencias NUEVAS.** `crc32fast` y `flate2` se declaran explícitas en `core`, pero ya estaban compiladas (vía `png`): el binario no crece. Está comprobado en `Cargo.lock`.
- **El core no abre archivos.** `empaquetar`/`desempaquetar` trabajan sobre bytes; abrir y escribir es del editor.
- **Un `.rcap` es un ZIP de verdad**, no un contenedor propio: renombrar a `.zip` y abrirlo con cualquier herramienta tiene que funcionar. Y al revés, un `.rcap` recomprimido por el Explorador de Windows (que usa deflate) tiene que seguir abriéndose — por eso la lectura acepta método 0 y 8.
- **Versionado explícito:** `version = 1` en el TOML. Un archivo con versión mayor se rechaza con un mensaje claro en vez de intentar interpretarlo.
- **TDD en `core`**: test primero, incluido un round-trip completo objeto a objeto.
- **Commit único al final**, propuesto al humano (`skills.md`).

---

## Estructura de archivos

| Archivo | Responsabilidad | Acción |
|---|---|---|
| `crates/core/src/output/contenedor.rs` | ZIP store-only: escribir y leer sobre bytes | **crear** |
| `crates/core/src/annotate/formato.rs` | `DocumentoGuardado`, empaquetar/desempaquetar, remapeo de familias | **crear** |
| `crates/core/src/annotate/{objeto,giro,style}.rs` | derives de serde; `Giro` como `f32` | modificar |
| `crates/core/src/annotate/document.rs` | `objetos()` y `from_objetos()` | modificar |
| `crates/core/src/ports/{geometry,frame}.rs` | `Rect` serializable; `Frame::from_png` | modificar |
| `crates/core/src/annotate/annotations/*.rs` | derives en los 9 tipos | modificar (9) |
| `crates/core/Cargo.toml` | `crc32fast`, `flate2` explícitas | modificar |
| `design/icons/output-open.svg` | icono de Abrir | **crear** |
| `crates/platform-win/src/editor/{math,mod,estado}.rs` | botón Abrir, guardar `.rcap`, cargar | modificar |
| `ideas.md`, `roadmap.md`, `arquitectura.md`, `diseno-frontend.md` | f.31 hecho, D6, icono | modificar |

---

### Task 1: Contenedor ZIP

Lo primero porque es autónomo y es donde están los detalles binarios que hay que acertar.

**Files:**
- Create: `crates/core/src/output/contenedor.rs`
- Modify: `crates/core/src/output/mod.rs`, `crates/core/Cargo.toml`

**Interfaces:**
- Produces:
  - `pub(crate) struct Miembro<'a> { pub nombre: &'a str, pub datos: &'a [u8] }`
  - `pub(crate) fn escribir(miembros: &[Miembro]) -> Vec<u8>`
  - `pub(crate) fn leer(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, ContenedorError>`
  - `pub enum ContenedorError { NoEsZip, Truncado, MetodoNoSoportado(u16), CrcMalo(String) }`

- [ ] **Step 1: Escribir el módulo con sus tests**

```rust
//! Contenedor ZIP mínimo para el formato re-editable (f.31): se escribe
//! SIN comprimir (el PNG ya lo está y el TOML es diminuto) y se lee
//! aceptando también deflate, para que un `.rcap` recomprimido con el
//! Explorador de Windows siga abriéndose.
//!
//! Se hace a mano y no con el crate `zip` porque `crc32fast` y `flate2` ya
//! están en el árbol (los arrastra `png`): así el contenedor no añade nada
//! al binario, que es prioridad del proyecto (f.4/f.5).
//!
//! Se lee por el DIRECTORIO CENTRAL, no recorriendo las cabeceras locales:
//! los zips de terceros pueden usar descriptor de datos y dejar los
//! tamaños a cero en la cabecera local, y entonces el recorrido secuencial
//! no sabe cuánto avanzar.

const LOCAL: u32 = 0x0403_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const FIN: u32 = 0x0605_4b50;
/// 1980-01-01 en formato MS-DOS: date 0 es inválido y algunas
/// herramientas lo avisan.
const FECHA_DOS: u16 = 0x0021;

#[derive(thiserror::Error, Clone, PartialEq, Eq, Debug)]
pub enum ContenedorError {
    #[error("no es un archivo ZIP")]
    NoEsZip,
    #[error("el archivo está truncado")]
    Truncado,
    #[error("compresión no soportada (método {0})")]
    MetodoNoSoportado(u16),
    #[error("{0}: los datos están corruptos (CRC)")]
    CrcMalo(String),
}

pub(crate) struct Miembro<'a> {
    pub nombre: &'a str,
    pub datos: &'a [u8],
}

pub(crate) fn escribir(miembros: &[Miembro]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for m in miembros {
        let offset = out.len() as u32;
        let crc = crc32fast::hash(m.datos);
        let largo = m.datos.len() as u32;
        let n = m.nombre.len() as u16;
        // Cabecera local + datos.
        out.extend_from_slice(&LOCAL.to_le_bytes());
        for v in [20u16, 0, 0, 0, FECHA_DOS] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&largo.to_le_bytes());
        out.extend_from_slice(&largo.to_le_bytes());
        out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(m.nombre.as_bytes());
        out.extend_from_slice(m.datos);
        // Entrada del directorio central.
        central.extend_from_slice(&CENTRAL.to_le_bytes());
        for v in [20u16, 20, 0, 0, 0, FECHA_DOS] {
            central.extend_from_slice(&v.to_le_bytes());
        }
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&largo.to_le_bytes());
        central.extend_from_slice(&largo.to_le_bytes());
        central.extend_from_slice(&n.to_le_bytes());
        for v in [0u16, 0, 0, 0] {
            central.extend_from_slice(&v.to_le_bytes());
        }
        central.extend_from_slice(&0u32.to_le_bytes()); // atributos externos
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(m.nombre.as_bytes());
    }
    let inicio_central = out.len() as u32;
    let tam_central = central.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&FIN.to_le_bytes());
    for v in [0u16, 0, miembros.len() as u16, miembros.len() as u16] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&tam_central.to_le_bytes());
    out.extend_from_slice(&inicio_central.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // comentario
    out
}

/// Lee u16/u32 little-endian sin panicar si el buffer se queda corto.
fn u16_en(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?))
}

fn u32_en(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

pub(crate) fn leer(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, ContenedorError> {
    // El EOCD está al final; puede llevar comentario, así que se busca su
    // firma hacia atrás.
    let fin = (0..bytes.len().saturating_sub(21))
        .rev()
        .find(|&i| u32_en(bytes, i) == Some(FIN))
        .ok_or(ContenedorError::NoEsZip)?;
    let total = u16_en(bytes, fin + 10).ok_or(ContenedorError::Truncado)? as usize;
    let mut pos = u32_en(bytes, fin + 16).ok_or(ContenedorError::Truncado)? as usize;

    let mut salida = Vec::with_capacity(total);
    for _ in 0..total {
        if u32_en(bytes, pos) != Some(CENTRAL) {
            return Err(ContenedorError::NoEsZip);
        }
        let metodo = u16_en(bytes, pos + 10).ok_or(ContenedorError::Truncado)?;
        let crc = u32_en(bytes, pos + 16).ok_or(ContenedorError::Truncado)?;
        let comprimido = u32_en(bytes, pos + 20).ok_or(ContenedorError::Truncado)? as usize;
        let n = u16_en(bytes, pos + 28).ok_or(ContenedorError::Truncado)? as usize;
        let extra = u16_en(bytes, pos + 30).ok_or(ContenedorError::Truncado)? as usize;
        let comentario = u16_en(bytes, pos + 32).ok_or(ContenedorError::Truncado)? as usize;
        let local = u32_en(bytes, pos + 42).ok_or(ContenedorError::Truncado)? as usize;
        let nombre = String::from_utf8_lossy(
            bytes.get(pos + 46..pos + 46 + n).ok_or(ContenedorError::Truncado)?,
        )
        .into_owned();
        pos += 46 + n + extra + comentario;

        // Los datos empiezan tras la cabecera LOCAL, cuyo `extra` puede
        // diferir del de la central.
        if u32_en(bytes, local) != Some(LOCAL) {
            return Err(ContenedorError::NoEsZip);
        }
        let n_local = u16_en(bytes, local + 26).ok_or(ContenedorError::Truncado)? as usize;
        let extra_local = u16_en(bytes, local + 28).ok_or(ContenedorError::Truncado)? as usize;
        let inicio = local + 30 + n_local + extra_local;
        let crudos = bytes
            .get(inicio..inicio + comprimido)
            .ok_or(ContenedorError::Truncado)?;

        let datos = match metodo {
            0 => crudos.to_vec(),
            8 => {
                use std::io::Read;
                let mut out = Vec::new();
                flate2::read::DeflateDecoder::new(crudos)
                    .read_to_end(&mut out)
                    .map_err(|_| ContenedorError::CrcMalo(nombre.clone()))?;
                out
            }
            otro => return Err(ContenedorError::MetodoNoSoportado(otro)),
        };
        if crc32fast::hash(&datos) != crc {
            return Err(ContenedorError::CrcMalo(nombre));
        }
        salida.push((nombre, datos));
    }
    Ok(salida)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta_con_dos_miembros() {
        let a = vec![1u8, 2, 3, 4, 5];
        let b = b"contenido de texto\ncon salto".to_vec();
        let zip = escribir(&[
            Miembro { nombre: "imagen.png", datos: &a },
            Miembro { nombre: "documento.toml", datos: &b },
        ]);
        // Firma de ZIP: cualquier herramienta lo reconocerá.
        assert_eq!(&zip[..4], &LOCAL.to_le_bytes());
        let leido = leer(&zip).unwrap();
        assert_eq!(leido.len(), 2);
        assert_eq!(leido[0], ("imagen.png".to_string(), a));
        assert_eq!(leido[1], ("documento.toml".to_string(), b));
    }

    #[test]
    fn un_miembro_vacio_no_rompe() {
        let zip = escribir(&[Miembro { nombre: "vacio", datos: &[] }]);
        assert_eq!(leer(&zip).unwrap(), vec![("vacio".to_string(), vec![])]);
    }

    #[test]
    fn lo_que_no_es_zip_se_rechaza() {
        assert_eq!(leer(b"").unwrap_err(), ContenedorError::NoEsZip);
        assert_eq!(leer(b"no soy un zip").unwrap_err(), ContenedorError::NoEsZip);
    }

    #[test]
    fn un_zip_truncado_no_panica() {
        let zip = escribir(&[Miembro { nombre: "x", datos: &[7; 100] }]);
        // Cortar por la mitad se lleva el EOCD: se detecta como "no es zip".
        assert!(leer(&zip[..zip.len() / 2]).is_err());
    }

    #[test]
    fn un_dato_alterado_se_detecta_por_crc() {
        let mut zip = escribir(&[Miembro { nombre: "d", datos: b"hola" }]);
        // Los datos van justo tras la cabecera local (30 + 1 de nombre).
        zip[31] ^= 0xFF;
        assert_eq!(
            leer(&zip).unwrap_err(),
            ContenedorError::CrcMalo("d".to_string())
        );
    }
}
```

- [ ] **Step 2: Declarar el módulo y las dependencias**

`crates/core/src/output/mod.rs`: `pub mod contenedor;` (público porque su error sale en la API del formato).

`crates/core/Cargo.toml`, en `[dependencies]`:

```toml
# Ya venían en el árbol vía `png`: declararlas no engorda el binario y
# evitan escribir CRC32 e inflate a mano (contenedor del .rcap, f.31).
crc32fast = "1"
flate2 = "1"
```

Añadirlas también a `[workspace.dependencies]` del `Cargo.toml` raíz y usar `.workspace = true`, como el resto.

- [ ] **Step 3: Ejecutar**

Run: `cargo test -p rustcapture-core output::contenedor`
Expected: 5 tests en verde.

- [ ] **Step 4: Comprobar que Windows lo reconoce como ZIP**

Generar un zip de prueba con el test y abrirlo con el Explorador (o `Expand-Archive`). Es la única forma de verificar que las cabeceras son correctas de verdad y no solo autoconsistentes.

---

### Task 2: Serde en los tipos del motor

**Files:**
- Modify: `crates/core/src/annotate/annotations/*.rs` (9), `objeto.rs`, `giro.rs`, `style.rs`
- Modify: `crates/core/src/ports/geometry.rs`

**Interfaces:**
- Produces: `Serialize`/`Deserialize` en `Rect`, `Color`, `Style`, `TextStyle`, `FamiliaId`, `CensorMode`, `Giro`, las 9 anotaciones, `Forma` y `Objeto`.

- [ ] **Step 1: Test de round-trip que falla**

En un `mod tests` nuevo de `crates/core/src/annotate/formato.rs` (se crea en la Task 3; hacer las dos seguidas) o provisionalmente en `objeto.rs`:

```rust
    /// Round-trip TOML de UNA variante de cada forma: si alguna no
    /// serializa, el documento entero se pierde.
    #[test]
    fn todas_las_formas_sobreviven_al_toml() {
        let ctx = ctx();
        for (i, original) in todos().into_iter().enumerate() {
            let texto = toml::to_string(&original).expect("serializar");
            let vuelta: Objeto = toml::from_str(&texto).expect("deserializar");
            assert_eq!(
                vuelta.bounds(&ctx),
                original.bounds(&ctx),
                "variante {i} cambió al ir y volver"
            );
        }
    }

    #[test]
    fn el_giro_sobrevive_como_angulo() {
        let mut o: Objeto = RectAnnotation {
            rect: Rect::new(1, 2, 10, 10),
            style: ESTILO,
        }
        .into();
        o.rotar(0.7);
        let vuelta: Objeto = toml::from_str(&toml::to_string(&o).unwrap()).unwrap();
        assert!((vuelta.giro.rad() - 0.7).abs() < 1e-6);
        // Y el seno/coseno se reconstruyen: girar sigue funcionando.
        assert_eq!(vuelta.bounds(&ctx()), o.bounds(&ctx()));
    }
```

- [ ] **Step 2: Ejecutar y ver que falla** (los tipos no implementan `Serialize`).

- [ ] **Step 3: Añadir los derives**

A `Rect`, `Color`, `Style`, `TextStyle`, `FamiliaId`, `CensorMode`, `Forma`, `Objeto` y las 9 anotaciones:

```rust
#[derive(serde::Serialize, serde::Deserialize, ...)]
```

`Giro` es el único caso especial: cachea seno y coseno, que no hay que guardar. Se serializa como el ángulo y se reconstruye al leer:

```rust
/// Se guarda solo el ángulo: seno y coseno son caché y se recalculan.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(into = "f32", from = "f32")]
pub struct Giro { /* … */ }

impl From<f32> for Giro {
    fn from(rad: f32) -> Self {
        Giro::new(rad)
    }
}

impl From<Giro> for f32 {
    fn from(g: Giro) -> Self {
        g.rad
    }
}
```

- [ ] **Step 4: Ejecutar** — `cargo test -p rustcapture-core`; PASS.

---

### Task 3: El documento `.rcap`

**Files:**
- Create: `crates/core/src/annotate/formato.rs`
- Modify: `crates/core/src/annotate/mod.rs`, `document.rs`, `crates/core/src/ports/frame.rs`

**Interfaces:**
- Produces:
  - `pub const VERSION_RCAP: u32 = 1;`
  - `pub struct DocumentoGuardado { pub version: u32, pub familias: Vec<String>, pub objetos: Vec<Objeto> }`
  - `pub fn empaquetar(base: &Frame, doc: &Document, ctx: &RenderContext) -> Result<Vec<u8>, FormatoError>`
  - `pub fn desempaquetar(bytes: &[u8]) -> Result<(Frame, DocumentoGuardado), FormatoError>`
  - `DocumentoGuardado::remapear_familias(&mut self, mapa: &[FamiliaId])`
  - `DocumentoGuardado::en_documento(self) -> Document`
  - `Document::objetos(&self) -> &[Objeto]`, `Document::from_objetos(Vec<Objeto>) -> Document`
  - `Frame::from_png(bytes: &[u8]) -> Result<Frame, FrameError>`

- [ ] **Step 1: Tests que fallan**

```rust
    #[test]
    fn un_documento_va_y_vuelve_entero() {
        let ctx = ctx_con_fuente();
        let base = Frame::filled(40, 30, [10, 20, 30, 255]);
        let mut doc = Document::new();
        let mut h = History::new();
        // Un objeto de cada familia de rasterizado, con giro y texto.
        for o in objetos_de_prueba(&mut ctx.clone()) {
            h.apply(&mut doc, Command::add(o));
        }
        let bytes = empaquetar(&base, &doc, &ctx).unwrap();
        let (base2, guardado) = desempaquetar(&bytes).unwrap();
        assert_eq!(base2, base, "la imagen base no sobrevivió");
        assert_eq!(guardado.version, VERSION_RCAP);
        assert_eq!(guardado.objetos.len(), doc.len());
        // El horneado del documento leído es idéntico al del original.
        let mut a = base.clone();
        doc.render_onto(&mut a, &ctx);
        let mut b = base.clone();
        guardado.en_documento().render_onto(&mut b, &ctx);
        assert_eq!(a, b, "lo pintado cambió al ir y volver");
    }

    #[test]
    fn las_familias_se_guardan_por_nombre_no_por_id() {
        // Un id de catálogo no significa nada en otra máquina.
        let mut ctx = RenderContext::nueva();
        let _relleno = ctx.registrar_familia("Relleno");
        let usada = ctx.registrar_familia("Consolas");
        ctx.cargar_cara(usada, false, &ttf_consolas()).unwrap();
        let mut doc = Document::new();
        let mut h = History::new();
        h.apply(
            &mut doc,
            Command::add(
                TextAnnotation {
                    pos: (1, 1),
                    text: "x".into(),
                    style: TextStyle {
                        color: Color::rgb(0, 0, 0),
                        size: 12.0,
                        bold: false,
                        familia: usada,
                    },
                }
                .into(),
            ),
        );
        let bytes = empaquetar(&Frame::filled(8, 8, [0; 4]), &doc, &ctx).unwrap();
        let (_, guardado) = desempaquetar(&bytes).unwrap();
        // Solo la familia REFERENCIADA, compactada al índice 0.
        assert_eq!(guardado.familias, vec!["Consolas".to_string()]);
    }

    #[test]
    fn una_version_futura_se_rechaza_con_mensaje() {
        let toml = format!("version = {}\nfamilias = []\nobjetos = []\n", VERSION_RCAP + 1);
        let zip = crate::output::contenedor::escribir(&[
            crate::output::contenedor::Miembro {
                nombre: "imagen.png",
                datos: &crate::output::encode(
                    &Frame::filled(2, 2, [0, 0, 0, 255]),
                    crate::output::ImageFormat::Png,
                )
                .unwrap(),
            },
            crate::output::contenedor::Miembro {
                nombre: "documento.toml",
                datos: toml.as_bytes(),
            },
        ]);
        assert!(matches!(
            desempaquetar(&zip),
            Err(FormatoError::VersionNoSoportada(_))
        ));
    }

    #[test]
    fn un_rcap_sin_sus_miembros_da_error_claro() {
        let zip = crate::output::contenedor::escribir(&[
            crate::output::contenedor::Miembro { nombre: "otra.cosa", datos: b"x" },
        ]);
        assert!(matches!(desempaquetar(&zip), Err(FormatoError::MiembroAusente(_))));
    }
```

- [ ] **Step 2: Ejecutar y ver que fallan.**

- [ ] **Step 3: Implementar**

`Document` gana los accesos que la serialización necesita:

```rust
    pub fn objetos(&self) -> &[Objeto] {
        &self.objetos
    }

    /// Documento a partir de objetos ya construidos (lo usa la carga de un
    /// `.rcap`). No pasa por `Command` porque no es una edición del usuario:
    /// es el estado inicial, y su historial arranca vacío.
    pub fn from_objetos(objetos: Vec<Objeto>) -> Self {
        Self { objetos }
    }
```

`Frame::from_png` con el decoder del crate `png` (mismo patrón que el test que ya existe en `encode.rs`), devolviendo `FrameError` si no es RGBA8 o las dimensiones no cuadran.

`formato.rs`:
- `empaquetar`: recorre `doc.objetos()`, recoge las familias referenciadas por los `Forma::Texto` (resolviendo el nombre con `ctx.nombre`), las compacta a una lista y clona los objetos remapeando `TextStyle.familia` al índice de esa lista; serializa a TOML; codifica el PNG; llama a `contenedor::escribir`.
- `desempaquetar`: `contenedor::leer`, busca `imagen.png` y `documento.toml` (error `MiembroAusente` si falta alguno), decodifica el PNG, parsea el TOML y valida `version <= VERSION_RCAP`.
- `remapear_familias`: recorre los objetos y sustituye `TextStyle.familia` según el mapa índice-de-archivo → `FamiliaId` del catálogo.

`FormatoError` envuelve `ContenedorError`, `FrameError`, el error de TOML y añade `VersionNoSoportada(u32)` y `MiembroAusente(&'static str)`.

- [ ] **Step 4: Ejecutar** — `cargo test -p rustcapture-core`; PASS.

---

### Task 4: Icono de Abrir

**Files:**
- Create: `design/icons/output-open.svg`
- Modify: `design/tools/genassets/src/main.rs`
- Regenera: `crates/platform-win/src/ui/iconos/atlas*.bin` y `atlas.rs`

- [ ] **Step 1: Crear el SVG** con la especificación del set (trazo 1.5 sobre rejilla 16×16, remates redondeados, `currentColor`): una carpeta abierta o una flecha entrando en una bandeja.

- [ ] **Step 2: Añadir `"output-open"` al final de `ICONOS`** y subir el tamaño del array a 41.

- [ ] **Step 3: Regenerar** — `cd design/tools/genassets && cargo run --release`. Debe decir «41 iconos» y `atlas.rs` traer `OutputOpen = 40`.

- [ ] **Step 4: Verificar el icono a ojo** volcando su máscara A8 a ASCII antes de usarlo (como se hizo con `edit-rotate`), para no descubrir en la GUI que sale mal.

---

### Task 5: Guardar y abrir en el editor

**Files:**
- Modify: `crates/platform-win/src/editor/math.rs`, `mod.rs`, `estado.rs`

**Interfaces:**
- Produces: `math::ID_ABRIR`, botón habilitado en la toolbar; el diálogo de guardar gana el filtro `.rcap`.

- [ ] **Step 1: Tests de la toolbar**

Actualizar `las_herramientas_del_motor_y_las_salidas_estan_habilitadas` con `ID_ABRIR` en su posición, y comprobar ids únicos (el test ya existente lo cubre).

- [ ] **Step 2: Botón en la toolbar**

`ID_ABRIR` con `Icono::OutputOpen`, tooltip «Abrir .rcap», habilitado, antes de Copiar.

- [ ] **Step 3: Guardar como `.rcap`**

`guardar_como` gana el filtro `RustCapture re-editable (*.rcap)` como **primera** opción (es el formato que no pierde nada) y PNG/JPEG detrás. Si el índice elegido es el del `.rcap`, llama a `annotate::formato::empaquetar(&state.base, &state.doc, &state.ctx)` — ojo: la **base**, no `committed`, porque el documento se guarda aparte. Con PNG/JPEG sigue guardando `committed` como hasta ahora.

- [ ] **Step 4: Abrir**

`abrir_rcap(hwnd, state)`:
1. Si `state.dirty`, confirmar el descarte (mismo `MessageBox` que `WM_CLOSE`).
2. `GetOpenFileNameW` con filtro `.rcap`.
3. Leer el archivo y `desempaquetar`; error → `alerts::error_box` con el mensaje del `FormatoError` y no tocar nada.
4. Registrar cada nombre de `guardado.familias` en `state.ctx` cargando sus caras del catálogo (`cargar_familia`); construir el mapa y `remapear_familias`.
5. Sustituir en el estado: `base`, `committed` y sus DIB (las dimensiones cambian, así que hay que **recrear** los DIB y los buffers de preview, no reutilizarlos), `doc`, `history` vacío, `pasos` reiniciado, selección y arrastres a `None`, `dirty = false`, `nombre` = el del archivo.
6. Título de la ventana y repintado.

**Cuidado:** todo lo que hoy asume que las dimensiones del frame no cambian nunca (`refresh_committed` copia sobre buffers existentes, `preview` reutiliza su DIB) deja de ser cierto al abrir otro archivo. Por eso el paso 5 recrea los buffers en vez de rellenarlos.

- [ ] **Step 5: Ejecutar** — `cargo test` y `cargo clippy --all-targets`; PASS sin warnings nuevos.

---

### Task 6: Verificación manual y documentación

- [ ] **Step 1: Guion manual**

**Compilar con la app CERRADA** (`Get-Process rustcapture-gui | Stop-Process` antes de `cargo build --release`): con el exe en uso el enlazado falla en silencio y se prueba el binario viejo.

1. Captura → anotar con flecha, texto (con otra fuente), paso numerado, resaltador, pixelado y algo girado.
2. **Guardar como** → `.rcap`. El archivo aparece con ese tamaño razonable.
3. Renombrar a `.zip` y abrirlo con el Explorador: dentro, `imagen.png` y `documento.toml`. Abrir el TOML: legible, con las familias por nombre.
4. Cerrar el editor. Nueva captura → botón **Abrir** → elegir el `.rcap`: sale la imagen original con **todas** las anotaciones.
5. Mover, girar y borrar un objeto recién cargado: se comporta como si se acabara de dibujar.
6. `Ctrl+Z` justo tras abrir: no debe deshacer nada (el historial arranca vacío).
7. Con cambios sin guardar, pulsar Abrir → pide confirmación.
8. Abrir un PNG cualquiera renombrado a `.rcap` → error claro, el editor intacto.
9. Editar el `documento.toml` dentro del zip con el Explorador (que recomprime con deflate) y reabrir: debe funcionar.
10. Abrir un `.rcap` de dimensiones distintas a la captura actual: la imagen y el canvas se ajustan.
11. Guardar un `.rcap` con un texto en una fuente de la carpeta portable, borrar esa fuente y reabrir: el texto sale con la de respaldo, sin caerse.

- [ ] **Step 2: Documentación**

- `ideas.md`: f.31 sin marcador de pendiente.
- `arquitectura.md` D6: el `.rcap` es un ZIP store-only con `imagen.png` + `documento.toml`; las familias van por nombre; el contenedor se hace a mano porque `crc32fast`/`flate2` ya estaban en el árbol.
- `roadmap.md`: F3 con el formato hecho.
- `diseno-frontend.md` V4: botón Abrir y el `.rcap` en las salidas.

- [ ] **Step 3: `verification-before-completion`** y **Step 4: proponer commit**.

---

## Autorrevisión

| Requisito | Tarea |
|---|---|
| Guardar imagen + objetos editables | 1, 3, 5 |
| Reabrir y seguir editando | 3, 5 |
| Es un ZIP inspeccionable | 1 (+ guion 3) |
| Tolerar recompresión externa | 1 (deflate en lectura, guion 9) |
| Familias portables entre máquinas | 3 (por nombre, guion 11) |
| Versión futura rechazada con mensaje | 3 |
| Cero dependencias nuevas | Constraint global |

**Riesgos anotados:**
- **Las dimensiones del frame dejan de ser inmutables.** Es la suposición más extendida del editor (`refresh_committed`, los DIB de preview). El paso 5 de la Task 5 recrea los buffers; si algo se salta eso, el síntoma será un canvas corrupto o un pánico de `copy_from_slice` por longitudes distintas.
- **El TOML es editable a mano y eso invita a romperlo.** El CRC del zip detecta corrupción binaria, pero un TOML sintácticamente válido con valores absurdos (un `size` negativo) entra tal cual. No se valida por ahora; queda anotado.
- **`.ttc` y fuentes ausentes:** un `.rcap` guardado con una fuente que la otra máquina no tiene cae a la de respaldo. Es la cadena de respaldo del slice G funcionando, no un fallo, pero el texto se verá distinto.
