//! Dominio puro de la app de captura: cero Win32, cero UI (D1, D2).
//!
//! Organizado por slices verticales (D3); las fronteras con el sistema
//! se expresan como traits en [`ports`].

pub mod annotate;
pub mod capture;
pub mod config;
pub mod output;
pub mod ports;
pub mod record;
pub mod tools;
