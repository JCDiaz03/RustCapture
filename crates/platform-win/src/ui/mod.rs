//! Infraestructura común de la UI rediseñada (F3.5): tema dual, iconos
//! tintables desde los atlas A8, botón owner-draw, tooltips, fuentes y
//! layout en unidades lógicas. Interno al crate: nada de `windows` sale
//! de aquí hacia fuera.

// PENDIENTE(F3.5): retirar cuando barra/editor/overlay consuman el módulo
// (S3-S6); hasta entonces la infraestructura aún no tiene llamadores.
#![allow(dead_code)]

pub(crate) mod boton;
pub(crate) mod fuentes;
pub(crate) mod iconos;
pub(crate) mod layout;
pub(crate) mod theme;
pub(crate) mod tooltip;
