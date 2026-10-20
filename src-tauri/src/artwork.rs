//! Shared source of pixels for the hardware and editor preview.
use crate::model::Action;
use base64::Engine;
use resvg::{tiny_skia, usvg};
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn fontdb() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}
fn icon_name(kind: &str) -> &'static str {
    if kind == "ai" || kind == "workflow" {
        "sparkles"
    } else if kind.starts_with("agent") {
        "bot"
    } else {
        match kind {
            "shortcut" | "text" => "keyboard",
            "open" | "shell" => "terminal",
            "media" => "play",
            "brightness" => "sun",
            _ => "circle",
        }
    }
}
// Limit labels to two readable lines; long labels end visibly in an ellipsis
// instead of shrinking the entire label into unreadable text.
fn title_lines(title: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in title.lines() {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let characters: Vec<_> = word.chars().collect();
            for chunk in characters.chunks(12) {
                if !line.is_empty() && line.chars().count() + chunk.len() + 1 > 12 {
                    lines.push(std::mem::take(&mut line));
                }
                if !line.is_empty() {
                    line.push(' ');
                }
                line.extend(chunk);
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    if lines.len() > 2 {
        lines.truncate(2);
        let last = lines.last_mut().unwrap();
        if last.chars().count() == 12 {
            last.pop();
        }
        last.push('…');
    }
    lines
}

// Fit the actual painted bounds, including strokes, rather than the SVG's
// padded viewBox. The same geometry feeds the device and native editor preview.
fn fit_fragment(
    fragment: &str,
    top: f32,
    height: f32,
    options: &usvg::Options,
) -> Result<String, String> {
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"96\" height=\"96\">{fragment}</svg>"
    );
    let tree = usvg::Tree::from_str(&svg, options).map_err(|e| e.to_string())?;
    let bounds = tree.root().abs_stroke_bounding_box();
    if bounds.width() == 0.0 || bounds.height() == 0.0 {
        return Ok(String::new());
    }
    let scale = (88.0 / bounds.width()).min(height / bounds.height());
    let x = (96.0 - bounds.width() * scale) / 2.0 - bounds.x() * scale;
    let y = top + (height - bounds.height() * scale) / 2.0 - bounds.y() * scale;
    Ok(format!(
        "<g transform=\"translate({x} {y}) scale({scale})\">{fragment}</g>"
    ))
}

fn key_svg(action: &Action, options: &usvg::Options) -> Result<String, String> {
    static ICONS: OnceLock<HashMap<String, String>> = OnceLock::new();
    let icons = ICONS.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/icons.json")).expect("bundled icon JSON")
    });
    let color = u32::from_str_radix(action.color.trim_start_matches('#'), 16).unwrap_or(0x292b38)
        & 0xffffff;
    let lines = title_lines(&action.title);
    let (icon_height, text_top, line_height) = match lines.len() {
        0 => (88.0, 0.0, 0.0),
        1 => (60.0, 68.0, 24.0),
        _ => (42.0, 50.0, 20.0),
    };
    let mut content = fit_fragment(&icons[icon_name(&action.kind)], 4.0, icon_height, options)?;
    for (i, line) in lines.iter().enumerate() {
        let text = format!(
            "<text x=\"0\" y=\"0\" font-family=\"Arial,Helvetica,DejaVu Sans,sans-serif\" font-size=\"24\" font-weight=\"700\" fill=\"#e8eaef\">{}</text>",
            escape(line)
        );
        content.push_str(&fit_fragment(
            &text,
            text_top + i as f32 * (line_height + 2.0),
            line_height,
            options,
        )?);
    }
    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"96\" height=\"96\" viewBox=\"0 0 96 96\"><rect width=\"96\" height=\"96\" fill=\"#{color:06x}\"/>{content}</svg>"
    ))
}
pub fn render(action: &Action, size: u32, rotation: u16) -> Result<Vec<u8>, String> {
    if !(32..=128).contains(&size) || ![0, 90, 180, 270].contains(&rotation) {
        return Err("Invalid frame geometry".into());
    }
    if let Some(data) = &action.image {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data.split_once(',').map(|(_, s)| s).unwrap_or(data))
            .map_err(|e| e.to_string())?;
        return crate::device::encode_image(&bytes, size, rotation);
    }
    let options = usvg::Options {
        fontdb: fontdb(),
        ..Default::default()
    };
    let svg = key_svg(action, &options)?;
    let tree = usvg::Tree::from_str(&svg, &options).map_err(|e| e.to_string())?;
    let mut pixmap = tiny_skia::Pixmap::new(size, size).ok_or("Could not allocate key image")?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(size as f32 / 96.0, size as f32 / 96.0),
        &mut pixmap.as_mut(),
    );
    let image = image::RgbaImage::from_raw(size, size, pixmap.data().to_vec())
        .ok_or("Invalid rendered frame")?;
    let image = image::DynamicImage::ImageRgba8(image);
    let image = match rotation {
        90 => image.rotate90(),
        180 => image.rotate180(),
        270 => image.rotate270(),
        _ => image,
    };
    let mut bytes = vec![];
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 90)
        .encode_image(&image.to_rgb8())
        .map_err(|e| e.to_string())?;
    Ok(bytes)
}
pub fn data_url(bytes: &[u8]) -> String {
    format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_bytes_are_exactly_usb_image_payload() {
        let a = Action {
            kind: "open".into(),
            title: "Terminal".into(),
            color: "#254d41".into(),
            ..Default::default()
        };
        let bytes = render(&a, 64, 90).unwrap();
        let preview = data_url(&bytes);
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(preview.split_once(',').unwrap().1)
            .unwrap();
        let packets = crate::protocol::image_packets(1, &bytes).unwrap();
        let mut sent: Vec<u8> = packets[1..]
            .iter()
            .flat_map(|p| p[1..].iter().copied())
            .collect();
        sent.truncate(bytes.len());
        assert_eq!(decoded, sent);
        let image = image::load_from_memory(&sent).unwrap();
        assert_eq!((image.width(), image.height()), (64, 64));
        assert_eq!(render(&a, 64, 90).unwrap(), bytes);
    }
    #[test]
    fn labels_cannot_inject_svg() {
        assert_eq!(escape("<script>&\""), "&lt;script&gt;&amp;&quot;");
    }

    #[test]
    fn readable_labels_wrap_explicit_lines_and_mark_overflow() {
        assert_eq!(title_lines("Copy"), ["Copy"]);
        assert_eq!(title_lines("Codex\nworking"), ["Codex", "working"]);
        assert_eq!(
            title_lines("Open agent dashboard now"),
            ["Open agent", "dashboard…"]
        );
        assert!(title_lines("   \n ").is_empty());
        let lines = title_lines("界".repeat(30).as_str());
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[1].chars().count(), 12);
        assert!(lines[1].ends_with('…'));
    }

    #[test]
    fn glyphs_fill_available_space_without_clipping_or_distortion() {
        let options = usvg::Options {
            fontdb: fontdb(),
            ..Default::default()
        };
        for (fragment, top, height) in [
            (
                "<rect x=\"8\" y=\"4\" width=\"12\" height=\"20\"/>",
                4.0,
                60.0,
            ),
            (
                "<rect x=\"2\" y=\"6\" width=\"24\" height=\"12\"/>",
                4.0,
                42.0,
            ),
            (
                "<text font-size=\"24\" font-weight=\"700\">Copy</text>",
                68.0,
                24.0,
            ),
            (
                "<text font-size=\"24\" font-weight=\"700\">WWWWWWWWWWWW</text>",
                72.0,
                20.0,
            ),
        ] {
            let fitted = fit_fragment(fragment, top, height, &options).unwrap();
            let svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"96\" height=\"96\">{fitted}</svg>");
            let tree = usvg::Tree::from_str(&svg, &options).unwrap();
            let bounds = tree.root().abs_stroke_bounding_box();
            assert!(bounds.x() >= 3.99 && bounds.right() <= 92.01);
            assert!(bounds.y() >= top - 0.01 && bounds.bottom() <= top + height + 0.01);
            assert!(
                (bounds.width() - 88.0).abs() < 0.01 || (bounds.height() - height).abs() < 0.01
            );
        }
    }
}
