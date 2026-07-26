# Diseño frontend — App de captura de pantalla para Windows

> **Mantenimiento de este documento — capa REFERENCIA (diseño de pantallas).**
>
> - Qué es: foto del estado ACTUAL del diseño de interfaz — inventario de ventanas, superficies e iconos y sus reglas. NO es un registro de cambios ni una hoja de ruta.
> - Presente, sin fechas; estado con marcadores (`(parcial)`, `(mock)`) si algo está a medias; el historial está en git.
> - Una sola casa por dato: características → `ideas.md` (se referencian como f.N) · decisiones técnicas → `arquitectura.md` (D.N) · fases → `roadmap.md`. Aquí solo vive la interfaz.
> - Documento pensado como brief de entrada para Claude Design: §5 define los entregables esperados.

## 1. Principios de diseño (no negociables)

1. **Eficiencia sobre estética.** La UI es una herramienta, no un producto visual. Cero animaciones decorativas, cero degradados, cero sombras difusas costosas de renderizar.
2. **Densidad funcional.** Estilo utilitario tipo FastStone/Greenshot: botones compactos, tooltips con el hotkey, todo a un clic.
3. **Ligereza técnica.** UI en Win32 puro pintada con GDI (decisión ratificada, D11/D13) → sin imágenes bitmap decorativas; toda la iconografía es vectorial monocroma tintable en runtime.
4. **DPI-aware per-monitor** (f.6): todo se define en unidades lógicas; los iconos deben ser nítidos a 100/125/150/200 %.
5. **Tema claro y oscuro** con la misma geometría; solo cambian los tintes.
6. **Teclado primero:** cada acción visible muestra su hotkey; la UI es la vía secundaria (f.3).

## 2. Tokens visuales

- **Tipografía:** Segoe UI (sistema), tamaños 12/13 px cuerpo, 11 px secundario. Sin fuentes embebidas.
- **Paleta neutra + 1 acento.** Claro: fondos #F3F3F3 / #FFFFFF, texto #1A1A1A, bordes #D0D0D0. Oscuro: fondos #202020 / #2B2B2B, texto #E8E8E8, bordes #3F3F3F. Acento único (selección, grabando, botón primario): #D83B01 para estado de grabación, #0067C0 para selección/acción. Nada más.
- **Geometría:** radios 4 px, bordes 1 px, espaciado en múltiplos de 4 px. Botones de toolbar 28×28 px lógicos con icono de 16 px.

## 3. Inventario de ventanas y superficies

### V1 — Barra flotante (f.1) `(parcial: horizontal implementada)`
Superficie principal y única siempre presente. Ventana sin bordes, siempre-encima opcional, arrastrable, acoplable a bordes de pantalla con auto-ocultado (asoma 4 px) `(acoplado pendiente)`.
- Orientación horizontal `(hecha)` y vertical `(pendiente)` (misma botonera).
- Botonera: un botón por modo de captura frecuente (pantalla, ventana, objeto, región, mano alzada, región fija, scroll) + retardo, separador, grabar vídeo, separador, utilidades (cuentagotas, lupa, regla, crosshair, pin), separador, ajustes — los modos sin lógica se muestran deshabilitados.
- Estado colapsado: solo asa de arrastre + botón de expandir `(pendiente)`.
- Cada botón: icono 16 px + tooltip "Nombre (Hotkey)" `(hecho)`.

### V2 — Menú de bandeja (f.2)
Menú contextual nativo: lista de modos de captura con hotkeys, grabar, abrir editor, ajustes, salir. Sin diseño custom: iconos 16 px + texto.

### V3 — Overlay de selección y anotación (f.13-f.15, f.20, D10) `(parcial: selección y lupa implementadas)`
Ventana fullscreen sin bordes sobre el frame congelado (oscurecido ~30 % fuera de la selección).
- Crosshair de precisión `(hecho, sin líneas guía a los bordes)`.
- Lupa flotante junto al cursor (zoom ~6×, píxel central marcado, `#RRGGBB`, coordenadas y tamaño de selección en px) `(hecha)`.
- Al cerrar la selección: toolbar contextual flotante bajo/sobre la selección con las herramientas de anotación (§V4, mismas strategies) + botones de salida (copiar, guardar, editor completo, cancelar). Enter = ejecutar salida por defecto; Esc = cancelar. `(F4, pendiente)`

### V4 — Editor (f.21-f.31) `(parcial: chrome + anotación in situ implementados)`
Ventana principal de edición, redimensionable.
- Toolbar superior: selección `(D)`, texto, flecha, línea, forma, elipse, lápiz, resaltador `(hechas)`, pasos numerados, leyenda, pixelado, goma `(D)` | recorte, redimensionar `(D)` | deshacer/rehacer `(hechos)`.
- Barra de propiedades contextual bajo la toolbar (grosor, color, tamaño de fuente, negrita) `(hecha, con chips + menú popup)`.
- Canvas central con ajuste a ventana `(hecho)`; zoom Ctrl+rueda `(diferido)`.
- Pestañas inferiores: una por captura abierta (miniatura + nombre) `(diferido)`.
- Barra de estado: dimensiones, % de encaje, formato/archivo y estado de guardado `(hecha)`.
- Salidas en la toolbar derecha: copiar, guardar-como `(hechas)`, imprimir, email `(D)`; formato propio f.31 `(pendiente)`.

