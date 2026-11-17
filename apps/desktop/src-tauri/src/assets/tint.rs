// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! The main colors of artwork, in Oklab, see
//! <https://bottosson.github.io/posts/oklab/>

use image::DynamicImage;
use image::imageops::FilterType;
use serde::{Deserialize, Serialize};

use crate::constants::{
    TINT_COLORFUL_MIN_CHROMA, TINT_COLORFUL_MIN_SHARE, TINT_HUE_BINS, TINT_MAX_LIGHTNESS,
    TINT_MAX_STRENGTH, TINT_MIN_ALPHA, TINT_MIN_LIGHTNESS, TINT_MIN_STRENGTH, TINT_SAMPLE_PX,
    TINT_SECOND_MIN_GAP_DEG, TINT_SECOND_MIN_SHARE, TINT_TYPICAL_CHROMA,
};

/// A color by Oklch hue in degrees and chroma.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ArtColor {
    pub hue: f64,
    pub chroma: f64,
}

/// What a game takes its colors from. The page turns it into a tint with
/// `tintFromArt` in `lib/game-tint.ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtTint {
    /// The main colors, the strongest first and at most two. Empty for
    /// black, white and grey art.
    pub colors: Vec<ArtColor>,
    /// How colorful the tint is, 1 for typical art and 0 for grey.
    pub strength: f64,
    /// The average lightness, from 0 to 1. None when no pixel is opaque.
    pub lightness: Option<f64>,
}

/// The tint of an image, read from a small copy of it.
pub fn tint_of_image(image: &DynamicImage) -> ArtTint {
    let sample = image.resize_exact(TINT_SAMPLE_PX, TINT_SAMPLE_PX, FilterType::Triangle);
    tint_of_pixels(sample.to_rgba8().as_raw())
}

#[derive(Debug, Default, Clone, Copy)]
struct Band {
    a: f64,
    b: f64,
    chroma: f64,
    count: f64,
}

impl Band {
    fn add(self, other: Self) -> Self {
        Self {
            a: self.a + other.a,
            b: self.b + other.b,
            chroma: self.chroma + other.chroma,
            count: self.count + other.count,
        }
    }
}

/// The tint of RGBA pixels. Its main colors are the strongest bands of hues
/// among the colorful pixels, so a cover in red and blue gives red and blue
/// instead of a mix of both. A second band counts when it is far enough from
/// the first and nearly as strong. Art with too few colorful pixels, like
/// black, white or grey art, gets no colors.
pub fn tint_of_pixels(rgba: &[u8]) -> ArtTint {
    let mut bins = [Band::default(); TINT_HUE_BINS];
    let (mut opaque, mut counted, mut colorful) = (0_u32, 0_u32, 0_u32);
    let (mut sum_lightness, mut sum_chroma) = (0.0, 0.0);
    for pixel in rgba.as_chunks::<4>().0 {
        if pixel[3] < TINT_MIN_ALPHA {
            continue;
        }
        opaque += 1;
        let (lightness, a, b) = srgb_to_oklab(pixel[0], pixel[1], pixel[2]);
        sum_lightness += lightness;
        if !(TINT_MIN_LIGHTNESS..=TINT_MAX_LIGHTNESS).contains(&lightness) {
            continue;
        }
        let chroma = a.hypot(b);
        sum_chroma += chroma;
        counted += 1;
        if chroma < TINT_COLORFUL_MIN_CHROMA {
            continue;
        }
        colorful += 1;
        let bin = bins_spanned(hue_of(a, b), false) % TINT_HUE_BINS;
        bins[bin] = bins[bin].add(Band {
            a,
            b,
            chroma,
            count: 1.0,
        });
    }
    if opaque == 0 {
        return ArtTint {
            colors: Vec::new(),
            strength: 0.0,
            lightness: None,
        };
    }
    let lightness = Some(sum_lightness / f64::from(opaque));
    if f64::from(colorful) / f64::from(opaque) < TINT_COLORFUL_MIN_SHARE {
        return ArtTint {
            colors: Vec::new(),
            strength: 0.0,
            lightness,
        };
    }

    // A band is a bin with its two neighbors.
    let band = |center: usize| {
        [TINT_HUE_BINS - 1, 0, 1]
            .iter()
            .map(|step| bins[(center + step) % TINT_HUE_BINS])
            .fold(Band::default(), Band::add)
    };
    let color_of = |band: Band| ArtColor {
        hue: hue_of(band.a, band.b),
        chroma: band.chroma / band.count,
    };
    // The first of equally strong bands wins.
    let strongest_of = |centers: &mut dyn Iterator<Item = usize>| {
        centers.fold(None, |best: Option<(usize, f64)>, center| {
            let weight = band(center).chroma;
            match best {
                Some((_, best_weight)) if weight <= best_weight => best,
                _ => Some((center, weight)),
            }
        })
    };
    let Some((strongest, weight)) = strongest_of(&mut (0..TINT_HUE_BINS)) else {
        unreachable!("the color wheel has bands");
    };
    let bin_gap = bins_spanned(TINT_SECOND_MIN_GAP_DEG, true);
    let apart = |center: &usize| {
        let distance = center.abs_diff(strongest);
        distance.min(TINT_HUE_BINS - distance) >= bin_gap
    };
    let mut colors = vec![color_of(band(strongest))];
    if let Some((runner_up, second)) = strongest_of(&mut (0..TINT_HUE_BINS).filter(apart))
        && second >= weight * TINT_SECOND_MIN_SHARE
    {
        colors.push(color_of(band(runner_up)));
    }
    let strength = (sum_chroma / f64::from(counted) / TINT_TYPICAL_CHROMA)
        .clamp(TINT_MIN_STRENGTH, TINT_MAX_STRENGTH);
    ArtTint {
        colors,
        strength,
        lightness,
    }
}

