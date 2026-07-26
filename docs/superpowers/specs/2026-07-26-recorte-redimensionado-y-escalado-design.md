# Recorte, redimensionado y escalado de objetos — diseño

Cubre f.26 (recorte y redimensionado de la imagen) y añade el escalado de
objetos ya colocados, que no estaba en el catálogo.

## Problema

El editor sabe anotar, seleccionar, mover y girar, pero no sabe cambiar el
**tamaño** de nada: ni encuadrar la captura después de tomarla, ni reducirla
para compartirla, ni ajustar una flecha que salió demasiado pequeña. Las ocho
asas cuadradas del recuadro de selección se pintan desde el slice de rotación
pero no hacen nada.

## Decisiones y su porqué

### El recorte y el tamaño de salida son parámetros del documento, no ediciones del frame

`Document` gana dos campos que no son objetos:

```rust
/// Parte de la base que se muestra y se exporta; `None` = toda.
recorte: Option<Rect>,
/// Tamaño al que se exporta; `None` = el de la base recortada.
salida: Option<(u32, u32)>,
```

**Por qué no modificar el frame directamente:** D6 promete undo ilimitado. Un
recorte destructivo obliga a que el Command guarde el frame anterior para
poder restaurarlo — 33 MB por paso en una captura 4K (3840×2160×4). Tres
recortes y dos redimensionados serían más de 150 MB en la pila de deshacer, lo
que contradice la prioridad número uno del proyecto (consumo mínimo). Con
parámetros, deshacer un recorte es **intercambiar dos valores**.

**Por qué en `Document` y no en `EditorState`:** `History` opera sobre
`Document`, así que ponerlos ahí los hace deshacibles sin maquinaria nueva.
Van documentados como «ajustes del lienzo, no objetos».

Los Commands guardan el valor **anterior**, no un delta:
`Crop { nuevo: Option<Rect>, anterior: Option<Rect> }` y su hermano para el
tamaño. No se usa el truco del delta negado de `Move`/`Rotate` porque aquí no
hay delta: pasar de un recorte a otro no es una suma, y quitar el recorte es
volver a `None`. Guardar el anterior son 16 bytes y hace el deshacer exacto.

**Encaja con D12 sin inventar nada:** D12 ya dice que Guardar y Copiar
*hornean bajo demanda* sobre el frame base. Recortar y escalar son dos
operaciones más de ese horneado.

### Al guardar se hornea todo, incluido el `.rcap`

Nada de recuperar píxeles recortados después de cerrar el archivo. La fuente
de una captura es la pantalla: si el recorte salió mal, se recaptura. Guardar
un `.rcap` con la captura entera solo haría los archivos más grandes para
cubrir un caso que no existe.

Consecuencia: **para el usuario esto se comporta como un recorte normal**. No
destructivo es un detalle de implementación que solo se nota en que no se
malgasta memoria, no una característica que haya que explicar.

Al hornear el `.rcap` las coordenadas de los objetos se trasladan por el
origen del recorte (exacto, es una suma) y, si hay tamaño de salida, su
geometría se escala (con redondeo). **Limitación conocida:** redimensionar,
guardar, reabrir y redimensionar otra vez escala dos veces y acumula
redondeo — un grosor de 3 px puede acabar en 2. Despreciable en un ciclo
normal.

### El lienzo aplica el recorte pero NO el tamaño de salida

|                | recorte | tamaño de salida |
|----------------|---------|------------------|
| Lienzo (`committed`) | sí | **no** |
| Exportar (PNG/JPEG/PDF/portapapeles/`.rcap`) | sí | sí |

Redimensionar da feedback en la barra de estado (`1920 × 1080 px · exporta
960 × 540`) y el tamaño real solo se aprecia en el archivo.

Esto tiene una consecuencia técnica que reduce mucho el riesgo: como el lienzo
nunca muestra la imagen escalada, **el mapeo de coordenadas es una simple
traslación**, no una transformación afín. Un clic solo suma el origen del
recorte para llegar a coordenadas de base. Sin esa decisión habría que
introducir una escala en `view_to_frame`, y con ella en el hit-test, el
recuadro de selección, el asa de rotación y la caja de texto.

### El escalado de objetos reutiliza `Command::Replace`

Escalar un objeto es sustituirlo por una copia escalada. `Command::Replace` ya
existe (lo introdujo la reedición de texto), ya conserva el z-order y su
deshacer es **exacto** porque guarda el objeto anterior completo.

