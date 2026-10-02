// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reading any image a player picks and cutting it into a cover.

use std::io::Cursor;
use std::path::Path;

use base64::Engine;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::{self, FilterType};
use image::{DynamicImage, GenericImageView, ImageFormat, ImageReader, Rgba, RgbaImage};
use resvg::{tiny_skia, usvg};
use serde::Serialize;

use crate::constants::{
    ARTWORK_DECODE_MAX_BYTES, ARTWORK_MAX_SIDE_PX, CACHED_JPEG_QUALITY, COVER_HEIGHT_PX,
    COVER_WIDTH_PX, CROP_BACKDROP_BLUR_SIGMA_PX, CROP_BACKDROP_BRIGHTNESS,
    CROP_BACKDROP_DARK_PLAIN_LUMA, CROP_BACKDROP_FALLBACK_RGB, CROP_BACKDROP_LIGHT_ART_LUMA,
    CROP_BACKDROP_LIGHT_PLAIN_LUMA, CROP_BACKDROP_OPAQUE_COVERAGE, CROP_BACKDROP_SAMPLE_PX,
    CROP_BACKDROP_WIDTH_PX, CROP_MAX_FRAME_OF_FIT, CROP_PREVIEW_MAX_PX, SVG_MAX_FILTERS,
    SVG_MAX_LAYER_BYTES, SVG_MAX_LAYER_DEPTH, SVG_MAX_NODES, SVG_RENDER_MAX_PX, SVG_RENDER_MIN_PX,
};
use crate::db::models::CropRect;
use crate::error::{Result, VaultimeError};

/// An image opened in the crop dialog.
#[derive(Debug, Clone, Serialize)]
pub struct ArtworkSource {
    /// The image, scaled down for the dialog.
    pub preview_data_url: String,
    /// What fills the cover where the image does not reach, small enough for
    /// the dialog to scale up.
    pub backdrop_data_url: String,
    /// Size of the full image in pixels.
    pub width: u32,
    pub height: u32,
    /// False when the original file is gone or changed and the cached cover
    /// stands in for it.
    pub from_original: bool,
    /// The crop in use, when it was cut from this same image.
    pub crop: Option<CropRect>,
}

impl ArtworkSource {
    pub fn new(image: &DynamicImage, from_original: bool, crop: Option<CropRect>) -> Result<Self> {
        let longest = image.width().max(image.height());
        let preview = if longest > CROP_PREVIEW_MAX_PX {
            image.thumbnail(CROP_PREVIEW_MAX_PX, CROP_PREVIEW_MAX_PX)
        } else {
            image.clone()
        };
        Ok(Self {
            preview_data_url: data_url(&preview)?,
            backdrop_data_url: data_url(&DynamicImage::ImageRgba8(small_backdrop(image)))?,
            width: image.width(),
            height: image.height(),
            from_original,
            crop,
        })
    }
}

/// Decodes an image file. SVG is drawn by resvg, everything else is read by
/// the image crate, which falls back to the file extension for formats
/// without a signature, such as TGA. Images too large to decode safely are
/// refused.
pub fn decode_artwork(bytes: &[u8], path: &Path) -> Result<DynamicImage> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    if extension.eq_ignore_ascii_case("svg") {
        return draw_svg(bytes, path);
    }

    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| {
            VaultimeError::Asset(format!(
                "failed to detect image format {}: {error}",
                path.display()
            ))
        })?;
    if reader.format().is_none()
        && let Some(format) = ImageFormat::from_extension(extension)
    {
        reader.set_format(format);
    }
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(ARTWORK_MAX_SIDE_PX);
    limits.max_image_height = Some(ARTWORK_MAX_SIDE_PX);
    limits.max_alloc = Some(ARTWORK_DECODE_MAX_BYTES);
    reader.limits(limits);
    reader.decode().map_err(|error| {
        VaultimeError::Asset(format!(
            "failed to decode image {}: {error}",
            path.display()
        ))
    })
}

