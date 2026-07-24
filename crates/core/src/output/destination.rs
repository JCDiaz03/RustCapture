//! Destino por defecto de las capturas de barra/hotkey (spec f.1-f.3).

/// A qué sink va una captura lanzada sin destino explícito.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum DestinationKind {
    #[default]
    Clipboard,
    File,
}

impl DestinationKind {
    /// Id del `OutputSink` registrado en el orquestador.
    pub fn sink_id(&self) -> &'static str {
        match self {
            DestinationKind::Clipboard => "clipboard",
            DestinationKind::File => "file",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_sink_ids_coinciden_con_los_sinks_reales() {
        assert_eq!(DestinationKind::Clipboard.sink_id(), "clipboard");
        assert_eq!(DestinationKind::File.sink_id(), "file");
    }
}
