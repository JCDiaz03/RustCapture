# F1 — Config TOML portable-first (f.4, D9) — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** El struct `Config` único de D9 serializado a TOML, con detección de modo al arrancar — `config.toml` junto al exe → portable; si no → `%APPDATA%\RustCapture\` — e integrado en la CLI para que `dir`/`format`/`prefix` del archivo sean los defaults reales de `--file`.

**Architecture:** Todo en el slice `config` de `core` (leer archivos y variables de entorno es std, no Win32). La detección de ruta es una función pura sobre parámetros (`exe_dir`, `appdata`) con un wrapper `default_location()` que lee el entorno. `Config::load` trata "archivo inexistente" como defaults (primera ejecución) pero un TOML roto es error explícito: el usuario debe enterarse. Campos desconocidos se ignoran (compat hacia delante con configs de versiones más nuevas). La CLI pasa a resolver `--dir`/`--format` como `flag > config > default`.

**Tech Stack:** `serde` 1 (derive) + `toml` 1. Ambos solo en `core`.

## Global Constraints

- D9: UN solo struct `Config`; portable es un detalle de runtime, no dos builds.
- `rustcapture-core` mantiene cero Win32; `%APPDATA%` se lee por variable de entorno (std).
- TDD en `core` (skills.md); la detección portable se verifica además con el binario real (config junto a `rustcapture.exe` en `target/debug`).
- Comentarios y rustdoc en español. `cargo fmt` antes de cada verificación.
- **Commits: SOLO con aprobación humana previa** (skills.md). Único commit al final: `v0.1.7 — F1: config TOML portable-first`.
- TOML parcial válido: campos ausentes → defaults (`#[serde(default)]` en todos los niveles). TOML inválido → `ConfigError::Parse`, nunca defaults silenciosos.
- `ImageFormat` en config acepta `"png"`, `"jpeg"` y el alias `"jpg"` (mismo vocabulario que la CLI).
- Nota de versión: si `toml` 1.x renombra APIs respecto a 0.8 (`from_str`/`to_string_pretty`), ajustar según el compilador sin cambiar el diseño.

---

### Task 1: `Config` + serde (`config/mod.rs`)

**Files:**
- Modify: `Cargo.toml` (workspace: añadir `serde`, `toml`)
- Modify: `crates/core/Cargo.toml` (añadir `serde`, `toml`)
- Modify: `crates/core/src/output/encode.rs` (derives serde en `ImageFormat`)
- Modify: `crates/core/src/config/mod.rs` (hoy solo doc-comment)

**Interfaces:**
- Consumes: `output::ImageFormat`.
- Produces: `Config { output: OutputConfig }` y `OutputConfig { dir: PathBuf, format: ImageFormat, prefix: String }` (ambos `Clone + PartialEq + Debug + Default` — defaults: `"."`, `Png`, `"captura"`); `Config::from_toml(&str) -> Result<Config, ConfigError>`; `Config::to_toml(&self) -> String`; `ConfigError { Parse(String), Io(PathBuf, String) }`. Tasks 2-3 los consumen.

- [ ] **Step 1: Declarar dependencias**

En el `Cargo.toml` del workspace, bajo `[workspace.dependencies]`:

```toml
serde = { version = "1", features = ["derive"] }
toml = "1"
```

En `crates/core/Cargo.toml`, bajo `[dependencies]`:

```toml
serde = { workspace = true }
toml = { workspace = true }
```

En `crates/core/src/output/encode.rs`, sustituir el derive de `ImageFormat` por:

```rust
/// Formatos de salida de imagen fija soportados por el MVP.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    Png,
    /// Calidad fija 90 hasta que la config (D9) la parametrice.
    #[serde(alias = "jpg")]
    Jpeg,
}
```

- [ ] **Step 2: Escribir los tests que fallan**

En `crates/core/src/config/mod.rs`, conservar el doc-comment existente y añadir:

```rust
use std::path::PathBuf;

use crate::output::ImageFormat;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_toml_vacio_da_los_defaults() {
        let config = Config::from_toml("").unwrap();
        assert_eq!(config, Config::default());
        assert_eq!(config.output.dir, PathBuf::from("."));
        assert_eq!(config.output.format, ImageFormat::Png);
        assert_eq!(config.output.prefix, "captura");
    }

    #[test]
    fn un_toml_parcial_completa_con_defaults() {
        let config = Config::from_toml("[output]\nformat = \"jpg\"\n").unwrap();
        assert_eq!(config.output.format, ImageFormat::Jpeg);
        assert_eq!(config.output.prefix, "captura"); // default intacto
    }

    #[test]
    fn un_toml_roto_es_error_y_no_defaults_silenciosos() {
        assert!(matches!(
            Config::from_toml("[output]\nformat = \"tiff\"\n").unwrap_err(),
            ConfigError::Parse(_)
        ));
    }

    #[test]
    fn ida_y_vuelta_por_toml_conserva_la_config() {
        let mut config = Config::default();
        config.output.dir = PathBuf::from("C:/caps");
        config.output.format = ImageFormat::Jpeg;
        assert_eq!(Config::from_toml(&config.to_toml()).unwrap(), config);
    }
}
```