fn draw_svg(bytes: &[u8], path: &Path) -> Result<DynamicImage> {
    // Other files the drawing names are never read. resvg is built without
    // raster images and text, so pictures inside the file and text that was
    // not turned into paths are left out.
    let options = usvg::Options {
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: usvg::ImageHrefResolver::default_data_resolver(),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    };
    draw_svg_with(bytes, path, &options)
}

fn draw_svg_with(bytes: &[u8], path: &Path, options: &usvg::Options) -> Result<DynamicImage> {
    let failed =
        |error: String| VaultimeError::Asset(format!("failed to draw {}: {error}", path.display()));
    let tree = usvg::Tree::from_data(bytes, options).map_err(|error| failed(error.to_string()))?;
    let size = tree.size();
    let longest = f64::from(size.width().max(size.height()));
    let scale = svg_scale(&tree, f64::from(SVG_RENDER_MAX_PX) / longest).map_err(failed)?;
    if longest * scale < f64::from(SVG_RENDER_MIN_PX) {
        return Err(failed("it needs too much memory to draw".into()));
    }
    let width = whole_pixels(f64::from(size.width()) * scale).max(1);
    let height = whole_pixels(f64::from(size.height()) * scale).max(1);
    let mut pixmap =
        tiny_skia::Pixmap::new(width, height).ok_or_else(|| failed("it has no size".into()))?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale as f32, scale as f32),
        &mut pixmap.as_mut(),
    );
    RgbaImage::from_raw(width, height, pixmap.take_demultiplied())
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| failed("the drawing has the wrong size".into()))
}

/// What drawing an SVG at scale 1 asks of resvg.
#[derive(Debug, Default)]
struct SvgLoad {
    nodes: usize,
    filters: usize,
    /// Largest sum of layer areas held at once, in square drawing units.
    layer_area: f64,
}

/// The scale an SVG can be drawn at, at most `wanted`. Layers grow with the
/// square of the scale, so a drawing whose nested layers would need more than
/// `SVG_MAX_LAYER_BYTES` is drawn smaller. Too many shapes, filters or nested
/// layers are refused outright.
fn svg_scale(tree: &usvg::Tree, wanted: f64) -> std::result::Result<f64, String> {
    let size = tree.size();
    let (width, height) = (f64::from(size.width()), f64::from(size.height()));
    // resvg clips each layer to the drawing with twice its size around it.
    let bounds = Bounds {
        left: -2.0 * width,
        top: -2.0 * height,
        right: 3.0 * width,
        bottom: 3.0 * height,
    };
    let mut load = SvgLoad::default();
    measure_group(tree.root(), 0, 0.0, bounds, &mut load)?;
    if load.layer_area <= 0.0 {
        return Ok(wanted);
    }
    // Four bytes per pixel.
    #[allow(clippy::cast_precision_loss, reason = "a byte budget fits f64")]
    let budget = SVG_MAX_LAYER_BYTES as f64 / 4.0;
    Ok(wanted.min((budget / load.layer_area).sqrt()))
}

