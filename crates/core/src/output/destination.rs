//! Destino por defecto de las capturas de barra/hotkey (spec f.1-f.3).

/// A qué sink va una captura lanzada sin destino explícito.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum DestinationKind {
    Clipboard,
    File,
    /// La captura aterriza en el editor (f.21) — flujo por defecto de la GUI.
    #[default]
    Editor,
}

impl DestinationKind {
    /// Id del `OutputSink` registrado en el orquestador.
    pub fn sink_id(&self) -> &'static str {
        match self {
            DestinationKind::Clipboard => "clipboard",
            DestinationKind::File => "file",
            DestinationKind::Editor => "editor",
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
        assert_eq!(DestinationKind::Editor.sink_id(), "editor");
    }

    #[test]
    fn el_default_es_editor() {
        assert_eq!(DestinationKind::default(), DestinationKind::Editor);
    }
}