- [ ] **Step 3: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — `cannot find struct Config`.

- [ ] **Step 4: Implementar**

En `config/mod.rs`, entre los `use` y los tests:

```rust
/// Configuración única de la app (D9). Campos desconocidos en el TOML se
/// ignoran: una config escrita por una versión más nueva no rompe esta.
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct Config {
    pub output: OutputConfig,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct OutputConfig {
    /// Directorio de destino de `FileSink`.
    pub dir: PathBuf,
    pub format: ImageFormat,
    /// Prefijo de los nombres automáticos (f.41).
    pub prefix: String,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            dir: PathBuf::from("."),
            format: ImageFormat::Png,
            prefix: "captura".to_string(),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("config inválida: {0}")]
    Parse(String),
    #[error("no se pudo acceder a {0:?}: {1}")]
    Io(PathBuf, String),
}

impl Config {
    pub fn from_toml(texto: &str) -> Result<Self, ConfigError> {
        toml::from_str(texto).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("Config siempre es serializable")
    }
}
```

- [ ] **Step 5: Verificar que pasa**

Run: `cargo fmt && cargo test -p rustcapture-core`
Expected: PASS (53 + 4 = 57).

- [ ] **Step 6: Staging**

```bash
git add Cargo.toml Cargo.lock crates/core/
```

---

### Task 2: Detección portable vs `%APPDATA%` + `load`/`save`

**Files:**
- Modify: `crates/core/src/config/mod.rs`

**Interfaces:**
- Consumes: `Config`, `ConfigError` (Task 1).
- Produces: `StorageMode { Portable, PerUser }` (`Clone + Copy + PartialEq + Eq + Debug`); `CONFIG_FILE: &str = "config.toml"`; `resolve_config_path(exe_dir: &Path, appdata: Option<&Path>) -> (PathBuf, StorageMode)` (pura sobre sus parámetros); `default_location() -> (PathBuf, StorageMode)` (lee `current_exe` y `%APPDATA%`); `Config::load(&Path) -> Result<Config, ConfigError>` (inexistente → defaults); `Config::save(&self, &Path) -> Result<(), ConfigError>` (crea el directorio padre). Task 3 consume `default_location` + `load`.

- [ ] **Step 1: Escribir los tests que fallan**

Añadir al módulo de tests de `config/mod.rs`:

```rust
    use std::path::Path;

    fn dir_temporal(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rustcapture_cfg_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn config_junto_al_exe_gana_y_es_portable() {
        let exe_dir = dir_temporal("portable");
        std::fs::write(exe_dir.join(CONFIG_FILE), "").unwrap();
        let (path, mode) = resolve_config_path(&exe_dir, Some(Path::new("C:/appdata")));
        assert_eq!(path, exe_dir.join(CONFIG_FILE));
        assert_eq!(mode, StorageMode::Portable);
        let _ = std::fs::remove_dir_all(&exe_dir);
    }

    #[test]
    fn sin_config_junto_al_exe_se_va_a_appdata() {
        let exe_dir = dir_temporal("peruser");
        let (path, mode) = resolve_config_path(&exe_dir, Some(Path::new("C:/appdata")));
        assert_eq!(path, Path::new("C:/appdata").join("RustCapture").join(CONFIG_FILE));
        assert_eq!(mode, StorageMode::PerUser);
        let _ = std::fs::remove_dir_all(&exe_dir);
    }

    #[test]
    fn sin_appdata_cae_a_portable() {
        let exe_dir = dir_temporal("sinappdata");
        let (path, mode) = resolve_config_path(&exe_dir, None);
        assert_eq!(path, exe_dir.join(CONFIG_FILE));
        assert_eq!(mode, StorageMode::Portable);
        let _ = std::fs::remove_dir_all(&exe_dir);
    }

    #[test]
    fn load_de_archivo_inexistente_da_defaults() {
        let dir = dir_temporal("noexiste");
        let config = Config::load(&dir.join(CONFIG_FILE)).unwrap();
        assert_eq!(config, Config::default());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_y_load_hacen_ida_y_vuelta_creando_directorios() {
        let dir = dir_temporal("saveload");
        let path = dir.join("sub").join(CONFIG_FILE);
        let mut config = Config::default();
        config.output.prefix = "shot".to_string();
        config.save(&path).unwrap();
        assert_eq!(Config::load(&path).unwrap(), config);
        let _ = std::fs::remove_dir_all(&dir);
    }
```

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p rustcapture-core`
Expected: FAIL — `cannot find CONFIG_FILE` / `resolve_config_path` / `StorageMode`.

- [ ] **Step 3: Implementar**

Añadir a `config/mod.rs` (los `use` de `Path` se integran arriba: `use std::path::{Path, PathBuf};`):

```rust
/// Dónde vive la config (f.4): decidido en runtime, no en build (D9).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StorageMode {
    /// `config.toml` junto al ejecutable.
    Portable,
    /// `%APPDATA%\RustCapture\config.toml`.
    PerUser,
}

