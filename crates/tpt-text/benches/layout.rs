//! Timing for shaping and layout of large paragraphs.
//!
//! Run with `cargo bench -p tpt-text`. This is a plain timing loop with no
//! external harness, so results are rough. Compare builds on the same machine.

#![allow(clippy::unwrap_used)]

use std::hint::black_box;
use std::time::Instant;

use tpt_text::{layout_paragraph, FontRegistry, Fx, TextStyle};

fn bench<F: FnMut()>(name: &str, iterations: u32, mut f: F) {
    f(); // Warm up.
    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    let per = start.elapsed().as_secs_f64() * 1e3 / f64::from(iterations);
    println!("{name:<32} {per:>10.3} ms/iter ({iterations} iters)");
}

fn main() {
    let reg = FontRegistry::new();
    reg.load_from_memory(
        include_bytes!("../tests/fonts/NotoSans-Regular.ttf").to_vec(),
        0,
        "Noto Sans",
        400,
        false,
    )
    .unwrap();
    reg.load_from_memory(
        include_bytes!("../tests/fonts/NotoSansArabic-Regular.ttf").to_vec(),
        0,
        "Noto Sans Arabic",
        400,
        false,
    )
    .unwrap();

    let latin = "lorem ipsum dolor sit amet consectetur ".repeat(250);
    let arabic = "\u{0628}\u{062A}\u{0628} 2026 \u{062A}\u{0628} ".repeat(250);
    let small = TextStyle::new("Noto Sans", Fx::from_int(12));
    let rtl = TextStyle::new("Noto Sans Arabic", Fx::from_int(12));

    bench("latin 10k chars, 480px", 20, || {
        black_box(
            layout_paragraph(&reg, black_box(&latin), &small, &[], Fx::from_int(480)).unwrap(),
        );
    });
    bench("arabic 10k chars, 480px", 20, || {
        black_box(
            layout_paragraph(&reg, black_box(&arabic), &rtl, &[], Fx::from_int(480)).unwrap(),
        );
    });
    bench("latin 10k chars, no wrap", 5, || {
        black_box(
            layout_paragraph(
                &reg,
                black_box(&latin),
                &small,
                &[],
                Fx::from_int(1_000_000),
            )
            .unwrap(),
        );
    });
}