La alternativa, un `Command::Scale { factor }` con deshacer
`escalar(1/factor)`, acumularía error de redondeo: la geometría son enteros y
`x * 1.3 / 1.3` no vuelve al mismo píxel. Se descarta por eso.

**El escalado no toca ningún rasterizador**, a diferencia del giro. Los nueve
rasterizadores ya consumen coordenadas y grosores en píxeles, así que escalar
es multiplicar números: el `rect` crece, los extremos de la línea se separan,
los puntos del lápiz se escalan, el tamaño de fuente se multiplica y el radio
del paso lo sigue.

### El escalado es uniforme y con ancla en el centro

Solo las 4 asas de las esquinas escalan; las 4 de los lados dejan de
pintarse. Un texto o un paso numerado deformado se ve mal, y el caso
frecuente es uniforme.

El factor sale de la **razón de distancias al centro del objeto**, no a la
esquina opuesta. Con ancla en la esquina opuesta el resultado sería ambiguo
cuando el objeto está girado, porque las asas están sobre la caja envolvente
del objeto ya rotado y esa caja no tiene esquinas «propias» del objeto.

## Interacción

**Recortar.** Botón de la toolbar → se arrastra el rectángulo a conservar
sobre el lienzo, lo de fuera se atenúa, se afina con las asas del rectángulo
y `Enter` confirma; `Esc` cancela. Recortar de nuevo recorta sobre lo ya
recortado, sin casos especiales.

**Redimensionar imagen.** Botón → diálogo flotante con ancho, alto, candado de
proporción y porcentaje. Al aceptar, la barra de estado refleja el tamaño de
exportación.

**Escalar objeto.** Con la herramienta Selección y un objeto elegido, se
arrastra una asa de esquina. El preview se pinta en vivo y al soltar se aplica
el `Command::Replace`.

## Limpieza que entra en el mismo trabajo

`Document` tiene hoy `render_onto_moved` y `render_onto_rotated`, casi
idénticas: pintan el documento sustituyendo un objeto por una versión
transformada. El preview del escalado necesitaría una tercera. Las tres se
unifican en una:

```rust
/// Pinta el documento sustituyendo el objeto `index` por `reemplazo`.
/// Es el preview de cualquier arrastre (mover, girar, escalar): el
/// documento no se toca hasta que el arrastre termina en un Command.
pub fn render_onto_reemplazando(
    &self, frame: &mut Frame, ctx: &RenderContext, index: usize, reemplazo: &Objeto,
)
```

Queda menos código del que hay ahora.

## Qué NO se hace

- **Un-crop después de guardar.** Descartado arriba con su porqué.
- **Escalado por eje** (estirar solo en horizontal). Deformaría textos y pasos.
- **Redimensionar arrastrando el lienzo.** Impreciso, y confuso porque el
  lienzo ya escala la imagen para que quepa en la ventana.
- **Reflejar el tamaño de salida en el lienzo.** Obligaría a remuestrear en
  cada repintado y a meter una escala en el mapeo de coordenadas.

## Riesgos

- **La traslación del recorte se olvida en algún sitio.** El síntoma sería
  señalar un objeto y seleccionar otro, o el recuadro de selección desplazado.
  Mitigación: un único punto de conversión (`view_to_frame` compuesto con el
  origen del recorte) y un test que compruebe, con un recorte activo, que un
  clic sobre un objeto lo selecciona.
- **Objetos que quedan fuera del recorte.** No se pierden: siguen en el
  documento y no se ven. Pero tampoco se pueden seleccionar, porque no hay
  dónde pulsar. `Ctrl+Z` del recorte los devuelve. Aceptado.
- **Escalar hasta cero.** Un factor muy pequeño podría dejar objetos de 0 px o
  tamaños de fuente en 0. Hay que acotar el factor por abajo.
- **Escalar un objeto girado.** El factor se calcula sobre la caja envolvente
  rotada, así que arrastrar la esquina no sigue exactamente al cursor cuando
  el giro no es múltiplo de 90°. Es aceptable y esperable; conviene verlo a
  ojo antes de dar el trabajo por bueno.

## Reparto en slices

Dos subsistemas distintos, dos planes:

1. **Recorte y redimensionado de imagen** — pipeline de horneado, `Document`,
   coordenadas, diálogo y barra de estado.
2. **Escalado de objetos** — motor de anotación, asas de esquina y la
   unificación de los tres `render_onto_*`.
