//! Shared engine for the Omarcade suite.
//!
//! The important thing here is [`backend`]: it is the seam games are
//! written against. A game depends on this crate and on nothing
//! platform-specific.
//!
//! [`theme`] reads the live Omarchy palette.

pub mod backend;
pub mod theme;

pub use backend::{Backend, Canvas, Color, Game, InputEvent, Key};
pub use theme::Theme;
