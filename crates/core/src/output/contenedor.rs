//! Contenedor ZIP mínimo para el formato re-editable (f.31): se escribe SIN
//! comprimir (el PNG ya lo está y el TOML es diminuto) y se lee aceptando
//! también deflate, para que un `.rcap` recomprimido con el Explorador de
//! Windows siga abriéndose.
//!
//! Se hace a mano y no con el crate `zip` porque `crc32fast` y `flate2` ya
//! estaban en el árbol (los arrastra `png`): así el contenedor no añade nada
//! al binario, que es prioridad del proyecto (f.4/f.5).
//!
//! Se lee por el DIRECTORIO CENTRAL, no recorriendo las cabeceras locales:
//! los zips de terceros pueden usar descriptor de datos y dejar los tamaños
//! a cero en la cabecera local, y entonces el recorrido secuencial no sabe
//! cuánto avanzar.

const LOCAL: u32 = 0x0403_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const FIN: u32 = 0x0605_4b50;
/// 1980-01-01 en formato MS-DOS: la fecha 0 es inválida y algunas
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
    #[error("{0}: los datos están corruptos")]
    CrcMalo(String),
}

pub struct Miembro<'a> {
    pub nombre: &'a str,
    pub datos: &'a [u8],
}

pub fn escribir(miembros: &[Miembro]) -> Vec<u8> {
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

/// Lecturas little-endian que devuelven `None` si el buffer se queda corto,
/// para no panicar con un archivo truncado o malicioso.
fn u16_en(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?))
}

fn u32_en(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

pub fn leer(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, ContenedorError> {
    // El EOCD está al final y puede llevar comentario: se busca su firma
    // hacia atrás.
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
            bytes
                .get(pos + 46..pos + 46 + n)
                .ok_or(ContenedorError::Truncado)?,
        )
        .into_owned();
        pos += 46 + n + extra + comentario;

        // Los datos empiezan tras la cabecera LOCAL, cuyo campo `extra`
        // puede diferir del de la central.
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
            Miembro {
                nombre: "imagen.png",
                datos: &a,
            },
            Miembro {
                nombre: "documento.toml",
                datos: &b,
            },
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
        let zip = escribir(&[Miembro {
            nombre: "vacio",
            datos: &[],
        }]);
        assert_eq!(leer(&zip).unwrap(), vec![("vacio".to_string(), vec![])]);
    }

    #[test]
    fn lo_que_no_es_zip_se_rechaza() {
        assert_eq!(leer(b"").unwrap_err(), ContenedorError::NoEsZip);
        assert_eq!(leer(b"no soy un zip").unwrap_err(), ContenedorError::NoEsZip);
    }

    #[test]
    fn un_zip_truncado_no_panica() {
        let zip = escribir(&[Miembro {
            nombre: "x",
            datos: &[7; 100],
        }]);
        // Cortar por la mitad se lleva el EOCD.
        assert!(leer(&zip[..zip.len() / 2]).is_err());
        // Y cortar solo la cola del EOCD también se detecta.
        assert!(leer(&zip[..zip.len() - 4]).is_err());
    }

    #[test]
    fn un_dato_alterado_se_detecta_por_crc() {
        let mut zip = escribir(&[Miembro {
            nombre: "d",
            datos: b"hola",
        }]);
        // Los datos van tras la cabecera local (30 bytes + 1 de nombre).
        zip[31] ^= 0xFF;
        assert_eq!(
            leer(&zip).unwrap_err(),
            ContenedorError::CrcMalo("d".to_string())
        );
    }

    #[test]
    fn un_miembro_grande_va_y_vuelve_intacto() {
        // 300 KB de datos no repetitivos: comprueba los offsets de 32 bits.
        let datos: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        let zip = escribir(&[Miembro {
            nombre: "grande.bin",
            datos: &datos,
        }]);
        let leido = leer(&zip).unwrap();
        assert_eq!(leido[0].1, datos);
    }
}

