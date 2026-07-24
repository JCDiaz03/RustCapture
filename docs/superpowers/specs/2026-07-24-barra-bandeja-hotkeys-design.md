# Spec — Barra flotante, bandeja y hotkeys globales (f.1-f.3)

Slice final de F1. Diseño validado con el humano (brainstorming 2026-07-24).

## Objetivo

`rustcapture-gui.exe`: una barra flotante siempre visible, un icono en la
bandeja del sistema y hotkeys globales, que disparan las capturas ya
existentes (pantalla completa, ventana activa) a través del bus de eventos.
Primer binario residente del proyecto.

## Decisión de base de UI

**Win32 puro con `windows-rs`** (consolidado como D11 en `arquitectura.md`).
egui queda descartado para la barra: arrastraría winit + renderer (+MB,
repintado) para 6 botones. La tecnología del editor de F3 queda SIN decidir
a propósito: se elegirá en su brainstorming.

## Componentes

Todo el interop en `platform-win` (un módulo por pieza); `gui` es binario
fino (D1) sin ventana de consola (`windows_subsystem = "windows"`).

- `platform-win/src/hotkeys.rs` — `Win32HotkeyProvider` implementa el
  puerto `HotkeyProvider`: `RegisterHotKey`/`UnregisterHotKey` con mapeo
  `Hotkey` (core) → `MOD_*`/VK. Los `WM_HOTKEY` llegan al bucle de
  mensajes del hilo UI, que publica `AppEvent::HotkeyPressed(id)`.
- `platform-win/src/tray.rs` — `Shell_NotifyIcon` + menú contextual:
  capturar pantalla, capturar ventana, mostrar/ocultar barra, salir (f.2).
- `platform-win/src/bar.rs` — ventana `WS_POPUP` + `WS_EX_TOPMOST` +
  `WS_EX_TOOLWINDOW` + `WS_EX_NOACTIVATE` (no roba el foco: "ventana
  activa" captura la ventana correcta), arrastrable. Seis botones con el
  layout definitivo: **pantalla** y **ventana** activos; **región (con
  lupa)**, **delay**, **grabador** y **configuración** visibles pero
  deshabilitados hasta su fase (F4, F2, F5 y diálogo futuro).
- `gui/src/main.rs` — cableado: DPI → config → canal mpsc → hilo
  orquestador → hotkeys + bindings → bucle de mensajes. El orquestador se
  construye DENTRO de su hilo (GDI + `create_mode` + `ClipboardSink` +
  `FileSink`): solo el `Receiver` cruza hilos, sin exigir `Send` a los
  trait objects. Salir (bandeja) → `AppEvent::Shutdown` + join.

## Config nueva (defaults de fábrica; todo configurable — f.3)

```toml
[hotkeys]
fullscreen = "printscreen"
window = "alt+printscreen"
region = "ctrl+printscreen"          # reservado: se activa en F4
delay = "ctrl+shift+printscreen"     # reservado: se activa en F2

[output]
destination = "clipboard"            # destino de barra y hotkeys: clipboard | file
```

- Defaults estilo FastStone elegidos por el humano; PrintScreen se
  registra globalmente y desplaza la captura nativa de Windows mientras la
  app corre (decisión consciente).
- `Hotkey::parse("ctrl+alt+printscreen")` se añade al core
  (`ports/hotkeys.rs`), puro y con TDD: tokens separados por `+`,
  modificadores `ctrl|alt|shift|win`, tecla final `a..z`, `0..9`,
  `f1..f24` o `printscreen`.
- Este slice solo registra `fullscreen` y `window`; `region` y `delay`
  viven ya en el schema para no romper configs futuras.

## Hilos

- **Hilo principal (UI):** barra, bandeja, hotkeys y bucle de mensajes.
  Único productor de eventos.
- **Hilo orquestador:** consume el canal y ejecuta capturar→entregar.
- Comunicación exclusivamente por el canal mpsc (D7). Nada de estado
  compartido.

## Errores

- Hotkey ya tomado por otra app → aviso no bloqueante (log/beep) y la app
  sigue con los demás atajos.
- Errores de captura/entrega en el observer del orquestador →
  `MessageBeep(MB_ICONERROR)`; las notificaciones visuales llegarán con
  más GUI.
- Config TOML rota → `MessageBox` con el error y salida con código 2 (en
  GUI no hay stderr visible).

## Limitaciones conocidas (aceptadas para el MVP)

- La barra sale en la captura de pantalla completa (FastStone se
  auto-oculta; mejora futura).
- Capturar "ventana" desde el menú de bandeja puede capturar la ventana
  que estaba activa antes de abrir el menú.

## Testing

- TDD: `Hotkey::parse` (core), claves nuevas de config (core), mapeo
  `Hotkey`→VK (función pura en `platform-win`).
- Smoke `#[ignore]`: registrar/desregistrar un hotkey real.
- Manual guiado (checklist del plan): arrancar la GUI, botones activos,
  hotkeys reales, bandeja, salir limpio.

## Fuera de alcance de este slice

Overlay de región con lupa (F4), delay (F2), grabación (F5), diálogo de
configuración, acople magnético a bordes y auto-ocultado de la barra
(f.1 completo), notificaciones toast.
