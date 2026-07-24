# Ideas — RustCapture

> **Mantenimiento de este documento — capa REFERENCIA.**
>
> - Qué es: catálogo del estado ACTUAL de las características del producto — qué se construye y por qué. NO es un registro de cambios ni una hoja de ruta.
> - Presente, sin fechas: nada de "(2026-...)", "última actualización", "antes era X / ahora Y", "se añadió/eliminó/decidió". El historial está en git.
> - Conserva el porqué, no el cuándo: documenta decisiones e inspiraciones (los "robos" llevan su origen); fuera anécdotas.
> - Estado, no fecha: si una característica está incompleta, márcala con un estado — `(parcial)`, `(no cableado)`, `(mock)` —, nunca con una fecha.
> - Una sola casa por dato: aquí vive el QUÉ (características). El CÓMO (decisiones de arquitectura) → ver `arquitectura.md`; no duplicar, resumir + enlazar.
> - §2 Fase 2 son ideas diferidas sin comprometer; no mezclar con la versión 1. §Descartado conserva los "no" y su porqué.

Proyecto: **RustCapture** — herramienta de captura estilo FastStone Capture (todo en uno, ligera, portable-first).
Stack previsto: Rust + interop Win32 (`windows-rs`), UI mínima (egui o Win32 puro).
Prioridades: peso mínimo, consumo mínimo de recursos, eficiencia por encima del aspecto visual.
Desarrollo: IA-first (Opus 4.8 / Fable 5).

## 1. Características (versión 1)

### Sistema y filosofía

1. Barra flotante como centro de la app: sin ventana principal, acoplable al borde de la pantalla, personalizable.
2. Icono en bandeja del sistema con menú rápido.
3. Hotkeys globales configurables para todos los modos de captura.
4. Portable-first: un único `.exe` estático, configuración en `config.toml` junto al ejecutable (o `%APPDATA%` si se detecta instalación). Instalador opcional en el futuro.
5. Consumo casi nulo en reposo; arranque instantáneo.
6. Soporte multi-monitor con DPI mixtos (per-monitor DPI awareness).
7. Soporte para múltiples escritorios virtuales de Windows 10/11.
8. Interfaz CLI: lanzar capturas desde línea de comandos o scripts (ej. `app.exe --region --clipboard`). *(Robo de Flameshot)*

### Modos de captura

9. Pantalla completa.
10. Ventana activa.
11. Objeto de ventana (controles individuales: botones, paneles, barras).
12. Menús desplegados.
13. Región rectangular.
14. Región a mano alzada.
15. Región fija (tamaño predefinido).
16. Ventana / página con scroll (scroll capture automático).
17. Captura con retardo (temporizador).
18. Repetir última captura (misma región/modo).
19. Permitir capturas diminutas (5×5 px o menos), configurable.

### Anotación y edición

20. Anotación directamente en el overlay de selección: la pantalla se congela y se dibuja sobre la selección antes de confirmar la captura. *(Robo de Flameshot — mejora clave de UX sobre FastStone)*
21. Editor integrado: la captura aterriza en él sin pasos intermedios.
22. Herramientas de anotación: texto, flechas, líneas, formas, resaltado.
23. Herramienta de pasos numerados (1, 2, 3…).
24. Leyendas/captions con estilos de borde.
25. Pixelado / desenfoque para censurar información.
26. Recorte y redimensionado.
27. Goma de borrar.
28. Nitidez y ajustes básicos de color.
29. Marca de agua.
30. Efectos de borde (sombra, borde rasgado).
31. Formato propio sin pérdida que conserva los objetos de anotación editables junto a la imagen (equivalente al `.fsc` de FastStone).

### Grabación de vídeo

32. Grabación de pantalla completa, ventana o región (DXGI Desktop Duplication).
33. Audio de micrófono y de altavoces (loopback).
34. Overlay de webcam opcional.
35. Resaltado de cursor y de clics del ratón.
36. Codificación a MP4/H.264 con Media Foundation (sin dependencias externas).
37. Ventana de inicio de grabación con modo simple y detallado, y activación de dispositivos desde ahí.
38. Recorte (trim) básico del vídeo grabado.
39. Exportación a GIF animado desde el mismo pipeline de grabación. *(Robo de ShareX)*

### Salidas

40. Portapapeles.
41. Archivo con generación automática de nombres.
42. Impresora.
43. Email.
44. Envío a editor externo configurable.
45. Formatos de imagen: PNG, JPEG, WebP (con opción alta calidad), BMP, GIF, TIFF, PDF.

### Utilidades

46. Pin-to-screen: fijar una captura flotando siempre visible sobre el escritorio.
47. Captura de texto (OCR) usando la API de OCR de Windows.
48. Cuentagotas de color.
49. Lupa de pantalla.
50. Crosshair (cruceta de precisión).
51. Regla de pantalla.

## 2. Fase 2

1. **Flujos post-captura simplificados** *(inspirado en ShareX)*: 3-4 cadenas predefinidas de acciones (ej. capturar → guardar → copiar ruta; capturar → subir → copiar URL). Sin el framework configurable completo de ShareX, que es lo que lo hace intimidante.
2. **Historial de capturas** *(inspirado en ShareX)*: galería persistente y navegable de todo lo capturado, con miniaturas y búsqueda.
3. **Auto-captura por intervalo** *(inspirado en ShareX)*: capturas automáticas cada N segundos para monitorización o timelapses. Coste bajo, utilidad de nicho.
4. **Escaneo de imágenes desde escáner** (WIA): FastStone lo tiene, pero es periférico al núcleo del proyecto; se pospone.
5. **Instalador opcional** (Inno Setup / WiX) manteniendo la versión portable como principal.
6. **Editor de vídeo ampliado**: dibujar/anotar sobre la grabación, no solo trim.

## Descartado (decisiones de diseño)

- Captura panorámica con cosido manual (Snagit): coste medio-alto (stitching en tiempo real) para un caso de nicho; el scroll capture automático cubre la necesidad principal.
- Subida a la nube, incluso acotada (FTP/SFTP + servicios): fuera del alcance del proyecto; las salidas locales (archivo, portapapeles, email) cubren el flujo.
- Catálogo masivo de destinos de subida (ShareX): mantenimiento perpetuo de APIs y OAuth.
- Sistema de plugins (Greenshot): API de plugins estable es un proyecto en sí; en Rust añade complejidad de ABI.
- Simplify / plantillas de documentación (Snagit): requiere visión por computador; dirección "suite pesada".
- Biblioteca en la nube con cuentas y sincronización (Snagit): va contra portable, ligero y sin dependencias.
- Electron/Tauri como base de UI: contrario a la prioridad de peso y consumo mínimos.