pub const CONFIG_FILE: &str = "config.toml";

/// Regla de D9, pura para poder testearla: si hay `config.toml` junto al
/// exe → portable; si no, `%APPDATA%`; sin `%APPDATA%` → portable.
pub fn resolve_config_path(exe_dir: &Path, appdata: Option<&Path>) -> (PathBuf, StorageMode) {
    let portable = exe_dir.join(CONFIG_FILE);
    if portable.exists() {
        return (portable, StorageMode::Portable);
    }
    match appdata {
        Some(base) => (
            base.join("RustCapture").join(CONFIG_FILE),
            StorageMode::PerUser,
        ),
        None => (portable, StorageMode::Portable),
    }
}

/// Ubicación real de la config de este proceso.
pub fn default_location() -> (PathBuf, StorageMode) {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from);
    resolve_config_path(&exe_dir, appdata.as_deref())
}

impl Config {
    /// Carga la config; que no exista aún no es error (primera ejecución).
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(texto) => Self::from_toml(&texto),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(ConfigError::Io(path.to_path_buf(), e.to_string())),
        }
    }

    /// Escribe la config creando el directorio si hace falta.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ConfigError::Io(parent.to_path_buf(), e.to_string()))?;
        }
        std::fs::write(path, self.to_toml())
            .map_err(|e| ConfigError::Io(path.to_path_buf(), e.to_string()))
    }
}
```

- [ ] **Step 4: Verificar que pasa**

Run: `cargo fmt && cargo test -p rustcapture-core`
Expected: PASS (57 + 5 = 62).

- [ ] **Step 5: Staging**

```bash
git add crates/core/src/config/
```

---

### Task 3: Integración en la CLI (`flag > config > default`)

**Files:**
- Modify: `crates/core/src/output/file_sink.rs` (añadir `with_prefix`)
- Modify: `crates/cli/src/args.rs` (`Destination::File` pasa a `Option`s)
- Modify: `crates/cli/src/main.rs` (cargar config y fusionar)

**Interfaces:**
- Consumes: `config::{Config, default_location}` (Tasks 1-2), `args`/`main` existentes.
- Produces: `FileSink::with_prefix(self, prefix: impl Into<String>) -> FileSink` (builder); `Destination::File { dir: Option<PathBuf>, format: Option<ImageFormat> }` — `None` = "decide la config"; `main` resuelve `flag > config > default` y falla con exit 2 si el TOML está roto.

- [ ] **Step 1: Escribir los tests que fallan**

En `crates/core/src/output/file_sink.rs`, añadir al módulo de tests:

```rust
    #[test]
    fn with_prefix_cambia_el_nombre_generado() {
        let dir = dir_temporal("prefijo");
        let mut sink = FileSink::new(&dir, ImageFormat::Png).with_prefix("shot");
        sink.deliver(&Frame::filled(1, 1, [0; 4])).unwrap();
        let nombre = std::fs::read_dir(&dir)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .file_name();
        assert!(nombre.to_string_lossy().starts_with("shot_"));
        let _ = std::fs::remove_dir_all(&dir);
    }
```

En `crates/cli/src/args.rs`, actualizar los DOS tests de `--file` a la nueva forma con `Option`:

```rust
    #[test]
    fn file_con_dir_y_format() {
        let opts = p(&["--file", "--dir", "C:/caps", "--format", "jpg"]).unwrap();
        assert_eq!(
            opts.destination,
            Destination::File {
                dir: Some(PathBuf::from("C:/caps")),
                format: Some(ImageFormat::Jpeg)
            }
        );
    }

    #[test]
    fn file_sin_dir_deja_la_decision_a_la_config() {
        let opts = p(&["--file"]).unwrap();
        assert_eq!(
            opts.destination,
            Destination::File {
                dir: None,
                format: None
            }
        );
    }
