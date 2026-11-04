//! Motor de focus-ambience: audio, biblioteca (sonidos, presets, meditaciones, rutinas), protocolo
//! y el [`Player`]. Rust puro: no lanza procesos ni busca rutas del sistema; el que lo usa le pasa
//! las carpetas.

pub mod audio;
pub mod library;
pub mod meditation;
pub mod player;
pub mod protocol;
pub mod routine;

pub use player::{Player, PlayerOptions};
