// SPDX-License-Identifier: AGPL-3.0-only
//! A file the person picked, dropped or pasted, made into the one PNG a project wears (`imageFile.ts`): refused from
//! its own bytes first where decoding it would cost the app (over 10 MB, over 4096 px a side, a type outside the five,
//! an SVG holding a web page), then fitted, centred with no crop, onto a clear 128 px square.

use std::io::Cursor;

use image::imageops::{self, FilterType};
use image::{ImageFormat, ImageReader, Limits, RgbaImage};

/// `IMAGE_MAX_BYTES` and its words.
pub const IMAGE_MAX_BYTES: u64 = 10 * 1024 * 1024;
const IMAGE_MAX_WORDS: &str = "10 MB";
/// `PROJECT_ICON_PX`: the one size the host keeps a project's image at.
pub const ICON_PX: u32 = 128;
/// `PROJECT_ICON_SOURCE_PX`: the widest and tallest file decoded.
const SOURCE_PX: u32 = 4096;

/// `ImageRefusal`: what happened, and what to do.
#[derive(Clone, Debug, PartialEq)]
pub struct Refused {
    pub said: String,
    pub fix: String,
}

fn refused(said: impl Into<String>, fix: &str) -> Refused {
    Refused { said: said.into(), fix: fix.to_owned() }
}

/// `IMAGE_REFUSALS`.
pub fn too_big() -> Refused {
    refused(format!("This file is over {IMAGE_MAX_WORDS}."), "Pick a smaller one.")
}

fn too_wide() -> Refused {
    refused(format!("This image is over {SOURCE_PX} by {SOURCE_PX} pixels."), "Scale it down, or pick a smaller one.")
}

fn wrong_type() -> Refused {
    refused("wsp draws PNG, JPEG, WebP, GIF and SVG.", "Pick a file in one of those.")
}

fn page() -> Refused {
    refused("This SVG holds a web page inside it, which wsp does not draw.", "Export it as a PNG and pick that.")
}

pub fn unreadable() -> Refused {
    refused("wsp could not read this file as an image.", "Open it in an image editor, save it again and pick that.")
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Raster(ImageFormat),
    Svg,
}

/// `sniffImage`: the type a file is, read off its bytes and never its name.
fn sniff(bytes: &[u8]) -> Option<Kind> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        return Some(Kind::Raster(ImageFormat::Png));
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some(Kind::Raster(ImageFormat::Jpeg));
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(Kind::Raster(ImageFormat::Gif));
    }
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some(Kind::Raster(ImageFormat::WebP));
    }
    svg_root(&String::from_utf8_lossy(&bytes[..bytes.len().min(4096)])).then_some(Kind::Svg)
}

