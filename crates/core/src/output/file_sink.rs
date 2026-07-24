//! Sink de archivo (f.40): codifica y guarda con nombre automático (f.41).

use std::path::PathBuf;

use super::{ImageFormat, auto_name, encode};
use crate::ports::{Frame, OutputError, OutputSink};

/// Guarda cada frame como `captura_YYYY-MM-DD_HHMMSS.ext` en `dir`.
pub struct FileSink {
    dir: PathBuf,
    format: ImageFormat,
    prefix: String,
}

impl FileSink {
    /// Prefijo fijo "captura" hasta que la config (D9) lo parametrice.
    pub fn new(dir: impl Into<PathBuf>, format: ImageFormat) -> Self {
        Self {
            dir: dir.into(),
            format,
            prefix: "captura".to_string(),
        }
    }
}

/// `YYYY-MM-DD_HHMMSS` en hora local; UTC si el offset local no es
/// determinable (proceso multihilo sin TZ fiable).
fn now_stamp() -> String {
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    format!(
        "{:04}-{:02}-{:02}_{:02}{:02}{:02}",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

impl OutputSink for FileSink {
    fn id(&self) -> &'static str {
        "file"
    }

    fn deliver(&mut self, frame: &Frame) -> Result<(), OutputError> {
        let bytes = encode(frame, self.format).map_err(|e| OutputError::Failed(e.to_string()))?;
        std::fs::create_dir_all(&self.dir)
            .map_err(|e| OutputError::Failed(format!("creando {:?}: {e}", self.dir)))?;
        let name = auto_name(&self.prefix, &now_stamp(), self.format.extension(), |n| {
            self.dir.join(n).exists()
        });
        let path = self.dir.join(&name);
        std::fs::write(&path, bytes)
            .map_err(|e| OutputError::Failed(format!("escribiendo {path:?}: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Directorio temporal único por test (limpio al entrar, borrado al salir).
    fn dir_temporal(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rustcapture_test_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn deliver_crea_el_directorio_y_escribe_un_png() {
        let dir = dir_temporal("png");
        let mut sink = FileSink::new(&dir, ImageFormat::Png);
        sink.deliver(&Frame::filled(2, 2, [1, 2, 3, 255])).unwrap();

        let files: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
        assert_eq!(files.len(), 1);
        let path = files[0].as_ref().unwrap().path();
        assert_eq!(path.extension().unwrap(), "png");
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            &bytes[..8],
            &[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dos_delivers_seguidos_no_se_pisan() {
        let dir = dir_temporal("dobles");
        let mut sink = FileSink::new(&dir, ImageFormat::Jpeg);
        sink.deliver(&Frame::filled(1, 1, [9, 9, 9, 255])).unwrap();
        sink.deliver(&Frame::filled(1, 1, [9, 9, 9, 255])).unwrap();
        // Mismo segundo → mismo stamp → auto_name debe sufijar.
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