```

(El test `file_sin_dir_usa_el_directorio_actual_y_png` se renombra así; el resto de tests no cambian.)

- [ ] **Step 2: Verificar que falla**

Run: `cargo test -p rustcapture-core -p cli`
Expected: FAIL — `no method named with_prefix`; en `cli`, mismatch de variantes de `Destination::File`.

- [ ] **Step 3: Implementar**

En `file_sink.rs`, añadir al `impl FileSink`:

```rust
    /// Sustituye el prefijo por el de la config (D9).
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }
```

En `args.rs`, cambiar la variante y su construcción:

```rust
#[derive(Debug, PartialEq)]
pub enum Destination {
    Clipboard,
    File {
        dir: Option<PathBuf>,
        format: Option<ImageFormat>,
    },
}
```

y en `parse`, el brazo `(false, true)` queda:

```rust
        (false, true) => {
            let format = match format_raw.as_deref() {
                None => None,
                Some("png") => Some(ImageFormat::Png),
                Some("jpg") | Some("jpeg") => Some(ImageFormat::Jpeg),
                Some(otro) => return Err(format!("formato desconocido: {otro} (png|jpg)")),
            };
            Destination::File { dir, format }
        }
```

En `main.rs`: añadir el import `use rustcapture_core::config::Config;` y, justo tras el parseo de `options`, cargar la config:

```rust
    let (config_path, _storage) = rustcapture_core::config::default_location();
    let config = match Config::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
```

y sustituir el brazo `File` del `match options.destination` por:

```rust
        args::Destination::File { dir, format } => {
            let dir = dir.unwrap_or_else(|| config.output.dir.clone());
            let format = format.unwrap_or(config.output.format);
            orch.add_sink(Box::new(
                FileSink::new(dir, format).with_prefix(config.output.prefix.clone()),
            ))
            .expect("primer sink registrado");
            "file"
        }
```

- [ ] **Step 4: Verificar que pasa**

Run: `cargo fmt && cargo test -p rustcapture-core -p cli`
Expected: PASS (62 core + 8 cli).

- [ ] **Step 5: Verificación manual del modo portable con el binario real**

```bash
# Config portable junto al exe (target/debug es donde vive rustcapture.exe con cargo run)
cargo build -p cli
printf '[output]\nformat = "jpg"\nprefix = "shot"\ndir = "<SCRATCH>/cfg_test"\n' > target/debug/config.toml
cargo run -q -p cli -- --region 0,0,40x40 --file
ls "<SCRATCH>/cfg_test"        # debe listar shot_*.jpg (dir, formato y prefijo de la config)
rm target/debug/config.toml    # limpiar: no dejar el modo portable activado
```

Expected: `shot_YYYY-MM-DD_HHMMSS.jpg` en el directorio de la config — la CLI tomó dir, formato y prefijo del `config.toml` portable sin flags.

- [ ] **Step 6: Staging**

```bash
git add crates/core/ crates/cli/
```

---

### Task 4: Verificación final del slice y cierre

**Files:**
- Modify: `roadmap.md` (marcar ✅ el ítem de config, solo tras verificar)

**Interfaces:**
- Consumes: todo lo anterior.
- Produces: slice verificado; propuesta de commit al humano.

- [ ] **Step 1: Verificación completa (skill `verification-before-completion`)**

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

Expected: build limpio; 62 core + 6 platform-win + 8 cli = 76 tests; clippy y formato limpios. Confirmar que `target/debug/config.toml` ya no existe.

- [ ] **Step 2: Revisión de contrato**

- Un solo struct `Config` (D9); el modo portable/per-user es runtime, no build.
- `core` sigue sin Win32: `%APPDATA%` llega por variable de entorno.
- La CLI resuelve `flag > config > default` y un TOML roto da error visible (exit 2), no defaults silenciosos.

- [ ] **Step 3: Actualizar roadmap**

En `roadmap.md` §2, cambiar:

```
- ⏳ Config TOML portable-first (f.4, D9).
```

por:

```
- ✅ Config TOML portable-first (f.4, D9).
```

- [ ] **Step 4: Proponer el commit al humano (NO ejecutar sin aprobación)**

Mensaje propuesto:

```
v0.1.7 — F1: config TOML portable-first

Config única (D9) con serde/toml: defaults por campo, TOML roto = error
visible, detección portable (config.toml junto al exe) vs %APPDATA% como
función pura testeada, save/load, y la CLI resolviendo flag > config >
default para dir/formato/prefijo. Verificado el modo portable con el
binario real.
```
