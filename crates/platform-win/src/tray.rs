//! Icono en la bandeja del sistema (f.2). El callback llega al wndproc
//! de la barra como `bar::WM_TRAY`; aquí vive el icono (RAII) y el menú.

use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, IDI_APPLICATION, LoadIconW,
    MF_SEPARATOR, MF_STRING, PostMessageW, SetForegroundWindow, TPM_BOTTOMALIGN, TrackPopupMenu,
    WM_COMMAND, WM_CONTEXTMENU, WM_LBUTTONUP, WM_RBUTTONUP,
};
use windows::core::w;

use crate::bar::{MENU_FULLSCREEN, MENU_QUIT, MENU_TOGGLE, MENU_WINDOW, WM_TRAY};

/// Icono de bandeja con quita-y-pon RAII.
pub struct Tray {
    data: NOTIFYICONDATAW,
}

impl Tray {
    pub fn new(hwnd_raw: isize) -> Result<Self, String> {
        Self::new_win32(hwnd_raw).map_err(|e| e.to_string())
    }

    fn new_win32(hwnd_raw: isize) -> windows::core::Result<Self> {
        let hwnd = HWND(hwnd_raw as *mut _);
        let mut data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: WM_TRAY,
            // SAFETY: icono de stock del sistema; no se libera.
            hIcon: unsafe { LoadIconW(None, IDI_APPLICATION)? },
            ..Default::default()
        };
        let tip: Vec<u16> = "RustCapture".encode_utf16().collect();
        data.szTip[..tip.len()].copy_from_slice(&tip);
        // SAFETY: data completa y con cbSize correcto.
        unsafe { Shell_NotifyIconW(NIM_ADD, &data).ok()? };
        Ok(Self { data })
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        // SAFETY: quita el icono añadido por new(); mismo uID/hWnd.
        unsafe { _ = Shell_NotifyIconW(NIM_DELETE, &self.data) };
    }
}

/// Rama `WM_TRAY` del wndproc de la barra.
pub(crate) fn on_tray_message(hwnd: HWND, lparam: LPARAM) {
    match (lparam.0 & 0xFFFF) as u32 {
        // Izquierdo: mostrar/ocultar la barra (mismo comando del menú).
        WM_LBUTTONUP => {
            // SAFETY: post a la propia ventana del wndproc.
            unsafe {
                _ = PostMessageW(
                    Some(hwnd),
                    WM_COMMAND,
                    WPARAM(MENU_TOGGLE as usize),
                    LPARAM(0),
                )
            };
        }
        WM_RBUTTONUP | WM_CONTEXTMENU => mostrar_menu(hwnd),
        _ => {}
    }
}

fn mostrar_menu(hwnd: HWND) {
    // SAFETY: menú efímero: crear → mostrar → destruir en esta función.
    // SetForegroundWindow es el requisito clásico para que el menú se
    // cierre al clicar fuera.
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        _ = AppendMenuW(
            menu,
            MF_STRING,
            MENU_FULLSCREEN as usize,
            w!("Capturar pantalla"),
        );
        _ = AppendMenuW(
            menu,
            MF_STRING,
            MENU_WINDOW as usize,
            w!("Capturar ventana"),
        );
        _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        _ = AppendMenuW(
            menu,
            MF_STRING,
            MENU_TOGGLE as usize,
            w!("Mostrar/ocultar barra"),
        );
        _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        _ = AppendMenuW(menu, MF_STRING, MENU_QUIT as usize, w!("Salir"));
        let mut pt = POINT::default();
        _ = GetCursorPos(&mut pt);
        _ = SetForegroundWindow(hwnd);
        _ = TrackPopupMenu(menu, TPM_BOTTOMALIGN, pt.x, pt.y, None, hwnd, None);
        _ = DestroyMenu(menu);
    }
}
