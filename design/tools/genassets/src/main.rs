//! Generador offline de assets del rediseño (F3.5).
//!
//! Entrada:  design/icons/*.svg (fuente de verdad de los iconos de toolbar)
//!           design/icons/app/app-icon-{16,24,32,48,256}.png (icono de app)
//! Salida:   crates/platform-win/src/ui/iconos/atlas_{16,20,24,28,32}.bin
//!           crates/platform-win/src/ui/iconos/atlas.rs (enum Icono generado)
//!           crates/gui/assets/rustcapture.ico
//!
//! Los archivos generados se commitean: el build del producto no necesita
//! este tool ni sus dependencias. Ejecutar con `cargo run` desde este
//! directorio tras cambiar un SVG o el inventario de ICONOS.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

/// Tallas físicas del atlas: 16/20/24/28/32 px cubren DPI 100/125/150/175/200 %.
const TALLAS: [u32; 5] = [16, 20, 24, 28, 32];

/// Inventario de iconos de esta fase, en el orden del enum generado.
/// Añadir aquí (al final, para no invalidar índices) y regenerar.
const ICONOS: [&str; 40] = [
    // Barra
    "sys-drag-handle",
    "capture-fullscreen",
    "capture-window",
    "capture-region",
    "capture-delay",
    "capture-object",
    "capture-freehand",
    "capture-fixed",
    "capture-scroll",
    "record-start",
    "util-eyedropper",
    "util-magnifier",
    "util-ruler",
    "util-crosshair",
    "util-pin",
    "sys-settings",
    "sys-collapse",
    "sys-close",
    // Editor
    "annotate-select",
    "annotate-text",
    "annotate-arrow",
    "annotate-line",
    "annotate-shape",
    "annotate-ellipse",
    "annotate-pencil",
    "annotate-highlight",
    "annotate-steps",
    "annotate-caption",
    "annotate-pixelate",
    "annotate-eraser",
    "annotate-crop",
    "edit-resize",
    "edit-undo",
    "edit-redo",
    "output-copy",
    "output-save",
    "output-save-as",
    "output-print",
    "output-email",
    // Asa de rotación del objeto seleccionado (f.53).
    "edit-rotate",
];

/// PNG del icono de app que entran en el .ico (lado, ¿se embebe como PNG?).
/// 256 va como PNG embebido (estándar de Vista+); el resto como DIB clásico.
const TAMANOS_ICO: [(u32, bool); 5] =
    [(16, false), (24, false), (32, false), (48, false), (256, true)];

fn main() -> Result<(), Box<dyn Error>> {
    let raiz = raiz_repo();
    let dir_svg = raiz.join("design").join("icons");
    let dir_atlas = raiz
        .join("crates")
        .join("platform-win")
        .join("src")
        .join("ui")
        .join("iconos");
    fs::create_dir_all(&dir_atlas)?;

    for talla in TALLAS {
        let mut blob = Vec::with_capacity(ICONOS.len() * (talla * talla) as usize);
        for nombre in ICONOS {
            let ruta = dir_svg.join(format!("{nombre}.svg"));
            let svg = fs::read_to_string(&ruta)
                .map_err(|e| format!("no se pudo leer {}: {e}", ruta.display()))?;
            let mascara = rasterizar_a8(&svg, talla)?;
            if mascara.iter().all(|&b| b == 0) {
                return Err(format!("máscara vacía: {nombre} a {talla} px").into());
            }
            blob.extend_from_slice(&mascara);
        }
        fs::write(dir_atlas.join(format!("atlas_{talla}.bin")), &blob)?;
        println!("atlas_{talla}.bin: {} iconos, {} bytes", ICONOS.len(), blob.len());
    }

    fs::write(dir_atlas.join("atlas.rs"), generar_atlas_rs())?;
    println!("atlas.rs: enum Icono con {} variantes", ICONOS.len());

    let dir_app = dir_svg.join("app");
    let entradas: Vec<EntradaIco> = TAMANOS_ICO
        .iter()
        .map(|&(lado, como_png)| {
            let crudo = fs::read(dir_app.join(format!("app-icon-{lado}.png")))?;
            Ok(if como_png {
                EntradaIco { lado, datos: DatosIco::Png(crudo) }
            } else {
                EntradaIco { lado, datos: DatosIco::Rgba(decodificar_png(&crudo, lado)?) }
            })
        })
        .collect::<Result<_, Box<dyn Error>>>()?;
    let ico = construir_ico(&entradas);
    let dir_ico = raiz.join("crates").join("gui").join("assets");
    fs::create_dir_all(&dir_ico)?;
    fs::write(dir_ico.join("rustcapture.ico"), &ico)?;
    println!("rustcapture.ico: {} imágenes, {} bytes", entradas.len(), ico.len());

    Ok(())
}

/// Raíz del repo: este crate vive en design/tools/genassets.
fn raiz_repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("design/tools/genassets debe colgar de la raíz del repo")
        .to_path_buf()
}

