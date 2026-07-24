//! Slice de captura: modos (f.9-f.19) como strategies `CaptureMode` (D4),
//! selección y scroll-stitching.

use crate::ports::{Frame, Rect, ScreenSource, ScreenSourceError};

pub mod modes;

#[derive(thiserror::Error, Clone, PartialEq, Eq, Debug)]
pub enum CaptureError {
    #[error(transparent)]
    Source(#[from] ScreenSourceError),
    /// El modo no tiene nada que capturar (sin ventana activa, etc.).
    #[error("nada que capturar: {0}")]
    NothingToCapture(String),
}

/// Strategy de captura (D4): recibe un `ScreenSource`, devuelve un `Frame`.
/// Las estrategias concretas (pantalla completa, ventana, región...) se
/// construyen desde un `ModeRequest` vía la mode factory del orquestador.
pub trait CaptureMode {
    fn capture(&self, source: &mut dyn ScreenSource) -> Result<Frame, CaptureError>;
}

/// Qué capturar (datos del evento, D7). La factory `create_mode` lo
/// convierte en su strategy. Vive aquí porque es vocabulario del dominio
/// de captura; `orchestrator::events` lo re-exporta.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModeRequest {
    Fullscreen,
    ActiveWindow,
    Region(Rect),
}

/// Factory real de strategies (D4). Firma compatible con
/// `orchestrator::ModeFactory`: el wiring es `Box::new(create_mode)`.
pub fn create_mode(request: &ModeRequest) -> Result<Box<dyn CaptureMode>, CaptureError> {
    Ok(match request {
        ModeRequest::Fullscreen => Box::new(modes::FullscreenMode),
        ModeRequest::ActiveWindow => Box::new(modes::ActiveWindowMode),
        ModeRequest::Region(rect) => Box::new(modes::RegionMode::new(*rect)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::mocks::MockScreenSource;
    use crate::ports::{Frame, ScreenSource, ScreenSourceError};

    /// Estrategia mínima para validar el contrato del trait.
    struct DesktopMode;

    impl CaptureMode for DesktopMode {
        fn capture(&self, source: &mut dyn ScreenSource) -> Result<Frame, CaptureError> {
            let rect = source.desktop_rect();
            // `?` prueba la conversión From<ScreenSourceError>.
            Ok(source.capture_region(rect)?)
        }
    }

    #[test]
    fn una_estrategia_captura_a_traves_del_puerto() {
        let mut source = MockScreenSource::new((0, 0), Frame::filled(2, 2, [9, 9, 9, 255]));
        let frame = DesktopMode.capture(&mut source).unwrap();
        assert_eq!((frame.width, frame.height), (2, 2));
        assert_eq!(frame.pixel(1, 1), Some([9, 9, 9, 255]));
    }

    #[test]
    fn los_errores_del_puerto_se_convierten_a_capture_error() {
        let mut source = MockScreenSource::new((0, 0), Frame::filled(1, 1, [0; 4]));
        source.fail_next(ScreenSourceError::Platform("GDI caído".into()));
        let err = DesktopMode.capture(&mut source).unwrap_err();
        assert_eq!(
            err,
            CaptureError::Source(ScreenSourceError::Platform("GDI caído".into()))
        );
    }

    #[test]
    fn create_mode_region_captura_la_region_pedida() {
        let pixels: Vec<u8> = (0..16u8).flat_map(|i| [i, 0, 0, 255]).collect();
        let mut source = MockScreenSource::new((0, 0), Frame::new(4, 4, pixels).unwrap());
        let mode = create_mode(&ModeRequest::Region(crate::ports::Rect::new(1, 1, 2, 2))).unwrap();
        let frame = mode.capture(&mut source).unwrap();
        assert_eq!((frame.width, frame.height), (2, 2));
        assert_eq!(frame.pixel(0, 0), Some([5, 0, 0, 255]));
    }

    #[test]
    fn create_mode_active_window_sin_ventana_falla_al_capturar() {
        let mut source = MockScreenSource::new((0, 0), Frame::filled(2, 2, [0; 4]));
        let mode = create_mode(&ModeRequest::ActiveWindow).unwrap();
        assert!(matches!(
            mode.capture(&mut source).unwrap_err(),
            CaptureError::NothingToCapture(_)
        ));
    }

    #[test]
    fn el_orquestador_funciona_con_la_factory_real() {
        use crate::orchestrator::{AppEvent, CaptureRequest, Flow, Orchestrator};
        use crate::ports::mocks::MockOutputSink;

        let sink = MockOutputSink::new("clipboard");
        let entregas = sink.delivered_handle();
        let source = MockScreenSource::new((0, 0), Frame::filled(3, 3, [8, 8, 8, 255]));
        let mut orch = Orchestrator::new(Box::new(source), Box::new(create_mode));
        orch.add_sink(Box::new(sink)).unwrap();

        let flow = orch
            .handle_event(AppEvent::CaptureRequested(CaptureRequest {
                mode: ModeRequest::Fullscreen,
                destination: "clipboard",
            }))
            .unwrap();

        assert_eq!(flow, Flow::Continue);
        let frames = entregas.lock().unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!((frames[0].width, frames[0].height), (3, 3));
    }
}
