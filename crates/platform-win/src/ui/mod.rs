//! Infraestructura común de la UI rediseñada (F3.5): tema dual, iconos
//! tintables desde los atlas A8, botón owner-draw, tooltips, fuentes y
//! layout en unidades lógicas. Interno al crate: nada de `windows` sale
//! de aquí hacia fuera.

pub(crate) mod boton;
pub(crate) mod botonera;
pub(crate) mod fuentes;
pub(crate) mod icono_app;
pub(crate) mod iconos;
pub(crate) mod layout;
pub(crate) mod lienzo;
pub(crate) mod theme;
pub(crate) mod tooltip;
pub(crate) mod ventana;
