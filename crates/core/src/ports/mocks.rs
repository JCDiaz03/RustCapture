//! Mocks de los puertos para tests de `core`, `cli` y `gui`.
//! Públicos a propósito: son parte del contrato de test del workspace (D2).

use super::{
    Frame, Hotkey, HotkeyError, HotkeyId, HotkeyProvider, OutputError, OutputSink, Rect,
    ScreenSource, ScreenSourceError,
};

/// `ScreenSource` respaldado por un frame en memoria.
pub struct MockScreenSource {
    origin: (i32, i32),
    base: Frame,
    active_window: Option<Rect>,
    next_error: Option<ScreenSourceError>,
    requests: Vec<Rect>,
}

impl MockScreenSource {
    /// `origin` es la esquina del escritorio virtual que representa `base`.
    pub fn new(origin: (i32, i32), base: Frame) -> Self {
        Self {
            origin,
            base,
            active_window: None,
            next_error: None,
            requests: Vec::new(),
        }
    }

    pub fn set_active_window(&mut self, rect: Option<Rect>) {
        self.active_window = rect;
    }

    /// La siguiente llamada a `capture_region` devolverá este error.
    pub fn fail_next(&mut self, error: ScreenSourceError) {
        self.next_error = Some(error);
    }

    /// Regiones solicitadas, en orden.
    pub fn requests(&self) -> &[Rect] {
        &self.requests
    }
}

impl ScreenSource for MockScreenSource {
    fn desktop_rect(&self) -> Rect {
        Rect::new(
            self.origin.0,
            self.origin.1,
            self.base.width,
            self.base.height,
        )
    }

    fn active_window_rect(&self) -> Option<Rect> {
        self.active_window
    }

    fn capture_region(&mut self, region: Rect) -> Result<Frame, ScreenSourceError> {
        self.requests.push(region);
        if let Some(err) = self.next_error.take() {
            return Err(err);
        }
        if !self.desktop_rect().contains(&region) || region.is_empty() {
            return Err(ScreenSourceError::OutOfBounds(region));
        }
        let local = Rect::new(
            region.x - self.origin.0,
            region.y - self.origin.1,
            region.width,
            region.height,
        );
        self.base
            .crop(&local)
            .map_err(|_| ScreenSourceError::OutOfBounds(region))
    }
}

/// `OutputSink` que acumula lo entregado en memoria.
pub struct MockOutputSink {
    id: &'static str,
    delivered: Vec<Frame>,
    next_error: Option<OutputError>,
}

impl MockOutputSink {
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            delivered: Vec::new(),
            next_error: None,
        }
    }

    /// La siguiente llamada a `deliver` devolverá este error.
    pub fn fail_next(&mut self, error: OutputError) {
        self.next_error = Some(error);
    }

    /// Frames entregados con éxito, en orden.
    pub fn delivered(&self) -> &[Frame] {
        &self.delivered
    }
}

impl OutputSink for MockOutputSink {
    fn id(&self) -> &'static str {
        self.id
    }

    fn deliver(&mut self, frame: &Frame) -> Result<(), OutputError> {
        if let Some(err) = self.next_error.take() {
            return Err(err);
        }
        self.delivered.push(frame.clone());
        Ok(())
    }
}

/// `HotkeyProvider` en memoria con ids incrementales.
pub struct MockHotkeyProvider {
    next_id: u32,
    registered: Vec<(HotkeyId, Hotkey)>,
}

impl MockHotkeyProvider {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            registered: Vec::new(),
        }
    }

    /// Atajos vivos (registrados y no liberados), en orden de registro.
    pub fn registered(&self) -> Vec<(HotkeyId, Hotkey)> {
        self.registered.clone()
    }
}

impl Default for MockHotkeyProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl HotkeyProvider for MockHotkeyProvider {
    fn register(&mut self, hotkey: Hotkey) -> Result<HotkeyId, HotkeyError> {
        if self.registered.iter().any(|(_, h)| *h == hotkey) {
            return Err(HotkeyError::AlreadyRegistered(hotkey));
        }
        let id = HotkeyId(self.next_id);
        self.next_id += 1;
        self.registered.push((id, hotkey));
        Ok(id)
    }

