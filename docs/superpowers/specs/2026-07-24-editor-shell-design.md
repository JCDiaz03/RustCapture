# Spec — Editor shell (Ventana1 mínima; f.21, Slice A de F3 adelantada)

Diseño validado con el humano (brainstorming 2026-07-24). Referencias
visuales del humano: `docs/superpowers/Ventana1.PNG` (editor FastStone) y
`docs/superpowers/Ventana2.jpg` (ventana de dibujo, futuro Slice C).

## Decisión de producto (cambia el flujo por defecto)

Tras CUALQUIER captura de la GUI (botones, hotkeys, delay, repetir), la
captura NO va al portapapeles: se abre la ventana del editor con la
imagen. La barra se oculta al abrirse el editor y reaparece al cerrarlo.
La CLI no cambia (flags directas a clipboard/file: es para scripts).

F3 se adelanta por decisión de producto; F2 queda en pausa (picking de
ventana/objeto, mano alzada, región fija, scroll y f.7/f.19 pendientes).
Slices de F3: **A** editor shell (esta spec) → **B** motor de anotación
en core (D5+D6, TDD puro) → **C** ventana de dibujo (Ventana2). B y C
tendrán brainstorming y spec propios.

## Slice A — comportamiento

- **Entrega:** nuevo `DestinationKind::Editor` (serde `"editor"`), NUEVO
  DEFAULT de `[output].destination`; `"clipboard"`/`"file"` siguen
  disponibles para el flujo directo antiguo.
- **`EditorSink`** (`platform-win`): `OutputSink` con id `"editor"`. Su
  `deliver` envía el `Frame` al hilo de UI (`PostMessageW` a la barra con
  el frame en un `Box`); el wndproc de la barra toma posesión, oculta la
  barra y abre el editor. Si el editor ya está abierto, `deliver` falla
  (`Failed("editor ocupado")` → beep del observer): un editor cada vez,
  sin tabs en el MVP.
- **Ventana del editor:** overlapped estándar (título, minimizar,
  maximizar, redimensionar), título `RustCapture Editor — <W>×<H>`.
  Lienzo con la captura centrada; si no cabe, se encaja manteniendo
  aspecto (sin zoom manual todavía). Bucle modal en el hilo de UI (como
  el overlay); mientras vive, no se procesan capturas nuevas.
- **Toolbar (4 botones):**
  - «Guardar como…»: `GetSaveFileNameW` con filtro PNG/JPEG; codifica
    con `output::encode` del core y escribe en la ruta elegida.
  - «Copiar»: entrega el frame a `ClipboardSink`.
  - «Draw»: visible y DESHABILITADO (se activa en el Slice C).
  - «Cerrar»: cierra sin aviso. Regla acordada: sin ediciones → cierra
    silencioso; con ediciones → avisará (el flag de sucio llega con C).
- **Al cerrar:** la ventana se destruye, la barra reaparece, el sistema
  queda listo para la siguiente captura.

## Lógica pura (TDD)

- `fit_rect(imagen: (u32,u32), lienzo: (i32,i32)) -> Rect` — encaje
  centrado manteniendo aspecto; si la imagen cabe entera, se centra a
  tamaño natural (sin ampliar).
- Rechazo por editor ocupado (flag en el estado de la barra, testeable
  la decisión vía `EditorSink` con hwnd inválido → `Failed`).

## Errores

- Editor ya abierto → `OutputError::Failed("editor ocupado")` → beep.
- Fallo del diálogo de guardado o de la escritura → MessageBox de error
  (el usuario está delante y no hay stderr).
- Fallo de `PostMessageW` (barra destruida) → el `Box` se libera sin
  fuga y `deliver` devuelve `Failed`.

## Fuera de alcance (slices B/C y posteriores)

Anotación y dibujo, flag de sucio + aviso al cerrar, tabs multi-captura,
zoom/scroll del lienzo, crop/resize/effects, resto de botones de
FastStone, impresión/email.
