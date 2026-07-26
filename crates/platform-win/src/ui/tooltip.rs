//! Tooltips nativos (comctl32) para los botones de icono: "Nombre (Hotkey)".

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Controls::{
    ICC_WIN95_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx, TOOLTIPS_CLASSW, TTF_IDISHWND,
    TTF_SUBCLASS, TTM_ADDTOOLW, TTS_ALWAYSTIP, TTS_NOPREFIX, TTTOOLINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CW_USEDEFAULT, CreateWindowExW, DestroyWindow, SendMessageW, WINDOW_EX_STYLE, WINDOW_STYLE,
    WS_POPUP,
};
use windows::core::{Error, PCWSTR, PWSTR, Result};

/// Un control de tooltips por ventana; conserva los textos UTF-16 vivos
/// por robustez (comctl32 copia el texto, pero no cuesta nada asegurarlo).
pub(crate) struct Tooltips {
    hwnd: HWND,
    textos: Vec<Vec<u16>>,
}

impl Tooltips {
    pub(crate) fn nuevo(padre: HWND) -> Result<Self> {
        // Con el manifest de comctl32 v6, el registro de las clases
        // comunes es explícito; una sola vez por proceso.
        static INIT: std::sync::Once = std::sync::Once::new();
        INIT.call_once(|| {
            let icc = INITCOMMONCONTROLSEX {
                dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
                dwICC: ICC_WIN95_CLASSES,
            };
            // SAFETY: struct local completo; sin más precondiciones.
            unsafe { _ = InitCommonControlsEx(&icc) };
        });
        // SAFETY: creación estándar de un control comctl32 sin padre de
        // clase propia; el HWND se destruye en Drop.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                TOOLTIPS_CLASSW,
                PCWSTR::null(),
                WS_POPUP | WINDOW_STYLE(TTS_ALWAYSTIP | TTS_NOPREFIX),
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                Some(padre),
                None,
                None,
                None,
            )?
        };
        Ok(Self { hwnd, textos: Vec::new() })
    }

    /// Asocia un tooltip al rect completo de un control hijo.
    pub(crate) fn agregar(&mut self, control: HWND, texto: &str) -> Result<()> {
        let mut wide: Vec<u16> = texto.encode_utf16().chain([0]).collect();
        let info = TTTOOLINFOW {
            cbSize: size_of::<TTTOOLINFOW>() as u32,
            uFlags: TTF_IDISHWND | TTF_SUBCLASS,
            hwnd: control,
            uId: control.0 as usize,
            lpszText: PWSTR(wide.as_mut_ptr()),
            ..Default::default()
        };
        // SAFETY: `info` y su texto viven durante la llamada; TTM_ADDTOOLW
        // copia lo que necesita.
        let ok = unsafe {
            SendMessageW(
                self.hwnd,
                TTM_ADDTOOLW,
                Some(WPARAM(0)),
                Some(LPARAM(&info as *const _ as isize)),
            )
        };
        self.textos.push(wide);
        if ok.0 == 0 {
            return Err(Error::from_thread());
        }
        Ok(())
    }
}

impl Drop for Tooltips {
    fn drop(&mut self) {
        // SAFETY: HWND creado por nuevo() y aún no destruido.
        unsafe { _ = DestroyWindow(self.hwnd) };
    }
}
