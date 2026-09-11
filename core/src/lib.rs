//! Shared engine for the Omarcade suite.
//!
//! The important thing here is [`backend`]: it is the seam games are
//! written against. A game depends on this crate and on nothing
//! platform-specific.
//!
//! [`geom`] is the shared vector and rectangle maths every title so
//! far has needed, [`sprite`] turns authored pixel art into drawable
//! pixels, and [`theme`] reads the live Omarchy palette.
//!
//! [`particles`] is a fixed-capacity pool for effects: allocated once,
//! never per frame, drawn as added light.
//!
//! [`audio`] is the second seam: core owns the output stream, the mixer
//! and the volume, and games hand it sounds. A game's engine noise
//! lives in that game, so a title that wants none pays for none.

pub mod audio;
pub mod backend;
pub mod ease;
pub mod geom;
pub mod particles;
pub mod pause;
pub mod scores;
pub mod sprite;
pub mod text;
pub mod volume;
pub mod theme;

pub use audio::{Audio, AudioSystem, SoundId, Voice, VoiceId, VoiceParams};
pub use pause::Pause;
pub use volume::VolumeIndicator;
pub use backend::{Backend, Canvas, Color, Game, InputEvent, Key};
pub use geom::{Axis, Rect, Vec2};
pub use particles::{Particle, ParticlePool};
pub use sprite::{Pose, Roll, Sprite};
pub use theme::Theme;