    fn unregister(&mut self, id: HotkeyId) -> Result<(), HotkeyError> {
        let pos = self
            .registered
            .iter()
            .position(|(i, _)| *i == id)
            .ok_or(HotkeyError::UnknownId(id))?;
        self.registered.remove(pos);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::{KeyCode, Modifiers};

    fn mock_2x2() -> MockScreenSource {
        // Canal R = índice del píxel (0..4) para distinguirlos.
        let pixels: Vec<u8> = (0..4u8).flat_map(|i| [i, 0, 0, 255]).collect();
        MockScreenSource::new((-1, -1), Frame::new(2, 2, pixels).unwrap())
    }

    #[test]
    fn desktop_rect_refleja_origen_y_tamano_del_frame_base() {
        let m = mock_2x2();
        assert_eq!(m.desktop_rect(), Rect::new(-1, -1, 2, 2));
    }

    #[test]
    fn capture_region_traduce_coordenadas_de_escritorio_a_frame() {
        let mut m = mock_2x2();
        // El píxel de escritorio (0, 0) es el local (1, 1) → índice 3.
        let f = m.capture_region(Rect::new(0, 0, 1, 1)).unwrap();
        assert_eq!(f.pixel(0, 0), Some([3, 0, 0, 255]));
    }

    #[test]
    fn capture_fuera_del_escritorio_devuelve_out_of_bounds() {
        let mut m = mock_2x2();
        let region = Rect::new(5, 5, 2, 2);
        assert_eq!(
            m.capture_region(region).unwrap_err(),
            ScreenSourceError::OutOfBounds(region)
        );
    }

    #[test]
    fn fail_next_inyecta_el_error_una_sola_vez() {
        let mut m = mock_2x2();
        m.fail_next(ScreenSourceError::Platform("GDI caído".into()));
        assert!(m.capture_region(Rect::new(-1, -1, 1, 1)).is_err());
        assert!(m.capture_region(Rect::new(-1, -1, 1, 1)).is_ok());
    }

    #[test]
    fn registra_las_regiones_solicitadas() {
        let mut m = mock_2x2();
        let r = Rect::new(-1, -1, 2, 2);
        let _ = m.capture_region(r);
        assert_eq!(m.requests(), &[r]);
    }

    #[test]
    fn active_window_es_configurable() {
        let mut m = mock_2x2();
        assert_eq!(m.active_window_rect(), None);
        m.set_active_window(Some(Rect::new(0, 0, 1, 1)));
        assert_eq!(m.active_window_rect(), Some(Rect::new(0, 0, 1, 1)));
    }

    #[test]
    fn el_sink_registra_los_frames_entregados() {
        let mut sink = MockOutputSink::new("clipboard");
        assert_eq!(sink.id(), "clipboard");
        sink.deliver(&Frame::filled(1, 1, [1, 2, 3, 255])).unwrap();
        assert_eq!(sink.delivered().len(), 1);
        assert_eq!(sink.delivered()[0].pixel(0, 0), Some([1, 2, 3, 255]));
    }

    #[test]
    fn fail_next_del_sink_falla_una_sola_vez_y_no_registra() {
        let mut sink = MockOutputSink::new("file");
        sink.fail_next(OutputError::Failed("disco lleno".into()));
        assert!(sink.deliver(&Frame::filled(1, 1, [0; 4])).is_err());
        assert!(sink.deliver(&Frame::filled(1, 1, [0; 4])).is_ok());
        assert_eq!(sink.delivered().len(), 1);
    }

    fn ctrl_shift(c: char) -> Hotkey {
        Hotkey {
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::default()
            },
            key: KeyCode::Char(c),
        }
    }

    #[test]
    fn register_asigna_ids_distintos_y_los_recuerda() {
        let mut hk = MockHotkeyProvider::new();
        let a = hk.register(ctrl_shift('a')).unwrap();
        let b = hk.register(ctrl_shift('b')).unwrap();
        assert_ne!(a, b);
        assert_eq!(
            hk.registered(),
            vec![(a, ctrl_shift('a')), (b, ctrl_shift('b'))]
        );
    }

    #[test]
    fn registrar_el_mismo_atajo_dos_veces_falla() {
        let mut hk = MockHotkeyProvider::new();
        hk.register(ctrl_shift('a')).unwrap();
        assert_eq!(
            hk.register(ctrl_shift('a')).unwrap_err(),
            HotkeyError::AlreadyRegistered(ctrl_shift('a'))
        );
    }

    #[test]
    fn unregister_libera_el_atajo_para_reuso() {
        let mut hk = MockHotkeyProvider::new();
        let id = hk.register(ctrl_shift('a')).unwrap();
        hk.unregister(id).unwrap();
        assert!(hk.registered().is_empty());
        assert!(hk.register(ctrl_shift('a')).is_ok());
    }

    #[test]
    fn unregister_con_id_desconocido_falla() {
        let mut hk = MockHotkeyProvider::new();
        assert_eq!(
            hk.unregister(HotkeyId(99)).unwrap_err(),
            HotkeyError::UnknownId(HotkeyId(99))
        );
    }
}
