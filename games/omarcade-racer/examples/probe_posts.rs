//! Can the car actually hit a sign, and where does it have to be?
//!
//! The collision is geometry, and geometry is the kind of thing that is
//! easy to write, easy to test against itself, and still wrong about the
//! game. So this reports the real numbers: where the posts stand, how far
//! the car can reach, and what the overlap is — in half-widths, against
//! the surfaces the driver can feel.
//!
//!   cargo run -p omarcade-racer --example probe_posts

#[path = "../src/road.rs"]
mod road;
#[path = "../src/drive.rs"]
mod drive;
#[path = "../src/track.rs"]
mod track;
#[path = "../src/art.rs"]
mod art;
#[path = "../src/scenery.rs"]
mod scenery;
#[path = "../src/structures.rs"]
mod structures;
#[path = "../src/traffic.rs"]
mod traffic;
#[path = "../src/collide.rs"]
mod collide;
#[path = "../src/pace.rs"]
mod pace;

use art::Art;
use drive::{Drive, Tuning, CAR_WIDTH_HALF_WIDTHS, MAX_STRAY, RUMBLE_FRACTION};
use omarcade_core::Theme;

fn main() {
    let theme = Theme::load();
    let art = Art::load(&theme);
    let road = track::grand_prix().build();
    let tuning = Tuning::from_corner(&road, 1.5);

    let sprite = &art.billboard_omarcade;
    let panel = art.billboard_omarcade_panel_rows();
    let panel_h = (panel.1 - panel.0 + 1) as f32;
    let (x0, _, x1, _) = sprite.ink_bounds().expect("the sign has ink");
    let w = structures::sign_width_half_widths((x1 - x0 + 1) as f32, panel_h);

    println!();
    println!("OMAPRIX — CAN YOU HIT A SIGN?");
    println!("=============================");
    println!();
    println!("  Surfaces, in half-widths from the centre line:");
    println!("    tarmac ends        1.000");
    println!("    rumble strip       {:.3} .. 1.000", 1.0 - RUMBLE_FRACTION);
    println!("    grass begins       1.000   (speed capped to 45%)");
    println!();
    println!("  The car:");
    println!("    width              {CAR_WIDTH_HALF_WIDTHS:.3}");
    println!("    steering limit     ±{MAX_STRAY:.3}  (hard clamp)");
    println!(
        "    outer edge reach   ±{:.3}",
        MAX_STRAY + CAR_WIDTH_HALF_WIDTHS * 0.5
    );
    println!();
    println!("  A sign:");
    println!("    ink width          {w:.3} half-widths");
    for kind in [
        structures::Structure::BillboardOmarcade { side: structures::Side::Right },
        structures::Structure::BillboardOmarcade { side: structures::Side::Left },
    ] {
        let (a, b) = structures::post_span(kind, w).expect("a roadside sign has posts");
        println!("    posts ({:>5?})     {a:+.3} .. {b:+.3}", kind.side().unwrap());
    }
    println!();

    // The real question: drive off the road and see whether it registers.
    let placements = structures::shipped();
    let sign = placements
        .iter()
        .find(|p| p.kind.side().is_some())
        .expect("there is a sign");
    let (near, _far) = structures::post_span(sign.kind, w).unwrap();

    println!("  REACHABILITY");
    println!("  ------------");
    let reach = MAX_STRAY + CAR_WIDTH_HALF_WIDTHS * 0.5;
    if reach >= near.abs() {
        println!(
            "    ✓ reachable — the car's outer edge gets to {:.3} and the near\n      \
             post starts at {:.3}, an overlap of {:.3} half-widths.",
            reach,
            near.abs(),
            reach - near.abs(),
        );
    } else {
        println!(
            "    ✗ UNREACHABLE — outer edge {:.3} never gets to the post at {:.3}.",
            reach,
            near.abs()
        );
    }
    println!(
        "    ⚠️ But the car is on GRASS from 1.000 out, so anything that\n       \
         touches a post is already off the road and capped to 45% speed."
    );
    println!();

    // Drive into one and confirm the check fires.
    println!("  DOES THE CHECK ACTUALLY FIRE?");
    println!("  -----------------------------");
    let mut car = Drive { z: road.wrap(sign.z - 4000.0), x: near, ..Drive::new() };
    car.speed = tuning.top_speed * 0.45;
    let dt = 1.0 / 60.0;
    let mut fired = None;
    for frame in 0..600 {
        let prev = car.z;
        car.update(dt, 1.0, 0.0, 0.0, &road, &tuning);
        if let Some(h) = collide::check_posts(&car, prev, &road, &placements, w) {
            fired = Some((frame, h));
            break;
        }
    }
    match fired {
        Some((frame, h)) => println!(
            "    ✓ hit on frame {frame} — {:?} at z {:.0}, x {:+.3}",
            h.what, h.z, h.x
        ),
        None => println!("    ✗ drove the whole way and never registered a post"),
    }
    println!();
}
