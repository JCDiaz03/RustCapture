// GENERADO POR design/tools/genassets — NO EDITAR.
// Regenerar: cd design/tools/genassets && cargo run

/// Iconos disponibles en los atlas A8. El discriminante es el índice
/// dentro de cada `atlas_N.bin` (offset del icono = índice × lado²).
// El inventario va por delante de los consumidores a propósito
// (iconos de fases futuras ya generados).
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(usize)]
pub enum Icono {
    SysDragHandle = 0,
    CaptureFullscreen = 1,
    CaptureWindow = 2,
    CaptureRegion = 3,
    CaptureDelay = 4,
    CaptureObject = 5,
    CaptureFreehand = 6,
    CaptureFixed = 7,
    CaptureScroll = 8,
    RecordStart = 9,
    UtilEyedropper = 10,
    UtilMagnifier = 11,
    UtilRuler = 12,
    UtilCrosshair = 13,
    UtilPin = 14,
    SysSettings = 15,
    SysCollapse = 16,
    SysClose = 17,
    AnnotateSelect = 18,
    AnnotateText = 19,
    AnnotateArrow = 20,
    AnnotateLine = 21,
    AnnotateShape = 22,
    AnnotateEllipse = 23,
    AnnotatePencil = 24,
    AnnotateHighlight = 25,
    AnnotateSteps = 26,
    AnnotateCaption = 27,
    AnnotatePixelate = 28,
    AnnotateEraser = 29,
    AnnotateCrop = 30,
    EditResize = 31,
    EditUndo = 32,
    EditRedo = 33,
    OutputCopy = 34,
    OutputSave = 35,
    OutputSaveAs = 36,
    OutputPrint = 37,
    OutputEmail = 38,
    EditRotate = 39,
    OutputOpen = 40,
}

pub const NUM_ICONOS: usize = 41;
pub const TALLAS: [u32; 5] = [16, 20, 24, 28, 32];
