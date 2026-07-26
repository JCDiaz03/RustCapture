//! Formato propio re-editable `.rcap` (f.31): la captura con sus objetos de
//! anotación intactos, para poder retomarla.
//!
//! Es un ZIP sin comprimir con dos miembros:
//! - `imagen.png` — el frame BASE, sin anotar. Las anotaciones no se hornean
//!   ahí: por eso el archivo sigue siendo editable.
//! - `documento.toml` — versión, familias tipográficas usadas y los objetos.
//!
//! Las familias van por NOMBRE, no por el `FamiliaId` del catálogo: ese id
//! es un índice que depende de las fuentes instaladas en la máquina y no
//! significa nada en otra. El documento lleva su propia lista compacta y los
//! objetos indexan en ella; al abrir, `remapear_familias` los reengancha al
//! catálogo de destino.
//!
//! El core no abre archivos (D1/D2): aquí se trabaja con bytes.

use crate::annotate::objeto::{Forma, Objeto};
use crate::annotate::style::FamiliaId;
use crate::annotate::text::RenderContext;
use crate::annotate::Document;
use crate::output::contenedor::{self, ContenedorError, Miembro};
use crate::output::{ImageFormat, encode};
use crate::ports::{Frame, FrameError};

/// Versión del formato. Se sube cuando un cambio deja de ser compatible.
pub const VERSION_RCAP: u32 = 1;

const MIEMBRO_IMAGEN: &str = "imagen.png";
const MIEMBRO_DOCUMENTO: &str = "documento.toml";

#[derive(thiserror::Error, Clone, PartialEq, Debug)]
pub enum FormatoError {
    #[error("{0}")]
    Contenedor(#[from] ContenedorError),
    #[error("falta «{0}» dentro del archivo")]
    MiembroAusente(&'static str),
    #[error("{0}")]
    Imagen(#[from] FrameError),
    #[error("el documento está mal formado: {0}")]
    Documento(String),
    #[error(
        "el archivo es de una versión más nueva ({0}); esta versión entiende \
         hasta la {VERSION_RCAP}"
    )]
    VersionNoSoportada(u32),
    #[error("no se pudo generar el PNG: {0}")]
    Codificacion(String),
}

/// Lo que viaja dentro de `documento.toml`.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct DocumentoGuardado {
    pub version: u32,
    /// Nombres de las familias usadas; los objetos indexan aquí.
    pub familias: Vec<String>,
    pub objetos: Vec<Objeto>,
}

impl DocumentoGuardado {
    /// Reengancha las familias al catálogo de destino. `mapa[i]` es el
    /// `FamiliaId` que le corresponde a `familias[i]`.
    pub fn remapear_familias(&mut self, mapa: &[FamiliaId]) {
        for objeto in &mut self.objetos {
            if let Forma::Texto(t) = &mut objeto.forma {
                // Un índice fuera del mapa cae a la familia de respaldo, que
                // es mejor que dejar el texto sin fuente.
                t.style.familia = mapa
                    .get(t.style.familia.0 as usize)
                    .copied()
                    .unwrap_or_default();
            }
        }
    }

    pub fn en_documento(self) -> Document {
        Document::from_objetos(self.objetos)
    }
}

/// Empaqueta la base sin anotar y el documento en bytes `.rcap`.
pub fn empaquetar(
    base: &Frame,
    doc: &Document,
    ctx: &RenderContext,
) -> Result<Vec<u8>, FormatoError> {
    // Recolectar SOLO las familias referenciadas y compactarlas, para que el
    // archivo no arrastre las 228 del catálogo.
    let mut familias: Vec<String> = Vec::new();
    let mut objetos = doc.objetos().to_vec();
    for objeto in &mut objetos {
        if let Forma::Texto(t) = &mut objeto.forma {
            let nombre = ctx.nombre(t.style.familia).unwrap_or("Segoe UI").to_string();
            let indice = match familias.iter().position(|n| *n == nombre) {
                Some(i) => i,
                None => {
                    familias.push(nombre);
                    familias.len() - 1
                }
            };
            t.style.familia = FamiliaId(indice as u16);
        }
    }

    let guardado = DocumentoGuardado {
        version: VERSION_RCAP,
        familias,
        objetos,
    };
    let toml = toml::to_string(&guardado).map_err(|e| FormatoError::Documento(e.to_string()))?;
    let png = encode(base, ImageFormat::Png).map_err(|e| FormatoError::Codificacion(e.to_string()))?;
    Ok(contenedor::escribir(&[
        Miembro {
            nombre: MIEMBRO_IMAGEN,
            datos: &png,
        },
        Miembro {
            nombre: MIEMBRO_DOCUMENTO,
            datos: toml.as_bytes(),
        },
    ]))
}