/// The hue of Oklab a and b, in degrees from 0 to 360.
fn hue_of(a: f64, b: f64) -> f64 {
    b.atan2(a).to_degrees().rem_euclid(360.0)
}

/// How many bins of the color wheel an angle spans, rounded down or up.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "angles are never negative and the bins are few"
)]
fn bins_spanned(degrees: f64, round_up: bool) -> usize {
    let bins = degrees / 360.0 * TINT_HUE_BINS as f64;
    (if round_up { bins.ceil() } else { bins }) as usize
}

fn to_linear(byte: u8) -> f64 {
    let c = f64::from(byte) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// sRGB bytes to Oklab lightness, a and b.
#[allow(
    clippy::many_single_char_names,
    reason = "the names of the Oklab paper"
)]
fn srgb_to_oklab(red: u8, green: u8, blue: u8) -> (f64, f64, f64) {
    let (r, g, b) = (to_linear(red), to_linear(green), to_linear(blue));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    (
        0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RGBA pixels from runs of `count` pixels in one color.
    fn pixels(runs: &[(usize, u8, u8, u8, u8)]) -> Vec<u8> {
        runs.iter()
            .flat_map(|&(count, red, green, blue, alpha)| {
                std::iter::repeat_n([red, green, blue, alpha], count).flatten()
            })
            .collect()
    }

    fn distance(a: f64, b: f64) -> f64 {
        let apart = (a - b).abs();
        apart.min(360.0 - apart)
    }

    fn hues(tint: &ArtTint) -> Vec<f64> {
        tint.colors.iter().map(|color| color.hue).collect()
    }

    #[test]
    fn black_white_and_grey_art_has_no_colors() {
        // A faint cast and a speck of color stay grey.
        for runs in [
            &[(900, 0, 0, 0, 255), (124, 38, 44, 38, 255)][..],
            &[(1024, 250, 250, 250, 255)],
            &[(1024, 128, 128, 128, 255)],
            &[(1000, 0, 0, 0, 255), (24, 220, 30, 30, 255)],
            &[(16, 0, 0, 0, 0)],
        ] {
            let tint = tint_of_pixels(&pixels(runs));
            assert!(tint.colors.is_empty(), "{runs:?}");
            assert!(tint.strength.abs() < f64::EPSILON);
        }
    }

    #[test]
    fn measures_how_light_the_art_is() {
        let lightness = |runs: &[(usize, u8, u8, u8, u8)]| tint_of_pixels(&pixels(runs)).lightness;
        assert!((lightness(&[(1024, 255, 255, 255, 255)]).unwrap() - 1.0).abs() < 0.005);
        let half = lightness(&[(512, 0, 0, 0, 255), (512, 255, 255, 255, 255)]).unwrap();
        assert!((half - 0.5).abs() < 0.005);
        assert_eq!(lightness(&[(1024, 0, 0, 0, 0)]), None);
    }

    #[test]
    fn takes_the_main_colors_instead_of_a_mix_of_them() {
        let tint = tint_of_pixels(&pixels(&[(600, 220, 30, 30, 255), (424, 30, 60, 230, 255)]));
        // Red is near 29 degrees and blue near 264, a mix would be magenta.
        let [red, blue] = hues(&tint)[..] else {
            panic!("two colors expected, got {tint:?}");
        };
        assert!(distance(red, 27.0) < 10.0);
        assert!(distance(blue, 264.0) < 10.0);
        assert!(tint.strength > TINT_MIN_STRENGTH);
    }

    #[test]
    fn keeps_both_colors_of_a_logo_in_gold_and_blue() {
        let tint = tint_of_pixels(&pixels(&[
            (300, 0, 0, 0, 255),
            (360, 200, 160, 70, 255),
            (364, 20, 110, 150, 255),
        ]));
        let hues = hues(&tint);
        assert_eq!(hues.len(), 2);
        assert!(hues.iter().any(|&hue| distance(hue, 75.0) < 15.0));
        assert!(hues.iter().any(|&hue| distance(hue, 225.0) < 20.0));
    }

    #[test]
    fn leaves_out_a_small_second_color_and_a_neighboring_shade() {
        let small = tint_of_pixels(&pixels(&[(900, 220, 30, 30, 255), (124, 30, 60, 230, 255)]));
        assert_eq!(small.colors.len(), 1);
        let shade = tint_of_pixels(&pixels(&[
            (512, 220, 30, 30, 255),
            (512, 230, 110, 30, 255),
        ]));
        assert_eq!(shade.colors.len(), 1);
    }

    #[test]
    fn finds_the_color_of_a_logo_on_a_transparent_field() {
        let tint = tint_of_pixels(&pixels(&[(900, 0, 0, 0, 0), (124, 40, 160, 60, 255)]));
        let [green] = hues(&tint)[..] else {
            panic!("one color expected, got {tint:?}");
        };
        assert!(distance(green, 145.0) < 10.0);
    }

    #[test]
    fn reads_an_image_from_a_small_copy() {
        let image = DynamicImage::ImageRgb8(image::RgbImage::from_fn(400, 200, |x, _| {
            if x < 200 {
                image::Rgb([220, 30, 30])
            } else {
                image::Rgb([30, 60, 230])
            }
        }));
        assert_eq!(tint_of_image(&image).colors.len(), 2);
    }
}
