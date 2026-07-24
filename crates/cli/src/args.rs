//! Parsing de flags (f.8): puro y testeable; los errores devuelven un
//! mensaje listo para stderr.

use std::ffi::OsString;
use std::path::PathBuf;

use rustcapture_core::capture::ModeRequest;
use rustcapture_core::output::ImageFormat;
use rustcapture_core::ports::Rect;

pub const USAGE: &str = "\
rustcapture — captura de pantalla (MVP F1)

USO:
  rustcapture [MODO] [DESTINO]

MODOS (por defecto: --fullscreen):
  --fullscreen            escritorio virtual completo
  --window                ventana activa
  --region X,Y,WxH        región en coordenadas de escritorio (p. ej. 0,0,800x600)

DESTINO (por defecto: --clipboard):
  --clipboard             copiar al portapapeles
  --file                  guardar en archivo con nombre automático
  --dir RUTA              directorio de destino (con --file; por defecto: .)
  --format png|jpg        formato de archivo (con --file; por defecto: png)
";

#[derive(Debug, PartialEq)]
pub struct CliOptions {
    pub mode: ModeRequest,
    pub destination: Destination,
}

#[derive(Debug, PartialEq)]
pub enum Destination {
    Clipboard,
    File { dir: PathBuf, format: ImageFormat },
}

pub fn parse(raw: Vec<OsString>) -> Result<CliOptions, String> {
    let mut args = pico_args::Arguments::from_vec(raw);

    let fullscreen = args.contains("--fullscreen");
    let window = args.contains("--window");
    let region: Option<String> = args
        .opt_value_from_str("--region")
        .map_err(|e| e.to_string())?;
    let mode = match (fullscreen, window, &region) {
        (_, false, None) => ModeRequest::Fullscreen,
        (false, true, None) => ModeRequest::ActiveWindow,
        (false, false, Some(spec)) => ModeRequest::Region(parse_region(spec)?),
        _ => return Err("elige un único modo: --fullscreen, --window o --region".into()),
    };

    let clipboard = args.contains("--clipboard");
    let file = args.contains("--file");
    let dir: Option<PathBuf> = args
        .opt_value_from_str("--dir")
        .map_err(|e| e.to_string())?;
    let format_raw: Option<String> = args
        .opt_value_from_str("--format")
        .map_err(|e| e.to_string())?;

    let destination = match (clipboard, file) {
        (true, true) => return Err("elige un único destino: --clipboard o --file".into()),
        (_, false) if dir.is_some() || format_raw.is_some() => {
            return Err("--dir y --format solo aplican con --file".into());
        }
        (_, false) => Destination::Clipboard,
        (false, true) => {
            let format = match format_raw.as_deref() {
                None | Some("png") => ImageFormat::Png,
                Some("jpg") | Some("jpeg") => ImageFormat::Jpeg,
                Some(otro) => return Err(format!("formato desconocido: {otro} (png|jpg)")),
            };
            Destination::File {
                dir: dir.unwrap_or_else(|| PathBuf::from(".")),
                format,
            }
        }
    };

    let sobrantes = args.finish();
    if !sobrantes.is_empty() {
        return Err(format!("argumentos no reconocidos: {sobrantes:?}"));
    }
    Ok(CliOptions { mode, destination })
}

/// "X,Y,WxH" → `Rect` (X e Y admiten negativos, multi-monitor).
fn parse_region(spec: &str) -> Result<Rect, String> {
    let err = || format!("región inválida: {spec} (esperado X,Y,WxH)");
    let mut partes = spec.split(',');
    let x: i32 = partes
        .next()
        .and_then(|s| s.trim().parse().ok())
        .ok_or_else(err)?;
    let y: i32 = partes
        .next()
        .and_then(|s| s.trim().parse().ok())
        .ok_or_else(err)?;
    let tam = partes.next().ok_or_else(err)?;
    if partes.next().is_some() {
        return Err(err());
    }
    let (w, h) = tam.trim().split_once('x').ok_or_else(err)?;
    let width: u32 = w.parse().map_err(|_| err())?;
    let height: u32 = h.parse().map_err(|_| err())?;
    Ok(Rect::new(x, y, width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<CliOptions, String> {
        parse(args.iter().map(OsString::from).collect())
    }

    #[test]
    fn sin_flags_es_fullscreen_a_portapapeles() {
        let opts = p(&[]).unwrap();
        assert_eq!(opts.mode, ModeRequest::Fullscreen);
        assert_eq!(opts.destination, Destination::Clipboard);
    }

    #[test]
    fn window_y_region_mapean_a_sus_modos() {
        assert_eq!(p(&["--window"]).unwrap().mode, ModeRequest::ActiveWindow);
        assert_eq!(
            p(&["--region", "10,-20,300x200"]).unwrap().mode,
            ModeRequest::Region(Rect::new(10, -20, 300, 200))
        );
    }

    #[test]
    fn file_con_dir_y_format() {
        let opts = p(&["--file", "--dir", "C:/caps", "--format", "jpg"]).unwrap();
        assert_eq!(
            opts.destination,
            Destination::File {
                dir: PathBuf::from("C:/caps"),
                format: ImageFormat::Jpeg
            }
        );
    }

    #[test]
    fn file_sin_dir_usa_el_directorio_actual_y_png() {
        let opts = p(&["--file"]).unwrap();
        assert_eq!(
            opts.destination,
            Destination::File {
                dir: PathBuf::from("."),
                format: ImageFormat::Png
            }
        );
    }

    #[test]
    fn dos_modos_a_la_vez_es_error() {
        assert!(p(&["--fullscreen", "--window"]).is_err());
    }

    #[test]
    fn region_mal_formada_es_error() {
        assert!(p(&["--region", "10,20"]).is_err());
        assert!(p(&["--region", "a,b,cxd"]).is_err());
    }

    #[test]
    fn format_sin_file_es_error() {
        assert!(p(&["--format", "png"]).is_err());
    }

    #[test]
    fn flag_desconocida_es_error() {
        assert!(p(&["--sepia"]).is_err());
    }
}
