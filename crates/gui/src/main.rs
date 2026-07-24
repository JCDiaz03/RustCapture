//! Binario GUI fino (D1, D11): barra + bandeja + hotkeys. El hilo
//! principal es la UI y único productor de eventos; el orquestador vive
//! en su propio hilo y se construye dentro de él (nada exige `Send`).

#![windows_subsystem = "windows"]

use std::process::ExitCode;
use std::sync::mpsc;
use std::thread;

use platform_win::bar::{Bar, run_message_loop};
use platform_win::clipboard::ClipboardSink;
use platform_win::gdi::GdiScreenSource;
use platform_win::hotkeys::Win32HotkeyProvider;
use platform_win::tray::Tray;
use rustcapture_core::capture::create_mode;
use rustcapture_core::config::Config;
use rustcapture_core::orchestrator::{AppEvent, CaptureRequest, Flow, ModeRequest, Orchestrator};
use rustcapture_core::output::FileSink;
use rustcapture_core::ports::{Hotkey, HotkeyProvider};

fn main() -> ExitCode {
    platform_win::dpi::ensure_per_monitor_dpi_awareness();

    let (config_path, _storage) = rustcapture_core::config::default_location();
    let config = match Config::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            platform_win::alerts::error_box("RustCapture", &e.to_string());
            return ExitCode::from(2);
        }
    };
    let destination = config.output.destination.sink_id();

    let (tx, rx) = mpsc::channel();

    // Hotkeys: registrar en ESTE hilo — WM_HOTKEY llega a su cola y lo
    // traduce run_message_loop. Fallos: beep y seguimos (spec §Errores).
    let delay_ms = config.capture.delay_ms();
    let mut hotkeys = Win32HotkeyProvider::new();
    let mut bindings = Vec::new();
    let captura =
        |mode: ModeRequest| AppEvent::CaptureRequested(CaptureRequest { mode, destination });
    let eventos = [
        (&config.hotkeys.fullscreen, captura(ModeRequest::Fullscreen)),
        (&config.hotkeys.window, captura(ModeRequest::ActiveWindow)),
        (
            &config.hotkeys.delay,
            AppEvent::DelayedCapture {
                request: CaptureRequest {
                    mode: ModeRequest::Fullscreen,
                    destination,
                },
                delay_ms,
            },
        ),
    ];
    for (spec, event) in eventos {
        let registrado =
            Hotkey::parse(spec).and_then(|hk| hotkeys.register(hk).map_err(|e| e.to_string()));
        match registrado {
            Ok(id) => bindings.push((id, event)),
            Err(_) => platform_win::alerts::error_beep(),
        }
    }

    // Hotkey de región: se resuelve en el hilo de UI (overlay), no en el
    // orquestador; run_message_loop lo traduce a WM_APP_REGION.
    let region_hotkey = Hotkey::parse(&config.hotkeys.region)
        .ok()
        .and_then(|hk| hotkeys.register(hk).ok());
    if region_hotkey.is_none() {
        platform_win::alerts::error_beep();
    }

    // Hilo orquestador: construido dentro para no exigir Send a los
    // trait objects; solo cruzan Receiver, bindings y el loopback.
    let loopback = tx.clone();
    let out = config.output.clone();
    let orch_thread = thread::spawn(move || {
        let mut orch = Orchestrator::new(Box::new(GdiScreenSource::new()), Box::new(create_mode));
        orch.set_loopback(loopback);
        orch.add_sink(Box::new(ClipboardSink::new()))
            .expect("sink único");
        orch.add_sink(Box::new(
            FileSink::new(out.dir, out.format).with_prefix(out.prefix),
        ))
        .expect("sink único");
        for (id, request) in bindings {
            orch.bind_hotkey(id, request);
        }
        // Feedback sonoro (verificación manual): confirmación al capturar,
        // error si algo falla.
        orch.run(rx, |event, result| match result {
            Err(_) => platform_win::alerts::error_beep(),
            Ok(Flow::Continue) => {
                if matches!(
                    event,
                    AppEvent::CaptureRequested(_) | AppEvent::HotkeyPressed(_)
                ) {
                    platform_win::alerts::capture_beep();
                }
            }
            Ok(Flow::Shutdown) => {}
        });
    });

    let bar = match Bar::create(tx.clone(), destination, delay_ms) {
        Ok(b) => b,
        Err(e) => {
            platform_win::alerts::error_box("RustCapture", &e);
            return ExitCode::FAILURE;
        }
    };
    let _tray = match Tray::new(bar.hwnd_raw()) {
        Ok(t) => t,
        Err(e) => {
            platform_win::alerts::error_box("RustCapture", &e);
            return ExitCode::FAILURE;
        }
    };

    run_message_loop(&tx, region_hotkey, &bar);

    // WM_DESTROY ya envió Shutdown; soltar nuestro Sender y esperar.
    drop(tx);
    drop(hotkeys); // desregistra los hotkeys globales
    let _ = orch_thread.join();
    ExitCode::SUCCESS
}
