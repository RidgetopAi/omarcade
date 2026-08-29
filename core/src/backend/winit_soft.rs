//! The shipping backend: winit window + softbuffer software rendering.
//!
//! This is the only module in the suite that names winit or softbuffer
//! types. Everything platform-specific is trapped here; games see the
//! seam in `super` and nothing else.
//!
//! winit's Wayland support *is* smithay-client-toolkit, so this runs
//! natively under Hyprland with no XWayland in the picture.

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};

use super::{Canvas, Game, InputEvent, Key};

/// Wayland app_id / X11 class. Hyprland window rules match on this, so
/// it is part of our public surface: changing it breaks users' configs.
pub const APP_ID: &str = "omarcade";

/// Backend errors. Deliberately opaque to games — a game cannot do
/// anything useful about a compositor failure except exit.
#[derive(Debug)]
pub enum Error {
    EventLoop(winit::error::EventLoopError),
    Os(winit::error::OsError),
    Surface(softbuffer::SoftBufferError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::EventLoop(e) => write!(f, "event loop: {e}"),
            Error::Os(e) => write!(f, "window creation: {e}"),
            Error::Surface(e) => write!(f, "surface: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<winit::error::EventLoopError> for Error {
    fn from(e: winit::error::EventLoopError) -> Self {
        Error::EventLoop(e)
    }
}

impl From<winit::error::OsError> for Error {
    fn from(e: winit::error::OsError) -> Self {
        Error::Os(e)
    }
}

impl From<softbuffer::SoftBufferError> for Error {
    fn from(e: softbuffer::SoftBufferError) -> Self {
        Error::Surface(e)
    }
}

/// How the loop should idle.
///
/// This is the setting the "~0% CPU at idle" requirement turns on, and
/// it is a real tradeoff rather than a constant:
///
/// - [`Idle::Wait`] blocks until the compositor sends something. A
///   static screen costs no CPU at all. But nothing generates events on
///   its own, so a game with motion would simply stop animating.
/// - [`Idle::Animate`] asks for a redraw at a target rate, so motion
///   works, at the cost of waking that many times a second.
///
/// Session 1 has nothing moving, so it ships `Wait` and genuinely idles
/// at zero. Breakout will switch to `Animate` when there is a ball to
/// move. Note that `Animate` uses `WaitUntil`, never `Poll` — `Poll`
/// spins the loop as fast as the CPU allows and is the busy-loop this
/// requirement exists to prevent.
#[derive(Debug, Clone, Copy)]
pub enum Idle {
    /// Redraw only when the compositor asks. True zero idle cost.
    Wait,
    /// Redraw continuously at roughly this many frames per second.
    Animate { fps: u32 },
}

/// Window configuration.
pub struct WinitBackend {
    title: String,
    width: u32,
    height: u32,
    idle: Idle,
}

impl WinitBackend {
    pub fn new(title: impl Into<String>, width: u32, height: u32) -> Self {
        WinitBackend { title: title.into(), width, height, idle: Idle::Wait }
    }

    /// Choose how the loop idles. See [`Idle`].
    pub fn idle(mut self, idle: Idle) -> Self {
        self.idle = idle;
        self
    }
}

impl super::Backend for WinitBackend {
    type Error = Error;

    fn run<G: Game>(self, game: G) -> Result<(), Error> {
        let event_loop = EventLoop::new()?;

        // Wait, not Poll: block until there is something to do. With
        // Poll this process would peg a core doing nothing.
        event_loop.set_control_flow(match self.idle {
            Idle::Wait => ControlFlow::Wait,
            // First frame due immediately; each painted frame then
            // schedules the next. Never Poll — Poll spins the loop as
            // fast as the CPU allows.
            Idle::Animate { .. } => ControlFlow::WaitUntil(Instant::now()),
        });

        let mut app = App {
            cfg: self,
            game,
            window: None,
            surface: None,
            last_frame: None,
            error: None,
        };

        event_loop.run_app(&mut app)?;

        // A failure inside a handler cannot return through winit, so it
        // is stashed and surfaced here instead of being swallowed.
        match app.error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

/// Live state for one run. Generic over the game, so this file never
/// names a concrete title either — it depends on the seam in both
/// directions.
struct App<G: Game> {
    cfg: WinitBackend,
    game: G,
    /// `Rc` because softbuffer's `Surface` holds the window too, and a
    /// struct owning both a window and a borrow of it cannot be
    /// expressed safely.
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    last_frame: Option<Instant>,
    error: Option<Error>,
}

impl<G: Game> App<G> {
    /// Record a fatal error and start unwinding the loop.
    fn fail(&mut self, event_loop: &ActiveEventLoop, e: Error) {
        if self.error.is_none() {
            self.error = Some(e);
        }
        event_loop.exit();
    }

    /// Paint one frame.
    ///
    /// The surface borrow is confined to `paint` so that error handling,
    /// which needs `&mut self` again, happens after it has ended.
    fn redraw(&mut self, event_loop: &ActiveEventLoop) {
        if let Err(e) = self.paint() {
            self.fail(event_loop, e);
        }
    }

    /// The frame itself. Returns `Ok(())` for "nothing to draw" as well
    /// as for a painted frame; only real failures come back as `Err`.
    fn paint(&mut self) -> Result<(), Error> {
        let (Some(window), Some(surface)) = (self.window.as_ref(), self.surface.as_mut()) else {
            return Ok(());
        };

        let size = window.inner_size();
        // A minimised window reports 0x0. NonZeroU32::new would fail and
        // there is nothing to draw anyway, so skip the frame entirely.
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return Ok(());
        };

        surface.resize(w, h)?;

        let now = Instant::now();
        let dt = self.last_frame.map_or(0.0, |t| (now - t).as_secs_f32());
        self.last_frame = Some(now);
        self.game.update(dt);

        let mut buffer = surface.buffer_mut()?;

        {
            let mut canvas = Canvas::new(&mut buffer, size.width, size.height);
            self.game.render(&mut canvas);
        }

        buffer.present()?;
        Ok(())
    }

    /// Hand an event to the game, exiting if it says to stop.
    fn deliver(&mut self, event_loop: &ActiveEventLoop, event: InputEvent) {
        if !self.game.on_input(event) {
            event_loop.exit();
        }
    }
}

impl<G: Game> ApplicationHandler for App<G> {
    /// winit 0.30 requires window creation here, not before the loop:
    /// some platforms refuse a render surface until the app is resumed.
    /// Resumed can fire more than once, so this must be idempotent.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = window_attributes(&self.cfg);

        let window = match event_loop.create_window(attrs) {
            Ok(w) => Rc::new(w),
            Err(e) => return self.fail(event_loop, e.into()),
        };

        let context = match softbuffer::Context::new(window.clone()) {
            Ok(c) => c,
            Err(e) => return self.fail(event_loop, e.into()),
        };

        let surface = match softbuffer::Surface::new(&context, window.clone()) {
            Ok(s) => s,
            Err(e) => return self.fail(event_loop, e.into()),
        };

        self.window = Some(window);
        self.surface = Some(surface);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            // The compositor asked us to close (Super+Q, title-bar X).
            // Handed to the game so it can save, but not refusable.
            WindowEvent::CloseRequested => {
                self.game.on_input(InputEvent::CloseRequested);
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                self.deliver(
                    event_loop,
                    InputEvent::Resized { width: size.width, height: size.height },
                );
            }

            WindowEvent::KeyboardInput {
                event: KeyEvent { physical_key, state, repeat, .. },
                ..
            } => {
                // Physical key, not logical: WASD should stay WASD on a
                // Dvorak layout, the way every other game behaves.
                let PhysicalKey::Code(code) = physical_key else {
                    return;
                };
                // Held keys autorepeat at the keyboard's rate; games
                // track held state themselves, so repeats are noise.
                if repeat {
                    return;
                }
                let Some(key) = translate_key(code) else {
                    return;
                };
                let event = match state {
                    ElementState::Pressed => InputEvent::KeyDown(key),
                    ElementState::Released => InputEvent::KeyUp(key),
                };
                self.deliver(event_loop, event);
            }

            WindowEvent::RedrawRequested => {
                self.redraw(event_loop);

                // Schedule the next frame. Doing it here, after the
                // frame is painted, means a slow frame delays the next
                // one rather than queuing up a backlog.
                if let Idle::Animate { fps } = self.cfg.idle {
                    let fps = fps.max(1);
                    let frame = std::time::Duration::from_nanos(1_000_000_000 / fps as u64);
                    event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + frame));
                }
            }

            _ => {}
        }
    }

    /// Fires when the `WaitUntil` deadline expires (or any event
    /// arrives). Requesting a redraw HERE, rather than on every loop
    /// iteration, is what keeps `Animate` at its target rate.
    ///
    /// Requesting unconditionally is a busy loop wearing a disguise: the
    /// redraw is queued instantly, so the loop never actually waits on
    /// the deadline it just set, and the process pegs a core at ~100%.
    /// Measured, not theorised — the first version of this did exactly
    /// that.
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        let due = matches!(
            cause,
            StartCause::ResumeTimeReached { .. } | StartCause::Init
        );
        if due && matches!(self.cfg.idle, Idle::Animate { .. }) {
            if let Some(window) = self.window.as_ref() {
                window.request_redraw();
            }
        }
    }
}