/// Rasteriza un SVG de rejilla 16×16 a una máscara de cobertura A8 de lado×lado.
/// El color no importa (los iconos son currentColor → negro): solo se conserva
/// el canal alpha, que lleva el antialiasing horneado.
fn rasterizar_a8(svg: &str, lado: u32) -> Result<Vec<u8>, Box<dyn Error>> {
    let arbol = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default())?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(lado, lado).ok_or("pixmap inválido")?;
    let escala = lado as f32 / 16.0;
    resvg::render(
        &arbol,
        resvg::tiny_skia::Transform::from_scale(escala, escala),
        &mut pixmap.as_mut(),
    );
    Ok(pixmap.data().chunks_exact(4).map(|px| px[3]).collect())
}

fn generar_atlas_rs() -> String {
    let mut s = String::new();
    s.push_str("// GENERADO POR design/tools/genassets — NO EDITAR.\n");
    s.push_str("// Regenerar: cd design/tools/genassets && cargo run\n\n");
    s.push_str("/// Iconos disponibles en los atlas A8. El discriminante es el índice\n");
    s.push_str("/// dentro de cada `atlas_N.bin` (offset del icono = índice × lado²).\n");
    s.push_str("// El inventario va por delante de los consumidores a propósito\n");
    s.push_str("// (iconos de fases futuras ya generados).\n");
    s.push_str("#[allow(dead_code)]\n");
    s.push_str("#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]\n");
    s.push_str("#[repr(usize)]\n");
    s.push_str("pub enum Icono {\n");
    for (i, nombre) in ICONOS.iter().enumerate() {
        s.push_str(&format!("    {} = {},\n", kebab_a_camel(nombre), i));
    }
    s.push_str("}\n\n");
    s.push_str(&format!("pub const NUM_ICONOS: usize = {};\n", ICONOS.len()));
    s.push_str("pub const TALLAS: [u32; 5] = [16, 20, 24, 28, 32];\n");
    s
}

