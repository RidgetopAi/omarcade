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
    fixed: bool,
}

impl WinitBackend {
    pub fn new(title: impl Into<String>, width: u32, height: u32) -> Self {
        WinitBackend { title: title.into(), width, height, idle: Idle::Wait, fixed: false }
    }

    /// Choose how the loop idles. See [`Idle`].
    pub fn idle(mut self, idle: Idle) -> Self {
        self.idle = idle;
        self
    }

    /// Render at a fixed resolution and scale the result to the window.
    ///
    /// ⚠️ OPT-IN, AND MOST GAMES SHOULD NOT TAKE IT. A game whose
    /// renderer reads `canvas.width()`/`height()` already adapts to any
    /// window, natively and at full sharpness; routing it through a
    /// fixed buffer would throw that away and scale a picture that did
    /// not need scaling. Pixel Break and Volley are in that group.
    ///
    /// This exists for a renderer that CANNOT adapt. The racer projects
    /// a pseudo-3D road from compile-time constants, and its collision
    /// geometry is derived from the renderer's own numbers — "the hitbox
    /// is the art". Resizing its window used to stretch a 960x720
    /// picture across the corner of a larger buffer. With this set the
    /// game keeps drawing exactly the frame it was tuned for and the
    /// BACKEND does the resizing, so the projection math and the
    /// hitboxes never learn the window changed.
    ///
    /// The scale is nearest-neighbour and that is a MEASURED choice, not
    /// a default — see `probe_scale`. Against the real wallpaper sky at
    /// 1920x1440 nearest cost 0.97 ms and bilinear 10.41 ms, and nearest
    /// also looked SHARPER: `backdrop.rs` box-filters the wallpaper once
    /// at start-up, so the image reaching the frame is already smooth and
    /// interpolating it again only blurs it.
    pub fn fixed(mut self, fixed: bool) -> Self {
        self.fixed = fixed;
        self
    }
}

/// Fits a fixed-size frame into a window and scales it there.
///
/// Owns the scratch buffer the game draws into, so the game's canvas is
/// always exactly the size it asked for no matter what the compositor
/// hands us.
struct Scaler {
    /// The resolution the game renders at. Never changes.
    src_w: u32,
    src_h: u32,
    /// What the game draws into.
    scratch: Vec<u32>,
    /// Destination rect inside the window, and the window size it was
    /// computed for.
    win_w: u32,
    win_h: u32,
    dst_x: u32,
    dst_y: u32,
    dst_w: u32,
    dst_h: u32,
    /// `cols[i]` is the source column for destination column `i`.
    ///
    /// Precomputed per window size rather than per pixel: the inner loop
    /// becomes an indexed copy instead of a multiply and a divide, and
    /// the table only changes when someone drags the window edge.
    cols: Vec<u32>,
}

impl Scaler {
    fn new(src_w: u32, src_h: u32) -> Scaler {
        Scaler {
            src_w,
            src_h,
            scratch: vec![0; (src_w as usize) * (src_h as usize)],
            win_w: 0,
            win_h: 0,
            dst_x: 0,
            dst_y: 0,
            dst_w: 0,
            dst_h: 0,
            cols: Vec::new(),
        }
    }

    /// Recompute the fit, but only when the window size actually changed.
    fn plan(&mut self, win_w: u32, win_h: u32) {
        if self.win_w == win_w && self.win_h == win_h {
            return;
        }
        self.win_w = win_w;
        self.win_h = win_h;

        // The largest source-aspect rect that fits. Integer math
        // throughout: a float ratio can land a pixel wide of the window
        // and panic the blit at exactly one window size, which is the
        // kind of bug that only shows up on someone else's monitor.
        let by_width = win_w * self.src_h / self.src_w.max(1);
        let (dw, dh) = if by_width <= win_h {
            (win_w, by_width)
        } else {
            (win_h * self.src_w / self.src_h.max(1), win_h)
        };
        self.dst_w = dw.max(1);
        self.dst_h = dh.max(1);
        // Centre it; the remainder goes to the right/bottom bar rather
        // than being split, so the two bars can differ by one pixel and
        // nothing overruns.
        self.dst_x = (win_w - self.dst_w) / 2;
        self.dst_y = (win_h - self.dst_h) / 2;

        self.cols = (0..self.dst_w).map(|x| x * self.src_w / self.dst_w).collect();
    }

