//! Shared engine for the Omarcade suite.
//!
//! The important thing here is [`backend`]: it is the seam games are
//! written against. A game depends on this crate and on nothing
//! platform-specific.
//!
//! `theme` lands in file 4.

pub mod backend;

pub use backend::{Backend, Canvas, Color, Game, InputEvent, Key};
