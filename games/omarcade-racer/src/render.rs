//! Painting the road.
//!
//! Split out of the debug example on purpose. The renderer used to live
//! in `examples/dump_art.rs`, which would have made a *debug harness* the
//! source of truth for how the game looks — so the still and the running
//! game could drift apart and each would look correct on its own terms.
//!
//! One implementation, two callers: the game draws a full window, the
//! example draws panels of a filmstrip. Both go through here.
//!
//! This module decides COLOURS and nothing else. Every coordinate comes
//! from `road.rs`; if the geometry looks wrong, it is wrong somewhere that
//! has tests.

use omarcade_core::{Canvas, Color, Pose, Theme};

use crate::art::Art;
use crate::drive::{Drive, Tuning};
use crate::road::{Camera, Road, Segment};
use crate::scenery;
use crate::structures;

/// The most the grass may differ from the road in luminance.
///
/// A ratio-free absolute in luminance units, which is the one place an
/// absolute is right: it is a statement about human contrast perception,
/// not about any theme's palette. Above roughly 40 the road edge starts
/// reading as a hard stripe rather than as a verge.
const MAX_GRASS_ROAD_LUMA_GAP: f32 = 34.0;

/// The strongest grass mix worth using when the gap allows it.
///
/// Light themes can take a lot of green before the gap closes; this stops
/// the grass going lurid just because it is allowed to.
const MAX_GRASS_MIX: f32 = 0.62;

/// Solve the grass mix so it sits within `MAX_GRASS_ROAD_LUMA_GAP` of the
/// road, taking as much of the theme's green as that permits.
///
/// Walking candidate mixes rather than solving algebraically because
/// `lerp` is per-channel and luminance is a weighted sum — the inverse is
/// not worth deriving for a value computed once per frame.
fn grass_for(theme: &Theme, road: Color) -> Color {
    let road_luma = luma(road);
    let mut best = theme.background.lerp(theme.green, 0.0);
    let mut mix = 0.0f32;
    while mix <= MAX_GRASS_MIX {
        let candidate = theme.background.lerp(theme.green, mix);
        if (luma(candidate) - road_luma).abs() <= MAX_GRASS_ROAD_LUMA_GAP {
            best = candidate;
        }
        mix += 0.01;
    }
    best
}

/// Rec. 709 luminance — the same weighting `Color::desaturated` uses, so
/// "how bright does this read" means one thing across the renderer.
fn luma(c: Color) -> f32 {
    0.2126 * c.r as f32 + 0.7152 * c.g as f32 + 0.0722 * c.b as f32
}

/// How much hue the road surface gives up, 0.0 to 1.0.
///
/// Not 1.0: a road pinned to pure grey stops belonging to the theme at
/// all, and the suite is theme-reactive on purpose. 0.75 leaves every
/// theme's road recognisably tinted — measured, it brings the worst
/// offenders from chroma 0.27 down to about 0.07 — while none of them
/// reads as a coloured surface any more.
const ROAD_DESATURATION: f32 = 0.75;

/// How tall a roadside prop stands, in road half-widths.
///
/// A ratio against the road rather than a pixel size, so props keep their
/// apparent size at any resolution and any road width (L019).
const PROP_HEIGHT_IN_HALF_WIDTHS: f32 = 0.38;

/// How many pixels of car art cover one road half-width.
///
/// ONE number, shared by the player and every rival — they are the same
/// car, so at the same distance they must be the same size on screen.
/// They used not to share it: the player drew at `probe / 105.0 * 1.5`
/// and a rival at `half_width / 105.0`, which is this same constant with
/// the 1.5 dropped — so a rival alongside the player rendered at
/// two-thirds size. Brian spotted it on the test track while passing.
///
/// 48px of car art / 70 ≈ 0.69 half-widths of road covered by a car.
/// The playground's scale check assumes this value; move them together.
///
/// Public because the crash fireball replaces a car and must be sized
/// against the same rule. A second copy of this number in the scene that
/// judges it is precisely the drift that put a structure constant in two
/// files last session.
pub const CAR_ART_PIXELS_PER_HALF_WIDTH: f32 = 70.0;

