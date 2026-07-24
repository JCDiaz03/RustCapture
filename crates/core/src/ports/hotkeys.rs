//! Puerto de atajos globales (D2, f.3). El trait cubre solo el registro;
//! las pulsaciones llegan como eventos por el canal mpsc del orquestador
//! (D7), con el que se construye cada adapter.

/// Teclas modificadoras. `win` es la tecla Windows.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

/// Tecla principal del atajo, independiente de códigos VK de Win32;
/// el adapter hace el mapeo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyCode {
    /// Letra o dígito, en minúscula ('a'..'z', '0'..'9').
    Char(char),
    /// Tecla de función F1..F24.
    F(u8),
    PrintScreen,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Hotkey {
    pub modifiers: Modifiers,
    pub key: KeyCode,
}

impl Hotkey {
    /// Parsea "ctrl+shift+printscreen" (config f.3). Insensible a
    /// mayúsculas y espacios; exactamente una tecla final.
    pub fn parse(spec: &str) -> Result<Hotkey, String> {
        fn poner(k: KeyCode, key: &mut Option<KeyCode>, spec: &str) -> Result<(), String> {
            if key.replace(k).is_some() {
                return Err(format!("más de una tecla final en \"{spec}\""));
            }
            Ok(())
        }
        let mut modifiers = Modifiers::default();
        let mut key: Option<KeyCode> = None;
        for token in spec.split('+') {
            let t = token.trim().to_ascii_lowercase();
            match t.as_str() {
                "ctrl" => modifiers.ctrl = true,
                "alt" => modifiers.alt = true,
                "shift" => modifiers.shift = true,
                "win" => modifiers.win = true,
                "printscreen" | "prtscn" => poner(KeyCode::PrintScreen, &mut key, spec)?,
                t if t.len() == 1 && t.chars().all(|c| c.is_ascii_alphanumeric()) => poner(
                    KeyCode::Char(t.chars().next().expect("len 1")),
                    &mut key,
                    spec,
                )?,
                t if t.starts_with('f')
                    && t[1..].parse::<u8>().is_ok_and(|n| (1..=24).contains(&n)) =>
                {
                    poner(
                        KeyCode::F(t[1..].parse().expect("validado")),
                        &mut key,
                        spec,
                    )?
                }
                otro => return Err(format!("token desconocido en \"{spec}\": \"{otro}\"")),
            }
        }
        key.map(|key| Hotkey { modifiers, key })
            .ok_or_else(|| format!("falta la tecla final en \"{spec}\""))
    }
}

/// Identificador opaco que asigna el provider al registrar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HotkeyId(pub u32);

#[derive(thiserror::Error, Clone, PartialEq, Eq, Debug)]
pub enum HotkeyError {
    #[error("el atajo {0:?} ya está registrado")]
    AlreadyRegistered(Hotkey),
    #[error("id de atajo desconocido: {0:?}")]
    UnknownId(HotkeyId),
    /// `RegisterHotKey` falló (atajo tomado por otra app, etc.).
    #[error("fallo de plataforma: {0}")]
    Platform(String),
}

pub trait HotkeyProvider {
    fn register(&mut self, hotkey: Hotkey) -> Result<HotkeyId, HotkeyError>;
    fn unregister(&mut self, id: HotkeyId) -> Result<(), HotkeyError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsea_modificadores_y_tecla() {
        assert_eq!(
            Hotkey::parse("ctrl+shift+f").unwrap(),
            Hotkey {
                modifiers: Modifiers {
                    ctrl: true,
                    shift: true,
                    ..Modifiers::default()
                },
                key: KeyCode::Char('f'),
            }
        );
    }

    #[test]
    fn parsea_printscreen_solo_y_con_alias() {
        assert_eq!(
            Hotkey::parse("printscreen").unwrap().key,
            KeyCode::PrintScreen
        );
        assert_eq!(
            Hotkey::parse("Alt + PrtScn").unwrap().key,
            KeyCode::PrintScreen
        );
        assert!(Hotkey::parse("alt+prtscn").unwrap().modifiers.alt);
    }

    #[test]
    fn parsea_teclas_de_funcion_y_digitos() {
        assert_eq!(Hotkey::parse("win+f12").unwrap().key, KeyCode::F(12));
        assert_eq!(Hotkey::parse("ctrl+7").unwrap().key, KeyCode::Char('7'));
    }

    #[test]
    fn sin_tecla_final_es_error() {
        assert!(Hotkey::parse("ctrl+shift").is_err());
        assert!(Hotkey::parse("").is_err());
    }

    #[test]
    fn dos_teclas_finales_es_error() {
        assert!(Hotkey::parse("f1+f2").is_err());
    }

    #[test]
    fn token_desconocido_es_error() {
        assert!(Hotkey::parse("ctrl+ñ").is_err());
        assert!(Hotkey::parse("ctrl+f25").is_err());
    }
}