/// Build the window attributes, setting the Wayland app_id so Hyprland
/// window rules can target us.
fn window_attributes(cfg: &WinitBackend) -> WindowAttributes {
    let attrs = Window::default_attributes()
        .with_title(cfg.title.clone())
        .with_inner_size(winit::dpi::LogicalSize::new(cfg.width, cfg.height));

    #[cfg(all(unix, not(target_os = "macos")))]
    let attrs = {
        use winit::platform::wayland::WindowAttributesExtWayland;
        attrs.with_name(APP_ID, "")
    };

    attrs
}

/// winit keycodes to our own [`Key`]. Unmapped keys return `None` and
/// are dropped before they reach the game.
fn translate_key(code: KeyCode) -> Option<Key> {
    Some(match code {
        KeyCode::ArrowLeft | KeyCode::KeyA => Key::Left,
        KeyCode::ArrowRight | KeyCode::KeyD => Key::Right,
        KeyCode::ArrowUp | KeyCode::KeyW => Key::Up,
        KeyCode::ArrowDown | KeyCode::KeyS => Key::Down,
        KeyCode::Space => Key::Space,
        KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,
        KeyCode::Escape => Key::Escape,
        KeyCode::KeyP => Key::P,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrows_and_wasd_are_the_same_keys() {
        assert_eq!(translate_key(KeyCode::ArrowLeft), Some(Key::Left));
        assert_eq!(translate_key(KeyCode::KeyA), Some(Key::Left));
        assert_eq!(translate_key(KeyCode::ArrowRight), Some(Key::Right));
        assert_eq!(translate_key(KeyCode::KeyD), Some(Key::Right));
    }

    #[test]
    fn escape_and_space_map() {
        assert_eq!(translate_key(KeyCode::Escape), Some(Key::Escape));
        assert_eq!(translate_key(KeyCode::Space), Some(Key::Space));
    }

    #[test]
    fn unmapped_keys_are_dropped() {
        assert_eq!(translate_key(KeyCode::F7), None);
        assert_eq!(translate_key(KeyCode::KeyQ), None);
    }
}
