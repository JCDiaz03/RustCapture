//! Binario fino de línea de comandos (D1, f.8): traduce flags a un evento
//! del bus (D7) y deja que el orquestador del core haga el resto.

mod args;

use std::process::ExitCode;
use std::sync::mpsc;

use platform_win::clipboard::ClipboardSink;
use platform_win::gdi::GdiScreenSource;
use rustcapture_core::capture::create_mode;
use rustcapture_core::config::Config;
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

    let (config_path, _storage) = rustcapture_core::config::default_location();
    let config = match Config::load(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
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
            let dir = dir.unwrap_or_else(|| config.output.dir.clone());
            let format = format.unwrap_or(config.output.format);
            orch.add_sink(Box::new(
                FileSink::new(dir, format).with_prefix(config.output.prefix.clone()),
            ))
            .expect("primer sink registrado");
            "file"
        }
    };

    // f.17 en CLI: proceso efímero, el retardo es un sleep local.
    if let Some(segundos) = options.delay_seconds {
        std::thread::sleep(std::time::Duration::from_secs(segundos));
    }

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
