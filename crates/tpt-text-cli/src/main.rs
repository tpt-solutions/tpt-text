//! `tpt-text-cli layout --font FILE [options] TEXT`
//!
//! Lays out TEXT in a font file and prints the result.
//!
//! Options:
//!   --size PX        font size in pixels (default 16)
//!   --width PX       line width in pixels (default 320)
//!   --align NAME     start, end, left, right, center or justify (default start)
//!   --format FORMAT  json (default), svg, digest or explain
//!   --rtl            force a right-to-left paragraph
//!   --font-index N   face index within a font collection (default 0)

use std::fs;
use std::process::ExitCode;

use tpt_text::svg::layout_to_svg;
use tpt_text::{
    explain_line_breaks, layout_paragraph_with, Align, Direction, FontRegistry, Fx, LayoutOptions,
    TextStyle,
};

const USAGE: &str = "usage: tpt-text-cli layout --font FILE [--size PX] [--width PX] \
[--align NAME] [--format json|svg|digest|explain] [--rtl] [--font-index N] TEXT";

#[derive(Debug)]
struct Args {
    font: String,
    font_index: u32,
    text: String,
    size: i32,
    width: i32,
    align: Align,
    format: Format,
    rtl: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Json,
    Svg,
    Digest,
    Explain,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut iter = args.iter();
    match iter.next().map(String::as_str) {
        Some("layout") => {}
        _ => return Err(USAGE.to_string()),
    }
    let mut out = Args {
        font: String::new(),
        font_index: 0,
        text: String::new(),
        size: 16,
        width: 320,
        align: Align::Start,
        format: Format::Json,
        rtl: false,
    };
    let mut text: Option<String> = None;
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--font" => out.font = value("--font")?,
            "--font-index" => {
                out.font_index = value("--font-index")?
                    .parse()
                    .map_err(|_| "bad --font-index")?
            }
            "--size" => out.size = value("--size")?.parse().map_err(|_| "bad --size")?,
            "--width" => out.width = value("--width")?.parse().map_err(|_| "bad --width")?,
            "--align" => {
                out.align = match value("--align")?.as_str() {
                    "start" => Align::Start,
                    "end" => Align::End,
                    "left" => Align::Left,
                    "right" => Align::Right,
                    "center" => Align::Center,
                    "justify" => Align::Justify,
                    other => return Err(format!("unknown alignment {other:?}")),
                }
            }
            "--format" => {
                out.format = match value("--format")?.as_str() {
                    "json" => Format::Json,
                    "svg" => Format::Svg,
                    "digest" => Format::Digest,
                    "explain" => Format::Explain,
                    other => return Err(format!("unknown format {other:?}")),
                }
            }
            "--rtl" => out.rtl = true,
            other if other.starts_with("--") => return Err(format!("unknown option {other}")),
            _ => text = Some(arg.clone()),
        }
    }
    if out.font.is_empty() {
        return Err("--font is required".into());
    }
    out.text = text.ok_or("TEXT is required")?;
    Ok(out)
}

fn run(args: &Args) -> Result<String, String> {
    let bytes = fs::read(&args.font).map_err(|e| format!("cannot read {}: {e}", args.font))?;
    let registry = FontRegistry::new();
    let face = registry
        .load_font(bytes, args.font_index)
        .map_err(|e| e.to_string())?;
    let style = TextStyle::new(face.family(), Fx::from_int(args.size));
    let options = LayoutOptions {
        align: args.align,
        direction: args.rtl.then_some(Direction::RightToLeft),
        ..LayoutOptions::default()
    };
    let layout = layout_paragraph_with(
        &registry,
        &args.text,
        &style,
        &[],
        Fx::from_int(args.width),
        &options,
    )
    .map_err(|e| e.to_string())?;

    Ok(match args.format {
        Format::Json => serde_json::to_string_pretty(&layout).map_err(|e| e.to_string())? + "\n",
        Format::Svg => layout_to_svg(&layout),
        Format::Digest => format!("{:016x}\n", layout.digest()),
        Format::Explain => {
            let mut out = String::new();
            for reason in explain_line_breaks(&layout) {
                out.push_str(&format!(
                    "line {} ends at byte {} by {}\n",
                    reason.line, reason.byte, reason.rule
                ));
            }
            if out.is_empty() {
                out.push_str("one line: no line ends\n");
            }
            out
        }
    })
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let result = parse(&raw).and_then(|args| run(&args));
    match result {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn parses_options_and_text() {
        let args = parse(&words(
            "layout --font a.ttf --size 20 --format svg hello world",
        ))
        .unwrap();
        assert_eq!(args.font, "a.ttf");
        assert_eq!(args.size, 20);
        assert_eq!(args.format, Format::Svg);
        assert_eq!(args.text, "world");
    }

    #[test]
    fn rejects_missing_font_and_unknown_options() {
        assert!(parse(&words("layout hi")).is_err());
        assert!(parse(&words("layout --font a.ttf --nope hi")).is_err());
    }
}