    /// Blit the scratch buffer into `out`, letterboxed.
    fn present(&self, out: &mut [u32]) {
        let win_w = self.win_w as usize;
        let (dst_x, dst_y) = (self.dst_x as usize, self.dst_y as usize);
        let (dst_w, dst_h) = (self.dst_w as usize, self.dst_h as usize);
        let src_w = self.src_w as usize;

        // The bars. Only the rows and columns outside the picture, so a
        // window that already matches the aspect writes nothing extra.
        let top = dst_y * win_w;
        out[..top].fill(0);
        let len = out.len();
        let bottom = ((dst_y + dst_h) * win_w).min(len);
        out[bottom..].fill(0);

        for dy in 0..dst_h {
            let sy = dy * self.src_h as usize / dst_h;
            let srow = &self.scratch[sy * src_w..][..src_w];
            let row = &mut out[(dst_y + dy) * win_w..][..win_w];
            row[..dst_x].fill(0);
            row[dst_x + dst_w..].fill(0);
            let drow = &mut row[dst_x..][..dst_w];
            for dx in 0..dst_w {
                drow[dx] = srow[self.cols[dx] as usize];
            }
        }
    }
}

impl super::Backend for WinitBackend {
    type Error = Error;

    fn run<G: Game>(self, game: G, audio: crate::AudioSystem) -> Result<(), Error> {
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

        // Every voice is registered by now, so opening the device here
        // is what makes registration startup-only: from this point the
        // audio thread is live and nothing new may be handed to it.
        let mut audio = audio;
        audio.start();

        let scaler = self.fixed.then(|| Scaler::new(self.width, self.height));

        let mut app = App {
            cfg: self,
            game,
            audio,
            window: None,
            surface: None,
            last_frame: None,
            error: None,
            scaler,
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
    /// Outlives every frame: dropping this stops the stream.
    audio: crate::AudioSystem,
    /// `Rc` because softbuffer's `Surface` holds the window too, and a
    /// struct owning both a window and a borrow of it cannot be
    /// expressed safely.
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    last_frame: Option<Instant>,
    error: Option<Error>,
    /// `Some` only for a game that asked for [`WinitBackend::fixed`].
    /// `None` is the native path, byte-for-byte as it was before this
    /// existed.
    scaler: Option<Scaler>,
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
        {
            let mut audio = self.audio.handle();
            self.game.update(dt, &mut audio);
        }

        let mut buffer = surface.buffer_mut()?;

        match &mut self.scaler {
            // Fixed resolution: the game draws into the scratch buffer at
            // the size it was built for, and the backend scales that into
            // the window. The game cannot tell the difference — `Canvas`
            // wraps any slice, so it sees exactly the dimensions it asked
            // for whatever the compositor gave us.
            Some(scaler) => {
                scaler.plan(size.width, size.height);
                {
                    let mut canvas =
                        Canvas::new(&mut scaler.scratch, scaler.src_w, scaler.src_h);
                    self.game.render(&mut canvas);
                }
                scaler.present(&mut buffer);
            }
            // Native: the game renders straight into the window at
            // whatever size it is, full sharpness, no copy.
            None => {
                let mut canvas = Canvas::new(&mut buffer, size.width, size.height);
                self.game.render(&mut canvas);
            }
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
                // ⚠️ A FIXED-RESOLUTION GAME IS NEVER RESIZED, so it is
                // told its own size, not the window's. Reporting the
                // window would hand it numbers its canvas never has — a
                // game that laid anything out from this event would put
                // it off-screen, and the bug would only appear once
                // someone dragged the window. The backend absorbs the
                // resize; the game's world is the size it always was.
                let (width, height) = match &self.scaler {
                    Some(s) => (s.src_w, s.src_h),
                    None => (size.width, size.height),
                };
                self.deliver(event_loop, InputEvent::Resized { width, height });
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

                // Volume belongs to the SUITE, not to any game, so these
                // never reach `on_input`. If they did, every title would
                // have to remember to forward three keys it does not
                // care about, and the first one to forget would ship
                // with mute silently doing nothing.
                if matches!(key, Key::M | Key::Minus | Key::Equals) {
                    if state == ElementState::Pressed {
                        match key {
                            Key::M => self.audio.toggle_mute(),
                            Key::Minus => self.audio.nudge_volume(false),
                            _ => self.audio.nudge_volume(true),
                        }
                    }
                    return;
                }

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

    // A floor for a scaled game, so the picture cannot be dragged down
    // to a few pixels. Quarter size still reads; below that the HUD text
    // is gone and there is nothing to look at.
    //
    // No MAXIMUM: the scale is ~1 ms at 1920x1440 (measured, probe_scale)
    // and the cost is in the destination pixels, which the compositor
    // would have to push anyway.
    let attrs = if cfg.fixed {
        attrs.with_min_inner_size(winit::dpi::LogicalSize::new(cfg.width / 4, cfg.height / 4))
    } else {
        attrs
    };

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
        KeyCode::KeyM => Key::M,
        KeyCode::Minus | KeyCode::NumpadSubtract => Key::Minus,
        KeyCode::Equal | KeyCode::NumpadAdd => Key::Equals,
        // ★ The developer key. Compiled out of release builds entirely —
        // see `Key::F8`.
        #[cfg(debug_assertions)]
        KeyCode::F8 => Key::F8,
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

    /// ★ The developer key reaches a debug build and nothing else does.
    ///
    /// ⚠️ This test can only ever run in debug — `cargo test` is a debug
    /// profile — so it proves the key ARRIVES, not that it is absent from
    /// release. The absence is enforced by `#[cfg(debug_assertions)]` on
    /// the variant itself: in release there is no `Key::F8` to name, so a
    /// release build that tried to use it would not compile.
    #[cfg(debug_assertions)]
    #[test]
    fn the_developer_key_is_delivered_in_debug() {
        assert_eq!(translate_key(KeyCode::F8), Some(Key::F8));
    }

    // ---- the fixed-resolution scaler -------------------------------
    //
    // ⚠️ These are about GEOMETRY, which is what actually broke: the old
    // behaviour drew a 960x720 picture into the corner of a larger
    // buffer. Whether the picture is pretty is a question for
    // `probe_scale` and for playing it; these pin down where it lands
    // and that nothing writes outside the window.

    /// The picture keeps its aspect and is centred, at any window size.
    #[test]
    fn the_fit_keeps_aspect_and_centres() {
        for (w, h) in [(960, 720), (1920, 1440), (1280, 720), (700, 1200), (241, 3001)] {
            let mut s = Scaler::new(960, 720);
            s.plan(w, h);

            assert!(s.dst_w <= w && s.dst_h <= h, "{w}x{h}: picture escapes the window");

            // 4:3 within a pixel of rounding.
            let err = (s.dst_w as i64 * 720 - s.dst_h as i64 * 960).abs();
            assert!(
                err <= 960.max(720) as i64,
                "{w}x{h}: aspect drifted — {}x{}",
                s.dst_w,
                s.dst_h
            );

            // Centred: the two bars differ by at most the odd pixel.
            let left = s.dst_x;
            let right = w - s.dst_w - s.dst_x;
            assert!(right as i64 - left as i64 <= 1, "{w}x{h}: not centred horizontally");
            let top = s.dst_y;
            let bottom = h - s.dst_h - s.dst_y;
            assert!(bottom as i64 - top as i64 <= 1, "{w}x{h}: not centred vertically");
        }
    }

    /// It fills the window — every pixel written, none written twice,
    /// none outside.
    ///
    /// ⚠️ THIS IS THE ONE THAT FAILS AGAINST THE OLD BEHAVIOUR. Drawing
    /// the source into the corner leaves the rest of a larger buffer
    /// untouched, which is exactly the symptom Brian reported.
    #[test]
    fn every_window_pixel_is_written() {
        for (w, h) in [(960, 720), (1920, 1440), (1500, 900), (640, 900), (301, 227)] {
            let mut s = Scaler::new(960, 720);
            s.plan(w, h);
            // A sentinel no scaled pixel can be, so "untouched" is visible.
            let mut out = vec![0xDEAD_BEEF_u32; (w as usize) * (h as usize)];
            s.scratch.fill(0x0011_2233);
            s.present(&mut out);
            assert!(
                !out.contains(&0xDEAD_BEEF),
                "{w}x{h}: left {} window pixels unpainted",
                out.iter().filter(|p| **p == 0xDEAD_BEEF).count()
            );
        }
    }

    /// A window that is not 4:3 gets bars, and they are actually bars —
    /// black, outside the picture, on the correct axis.
    #[test]
    fn the_margin_is_letterboxed_not_stretched() {
        // Far too wide: pillars left and right, no bars top or bottom.
        let mut s = Scaler::new(960, 720);
        s.plan(2000, 750);
        s.scratch.fill(0x00FF_FFFF);
        let mut out = vec![0u32; 2000 * 750];
        s.present(&mut out);
        assert!(s.dst_x > 0, "expected pillars");
        assert_eq!(s.dst_y, 0, "expected no letterbox on the short axis");
        let mid = 375 * 2000;
        assert_eq!(out[mid], 0, "the left pillar should be black");
        assert_eq!(out[mid + s.dst_x as usize + 1], 0x00FF_FFFF, "the picture should be lit");
        assert_eq!(out[mid + 1999], 0, "the right pillar should be black");
    }

    /// The plan is rebuilt when the window changes and not before —
    /// the column table is per-size work, not per-frame work.
    #[test]
    fn the_plan_is_rebuilt_only_on_a_real_resize() {
        let mut s = Scaler::new(960, 720);
        s.plan(1280, 960);
        let first = s.cols.as_ptr();
        s.plan(1280, 960);
        assert_eq!(s.cols.as_ptr(), first, "recomputed for an unchanged size");
        s.plan(1281, 960);
        assert_eq!(s.cols.len(), s.dst_w as usize, "table did not follow the new width");
    }

    /// Every source column the table names is in range.
    ///
    /// An off-by-one here indexes past the scratch row and panics mid
    /// frame — on one particular window width, on someone else's monitor.
    #[test]
    fn the_column_table_stays_in_range() {
        for (w, h) in [(960, 720), (1920, 1440), (3000, 2000), (100, 100), (7, 9)] {
            let mut s = Scaler::new(960, 720);
            s.plan(w, h);
            assert_eq!(s.cols.len(), s.dst_w as usize);
            for (i, c) in s.cols.iter().enumerate() {
                assert!(*c < s.src_w, "{w}x{h}: column {i} samples {c}, outside {}", s.src_w);
            }
        }
    }

    /// At 1:1 the scaled path reproduces the source EXACTLY.
    ///
    /// The guarantee that turning this on costs a game nothing when the
    /// window is left alone: same pixels, not merely similar ones.
    #[test]
    fn one_to_one_is_lossless() {
        let mut s = Scaler::new(960, 720);
        s.plan(960, 720);
        for (i, px) in s.scratch.iter_mut().enumerate() {
            *px = (i as u32).wrapping_mul(2_654_435_761) & 0x00FF_FFFF;
        }
        let mut out = vec![0u32; 960 * 720];
        s.present(&mut out);
        assert_eq!(out, s.scratch, "1:1 should be a copy");
    }
}