/// Lee unos bytes `.rcap`. Devuelve la base y el documento con las familias
/// AÚN sin remapear (el catálogo lo pone quien tiene acceso a las fuentes).
pub fn desempaquetar(bytes: &[u8]) -> Result<(Frame, DocumentoGuardado), FormatoError> {
    let miembros = contenedor::leer(bytes)?;
    let buscar = |nombre: &'static str| {
        miembros
            .iter()
            .find(|(n, _)| n == nombre)
            .map(|(_, d)| d.as_slice())
            .ok_or(FormatoError::MiembroAusente(nombre))
    };
    let base = Frame::from_png(buscar(MIEMBRO_IMAGEN)?)?;
    let texto = String::from_utf8_lossy(buscar(MIEMBRO_DOCUMENTO)?).into_owned();
    let guardado: DocumentoGuardado =
        toml::from_str(&texto).map_err(|e| FormatoError::Documento(e.to_string()))?;
    if guardado.version > VERSION_RCAP {
        return Err(FormatoError::VersionNoSoportada(guardado.version));
    }
    Ok((base, guardado))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annotate::annotations::{
        ArrowAnnotation, PixelateAnnotation, RectAnnotation, StepAnnotation, TextAnnotation,
    };
    use crate::annotate::style::{CensorMode, Color, Style, TextStyle};
    use crate::annotate::{Command, History};
    use crate::ports::Rect;

    const ESTILO: Style = Style {
        color: Color::rgb(255, 0, 0),
        thickness: 3,
    };

    fn ttf(archivo: &str) -> Vec<u8> {
        std::fs::read(format!("C:/Windows/Fonts/{archivo}")).expect("fuente del sistema")
    }

    fn ctx_con(nombres: &[(&str, &str)]) -> RenderContext {
        let mut ctx = RenderContext::nueva();
        for (nombre, archivo) in nombres {
            let id = ctx.registrar_familia(nombre);
            ctx.cargar_cara(id, false, &ttf(archivo)).unwrap();
        }
        ctx
    }

    /// Documento con un objeto de cada familia de rasterizado, uno girado.
    fn documento(ctx: &RenderContext) -> Document {
        let familia = ctx.familias().first().map(|(id, _)| *id).unwrap_or_default();
        let mut doc = Document::new();
        let mut h = History::new();
        for o in [
            Objeto::from(RectAnnotation {
                rect: Rect::new(2, 3, 10, 8),
                style: ESTILO,
            }),
            Objeto::from(ArrowAnnotation {
                from: (1, 1),
                to: (20, 15),
                style: ESTILO,
            }),
            Objeto::from(PixelateAnnotation {
                rect: Rect::new(5, 5, 12, 12),
                mode: CensorMode::Blur { radius: 4 },
            }),
            Objeto::from(StepAnnotation {
                center: (25, 20),
                number: 7,
                color: Color::rgb(0xD8, 0x3B, 0x01),
                font_size: 20.0,
            }),
            Objeto::from(TextAnnotation {
                pos: (3, 20),
                text: "Hola\nmundo".to_string(),
                style: TextStyle {
                    color: Color::rgb(9, 8, 7),
                    size: 18.0,
                    bold: true,
                    familia,
                },
            }),
        ] {
            h.apply(&mut doc, Command::add(o));
        }
        // Y uno girado, que es lo que ejercita el Giro serializado.
        h.apply(&mut doc, Command::rotate_by(0, 0.6));
        doc
    }

    #[test]
    fn un_documento_va_y_vuelve_entero() {
        let ctx = ctx_con(&[("Segoe UI", "segoeui.ttf")]);
        let base = Frame::filled(40, 30, [10, 20, 30, 255]);
        let doc = documento(&ctx);

        let bytes = empaquetar(&base, &doc, &ctx).unwrap();
        let (base2, guardado) = desempaquetar(&bytes).unwrap();

        assert_eq!(base2, base, "la imagen base no sobrevivió");
        assert_eq!(guardado.version, VERSION_RCAP);
        assert_eq!(guardado.objetos.len(), doc.len());

        // Lo horneado tiene que ser idéntico píxel a píxel.
        let mut original = base.clone();
        doc.render_onto(&mut original, &ctx);
        let mut recargado = base.clone();
        let mut g = guardado;
        g.remapear_familias(&[FamiliaId(0)]);
        g.en_documento().render_onto(&mut recargado, &ctx);
        assert_eq!(original, recargado, "lo pintado cambió al ir y volver");
    }

    #[test]
    fn las_familias_se_guardan_por_nombre_y_solo_las_usadas() {
        // Un id de catálogo no significa nada en otra máquina, y el archivo
        // no debe arrastrar familias que nadie usa.
        let ctx = ctx_con(&[("Segoe UI", "segoeui.ttf"), ("Consolas", "consola.ttf")]);
        let usada = ctx.familias()[1].0; // Consolas
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
        let bytes = empaquetar(&Frame::filled(8, 8, [0, 0, 0, 255]), &doc, &ctx).unwrap();
        let (_, guardado) = desempaquetar(&bytes).unwrap();
        assert_eq!(guardado.familias, vec!["Consolas".to_string()]);
        // Y el objeto quedó apuntando al índice 0 de esa lista compacta.
        let Forma::Texto(t) = &guardado.objetos[0].forma else {
            panic!("no es texto");
        };
        assert_eq!(t.style.familia, FamiliaId(0));
    }

    #[test]
    fn remapear_reengancha_al_catalogo_de_destino() {
        let mut g = DocumentoGuardado {
            version: VERSION_RCAP,
            familias: vec!["Consolas".into()],
            objetos: vec![
                TextAnnotation {
                    pos: (0, 0),
                    text: "x".into(),
                    style: TextStyle {
                        color: Color::rgb(0, 0, 0),
                        size: 10.0,
                        bold: false,
                        familia: FamiliaId(0),
                    },
                }
                .into(),
            ],
        };
        // En la máquina de destino Consolas es la familia 42.
        g.remapear_familias(&[FamiliaId(42)]);
        let Forma::Texto(t) = &g.objetos[0].forma else {
            panic!()
        };
        assert_eq!(t.style.familia, FamiliaId(42));

        // Un índice que no está en el mapa cae a la de respaldo.
        g.remapear_familias(&[]);
        let Forma::Texto(t) = &g.objetos[0].forma else {
            panic!()
        };
        assert_eq!(t.style.familia, FamiliaId::default());
    }

    fn zip_con_documento(toml: &str) -> Vec<u8> {
        let png = encode(
            &Frame::filled(2, 2, [0, 0, 0, 255]),
            ImageFormat::Png,
        )
        .unwrap();
        contenedor::escribir(&[
            Miembro {
                nombre: MIEMBRO_IMAGEN,
                datos: &png,
            },
            Miembro {
                nombre: MIEMBRO_DOCUMENTO,
                datos: toml.as_bytes(),
            },
        ])
    }

    #[test]
    fn una_version_futura_se_rechaza_con_mensaje() {
        let zip = zip_con_documento(&format!(
            "version = {}\nfamilias = []\nobjetos = []\n",
            VERSION_RCAP + 1
        ));
        let err = desempaquetar(&zip).unwrap_err();
        assert!(matches!(err, FormatoError::VersionNoSoportada(v) if v == VERSION_RCAP + 1));
        // El mensaje tiene que decirle al usuario qué pasa.
        assert!(err.to_string().contains("más nueva"), "{err}");
    }

    #[test]
    fn un_documento_vacio_es_valido() {
        let zip = zip_con_documento("version = 1\nfamilias = []\nobjetos = []\n");
        let (base, g) = desempaquetar(&zip).unwrap();
        assert_eq!((base.width, base.height), (2, 2));
        assert!(g.objetos.is_empty());
    }

    #[test]
    fn un_toml_roto_da_error_de_documento() {
        let zip = zip_con_documento("esto no es = = toml\n");
        assert!(matches!(
            desempaquetar(&zip).unwrap_err(),
            FormatoError::Documento(_)
        ));
    }

    #[test]
    fn un_rcap_sin_sus_miembros_da_error_claro() {
        let zip = contenedor::escribir(&[Miembro {
            nombre: "otra.cosa",
            datos: b"x",
        }]);
        assert_eq!(
            desempaquetar(&zip).unwrap_err(),
            FormatoError::MiembroAusente(MIEMBRO_IMAGEN)
        );
    }

    #[test]
    fn un_png_que_no_es_png_da_error_de_imagen() {
        let zip = contenedor::escribir(&[
            Miembro {
                nombre: MIEMBRO_IMAGEN,
                datos: b"no soy un png",
            },
            Miembro {
                nombre: MIEMBRO_DOCUMENTO,
                datos: b"version = 1\nfamilias = []\nobjetos = []\n",
            },
        ]);
        assert!(matches!(
            desempaquetar(&zip).unwrap_err(),
            FormatoError::Imagen(_)
        ));
    }

    #[test]
    fn lo_que_no_es_un_rcap_se_rechaza_sin_panicar() {
        assert!(desempaquetar(b"").is_err());
        assert!(desempaquetar(b"\x89PNG\r\n\x1a\n cualquier cosa").is_err());
    }
}