### V5 — Inicio de grabación (f.37)
Ventana compacta con dos vistas conmutables por un enlace:
- **Simple:** selector de área (pantalla/ventana/región), toggles micrófono/altavoces/webcam, botón grande Grabar.
- **Detallada:** añade fps, calidad, cursor/clics resaltados, cuenta atrás, carpeta de salida.

### V6 — HUD de grabación
Mini barra flotante durante la grabación: punto rojo pulsante (única animación permitida de la app), tiempo transcurrido, pausa, detener, dibujar-en-pantalla `(fase 2)`. Debe poder excluirse de la propia grabación.

### V7 — Recorte de vídeo (f.38)
Ventana simple: preview, línea de tiempo con dos asas de recorte, botones guardar MP4 / exportar GIF (f.39, con aviso de tamaño estimado).

### V8 — Pin-to-screen (f.46)
Ventana sin bordes ni chrome, solo la imagen con borde de 1 px, siempre encima. Rueda = zoom, arrastre = mover, doble clic = opacidad, menú contextual (copiar, guardar, cerrar).

### V9 — Utilidades (f.48-f.51)
- **Cuentagotas:** lupa + valor HEX/RGB junto al cursor; clic copia.
- **Lupa:** ventana flotante redimensionable con zoom 2-8×.
- **Regla:** regla translúcida horizontal/vertical en px, rotable, con marcas cada 5/50 px.
- **Crosshair:** dos líneas guía fullscreen + coordenadas.

### V10 — Ajustes
Ventana con lista lateral de secciones: General (idioma, arranque, tema, portable), Hotkeys (tabla reasignable), Captura (formato por defecto, nombres automáticos, diminutas f.19), Vídeo (parámetros D8), Salidas, Acerca de.

## 4. Inventario de iconos

**Especificación común:** SVG monocromo, trazo 1.5 px sobre rejilla 16×16 (exportar también 20 y 24), esquinas y remates redondeados, sin rellenos salvo indicadores de estado, un solo color = `currentColor` (tintado runtime claro/oscuro). Estilo geométrico-utilitario, legible a 16 px reales.

- **Icono de aplicación** (único multicolor): concepto = marco de captura + destello/rayo (ligereza). Entregar en 16/24/32/48/256 px para el .ico y variante monocroma para bandeja (claro y oscuro).
- **Modos de captura (7):** pantalla completa, ventana activa, objeto de ventana, región rectangular, mano alzada, región fija, scroll.
- **Captura extra (3):** temporizador/retardo, repetir última, menú desplegado.
- **Grabación (6):** grabar (círculo), pausa, detener, micrófono, altavoz, webcam.
- **Anotación (11):** cursor/selección, texto, flecha, línea, rectángulo/forma, resaltador, pasos numerados, leyenda/caption, pixelado, goma, recorte.
- **Edición (5):** deshacer, rehacer, redimensionar, marca de agua, efectos de borde.
- **Salidas (6):** copiar, guardar, guardar-como, imprimir, email, editor externo.
- **Utilidades (6):** cuentagotas, lupa, regla, crosshair, pin, OCR/texto.
- **Sistema (5):** ajustes, expandir/colapsar barra, asa de arrastre, cerrar, tema claro/oscuro.

Total: ~50 iconos + icono de aplicación (+ `annotate-ellipse` y `annotate-pencil`, añadidos al set con la misma especificación).

**Fuente de verdad:** los `.svg` de `design/icons/` (el `icons.js`/`dc.html` del mockup es un derivado, no se embarca). Para el icono de app, los PNG de `design/icons/app/` son la fuente raster ya renderizada y `app-icon.svg` la fuente editorial. Los artefactos embebibles (atlas A8 por DPI y `rustcapture.ico`) los genera `design/tools/genassets` (D13) y se commitean.

## 5. Entregables esperados de Claude Design `(recibidos: viven en design/)`

1. Mockup de cada superficie V1-V10 en tema claro y oscuro (V1 en ambas orientaciones; V3 con selección activa y toolbar visible; V4 con una herramienta activa).
2. Set completo de iconos §4 como SVG individuales `currentColor`, sobre la rejilla y trazo especificados, con nombre de archivo = acción (`capture-region.svg`, `annotate-arrow.svg`...).
3. Icono de aplicación en los tamaños listados + variante bandeja.
4. Hoja de tokens (colores, tamaños, espaciados de §2) como referencia única.
5. Restricción global: nada que no pueda reproducirse en Win32/GDI con rectángulos, texto y paths — sin blur, sin transparencias complejas, sin imágenes raster.
