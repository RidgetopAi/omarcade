//! Shared engine for the Omarcade suite.
//!
//! The important thing here is [`backend`]: it is the seam games are
//! written against. A game depends on this crate and on nothing
//! platform-specific.
//!
//! [`geom`] is the shared vector and rectangle maths every title so
//! far has needed, and [`theme`] reads the live Omarchy palette.

pub mod backend;
pub mod ease;
pub mod geom;
pub mod scores;
pub mod theme;

pub use backend::{Backend, Canvas, Color, Game, InputEvent, Key};
pub use geom::{Axis, Rect, Vec2};
pub use theme::Theme;
