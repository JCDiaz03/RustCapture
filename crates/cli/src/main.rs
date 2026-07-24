//! Binario fino de línea de comandos (D1, f.8): traduce flags a un evento
//! del bus (D7) y deja que el orquestador del core haga el resto.

mod args;

use std::process::ExitCode;
use std::sync::mpsc;

use platform_win::clipboard::ClipboardSink;
use platform_win::gdi::GdiScreenSource;
use rustcapture_core::capture::create_mode;
use rustcapture_core::orchestrator::{AppEvent, CaptureRequest, Orchestrator};
use rustcapture_core::output::FileSink;

fn main() -> ExitCode {
    platform_win::dpi::ensure_per_monitor_dpi_awareness();

    let options = match args::parse(std::env::args_os().skip(1).collect()) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}\n\n{}", args::USAGE);
            return ExitCode::from(2);
        }
    };

    let mut orch = Orchestrator::new(Box::new(GdiScreenSource::new()), Box::new(create_mode));
    let destination = match options.destination {
        args::Destination::Clipboard => {
            orch.add_sink(Box::new(ClipboardSink::new()))
                .expect("primer sink registrado");
            "clipboard"
        }
        args::Destination::File { dir, format } => {
            orch.add_sink(Box::new(FileSink::new(dir, format)))
                .expect("primer sink registrado");
            "file"
        }
    };

    let (tx, rx) = mpsc::channel();
    tx.send(AppEvent::CaptureRequested(CaptureRequest {
        mode: options.mode,
        destination,
    }))
    .expect("canal recién creado");
    drop(tx); // sin más productores: run termina al vaciar la cola

    let mut fallo = None;
    orch.run(rx, |_, result| {
        if let Err(e) = result {
            fallo = Some(e.to_string());
        }
    });

    match fallo {
        Some(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
        None => {
            println!("captura entregada a {destination}");
            ExitCode::SUCCESS
        }
    }
}