/// Whether the text opens on an svg root, after any XML declaration, comments and doctype.
fn svg_root(head: &str) -> bool {
    let mut rest = head.trim_start_matches('\u{feff}').trim_start();
    if let Some(after) = rest.strip_prefix("<?xml") {
        let Some(end) = after.find("?>") else { return false };
        rest = after[end + 2..].trim_start();
    }
    loop {
        if let Some(after) = rest.strip_prefix("<!--") {
            let Some(end) = after.find("-->") else { return false };
            rest = after[end + 3..].trim_start();
        } else if rest.get(..9).is_some_and(|word| word.eq_ignore_ascii_case("<!doctype")) {
            let after = &rest[9..];
            // An internal subset in brackets may hold `>` of its own.
            let end = match after.find('[') {
                Some(open) if after.find('>').is_some_and(|close| open < close) => {
                    after[open..].find(']').and_then(|shut| after[open + shut..].find('>').map(|close| open + shut + close))
                }
                _ => after.find('>'),
            };
            let Some(end) = end else { return false };
            rest = after[end + 1..].trim_start();
        } else {
            break;
        }
    }
    let Some(tag) = rest.strip_prefix('<') else { return false };
    let name_end = tag.find(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_' || c == ':')).unwrap_or(tag.len());
    let name = &tag[..name_end];
    let local = name.rsplit(':').next().unwrap_or(name);
    local.eq_ignore_ascii_case("svg") && tag[name_end..].starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/')
}

/// The file as the PNG a project wears; `svg` draws an SVG at a width of 128 px, as straight RGBA.
pub fn fit(bytes: &[u8], svg: impl FnOnce(&[u8]) -> Option<RgbaImage>) -> Result<Vec<u8>, Refused> {
    if bytes.len() as u64 > IMAGE_MAX_BYTES {
        return Err(too_big());
    }
    let picture = match sniff(bytes).ok_or_else(wrong_type)? {
        Kind::Svg => {
            // Named anywhere, an entity or a reference included.
            if String::from_utf8_lossy(bytes).to_ascii_lowercase().contains("foreignobject") {
                return Err(page());
            }
            svg(bytes).ok_or_else(unreadable)?
        }
        Kind::Raster(format) => {
            let (width, height) = ImageReader::with_format(Cursor::new(bytes), format).into_dimensions().map_err(|_| unreadable())?;
            if width == 0 || height == 0 {
                return Err(unreadable());
            }
            if width > SOURCE_PX || height > SOURCE_PX {
                return Err(too_wide());
            }
            let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
            let mut limits = Limits::default();
            limits.max_image_width = Some(SOURCE_PX);
            limits.max_image_height = Some(SOURCE_PX);
            reader.limits(limits);
            reader.decode().map_err(|_| unreadable())?.to_rgba8()
        }
    };
    let (width, height) = (picture.width().max(1), picture.height().max(1));
    let scale = (ICON_PX as f32 / width as f32).min(ICON_PX as f32 / height as f32);
    let (w, h) = (((width as f32 * scale).round() as u32).clamp(1, ICON_PX), ((height as f32 * scale).round() as u32).clamp(1, ICON_PX));
    // Scaled with its colour weighed by its cover, as a canvas does, so a clear edge does not darken.
    let mut picture = picture;
    premultiply(&mut picture, true);
    let mut fitted = imageops::resize(&picture, w, h, FilterType::Lanczos3);
    premultiply(&mut fitted, false);
    let mut square = RgbaImage::new(ICON_PX, ICON_PX);
    imageops::overlay(&mut square, &fitted, ((ICON_PX - w) / 2) as i64, ((ICON_PX - h) / 2) as i64);
    let mut png = Vec::new();
    square.write_to(&mut Cursor::new(&mut png), ImageFormat::Png).map_err(|_| unreadable())?;
    Ok(png)
}

fn premultiply(picture: &mut RgbaImage, on: bool) {
    for pixel in picture.pixels_mut() {
        let a = pixel[3] as f32 / 255.;
        for c in &mut pixel.0[..3] {
            let v = if on {
                *c as f32 * a
            } else if a > 0. {
                *c as f32 / a
            } else {
                0.
            };
            *c = v.round().clamp(0., 255.) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgba};

    fn encoded(picture: RgbaImage, format: ImageFormat) -> Vec<u8> {
        let mut bytes = Vec::new();
        let picture = if format == ImageFormat::Jpeg {
            DynamicImage::ImageRgb8(DynamicImage::ImageRgba8(picture).to_rgb8())
        } else {
            DynamicImage::ImageRgba8(picture)
        };
        picture.write_to(&mut Cursor::new(&mut bytes), format).unwrap();
        bytes
    }

    fn no_svg(_: &[u8]) -> Option<RgbaImage> {
        panic!("a raster file never reaches the SVG road")
    }

    #[test]
    fn a_wide_picture_lands_centred_on_a_clear_square() {
        let png = fit(&encoded(RgbaImage::from_pixel(400, 100, Rgba([200, 20, 20, 255])), ImageFormat::Png), no_svg).unwrap();
        let back = image::load_from_memory_with_format(&png, ImageFormat::Png).unwrap();
        assert_eq!((back.width(), back.height()), (ICON_PX, ICON_PX));
        assert!(matches!(back, DynamicImage::ImageRgba8(_)), "8 bits of RGBA, as the host takes");
        let back = back.to_rgba8();
        assert_eq!(back.get_pixel(64, 64).0, [200, 20, 20, 255], "the picture fills the middle");
        assert_eq!(back.get_pixel(64, 5)[3], 0, "above a 128 by 32 band the square stays clear");
        assert_eq!(back.get_pixel(64, 122)[3], 0, "and below it");
        assert!(png.len() <= 80 * 1024, "within PROJECT_ICON_MAX_BYTES");
    }

    #[test]
    fn every_raster_type_the_web_takes_is_fitted() {
        for format in [ImageFormat::Jpeg, ImageFormat::Gif, ImageFormat::WebP] {
            let file = encoded(RgbaImage::from_pixel(10, 20, Rgba([0, 0, 255, 255])), format);
            let back = image::load_from_memory(&fit(&file, no_svg).unwrap()).unwrap().to_rgba8();
            assert_eq!(back.dimensions(), (ICON_PX, ICON_PX), "{format:?}");
            assert_eq!(back.get_pixel(64, 64)[3], 255, "{format:?}");
            assert_eq!(back.get_pixel(5, 64)[3], 0, "{format:?} is narrow, so its sides stay clear");
        }
    }

    #[test]
    fn a_file_is_refused_from_its_bytes_before_it_decodes() {
        let huge = vec![0u8; IMAGE_MAX_BYTES as usize + 1];
        assert_eq!(fit(&huge, no_svg), Err(too_big()));
        assert_eq!(fit(b"plain words", no_svg), Err(wrong_type()));
        assert_eq!(fit(&encoded(RgbaImage::new(4097, 1), ImageFormat::Png), no_svg), Err(too_wide()));
        assert!(fit(&encoded(RgbaImage::new(4096, 1), ImageFormat::Png), no_svg).is_ok(), "4096 itself still fits");
        let mut cut = encoded(RgbaImage::from_pixel(8, 8, Rgba([1, 2, 3, 255])), ImageFormat::Png);
        cut.truncate(40);
        assert_eq!(fit(&cut, no_svg), Err(unreadable()));
    }

    #[test]
    fn an_svg_is_told_by_its_root_and_a_page_inside_is_refused() {
        assert_eq!(sniff(b"<svg xmlns='http://www.w3.org/2000/svg'/>"), Some(Kind::Svg));
        assert_eq!(
            sniff(b"\xef\xbb\xbf<?xml version='1.0'?>\n<!-- made -->\n<!DOCTYPE svg [ <!ENTITY a 'b'> ]>\n<svg:svg>"),
            Some(Kind::Svg)
        );
        assert_eq!(sniff(b"<svgx>"), None);
        assert_eq!(sniff(b"<html><svg>"), None);
        let drawn = fit(b"<svg viewBox='0 0 1 1'/>", |_| Some(RgbaImage::from_pixel(128, 64, Rgba([0, 255, 0, 255])))).unwrap();
        assert_eq!(image::load_from_memory(&drawn).unwrap().to_rgba8().get_pixel(64, 64).0, [0, 255, 0, 255]);
        assert_eq!(fit(b"<svg><foreignObject/></svg>", |_| panic!("never drawn")), Err(page()));
        assert_eq!(fit(b"<svg>", |_| None), Err(unreadable()));
    }

    #[test]
    fn a_clear_edge_keeps_its_colour() {
        // Red beside a clear pixel that holds blue, drawn up to the square: a clear pixel gives no colour to its edge.
        let mut picture = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 255, 0]));
        picture.put_pixel(0, 0, Rgba([255, 0, 0, 255]));
        picture.put_pixel(0, 1, Rgba([255, 0, 0, 255]));
        let back = image::load_from_memory(&fit(&encoded(picture, ImageFormat::Png), no_svg).unwrap()).unwrap().to_rgba8();
        let blended: Vec<_> = back.pixels().filter(|p| p[3] > 16 && p[3] < 240).collect();
        assert!(!blended.is_empty(), "the edge blends");
        assert!(blended.iter().all(|p| p[0] > 230 && p[2] < 25), "the edge stays red: {:?}", blended.iter().map(|p| p[2]).max());
    }
}
