//! Adapter del puerto `HotkeyProvider` (f.3): `RegisterHotKey` global.
//!
//! Hilos: registrar y desregistrar SIEMPRE desde el hilo del bucle de
//! mensajes — `RegisterHotKey(None, ...)` entrega los `WM_HOTKEY` a la
//! cola del hilo que registró (los consume `bar::run_message_loop`).

use rustcapture_core::ports::{Hotkey, HotkeyError, HotkeyId, HotkeyProvider, KeyCode, Modifiers};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, RegisterHotKey,
    UnregisterHotKey, VK_F1, VK_SNAPSHOT,
};

/// Modificadores del core → flags `MOD_*`.
fn mods_of(m: &Modifiers) -> u32 {
    let mut mods = 0;
    if m.ctrl {
        mods |= MOD_CONTROL.0;
    }
    if m.alt {
        mods |= MOD_ALT.0;
    }
    if m.shift {
        mods |= MOD_SHIFT.0;
    }
    if m.win {
        mods |= MOD_WIN.0;
    }
    mods
}

/// Tecla del core → virtual-key code. `None` si no es representable.
fn vk_of(key: KeyCode) -> Option<u16> {
    match key {
        // Para a-z y 0-9 el VK es el ASCII en mayúscula.
        KeyCode::Char(c) if c.is_ascii_lowercase() => Some(c.to_ascii_uppercase() as u16),
        KeyCode::Char(c) if c.is_ascii_digit() => Some(c as u16),
        KeyCode::Char(_) => None,
        KeyCode::F(n) if (1..=24).contains(&n) => Some(VK_F1.0 + (n as u16 - 1)),
        KeyCode::F(_) => None,
        KeyCode::PrintScreen => Some(VK_SNAPSHOT.0),
    }
}

/// `HotkeyProvider` real. Guarda lo registrado para validar duplicados
/// y liberar por id.
pub struct Win32HotkeyProvider {
    next_id: u32,
    registered: Vec<(HotkeyId, Hotkey)>,
}

impl Win32HotkeyProvider {
    #[expect(
        clippy::new_without_default,
        reason = "simetría con el resto de adapters"
    )]
    pub fn new() -> Self {
        Self {
            next_id: 1,
            registered: Vec::new(),
        }
    }
}

impl HotkeyProvider for Win32HotkeyProvider {
    fn register(&mut self, hotkey: Hotkey) -> Result<HotkeyId, HotkeyError> {
        if self.registered.iter().any(|(_, h)| *h == hotkey) {
            return Err(HotkeyError::AlreadyRegistered(hotkey));
        }
        let vk = vk_of(hotkey.key)
            .ok_or_else(|| HotkeyError::Platform(format!("tecla no mapeable: {:?}", hotkey.key)))?;
        let id = HotkeyId(self.next_id);
        // SAFETY: hwnd None → WM_HOTKEY a la cola de este hilo; el id es
        // único dentro del proceso (contador propio).
        unsafe {
            RegisterHotKey(
                None,
                id.0 as i32,
                HOT_KEY_MODIFIERS(mods_of(&hotkey.modifiers) | MOD_NOREPEAT.0),
                vk as u32,
            )
        }
        .map_err(|e| HotkeyError::Platform(e.to_string()))?;
        self.next_id += 1;
        self.registered.push((id, hotkey));
        Ok(id)
    }

    fn unregister(&mut self, id: HotkeyId) -> Result<(), HotkeyError> {
        let pos = self
            .registered
            .iter()
            .position(|(i, _)| *i == id)
            .ok_or(HotkeyError::UnknownId(id))?;
        // SAFETY: id registrado por este provider en este hilo.
        unsafe { UnregisterHotKey(None, id.0 as i32) }
            .map_err(|e| HotkeyError::Platform(e.to_string()))?;
        self.registered.remove(pos);
        Ok(())
    }
}

impl Drop for Win32HotkeyProvider {
    fn drop(&mut self) {
        for (id, _) in &self.registered {
            // SAFETY: registrados por este provider; liberar al morir.
            unsafe { _ = UnregisterHotKey(None, id.0 as i32) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapea_modificadores_a_flags_win32() {
        let m = Modifiers {
            ctrl: true,
            alt: true,
            ..Modifiers::default()
        };
        assert_eq!(mods_of(&m), MOD_CONTROL.0 | MOD_ALT.0);
        assert_eq!(mods_of(&Modifiers::default()), 0);
    }

    #[test]
    fn mapea_teclas_a_vk() {
        assert_eq!(vk_of(KeyCode::Char('a')), Some(0x41));
        assert_eq!(vk_of(KeyCode::Char('7')), Some(0x37));
        assert_eq!(vk_of(KeyCode::F(12)), Some(VK_F1.0 + 11));
        assert_eq!(vk_of(KeyCode::PrintScreen), Some(VK_SNAPSHOT.0));
    }

    #[test]
    fn teclas_fuera_de_rango_no_mapean() {
        assert_eq!(vk_of(KeyCode::Char('ñ')), None);
        assert_eq!(vk_of(KeyCode::F(25)), None);
    }

    /// Humo: registra un atajo real improbable y lo libera.
    #[test]
    #[ignore = "registra un hotkey global real"]
    fn registrar_y_desregistrar_un_hotkey_real() {
        let mut provider = Win32HotkeyProvider::new();
        let hotkey = Hotkey::parse("ctrl+shift+f9").unwrap();
        let id = provider.register(hotkey).unwrap();
        provider.unregister(id).unwrap();
    }
}
