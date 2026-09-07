//! The only file that names cpal.
//!
//! Everything above this talks in [`Voice`], `Command` and `Mixer`. If
//! the device layer is ever replaced — a different crate, or PipeWire
//! directly for the layer-shell work — this file is what changes, and
//! the games do not. That is the same seam
//! [`backend`](crate::backend) draws for the window.

use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use super::mixer::{Mixer, Voice};
use super::ring::Ring;

/// Anything that must stay alive for the stream to keep playing.
///
/// cpal stops a stream when its handle drops, so the caller holds this
/// and never looks inside it.
type Handle = Box<dyn super::StreamHandle>;

/// Open the default output device and start playing.
///
/// Returns an error rather than panicking for every failure — no host,
/// no device, no supported config, a busy server. The caller turns that
/// into one line on stderr and a silent game.
pub(super) fn open(
    slots: Vec<(Box<dyn Voice>, bool)>,
    ring: Arc<Ring>,
    master: f32,
) -> Result<Handle, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "no default output device".to_string())?;
    let config = device
        .default_output_config()
        .map_err(|e| format!("no usable output config: {e}"))?;

    let sample_rate = config.sample_rate() as f32;
    let channels = config.channels() as usize;

    // The largest block the device may ask for. cpal will not say in
    // advance, so this is a ceiling generous enough that `Mixer` never
    // needs to grow its scratch — which it could not do, since growing
    // means allocating on the audio thread.
    const MAX_BLOCK: usize = 4096;

    let mut mixer = Mixer::new(slots, ring, sample_rate, MAX_BLOCK, master);

    // Moving `mixer` into this closure is the one time anything crosses
    // to the audio thread, and it happens before the stream starts. From
    // here on the two threads share only the ring and its atomics.
    let stream = device
        .build_output_stream(
            config.into(),
            move |out: &mut [f32], _: &cpal::OutputCallbackInfo| {
                mixer.fill(out, channels);
            },
            |e| eprintln!("omarcade: audio stream error: {e}"),
            None,
        )
        .map_err(|e| format!("could not build the output stream: {e}"))?;

    stream
        .play()
        .map_err(|e| format!("could not start the stream: {e}"))?;

    Ok(Box::new(stream))
}
