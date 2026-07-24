---
name: windows-rs-interop
description: Convenciones de interop Win32/COM/Media Foundation de RustCapture. Usar SIEMPRE antes de escribir o modificar código que toque windows-rs, HRESULT, COM, GDI, WGC, DXGI, Media Foundation, WASAPI, hotkeys globales o WIA — es decir, cualquier código del crate platform-win.
---

# Interop Win32 en RustCapture

Reglas de obligado cumplimiento para todo código que cruce la frontera con Windows. Complementa `arquitectura.md` (D2, D8); ante conflicto, gana `arquitectura.md`.

## Crate y dependencias

- **Solo `windows-rs`** (crate `windows`). Nunca `winapi`, nunca `windows-sys` directo.
- Activar únicamente las features de `windows` que el módulo necesita (`Win32_Graphics_Gdi`, `Win32_Media_MediaFoundation`, …). Cada feature nueva se justifica en el PR/commit que la introduce.
- Todo el interop vive en `crates/platform-win`. El crate `rustcapture-core` no importa `windows` jamás — si lo necesita, falta un puerto (trait en `core/src/ports.rs`).

## Estructura de adapters

- Un módulo por tecnología: `gdi`, `wgc`, `dxgi`, `mf`, `wasapi`, `hotkeys`, `wia`.
- Cada adapter implementa un puerto del core (`ScreenSource`, `VideoEncoder`, `OutputSink`, `HotkeyProvider`). La API pública del adapter es 100 % segura: ningún tipo de `windows` ni `unsafe` se filtra en firmas públicas.
- Recursos del sistema (HDC, HBITMAP, interfaces COM, handles) se envuelven en tipos RAII con `Drop`. Prohibido liberar a mano en el flujo normal.

## Errores y HRESULT

- Propagar con `windows::core::Result<T>` dentro del crate; convertir a los errores del dominio del core en la frontera del puerto (los puertos no exponen `windows::core::Error`).
- HRESULT: usar `.ok()?` sobre el `HRESULT` devuelto; nunca comparar contra `S_OK` a mano ni ignorar el valor.
- APIs que devuelven `BOOL`/handles nulos: comprobar y elevar con `windows::core::Error::from_win32()` (captura `GetLastError`).
- Nunca `unwrap()`/`expect()` sobre resultados de APIs de Windows fuera de tests.

## unsafe

- `unsafe` solo en `platform-win`, en bloques mínimos, cada bloque con un comentario `// SAFETY:` que justifique las precondiciones.
- No agrupar varias llamadas sin relación en un mismo bloque `unsafe`.

## COM y ciclo de vida

- Inicialización por hilo explícita y documentada: `CoInitializeEx` (apartment según la API), `MFStartup`/`MFShutdown` para Media Foundation. Emparejar siempre init/shutdown vía guard RAII, nunca llamadas sueltas.
- Las interfaces COM de `windows-rs` ya gestionan AddRef/Release; no llamar a `Release` manualmente.
- Cuidado con los hilos: objetos ligados a un apartment no se mueven entre hilos sin marshaling. Documentar en el adapter qué hilo posee qué.

## Tests

- El adapter se testea sin hardware donde sea posible: extraer la lógica pura (conversiones BGRA→NV12, cálculo de regiones, nombres de dispositivo) a funciones sin `unsafe` y testear esas. Lo que exige pantalla/audio real queda en tests `#[ignore]` de humo, ejecutables a mano.
- Los mocks de los puertos viven en `core`, no aquí (D2).

## Al depurar interop

Aplicar `systematic-debugging`: ante un HRESULT de error, buscar primero su significado exacto (código + facility) y qué precondición de la API se incumple; prohibido reordenar llamadas al azar. Frames negros o vacíos en captura: comprobar primero DPI awareness, apartment COM del hilo y formato de píxel antes de tocar el pipeline.
