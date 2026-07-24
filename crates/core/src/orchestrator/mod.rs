//! Orquestador (D7): consume `AppEvent` del canal mpsc y ejecuta el
//! pipeline capturar → entregar. Hotkeys, barra y CLI son solo
//! productores; este módulo es el único consumidor.

mod events;

pub use events::{AppEvent, CaptureRequest, ModeRequest};

use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use crate::capture::{CaptureError, CaptureMode};
use crate::ports::{HotkeyId, OutputError, OutputSink, ScreenSource};

/// Convierte la petición (datos) en una strategy (comportamiento).
/// El slice de modos (D4) aporta la factory real; los tests, una fake.
pub type ModeFactory = Box<dyn Fn(&ModeRequest) -> Result<Box<dyn CaptureMode>, CaptureError>>;

#[derive(thiserror::Error, Clone, PartialEq, Eq, Debug)]
pub enum OrchestratorError {
    #[error("sink duplicado: {0}")]
    DuplicateSink(&'static str),
    #[error("sink desconocido: {0}")]
    UnknownSink(&'static str),
    #[error("atajo sin binding: {0:?}")]
    UnknownHotkey(HotkeyId),
    /// `DelayedCapture` exige loopback (`set_loopback`); la CLI no lo usa.
    #[error("captura con retardo no disponible sin loopback")]
    DelayUnavailable,
    #[error("no hay captura previa que repetir")]
    NothingToRepeat,
    #[error(transparent)]
    Capture(#[from] CaptureError),
    #[error(transparent)]
    Output(#[from] OutputError),
}

/// Qué hacer tras procesar un evento: seguir consumiendo o parar el bucle.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flow {
    Continue,
    Shutdown,
}

pub struct Orchestrator {
    source: Box<dyn ScreenSource>,
    mode_factory: ModeFactory,
    sinks: Vec<Box<dyn OutputSink>>,
    bindings: Vec<(HotkeyId, AppEvent)>,
    loopback: Option<Sender<AppEvent>>,
    last_request: Option<CaptureRequest>,
}

impl Orchestrator {
    pub fn new(source: Box<dyn ScreenSource>, mode_factory: ModeFactory) -> Self {
        Self {
            source,
            mode_factory,
            sinks: Vec::new(),
            bindings: Vec::new(),
            loopback: None,
            last_request: None,
        }
    }

    /// Canal de reentrada para eventos programados (D7): el hilo
    /// temporizador de `DelayedCapture` publica aquí.
    pub fn set_loopback(&mut self, tx: Sender<AppEvent>) {
        self.loopback = Some(tx);
    }

    pub fn add_sink(&mut self, sink: Box<dyn OutputSink>) -> Result<(), OrchestratorError> {
        if self.sinks.iter().any(|s| s.id() == sink.id()) {
            return Err(OrchestratorError::DuplicateSink(sink.id()));
        }
        self.sinks.push(sink);
        Ok(())
    }

    /// Asocia un hotkey a un evento; rebindear reemplaza (recarga de config).
    pub fn bind_hotkey(&mut self, id: HotkeyId, event: AppEvent) {
        if let Some(entry) = self.bindings.iter_mut().find(|(i, _)| *i == id) {
            entry.1 = event;
        } else {
            self.bindings.push((id, event));
        }
    }

    pub fn binding(&self, id: HotkeyId) -> Option<&AppEvent> {
        self.bindings.iter().find(|(i, _)| *i == id).map(|(_, e)| e)
    }

    /// Procesa un evento de forma síncrona. `run` lo llama en bucle; los
    /// tests lo llaman directamente.
    pub fn handle_event(&mut self, event: AppEvent) -> Result<Flow, OrchestratorError> {
        match event {
            AppEvent::CaptureRequested(request) => {
                self.capture_and_deliver(&request)?;
                self.last_request = Some(request);
                Ok(Flow::Continue)
            }
            AppEvent::DelayedCapture { request, delay_ms } => {
                let tx = self
                    .loopback
                    .clone()
                    .ok_or(OrchestratorError::DelayUnavailable)?;
                // D7: el orquestador nunca duerme; espera un hilo productor.
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(delay_ms));
                    let _ = tx.send(AppEvent::CaptureRequested(request));
                });
                Ok(Flow::Continue)
            }
            AppEvent::RepeatLast => {
                let request = self
                    .last_request
                    .clone()
                    .ok_or(OrchestratorError::NothingToRepeat)?;
                self.capture_and_deliver(&request)?;
                Ok(Flow::Continue)
            }
            AppEvent::HotkeyPressed(id) => {
                let event = self
                    .binding(id)
                    .cloned()
                    .ok_or(OrchestratorError::UnknownHotkey(id))?;
                // Un binding es cualquier evento (captura, retardada...).
                self.handle_event(event)
            }
            AppEvent::Shutdown => Ok(Flow::Shutdown),
        }
    }

