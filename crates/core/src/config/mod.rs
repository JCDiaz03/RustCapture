//! Slice de configuración: struct `Config` en TOML con detección de modo
//! portable vs `%APPDATA%` (D9, f.4).

use std::path::{Path, PathBuf};

use crate::output::ImageFormat;

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

#[cfg(test)]
mod tests {
    use super::*;
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
        assert_eq!(
            path,
            Path::new("C:/appdata")
                .join("RustCapture")
                .join(CONFIG_FILE)
        );
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