/// A rectangle in drawing units.
#[derive(Debug, Clone, Copy)]
struct Bounds {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

/// Counts the shapes, filters and the layer area of a group and what it
/// holds, as resvg would allocate it.
fn measure_group(
    group: &usvg::Group,
    depth: usize,
    area_above: f64,
    bounds: Bounds,
    load: &mut SvgLoad,
) -> std::result::Result<(), String> {
    for node in group.children() {
        load.nodes += 1;
        if load.nodes > SVG_MAX_NODES {
            return Err("it has too many shapes".into());
        }
        // Clip paths, masks and patterns hold groups of their own.
        let mut nested = Ok(());
        node.subroots(|root| {
            if nested.is_ok() {
                nested = measure_group(root, depth, area_above, bounds, load);
            }
        });
        nested?;
        let usvg::Node::Group(child) = node else {
            continue;
        };
        let (depth, area) = if child.should_isolate() {
            load.filters += child.filters().len();
            if load.filters > SVG_MAX_FILTERS {
                return Err("it has too many filters".into());
            }
            if depth + 1 > SVG_MAX_LAYER_DEPTH {
                return Err("it nests too many layers".into());
            }
            let rect = child.abs_layer_bounding_box();
            let width =
                f64::from(rect.right()).min(bounds.right) - f64::from(rect.left()).max(bounds.left);
            let height =
                f64::from(rect.bottom()).min(bounds.bottom) - f64::from(rect.top()).max(bounds.top);
            let area = area_above + width.max(0.0) * height.max(0.0);
            load.layer_area = load.layer_area.max(area);
            (depth + 1, area)
        } else {
            (depth, area_above)
        };
        measure_group(child, depth, area, bounds, load)?;
    }
    Ok(())
}

/// The crop that fills the cover with the middle of an image, the cut
/// covers had before players could choose.
pub fn fill_crop(width: u32, height: u32) -> CropRect {
    let cover_aspect = f64::from(COVER_WIDTH_PX) / f64::from(COVER_HEIGHT_PX);
    let image_aspect = f64::from(width) / f64::from(height);
    if image_aspect > cover_aspect {
        let share = cover_aspect / image_aspect;
        CropRect {
            x: (1.0 - share) / 2.0,
            y: 0.0,
            width: share,
            height: 1.0,
        }
    } else {
        let share = image_aspect / cover_aspect;
        CropRect {
            x: 0.0,
            y: (1.0 - share) / 2.0,
            width: 1.0,
            height: share,
        }
    }
}

/// A crop in source pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PixelRect {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

/// The crop in source pixels. Its width counts and the height follows from
/// the cover's aspect ratio around the same middle, so a slightly uneven crop
/// never stretches the cover. A crop that misses the image, is smaller than a
/// pixel or far larger than the image is refused.
fn pixel_rect(crop: CropRect, width: u32, height: u32) -> Result<PixelRect> {
    let invalid = |reason: &str| VaultimeError::Invalid(format!("the crop {reason}"));
    let (image_width, image_height) = (f64::from(width), f64::from(height));
    if ![crop.x, crop.y, crop.width, crop.height]
        .iter()
        .all(|value| value.is_finite())
    {
        return Err(invalid("is not a number"));
    }

    let rect_width = crop.width * image_width;
    let rect_height = rect_width * f64::from(COVER_HEIGHT_PX) / f64::from(COVER_WIDTH_PX);
    let fit_width =
        image_width.max(image_height * f64::from(COVER_WIDTH_PX) / f64::from(COVER_HEIGHT_PX));
    if rect_width < 1.0 {
        return Err(invalid("is smaller than a pixel"));
    }
    if rect_width > fit_width * CROP_MAX_FRAME_OF_FIT {
        return Err(invalid("is too far zoomed out"));
    }

    let middle = (crop.y + crop.height / 2.0) * image_height;
    let rect = PixelRect {
        left: crop.x * image_width,
        top: middle - rect_height / 2.0,
        width: rect_width,
        height: rect_height,
    };
    if rect.left >= image_width
        || rect.left + rect.width <= 0.0
        || rect.top >= image_height
        || rect.top + rect.height <= 0.0
    {
        return Err(invalid("misses the image"));
    }
    Ok(rect)
}

/// Cuts a cover from an image. Where the crop reaches past the image, or the
/// image is see-through, the backdrop shows.
pub fn render_cover(image: &DynamicImage, crop: CropRect) -> Result<RgbaImage> {
    let (width, height) = image.dimensions();
    let rect = pixel_rect(crop, width, height)?;
    let mut cover = imageops::resize(
        &small_backdrop(image),
        COVER_WIDTH_PX,
        COVER_HEIGHT_PX,
        FilterType::Triangle,
    );

    // Whole pixels of the image inside the crop, placed where they land.
    let scale = f64::from(COVER_WIDTH_PX) / rect.width;
    let left = rect.left.max(0.0).floor();
    let top = rect.top.max(0.0).floor();
    let right = (rect.left + rect.width).min(f64::from(width)).ceil();
    let bottom = (rect.top + rect.height).min(f64::from(height)).ceil();
    let target_width = whole_pixels((right - left) * scale).max(1);
    let target_height = whole_pixels((bottom - top) * scale).max(1);
    // Resized from a view, so a large image is not copied first.
    let piece = imageops::resize(
        &*imageops::crop_imm(
            image,
            whole_pixels(left),
            whole_pixels(top),
            whole_pixels(right - left),
            whole_pixels(bottom - top),
        ),
        target_width,
        target_height,
        FilterType::Lanczos3,
    );
    imageops::overlay(
        &mut cover,
        &piece,
        ((left - rect.left) * scale).round() as i64,
        ((top - rect.top) * scale).round() as i64,
    );
    Ok(cover)
}

/// The backdrop of a cover at a small size. Opaque art shows itself blurred
/// and dimmed, see-through art like a logo gets a plain that sets it off.
fn small_backdrop(image: &DynamicImage) -> RgbaImage {
    let width = CROP_BACKDROP_WIDTH_PX;
    let height = width * COVER_HEIGHT_PX / COVER_WIDTH_PX;
    let sample = image
        .resize_exact(
            CROP_BACKDROP_SAMPLE_PX,
            CROP_BACKDROP_SAMPLE_PX,
            FilterType::Triangle,
        )
        .to_rgba8();
    let [red, green, blue] = backdrop_plain(&sample);
    let mut backdrop = RgbaImage::from_pixel(width, height, Rgba([red, green, blue, u8::MAX]));
    let coverage = sample
        .pixels()
        .map(|pixel| f64::from(pixel[3]) / f64::from(u8::MAX))
        .sum::<f64>()
        / f64::from(sample.width() * sample.height());
    if coverage < CROP_BACKDROP_OPAQUE_COVERAGE {
        return backdrop;
    }

    let mut blurred = imageops::blur(
        &image
            .resize_to_fill(width, height, FilterType::Triangle)
            .to_rgba8(),
        CROP_BACKDROP_BLUR_SIGMA_PX,
    );
    for pixel in blurred.pixels_mut() {
        for channel in &mut pixel.0[..3] {
            *channel = channel_byte(f64::from(*channel) * f64::from(CROP_BACKDROP_BRIGHTNESS));
        }
    }
    imageops::overlay(&mut backdrop, &blurred, 0, 0);
    backdrop
}

/// The plain color behind see-through art, in the hue of the art: dark behind
/// light art and light behind dark art, so a black logo stays visible.
fn backdrop_plain(sample: &RgbaImage) -> [u8; 3] {
    let mut sums = [0.0_f64; 3];
    let mut weight = 0.0;
    for pixel in sample.pixels() {
        let alpha = f64::from(pixel[3]) / f64::from(u8::MAX);
        for (sum, channel) in sums.iter_mut().zip(pixel.0) {
            *sum += f64::from(channel) / f64::from(u8::MAX) * alpha;
        }
        weight += alpha;
    }
    if weight == 0.0 {
        return CROP_BACKDROP_FALLBACK_RGB;
    }

    let mean = sums.map(|sum| sum / weight);
    // Rec. 709 luma weights.
    let luma = 0.2126 * mean[0] + 0.7152 * mean[1] + 0.0722 * mean[2];
    let plain = if luma >= CROP_BACKDROP_LIGHT_ART_LUMA {
        mean.map(|channel| channel * CROP_BACKDROP_DARK_PLAIN_LUMA / luma)
    } else {
        let darkness = (1.0 - CROP_BACKDROP_LIGHT_PLAIN_LUMA) / (1.0 - luma);
        mean.map(|channel| 1.0 - (1.0 - channel) * darkness)
    };
    plain.map(|channel| channel_byte(channel * f64::from(u8::MAX)))
}

/// Rounds a size or position to whole pixels, never below zero.
#[allow(clippy::cast_sign_loss)]
fn whole_pixels(value: f64) -> u32 {
    value.round().clamp(0.0, f64::from(u32::MAX)) as u32
}

/// Rounds a color channel to a byte.
#[allow(clippy::cast_sign_loss)]
fn channel_byte(value: f64) -> u8 {
    value.round().clamp(0.0, f64::from(u8::MAX)) as u8
}

/// JPEG for opaque images, PNG for see-through ones.
fn data_url(image: &DynamicImage) -> Result<String> {
    let opaque =
        !image.color().has_alpha() || image.to_rgba8().pixels().all(|pixel| pixel[3] == u8::MAX);
    let mut bytes = Vec::new();
    let written = if opaque {
        JpegEncoder::new_with_quality(&mut bytes, CACHED_JPEG_QUALITY)
            .encode_image(&image.to_rgb8())
    } else {
        image.write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
    };
    written
        .map_err(|error| VaultimeError::Asset(format!("failed to encode a preview: {error}")))?;
    let mime = if opaque { "image/jpeg" } else { "image/png" };
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(format!("data:{mime};base64,{encoded}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    const RED: Rgba<u8> = Rgba([220, 30, 30, 255]);
    const BLUE: Rgba<u8> = Rgba([30, 60, 230, 255]);

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn fill_crop_takes_the_middle() {
        let wide = fill_crop(1600, 900);
        assert!(close(wide.height, 1.0) && close(wide.y, 0.0));
        assert!(close(wide.width * 1600.0 / 900.0, 0.75));
        assert!(close(wide.x, (1.0 - wide.width) / 2.0));

        let tall = fill_crop(300, 1000);
        assert!(close(tall.width, 1.0) && close(tall.x, 0.0));
        assert!(close(tall.height * 1000.0, 400.0));

        let cover = fill_crop(COVER_WIDTH_PX, COVER_HEIGHT_PX);
        assert!(close(cover.width, 1.0) && close(cover.height, 1.0));
    }

    #[test]
    fn crop_keeps_the_cover_aspect_around_its_middle() {
        // A height that does not match the width is evened out around the middle.
        let rect = pixel_rect(
            CropRect {
                x: 0.25,
                y: 0.0,
                width: 0.5,
                height: 0.8,
            },
            1000,
            1000,
        )
        .unwrap();
        assert!(close(rect.left, 250.0));
        assert!(close(rect.width, 500.0));
        assert!(close(rect.height, 500.0 * 4.0 / 3.0));
        assert!(close(rect.top + rect.height / 2.0, 400.0));
    }

    #[test]
    fn crop_may_reach_past_the_edges() {
        // A wide logo zoomed out to fit: the frame is as wide as the image.
        let rect = pixel_rect(
            CropRect {
                x: 0.0,
                y: -1.5,
                width: 1.0,
                height: 4.0,
            },
            1200,
            400,
        )
        .unwrap();
        assert!(close(rect.width, 1200.0));
        assert!(rect.top < 0.0 && rect.top + rect.height > 400.0);
    }

    #[test]
    fn refuses_crops_that_miss_or_overreach() {
        let crop = |x: f64, y: f64, width: f64| CropRect {
            x,
            y,
            width,
            height: width,
        };
        assert!(pixel_rect(crop(1.2, 0.0, 0.5), 100, 100).is_err());
        assert!(pixel_rect(crop(-0.6, 0.0, 0.5), 100, 100).is_err());
        assert!(pixel_rect(crop(0.0, 0.0, 0.001), 100, 100).is_err());
        assert!(pixel_rect(crop(0.0, 0.0, 9.0), 100, 100).is_err());
        assert!(pixel_rect(crop(f64::NAN, 0.0, 0.5), 100, 100).is_err());
        assert!(pixel_rect(crop(0.0, 0.0, 0.5), 100, 100).is_ok());
    }

    #[test]
    fn cover_shows_the_cropped_part() {
        // Left half red, right half blue. Cropping the right half gives a blue cover.
        let image = RgbaImage::from_fn(400, 300, |x, _| if x < 200 { RED } else { BLUE });
        let crop = CropRect {
            x: 0.5,
            y: 0.0,
            width: 0.5,
            height: 200.0 * 4.0 / 3.0 / 300.0,
        };
        let cover = render_cover(&DynamicImage::ImageRgba8(image), crop).unwrap();
        assert_eq!(cover.dimensions(), (COVER_WIDTH_PX, COVER_HEIGHT_PX));
        let middle = cover.get_pixel(COVER_WIDTH_PX / 2, COVER_HEIGHT_PX / 2);
        assert!(middle[2] > middle[0], "{middle:?}");
    }

    #[test]
    fn a_zoomed_out_logo_sits_on_a_dimmer_backdrop() {
        let logo = RgbaImage::from_pixel(1200, 400, RED);
        let crop = CropRect {
            x: 0.0,
            y: -1.5,
            width: 1.0,
            height: 4.0,
        };
        let cover = render_cover(&DynamicImage::ImageRgba8(logo), crop).unwrap();
        let middle = cover.get_pixel(COVER_WIDTH_PX / 2, COVER_HEIGHT_PX / 2);
        let top = cover.get_pixel(COVER_WIDTH_PX / 2, 4);
        assert_eq!(middle[0], RED[0]);
        // Above the logo the dimmed, blurred copy shows, not black.
        assert!(top[0] < middle[0] && top[0] > 0, "{top:?}");
    }

    #[test]
    fn see_through_art_gets_a_plain_that_sets_it_off() {
        let white_logo = RgbaImage::from_fn(64, 64, |x, _| {
            if x < 16 {
                Rgba([255, 255, 255, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });
        let black_logo = RgbaImage::from_fn(64, 64, |x, _| {
            if x < 16 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });
        let luma = |[red, green, blue]: [u8; 3]| {
            (0.2126 * f64::from(red) + 0.7152 * f64::from(green) + 0.0722 * f64::from(blue))
                / f64::from(u8::MAX)
        };
        let behind_white = backdrop_plain(&white_logo);
        let behind_black = backdrop_plain(&black_logo);
        assert!((luma(behind_white) - CROP_BACKDROP_DARK_PLAIN_LUMA).abs() < 0.01);
        assert!((luma(behind_black) - CROP_BACKDROP_LIGHT_PLAIN_LUMA).abs() < 0.01);
        let empty = RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 0]));
        assert_eq!(backdrop_plain(&empty), CROP_BACKDROP_FALLBACK_RGB);
        // A logo gets the plain alone, without a blurred copy of itself.
        let logo = small_backdrop(&DynamicImage::ImageRgba8(black_logo));
        let [red, green, blue] = behind_black;
        assert!(
            logo.pixels()
                .all(|pixel| pixel.0 == [red, green, blue, u8::MAX])
        );
    }

    fn encode(format: ImageFormat, image: &DynamicImage) -> Vec<u8> {
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), format)
            .unwrap();
        bytes
    }

    #[test]
    fn decodes_the_added_formats() {
        let image = DynamicImage::ImageRgb8(RgbImage::from_pixel(6, 4, Rgb([200, 40, 40])));
        for (format, name) in [
            (ImageFormat::Gif, "logo.gif"),
            (ImageFormat::Tiff, "logo.tiff"),
            (ImageFormat::Tga, "logo.tga"),
            (ImageFormat::Qoi, "logo.qoi"),
            (ImageFormat::Pnm, "logo.ppm"),
            (ImageFormat::Png, "logo.png"),
        ] {
            let decoded = decode_artwork(&encode(format, &image), Path::new(name)).unwrap();
            assert_eq!(decoded.dimensions(), (6, 4), "{name}");
            assert!(decoded.to_rgb8().get_pixel(1, 1)[0] > 150, "{name}");
        }
    }

    #[test]
    fn reads_the_first_frame_of_an_animation() {
        use image::codecs::gif::GifEncoder;
        use image::{Delay, Frame};
        let mut bytes = Vec::new();
        {
            let mut encoder = GifEncoder::new(&mut bytes);
            for color in [RED, BLUE] {
                encoder
                    .encode_frame(Frame::from_parts(
                        RgbaImage::from_pixel(4, 4, color),
                        0,
                        0,
                        Delay::from_numer_denom_ms(100, 1),
                    ))
                    .unwrap();
            }
        }
        let decoded = decode_artwork(&bytes, Path::new("logo.gif")).unwrap();
        let pixel = decoded.to_rgba8().get_pixel(1, 1).0;
        assert!(pixel[0] > pixel[2], "{pixel:?}");
    }

    #[test]
    fn draws_svg() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100">
            <rect width="100" height="100" fill="#d81e1e"/></svg>"##;
        let decoded = decode_artwork(svg, Path::new("logo.SVG")).unwrap();
        assert_eq!(
            decoded.dimensions(),
            (SVG_RENDER_MAX_PX, SVG_RENDER_MAX_PX / 2)
        );
        let pixels = decoded.to_rgba8();
        assert!(pixels.get_pixel(10, 10)[0] > 200);
        assert_eq!(pixels.get_pixel(SVG_RENDER_MAX_PX - 10, 10)[3], 0);
    }