fn kebab_a_camel(kebab: &str) -> String {
    kebab
        .split('-')
        .map(|parte| {
            let mut c = parte.chars();
            match c.next() {
                Some(inicial) => inicial.to_ascii_uppercase().to_string() + c.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

struct EntradaIco {
    lado: u32,
    datos: DatosIco,
}

enum DatosIco {
    /// RGBA 8888 top-down, lado×lado — se codifica como DIB clásico.
    Rgba(Vec<u8>),
    /// Bytes PNG tal cual (entrada de 256 px).
    Png(Vec<u8>),
}

fn decodificar_png(crudo: &[u8], lado_esperado: u32) -> Result<Vec<u8>, Box<dyn Error>> {
    let decoder = png::Decoder::new(std::io::Cursor::new(crudo));
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0u8; reader.output_buffer_size().ok_or("png sin tamaño")?];
    let info = reader.next_frame(&mut buf)?;
    if info.width != lado_esperado || info.height != lado_esperado {
        return Err(format!(
            "png de {}×{}, se esperaba {lado_esperado}×{lado_esperado}",
            info.width, info.height
        )
        .into());
    }
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err("el png del icono debe ser RGBA de 8 bits".into());
    }
    buf.truncate(info.buffer_size());
    Ok(buf)
}

/// Construye un .ico multiimagen: cabecera ICONDIR + directorio + imágenes.
fn construir_ico(entradas: &[EntradaIco]) -> Vec<u8> {
    const CABECERA: usize = 6;
    const ENTRADA_DIR: usize = 16;
    let imagenes: Vec<Vec<u8>> = entradas
        .iter()
        .map(|e| match &e.datos {
            DatosIco::Png(bytes) => bytes.clone(),
            DatosIco::Rgba(rgba) => codificar_dib(rgba, e.lado),
        })
        .collect();

    let mut ico = Vec::new();
    ico.extend_from_slice(&0u16.to_le_bytes()); // reservado
    ico.extend_from_slice(&1u16.to_le_bytes()); // tipo: icono
    ico.extend_from_slice(&(entradas.len() as u16).to_le_bytes());

    let mut offset = CABECERA + ENTRADA_DIR * entradas.len();
    for (e, img) in entradas.iter().zip(&imagenes) {
        let lado_dir = if e.lado >= 256 { 0u8 } else { e.lado as u8 };
        ico.push(lado_dir); // ancho (0 = 256)
        ico.push(lado_dir); // alto
        ico.push(0); // colores de paleta
        ico.push(0); // reservado
        ico.extend_from_slice(&1u16.to_le_bytes()); // planos
        ico.extend_from_slice(&32u16.to_le_bytes()); // bits por píxel
        ico.extend_from_slice(&(img.len() as u32).to_le_bytes());
        ico.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += img.len();
    }
    for img in &imagenes {
        ico.extend_from_slice(img);
    }
    ico
}

/// DIB clásico de icono: BITMAPINFOHEADER (alto doble), XOR BGRA bottom-up
/// y máscara AND a cero (el alpha de 32 bpp manda desde Windows XP).
fn codificar_dib(rgba: &[u8], lado: u32) -> Vec<u8> {
    let l = lado as usize;
    assert_eq!(rgba.len(), l * l * 4);
    let fila_and = l.div_ceil(32) * 4; // filas de la máscara AND alineadas a 32 bits
    let mut dib = Vec::with_capacity(40 + l * l * 4 + fila_and * l);
    dib.extend_from_slice(&40u32.to_le_bytes()); // biSize
    dib.extend_from_slice(&(lado as i32).to_le_bytes()); // biWidth
    dib.extend_from_slice(&((lado * 2) as i32).to_le_bytes()); // biHeight (XOR+AND)
    dib.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    dib.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    dib.extend_from_slice(&[0u8; 24]); // compresión, tamaños y resto a cero
    for fila in (0..l).rev() {
        for col in 0..l {
            let p = (fila * l + col) * 4;
            dib.extend_from_slice(&[rgba[p + 2], rgba[p + 1], rgba[p], rgba[p + 3]]);
        }
    }
    dib.extend(std::iter::repeat_n(0u8, fila_and * l));
    dib
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kebab_a_camel_convierte_nombres_del_set() {
        assert_eq!(kebab_a_camel("capture-fullscreen"), "CaptureFullscreen");
        assert_eq!(kebab_a_camel("sys-drag-handle"), "SysDragHandle");
        assert_eq!(kebab_a_camel("annotate-save-as"), "AnnotateSaveAs");
    }

    #[test]
    fn el_enum_generado_lista_todos_los_iconos_en_orden() {
        let src = generar_atlas_rs();
        assert!(src.contains("SysDragHandle = 0,"));
        assert!(src.contains(&format!("OutputEmail = {},", ICONOS.len() - 1)));
        assert!(src.contains(&format!("NUM_ICONOS: usize = {};", ICONOS.len())));
    }

    #[test]
    fn rasterizar_devuelve_cobertura_del_tamano_pedido() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><rect x="2" y="2" width="12" height="12" fill="#000"/></svg>"##;
        for talla in TALLAS {
            let m = rasterizar_a8(svg, talla).unwrap();
            assert_eq!(m.len(), (talla * talla) as usize);
            // centro opaco, esquina transparente
            let centro = (talla / 2 * talla + talla / 2) as usize;
            assert_eq!(m[centro], 255);
            assert_eq!(m[0], 0);
        }
    }

    #[test]
    fn el_ico_tiene_cabecera_directorio_y_offsets_consistentes() {
        let rgba4: Vec<u8> = (0..4 * 4 * 4).map(|i| i as u8).collect();
        let png_falso = vec![0x89, b'P', b'N', b'G', 1, 2, 3];
        let entradas = vec![
            EntradaIco { lado: 4, datos: DatosIco::Rgba(rgba4) },
            EntradaIco { lado: 256, datos: DatosIco::Png(png_falso.clone()) },
        ];
        let ico = construir_ico(&entradas);

        assert_eq!(&ico[0..6], &[0, 0, 1, 0, 2, 0]); // reservado, tipo 1, 2 imágenes
        let entrada = |i: usize| &ico[6 + i * 16..6 + (i + 1) * 16];
        let u32_en = |b: &[u8], p: usize| u32::from_le_bytes(b[p..p + 4].try_into().unwrap());

        // Entrada 0: 4×4 DIB — 40 cabecera + 64 XOR + 16 AND (4 filas de 4 bytes)
        assert_eq!(entrada(0)[0], 4);
        let tam0 = u32_en(entrada(0), 8) as usize;
        let off0 = u32_en(entrada(0), 12) as usize;
        assert_eq!(tam0, 40 + 64 + 16);
        assert_eq!(off0, 6 + 2 * 16);

        // Entrada 1: 256 se codifica como 0 y es el PNG tal cual a continuación
        assert_eq!(entrada(1)[0], 0);
        let tam1 = u32_en(entrada(1), 8) as usize;
        let off1 = u32_en(entrada(1), 12) as usize;
        assert_eq!(off1, off0 + tam0);
        assert_eq!(&ico[off1..off1 + tam1], &png_falso[..]);
        assert_eq!(ico.len(), off1 + tam1);
    }

    #[test]
    fn el_dib_dobla_el_alto_y_reordena_a_bgra_bottom_up() {
        // 1×1 rojo semitransparente: RGBA (255, 0, 0, 128)
        let dib = codificar_dib(&[255, 0, 0, 128], 1);
        assert_eq!(u32::from_le_bytes(dib[0..4].try_into().unwrap()), 40);
        assert_eq!(i32::from_le_bytes(dib[8..12].try_into().unwrap()), 2); // alto doble
        assert_eq!(&dib[40..44], &[0, 0, 255, 128]); // BGRA
        assert_eq!(dib.len(), 40 + 4 + 4); // cabecera + XOR + fila AND alineada
    }
}
