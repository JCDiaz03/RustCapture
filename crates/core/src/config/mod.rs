//! Slice de configuración: struct `Config` en TOML con detección de modo
//! portable vs `%APPDATA%` (D9, f.4).

use std::path::{Path, PathBuf};

use crate::output::{DestinationKind, ImageFormat};

/// Configuración única de la app (D9). Campos desconocidos en el TOML se
/// ignoran: una config escrita por una versión más nueva no rompe esta.
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct Config {
    pub output: OutputConfig,
    pub hotkeys: HotkeysConfig,
    pub capture: CaptureConfig,
    pub theme: ThemeConfig,
    pub text: TextConfig,
}

/// Tipografía por defecto de la herramienta de texto (f.54). Si la familia
/// no existe en el sistema, el editor cae a Segoe UI y luego a la primera
/// del catálogo: un nombre mal escrito no deja el texto sin fuente.
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct TextConfig {
    pub familia: String,
}

impl Default for TextConfig {
    fn default() -> Self {
        Self {
            familia: "Segoe UI".to_string(),
        }
    }
}

/// Apariencia de la GUI: claro/oscuro o seguir al sistema.
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct ThemeConfig {
    pub mode: ThemeMode,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    /// Sigue el tema de aplicaciones de Windows.
    #[default]
    Auto,
    Light,
    Dark,
}

/// Parámetros de captura: retardo del botón Delay (f.17) y tamaño con el que
/// arranca la región fija (f.15), que la rueda ajusta durante la selección.
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct CaptureConfig {
    pub delay_seconds: u32,
    pub fixed_width: u32,
    pub fixed_height: u32,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            delay_seconds: 5,
            fixed_width: 800,
            fixed_height: 600,
        }
    }
}

impl CaptureConfig {
    pub fn delay_ms(&self) -> u64 {
        u64::from(self.delay_seconds) * 1_000
    }
}

/// Atajos globales (f.3) como strings "ctrl+alt+tecla"; se parsean con
/// `Hotkey::parse` al arrancar. `region` y `delay` están reservados:
/// el schema ya los conoce, se registran cuando llegue su fase (F4, F2).
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct HotkeysConfig {
    pub fullscreen: String,
    pub window: String,
    pub region: String,
    pub delay: String,
}

impl Default for HotkeysConfig {
    fn default() -> Self {
        Self {
            fullscreen: "printscreen".to_string(),
            window: "alt+printscreen".to_string(),
            region: "ctrl+printscreen".to_string(),
            delay: "ctrl+shift+printscreen".to_string(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct OutputConfig {
    /// Directorio de destino de `FileSink`.
    pub dir: PathBuf,
    pub format: ImageFormat,
    /// Prefijo de los nombres automáticos (f.41).
    pub prefix: String,
    /// Destino de las capturas de barra y hotkeys (f.1, f.3).
    pub destination: DestinationKind,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            dir: PathBuf::from("."),
            format: ImageFormat::Png,
            prefix: "captura".to_string(),
            destination: DestinationKind::Editor,
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
    fn el_retardo_por_defecto_es_cinco_segundos() {
        let config = Config::default();
        assert_eq!(config.capture.delay_seconds, 5);
        assert_eq!(config.capture.delay_ms(), 5_000);
    }

    #[test]
    fn el_retardo_se_configura_en_toml() {
        let config = Config::from_toml("[capture]\ndelay_seconds = 3\n").unwrap();
        assert_eq!(config.capture.delay_ms(), 3_000);
        // Y no arrastra los demás campos de la sección a cero.
        assert_eq!(config.capture.fixed_width, 800);
        assert_eq!(config.capture.fixed_height, 600);
    }

    #[test]
    fn el_tamano_de_la_region_fija_sale_de_la_config() {
        let config = Config::default();
        assert_eq!((config.capture.fixed_width, config.capture.fixed_height), (800, 600));
        let config =
            Config::from_toml("[capture]\nfixed_width = 1280\nfixed_height = 720\n").unwrap();
        assert_eq!((config.capture.fixed_width, config.capture.fixed_height), (1280, 720));
        // El retardo mantiene su default aunque no se mencione.
        assert_eq!(config.capture.delay_seconds, 5);
    }

    #[test]
    fn los_hotkeys_por_defecto_son_estilo_faststone_y_parsean() {
        use crate::ports::Hotkey;
        let config = Config::default();
        assert_eq!(config.hotkeys.fullscreen, "printscreen");
        assert_eq!(config.hotkeys.window, "alt+printscreen");
        assert_eq!(config.hotkeys.region, "ctrl+printscreen");
        assert_eq!(config.hotkeys.delay, "ctrl+shift+printscreen");
        for spec in [
            &config.hotkeys.fullscreen,
            &config.hotkeys.window,
            &config.hotkeys.region,
            &config.hotkeys.delay,
        ] {
            assert!(Hotkey::parse(spec).is_ok(), "default no parseable: {spec}");
        }
    }

    #[test]
    fn el_destino_por_defecto_es_editor_y_se_puede_cambiar() {
        use crate::output::DestinationKind;
        assert_eq!(
            Config::default().output.destination,
            DestinationKind::Editor
        );
        let config = Config::from_toml("[output]\ndestination = \"clipboard\"\n").unwrap();
        assert_eq!(config.output.destination, DestinationKind::Clipboard);
    }

    #[test]
    fn hotkeys_parciales_completan_con_defaults() {
        let config = Config::from_toml("[hotkeys]\nfullscreen = \"ctrl+f1\"\n").unwrap();
        assert_eq!(config.hotkeys.fullscreen, "ctrl+f1");
        assert_eq!(config.hotkeys.window, "alt+printscreen");
    }

    #[test]
    fn el_tema_por_defecto_es_auto() {
        assert_eq!(Config::default().theme.mode, ThemeMode::Auto);
        assert_eq!(Config::from_toml("").unwrap().theme.mode, ThemeMode::Auto);
        // La familia por defecto no depende de que exista la sección.
        assert_eq!(Config::default().text.familia, "Segoe UI");
        assert_eq!(Config::from_toml("").unwrap().text.familia, "Segoe UI");
        let c = Config::from_toml("[text]\nfamilia = \"Consolas\"\n").unwrap();
        assert_eq!(c.text.familia, "Consolas");
    }

    #[test]
    fn el_tema_se_configura_en_toml_en_minusculas() {
        let config = Config::from_toml("[theme]\nmode = \"dark\"\n").unwrap();
        assert_eq!(config.theme.mode, ThemeMode::Dark);
        let config = Config::from_toml("[theme]\nmode = \"light\"\n").unwrap();
        assert_eq!(config.theme.mode, ThemeMode::Light);
    }

    #[test]
    fn un_tema_desconocido_es_error() {
        assert!(matches!(
            Config::from_toml("[theme]\nmode = \"sepia\"\n").unwrap_err(),
            ConfigError::Parse(_)
        ));
    }

    #[test]
    fn el_tema_sobrevive_la_ida_y_vuelta_por_toml() {
        let mut config = Config::default();
        config.theme.mode = ThemeMode::Dark;
        assert_eq!(Config::from_toml(&config.to_toml()).unwrap(), config);
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
