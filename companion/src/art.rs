//! Album art -> tiny RGB565 thumbnail + accent colour for the pet.

use base64::Engine;
use image::imageops::FilterType;

/// Must match ART_SIZE in src/pet/music.h.
pub const ART_SIZE: u32 = 56;

pub struct Art {
    /// base64 of ART_SIZE*ART_SIZE big-endian RGB565 pixels.
    pub b64: String,
    /// "#rrggbb" vivid accent picked from the cover.
    pub color: String,
}

pub fn process(bytes: &[u8]) -> Option<Art> {
    let img = image::load_from_memory(bytes).ok()?.to_rgb8();
    let (w, h) = img.dimensions();
    let side = w.min(h);
    let cropped = image::imageops::crop_imm(&img, (w - side) / 2, (h - side) / 2, side, side).to_image();
    let small = image::imageops::resize(&cropped, ART_SIZE, ART_SIZE, FilterType::Triangle);

    let mut raw = Vec::with_capacity((ART_SIZE * ART_SIZE * 2) as usize);
    // Saturation-weighted average gives a colour that "feels" like the cover.
    let (mut sr, mut sg, mut sb, mut sw) = (0f64, 0f64, 0f64, 0f64);
    for p in small.pixels() {
        let [r, g, b] = p.0;
        let v: u16 = ((r as u16 >> 3) << 11) | ((g as u16 >> 2) << 5) | (b as u16 >> 3);
        raw.extend_from_slice(&v.to_be_bytes());

        let mx = r.max(g).max(b) as f64;
        let mn = r.min(g).min(b) as f64;
        let sat = if mx > 0.0 { (mx - mn) / mx } else { 0.0 };
        let wgt = sat * sat * (mx / 255.0) + 0.002;
        sr += r as f64 * wgt;
        sg += g as f64 * wgt;
        sb += b as f64 * wgt;
        sw += wgt;
    }
    let (mut r, mut g, mut b) = (sr / sw, sg / sw, sb / sw);
    // Lift dark colours so the accent always pops on black.
    let mx = r.max(g).max(b).max(1.0);
    if mx < 200.0 {
        let k = 200.0 / mx;
        r = (r * k).min(255.0);
        g = (g * k).min(255.0);
        b = (b * k).min(255.0);
    }

    Some(Art {
        b64: base64::engine::general_purpose::STANDARD.encode(&raw),
        color: format!("#{:02x}{:02x}{:02x}", r as u8, g as u8, b as u8),
    })
}