    #[test]
    fn svg_never_reads_other_files() {
        let dir = std::env::temp_dir().join(format!("vaultime-crop-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let outside = dir.join("other.svg");
        std::fs::write(
            &outside,
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="#d81e1e"/></svg>"##,
        )
        .unwrap();
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 10 10">
                <image width="10" height="10" xlink:href="{}"/></svg>"#,
            outside.display()
        );
        let path = Path::new("logo.svg");
        let opaque = |image: DynamicImage| image.to_rgba8().pixels().any(|pixel| pixel[3] > 0);
        // The test means something only when a plain parser would load the file.
        let loose = draw_svg_with(svg.as_bytes(), path, &usvg::Options::default()).unwrap();
        assert!(opaque(loose));
        assert!(!opaque(decode_artwork(svg.as_bytes(), path).unwrap()));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn refuses_files_that_are_not_images() {
        assert!(decode_artwork(b"not an image", Path::new("logo.png")).is_err());
        assert!(decode_artwork(b"<svg", Path::new("logo.svg")).is_err());
    }

    #[test]
    fn refuses_images_too_large_to_decode() {
        let wide = DynamicImage::ImageRgb8(RgbImage::new(ARTWORK_MAX_SIDE_PX + 1, 1));
        assert!(decode_artwork(&encode(ImageFormat::Png, &wide), Path::new("wide.png")).is_err());
    }

    #[test]
    fn refuses_svg_with_deeply_nested_layers() {
        let depth = SVG_MAX_LAYER_DEPTH + 1;
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10">{}<rect x="-100000" y="-100000" width="200000" height="200000"/>{}</svg>"#,
            r#"<g opacity="0.9">"#.repeat(depth),
            "</g>".repeat(depth),
        );
        let started = std::time::Instant::now();
        assert!(decode_artwork(svg.as_bytes(), Path::new("bomb.svg")).is_err());
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }

    #[test]
    fn draws_svg_with_large_layers_smaller() {
        // Three nested layers as large as resvg allows would need far more
        // than the budget at the full size.
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><g opacity="0.9"><g opacity="0.9"><g opacity="0.9"><rect x="-100000" y="-100000" width="200000" height="200000" fill="red"/></g></g></g></svg>"#;
        let decoded = decode_artwork(svg.as_bytes(), Path::new("layers.svg")).unwrap();
        assert!(decoded.width() < SVG_RENDER_MAX_PX);
        assert!(decoded.width() >= SVG_RENDER_MIN_PX);
    }

    #[test]
    fn previews_keep_transparency_only_when_needed() {
        let opaque = DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 4, RED));
        let clear = DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 0])));
        let source = ArtworkSource::new(&opaque, true, None).unwrap();
        assert!(
            source
                .preview_data_url
                .starts_with("data:image/jpeg;base64,")
        );
        assert!(
            ArtworkSource::new(&clear, true, None)
                .unwrap()
                .preview_data_url
                .starts_with("data:image/png;base64,")
        );
        let large = DynamicImage::ImageRgb8(RgbImage::new(CROP_PREVIEW_MAX_PX * 2, 10));
        let source = ArtworkSource::new(&large, true, None).unwrap();
        assert_eq!((source.width, source.height), (CROP_PREVIEW_MAX_PX * 2, 10));
    }
}