pub fn demo_track() -> Road {
    let mut segs = Vec::new();
    // Only the first ~15 segments occupy real screen height; past that a
    // band is under a pixel tall (measured). So the bend has to start
    // close, or it renders into a sub-pixel sliver and looks straight.
    segs.extend(std::iter::repeat_n(Segment::STRAIGHT, 4));
    // Eased in and out, because a curve that starts at full strength
    // reads as a kink. This is the shape a real track section has.
    for i in 0..6 {
        segs.push(Segment::curving(90.0 * (i as f32 / 6.0)));
    }
    segs.extend(std::iter::repeat_n(Segment::curving(90.0), 40));
    for i in 0..6 {
        segs.push(Segment::curving(90.0 * (1.0 - i as f32 / 6.0)));
    }
    // A lap, not a loop: at A's top speed 400 segments is a two-second
    // lap. Sized for driving rather than for looking at.
    segs.extend(std::iter::repeat_n(Segment::STRAIGHT, 3_944));
    Road::new(segs, 200.0, 2200.0)
}

/// `draw_road` for an arbitrary sub-rectangle, driven by a live car.
///
/// The full-screen scene is the special case of this; keeping one
/// implementation is what stops the filmstrip and the real view drifting
/// apart and showing different things.
#[allow(clippy::too_many_arguments)]
pub fn draw_road_into(
    c: &mut Canvas<'_>,
    art: &Art,
    theme: &Theme,
    road: &Road,
    tuning: &Tuning,
    car: &Drive,
    roll: f32,
    rivals: &[(f32, f32, usize)],
    ox: u32,
    oy: u32,
    w: u32,
    h: u32,
) {
    // The sky keeps its colour; the HAZE does not.
    //
    // These were one colour and had to be split. `theme.blue` is a teal on
    // this theme (#7fbbb3, green channel above blue, green excess +34), so
    // hazing grey tarmac toward the sky tinted the road green — worst near
    // the horizon where the haze is thickest, and reported as green
    // striping on the track.
    //
    // A first attempt desaturated the sky toward `theme.background` and
    // achieved nothing: background is itself near-neutral, so mixing
    // toward it lowers saturation without touching the blue's hue
    // dominance. Measured, that "fix" moved the sky's green excess from
    // +10.5 to +11.5 — very slightly WORSE. It only looked better because
    // the same change softened the haze alpha, applying less of it.
    //
    // The haze colour is now neutralised outright: green forced to the
    // mean of red and blue, which holds the hazed road at a green excess
    // of ~0 at every depth. The sky keeps its own tone because it is a
    // separate colour now — a sky should look like sky, and distance
    // should not repaint the road.
    let sky = theme.background.lerp(theme.blue, 0.30);
    let haze_tint = {
        let neutral_g = ((sky.r as u16 + sky.b as u16) / 2) as u8;
        Color::rgb(sky.r, neutral_g, sky.b)
    };

    // One shade, desaturated so it reads as TARMAC rather than as paint.
    //
    // Measured across the installed Omarchy themes, a road derived
    // straight from the theme slots ranged from chroma 0.000 to 0.273 —
    // catppuccin, lumon, tokyo-night and retro-82 all put a vividly
    // coloured "road" on screen, and even a mild theme like everforest
    // gave enough hue that the contrast against its much more saturated
    // grass (0.047 road vs 0.258 grass) read as green striping on the
    // track.
    //
    // Capping the road's saturation keeps every theme recolouring the
    // scene — grass, sky, props and cars are untouched — while stopping
    // the one surface that should look like asphalt from taking the
    // theme's hue at full strength. `desaturated` preserves luminance, so
    // this changes the road's colour without changing how bright it reads.
    let road_flat = theme
        .dark_background
        .lerp(theme.foreground, 0.16)
        .desaturated(ROAD_DESATURATION);
    // Grass, mixed to hold a fixed LUMINANCE GAP against the road rather
    // than to a fixed amount of green.
    //
    // A constant mix was the bug. At 0.57 the grass came out BRIGHT while
    // the road on a dark theme is dark, so the road edge became a hard
    // high-contrast boundary — measured, a luminance gap of 62 on
    // everforest and 56 on gruvbox against only 31 on flexoki-light.
    // That is exactly why the striping was obvious on dark themes and
    // barely there on light ones, and why chasing it as a HUE problem
    // through four attempts never landed: the hue was a symptom and the
    // contrast was the cause.
    //
    // A single mix cannot serve both: dark themes need a small one (0.33
    // to 0.41) and light themes a large one (0.60+). So the gap is the
    // constant and the mix is solved for it — L015, one more time.
    let grass_flat = grass_for(theme, road_flat);
    let rumble_a = theme.red.lerp(Color::WHITE, 0.15);
    let rumble_b = theme.foreground.lerp(Color::WHITE, 0.4);
    let line = theme.foreground.lerp(Color::WHITE, 0.5);

    let camera = Camera::for_road(road, 0.85);
    let fx = ox as f32;
    let fy = oy as f32;
    let fw = w as f32;
    let fh = h as f32;
    let horizon = fh / 2.0;

    let x_offset = car.x_offset(road);

    for y in 0..horizon as u32 {
        let t = y as f32 / horizon;
        c.fill_rect(ox as i32, (oy + y) as i32, w, 1, sky.lerp(theme.background, 1.0 - t * 0.7));
    }
    c.fill_rect(ox as i32, (oy + horizon as u32) as i32, w, h - horizon as u32, grass_flat);

    let bands = road.visible(&camera, car.z, x_offset, fw, fh);

    for pair in bands.windows(2).rev() {
        let (near, far) = (pair[0], pair[1]);
        let y0 = far.y.max(horizon);
        let y1 = near.y.min(fh);
        if y1 <= y0 {
            continue;
        }
        let span = near.y - far.y;
        let mut y = y0;
        while y < y1 {
            let bh = (1.0f32).min(y1 - y);
            let t = if span > 0.001 { ((y - far.y) / span).clamp(0.0, 1.0) } else { 1.0 };
            let cx = fx + far.x + (near.x - far.x) * t;
            let hw = far.half_width + (near.half_width - far.half_width) * t;
            let dist = far.distance + (near.distance - far.distance) * t;

            // Two separate groupings, because the ground and the markings
            // have different jobs. The ground is coarse so it cannot
            // appear to run backwards; the markings stay fine so the near
            // road keeps the texture that reads as speed. Neither is a
            // raw divide by segment_length — that is what aliased.
            let z_here = dist + car.z;

            // FLAT. No band, no gradient, no wave.
            //
            // Three attempts to get motion out of this surface all failed:
            // fine bands aliased into running backwards, coarse bands
            // toggled the whole screen at once, and a smooth cosine read
            // as ocean swells — because a smooth luminance gradient along
            // z is exactly the image a corrugated surface makes under
            // diffuse light, and vision reads gradients as curvature. The
            // corrugation was in the still frame; motion only animated it.
            //
            // Pole Position, this game's lineage, had no ground banding
            // either. Large surfaces carry speed MAGNITUDE; discrete
            // world-anchored objects carry motion DIRECTION. The markings
            // below and the props in `scenery.rs` are the motion channel.
            let mark = road.marking_index(z_here) % 2 == 0;
            let sy = fy + y;

            c.fill_rect_f(fx, sy, fw, bh, grass_flat);
            c.fill_rect_f(cx - hw, sy, hw * 2.0, bh, road_flat);

            // `.max(0.7)`: a screen minimum so the strip never vanishes at
            // distance. The FRACTION is shared with physics — the width
            // drawn here is the width that drags there.
            let rumble = (hw * crate::drive::RUMBLE_FRACTION).max(0.7);
            let rc = if mark { rumble_a } else { rumble_b };
            c.fill_rect_f(cx - hw, sy, rumble, bh, rc);
            c.fill_rect_f(cx + hw - rumble, sy, rumble, bh, rc);

            if mark {
                let lw = (hw * 0.035).max(0.5);
                c.fill_rect_f(cx - lw / 2.0, sy, lw, bh, line);
            }

            // Haze measured against the road's ACTUAL reach, not a fixed
            // 60,000 units. That figure predated the road being retuned to
            // a 24,000-unit draw distance, so haze topped out at 36% at
            // the horizon instead of completing — distant road stayed too
            // present, and the partial mix is what let the sky's hue tint
            // it. L015: a constant is measured against something, or it is
            // an untested assumption.
            let reach = road.draw_distance() as f32 * road.segment_length();
            let ht = (dist / reach).clamp(0.0, 1.0);
            // Capped below full: at 235 the far road disappeared into the
            // sky entirely and left a hard line at the horizon. Haze
            // should say "far away", not "gone".
            let a = (ht.powf(1.5) * 175.0) as u8;
            if a > 2 {
                c.fill_rect_f(fx, sy, fw, bh, haze_tint.with_alpha(a));
            }
            y += bh;
        }
    }

    // The player, leaning into whatever bend it is actually in.
    //
    // Scale follows the ROAD's width under the bumper, not the panel
    // height. Tying it to height made the car swamp a narrow panel while
    // the road (which comes from panel width) stayed thin — the two
    // disagreed about how big the world was.
    // Roadside props. THIS is the motion channel — see scenery.rs for why
    // the ground surfaces are not.
    //
    // Placement follows the rival cars exactly, and that is deliberate:
    // an object owns a `z` and is projected. Two earlier attempts at
    // roadside detail positioned it as a function of the SCANLINE instead
    // — a fraction of verge screen-width, then a multiple of the road's
    // screen half-width — and both smeared into diagonal rays converging
    // on the horizon, because both shrink with distance.
    // Structures BEFORE the props and the traffic: they are the biggest
    // things on the track and the furthest from the camera at any given
    // moment, so anything nearer has to be able to paint over them. A
    // billboard drawn last would sit on top of the car passing it.
    structures::draw(
        c,
        art,
        road,
        &camera,
        car.z,
        x_offset,
        &structures::shipped(),
        fx,
        fy,
        fw,
        fh,
    );

    let mut props = scenery::visible_props(road, car.z, art.prop_kinds());
    props.sort_by(|a, b| b.z.total_cmp(&a.z));
    for prop in &props {
        let Some(p) = road.project(&camera, car.z, x_offset, prop.z, fw, fh) else {
            continue;
        };
        if p.y <= horizon + 1.0 || p.y > fh + 200.0 {
            continue;
        }
        let sprite = art.prop(prop.kind);
        // Scale so a prop stands a sensible fraction of the ROAD's width,
        // rather than at a factor copied from elsewhere.
        //
        // A post should read as roughly waist-high next to a car: the car
        // is 64px of art covering about 0.9 of a half-width, so one unit
        // of half-width is ~71px of art. A post 8 rows tall wants to be
        // about a third of a half-width, which puts its scale at
        // half_width * 0.33 / 8.
        //
        // The previous value (half_width / 105.0 * 1.6) was inherited from
        // the old post-drawing code and derived from nothing; at that
        // scale a post was a couple of pixels and never registered.
        let s = p.half_width * PROP_HEIGHT_IN_HALF_WIDTHS / sprite.height() as f32;
        let w = sprite.width() as f32 * s;
        let h = sprite.height() as f32 * s;
        let haze = (p.distance / (road.draw_distance() as f32 * road.segment_length()))
            .clamp(0.0, 1.0)
            * 0.85;
        sprite.draw_tinted(
            c,
            fx + p.x + prop.lane * p.half_width - w / 2.0,
            fy + p.y - h,
            s,
            Some((haze_tint, haze)),
        );
    }

    // Rivals, placed by track position and lane, drawn far-to-near so a
    // nearer car occludes a further one.
    let mut traffic: Vec<&(f32, f32, usize)> = rivals.iter().collect();
    traffic.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (rz, lane, livery) in traffic {
        let Some(p) = road.project(&camera, car.z, x_offset, *rz, fw, fh) else {
            continue;
        };
        if p.y <= horizon + 1.0 || p.y > fh + 200.0 {
            continue;
        }
        let s = p.half_width / CAR_ART_PIXELS_PER_HALF_WIDTH;
        let haze = (p.distance / (road.draw_distance() as f32 * road.segment_length()))
            .clamp(0.0, 1.0)
            * 0.8;
        let rival = art.rival(*livery);
        let w = rival.width() as f32 * s;
        let h = rival.height() as f32 * s;
        rival.draw_tinted(
            c,
            fx + p.x + lane * p.half_width - w / 2.0,
            fy + p.y - h,
            s,
            Some((haze_tint, haze)),
        );
    }

    // Scale from the road at a FIXED distance ahead, never from
    // `bands.first()`: the nearest band changes identity as the camera
    // crosses a segment boundary — whole segment one frame, sliver the
    // next — so anything sized from it pulses. A fixed probe distance is
    // continuous, which is what a sprite scale has to be.
    let pose = Pose::cornering(car.cornering(road, tuning));
    let probe = road
        .project(&camera, car.z, x_offset, car.z + road.segment_length(), fw, fh)
        .map(|p| p.half_width)
        .unwrap_or(fw * 0.4);
    let scale = probe / CAR_ART_PIXELS_PER_HALF_WIDTH;

    // The player sits where the camera is — dead centre — because the
    // camera IS the car. Steering moves the world, not the sprite.
    art.player.draw_ground_rolling(
        c,
        fx + fw / 2.0,
        fy + fh * 0.98,
        scale,
        pose,
        roll,
        None,
    );
}