    /// Bucle consumidor (D7): un evento cada vez, hasta `Shutdown` o hasta
    /// que todos los productores suelten su `Sender`. Los errores de un
    /// evento no tumban el bucle: se notifican al observer (futuros toasts
    /// de la GUI, stderr en la CLI).
    pub fn run<F>(&mut self, events: Receiver<AppEvent>, mut observer: F)
    where
        F: FnMut(&AppEvent, &Result<Flow, OrchestratorError>),
    {
        for event in events {
            let result = self.handle_event(event.clone());
            observer(&event, &result);
            if matches!(result, Ok(Flow::Shutdown)) {
                break;
            }
        }
    }

    fn capture_and_deliver(&mut self, request: &CaptureRequest) -> Result<(), OrchestratorError> {
        // Resolver el sink primero: no capturamos si el destino no existe.
        let sink = self
            .sinks
            .iter_mut()
            .find(|s| s.id() == request.destination)
            .ok_or(OrchestratorError::UnknownSink(request.destination))?;
        let mode = (self.mode_factory)(&request.mode)?;
        let frame = mode.capture(self.source.as_mut())?;
        sink.deliver(&frame)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::{CaptureError, CaptureMode};
    use crate::ports::mocks::{MockOutputSink, MockScreenSource};
    use crate::ports::{Frame, HotkeyId, ScreenSource};

    /// Fuente 2x2 con canal R = índice del píxel, origen (0, 0).
    fn source_2x2() -> Box<dyn ScreenSource> {
        let pixels: Vec<u8> = (0..4u8).flat_map(|i| [i, 0, 0, 255]).collect();
        Box::new(MockScreenSource::new(
            (0, 0),
            Frame::new(2, 2, pixels).unwrap(),
        ))
    }

    struct DesktopMode;

    impl CaptureMode for DesktopMode {
        fn capture(&self, source: &mut dyn ScreenSource) -> Result<Frame, CaptureError> {
            let rect = source.desktop_rect();
            Ok(source.capture_region(rect)?)
        }
    }

    /// Factory de test: solo soporta `Fullscreen`; el resto simula un
    /// modo aún no implementado.
    fn test_factory() -> ModeFactory {
        Box::new(|req| match req {
            ModeRequest::Fullscreen => Ok(Box::new(DesktopMode)),
            _ => Err(CaptureError::NothingToCapture(
                "modo no soportado en test".into(),
            )),
        })
    }

    fn orquestador() -> Orchestrator {
        Orchestrator::new(source_2x2(), test_factory())
    }

    #[test]
    fn add_sink_rechaza_ids_duplicados() {
        let mut orch = orquestador();
        orch.add_sink(Box::new(MockOutputSink::new("clipboard")))
            .unwrap();
        let err = orch
            .add_sink(Box::new(MockOutputSink::new("clipboard")))
            .unwrap_err();
        assert_eq!(err, OrchestratorError::DuplicateSink("clipboard"));
    }

    fn peticion(destination: &'static str) -> AppEvent {
        AppEvent::CaptureRequested(CaptureRequest {
            mode: ModeRequest::Fullscreen,
            destination,
        })
    }

    #[test]
    fn capture_requested_entrega_el_frame_al_sink_destino() {
        let sink = MockOutputSink::new("clipboard");
        let entregas = sink.delivered_handle();
        let otro = MockOutputSink::new("file");
        let otras_entregas = otro.delivered_handle();
        let mut orch = orquestador();
        orch.add_sink(Box::new(sink)).unwrap();
        orch.add_sink(Box::new(otro)).unwrap();

        let flow = orch.handle_event(peticion("clipboard")).unwrap();

        assert_eq!(flow, Flow::Continue);
        let frames = entregas.lock().unwrap();
        assert_eq!(frames.len(), 1);
        // Fullscreen del mock 2x2: el píxel (1,1) es el índice 3.
        assert_eq!(frames[0].pixel(1, 1), Some([3, 0, 0, 255]));
        assert!(otras_entregas.lock().unwrap().is_empty());
    }

    #[test]
    fn destino_no_registrado_devuelve_unknown_sink() {
        let mut orch = orquestador();
        assert_eq!(
            orch.handle_event(peticion("printer")).unwrap_err(),
            OrchestratorError::UnknownSink("printer")
        );
    }

    #[test]
    fn un_fallo_de_la_factory_se_propaga_como_capture() {
        let mut orch = orquestador();
        orch.add_sink(Box::new(MockOutputSink::new("clipboard")))
            .unwrap();
        let evento = AppEvent::CaptureRequested(CaptureRequest {
            mode: ModeRequest::ActiveWindow,
            destination: "clipboard",
        });
        assert_eq!(
            orch.handle_event(evento).unwrap_err(),
            OrchestratorError::Capture(CaptureError::NothingToCapture(
                "modo no soportado en test".into()
            ))
        );
    }

    #[test]
    fn un_fallo_del_sink_se_propaga_como_output() {
        use crate::ports::OutputError;
        let mut sink = MockOutputSink::new("clipboard");
        sink.fail_next(OutputError::Failed("portapapeles bloqueado".into()));
        let entregas = sink.delivered_handle();
        let mut orch = orquestador();
        orch.add_sink(Box::new(sink)).unwrap();
        assert_eq!(
            orch.handle_event(peticion("clipboard")).unwrap_err(),
            OrchestratorError::Output(OutputError::Failed("portapapeles bloqueado".into()))
        );
        assert!(entregas.lock().unwrap().is_empty());
    }

    #[test]
    fn shutdown_devuelve_flow_shutdown() {
        let mut orch = orquestador();
        assert_eq!(
            orch.handle_event(AppEvent::Shutdown).unwrap(),
            Flow::Shutdown
        );
    }

    #[test]
    fn hotkey_con_binding_ejecuta_el_evento_asociado() {
        let sink = MockOutputSink::new("clipboard");
        let entregas = sink.delivered_handle();
        let mut orch = orquestador();
        orch.add_sink(Box::new(sink)).unwrap();
        orch.bind_hotkey(HotkeyId(1), peticion("clipboard"));

        let flow = orch
            .handle_event(AppEvent::HotkeyPressed(HotkeyId(1)))
            .unwrap();

        assert_eq!(flow, Flow::Continue);
        assert_eq!(entregas.lock().unwrap().len(), 1);
    }

    #[test]
    fn delayed_sin_loopback_devuelve_delay_unavailable() {
        let mut orch = orquestador();
        let evento = AppEvent::DelayedCapture {
            request: CaptureRequest {
                mode: ModeRequest::Fullscreen,
                destination: "clipboard",
            },
            delay_ms: 1,
        };
        assert_eq!(
            orch.handle_event(evento).unwrap_err(),
            OrchestratorError::DelayUnavailable
        );
    }

    #[test]
    fn delayed_con_loopback_reenvia_la_peticion_tras_el_retardo() {
        let mut orch = orquestador();
        let (tx, rx) = std::sync::mpsc::channel();
        orch.set_loopback(tx);
        let request = CaptureRequest {
            mode: ModeRequest::Fullscreen,
            destination: "clipboard",
        };
        let flow = orch
            .handle_event(AppEvent::DelayedCapture {
                request: request.clone(),
                delay_ms: 10,
            })
            .unwrap();
        assert_eq!(flow, Flow::Continue);
        // El hilo temporizador reenvía por el loopback.
        let evento = rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .expect("debería llegar la petición reenviada");
        assert_eq!(evento, AppEvent::CaptureRequested(request));
    }

    #[test]
    fn repeat_sin_captura_previa_devuelve_nothing_to_repeat() {
        let mut orch = orquestador();
        assert_eq!(
            orch.handle_event(AppEvent::RepeatLast).unwrap_err(),
            OrchestratorError::NothingToRepeat
        );
    }

    #[test]
    fn repeat_reejecuta_la_ultima_captura_con_exito() {
        let sink = MockOutputSink::new("clipboard");
        let entregas = sink.delivered_handle();
        let mut orch = orquestador();
        orch.add_sink(Box::new(sink)).unwrap();
        orch.handle_event(peticion("clipboard")).unwrap();

        orch.handle_event(AppEvent::RepeatLast).unwrap();

        assert_eq!(entregas.lock().unwrap().len(), 2);
    }

    #[test]
    fn una_captura_fallida_no_se_convierte_en_ultima() {
        let mut orch = orquestador();
        // Sink inexistente: falla antes de capturar.
        let _ = orch.handle_event(peticion("printer"));
        assert_eq!(
            orch.handle_event(AppEvent::RepeatLast).unwrap_err(),
            OrchestratorError::NothingToRepeat
        );
    }

    #[test]
    fn hotkey_sin_binding_devuelve_unknown_hotkey() {
        let mut orch = orquestador();
        assert_eq!(
            orch.handle_event(AppEvent::HotkeyPressed(HotkeyId(7)))
                .unwrap_err(),
            OrchestratorError::UnknownHotkey(HotkeyId(7))
        );
    }

    #[test]
    fn run_procesa_hasta_shutdown_y_no_muere_por_errores() {
        let sink = MockOutputSink::new("clipboard");
        let entregas = sink.delivered_handle();
        let mut orch = orquestador();
        orch.add_sink(Box::new(sink)).unwrap();

        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(peticion("clipboard")).unwrap(); // ok
        tx.send(peticion("printer")).unwrap(); // error: sigue vivo
        tx.send(AppEvent::Shutdown).unwrap();
        tx.send(peticion("clipboard")).unwrap(); // tras shutdown: ignorado
        drop(tx);

        let mut log = Vec::new();
        orch.run(rx, |event, result| {
            log.push((event.clone(), result.clone()))
        });

        assert_eq!(log.len(), 3);
        assert_eq!(log[0].1, Ok(Flow::Continue));
        assert_eq!(log[1].1, Err(OrchestratorError::UnknownSink("printer")));
        assert_eq!(log[2].1, Ok(Flow::Shutdown));
        assert_eq!(entregas.lock().unwrap().len(), 1);
    }

    #[test]
    fn run_termina_al_desconectarse_todos_los_productores() {
        let mut orch = orquestador();
        orch.add_sink(Box::new(MockOutputSink::new("clipboard")))
            .unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(peticion("clipboard")).unwrap();
        drop(tx);

        let mut procesados = 0;
        orch.run(rx, |_, _| procesados += 1);

        assert_eq!(procesados, 1);
    }

    #[test]
    fn bind_hotkey_reemplaza_el_binding_anterior() {
        let mut orch = orquestador();
        orch.bind_hotkey(HotkeyId(1), peticion("clipboard"));
        orch.bind_hotkey(HotkeyId(1), peticion("file"));
        assert_eq!(orch.binding(HotkeyId(1)), Some(&peticion("file")));
        assert_eq!(orch.binding(HotkeyId(2)), None);
    }
}
