//! Procedural terrain: an analytic height function plus streamed chunk meshes.
//! There are no height maps, no splat maps and no textures - every colour is
//! computed from height/slope at mesh build time and baked into vertex colours.

use bevy::prelude::*;

use crate::noise;

pub const WATER_LEVEL: f32 = 0.0;
/// Chunk edge length in metres.
pub const CHUNK_SIZE: f32 = 32.0;
/// Quads per chunk edge.
pub const CHUNK_RES: usize = 16;

/// Analytic terrain height. Cheap enough to be sampled thousands of times per
/// frame (it is used for collision, not only for rendering).
pub fn height(x: f32, z: f32, seed: u32) -> f32 {
    let continents = noise::fbm(x * 0.0032, z * 0.0032, seed, 4);
    let land = continents * 2.0 - 0.85;

    let mountains = noise::ridge(x * 0.0068, z * 0.0068, seed ^ 0x1234_5678, 4);
    let mountain_mask = (continents - 0.55).clamp(0.0, 1.0) * 2.2;

    let hills = noise::fbm(x * 0.021, z * 0.021, seed ^ 0xabcd_ef01, 3) - 0.5;
    let detail = noise::value_noise2(x * 0.11, z * 0.11, seed ^ 0x55aa_55aa) - 0.5;

    let mut h = land * 26.0;
    h += mountains * mountain_mask * 34.0;
    h += hills * 4.5;
    h += detail * 0.55;

    // Carve a flat-ish basin around the spawn point so the player always has a
    // sane place to start building.
    let d = (x * x + z * z).sqrt();
    let flatten = (1.0 - (d / 90.0).clamp(0.0, 1.0)).powf(1.5);
    h = h * (1.0 - flatten) + 1.6 * flatten;

    h
}

/// Central-difference terrain normal.
pub fn normal_at(x: f32, z: f32, seed: u32) -> Vec3 {
    let e = 0.6;
    let hl = height(x - e, z, seed);
    let hr = height(x + e, z, seed);
    let hd = height(x, z - e, seed);
    let hu = height(x, z + e, seed);
    Vec3::new(hl - hr, 2.0 * e, hd - hu).normalize()
}

/// Approximate terrain slope (0 = flat, 1 = vertical).
pub fn slope_at(x: f32, z: f32, seed: u32) -> f32 {
    (1.0 - normal_at(x, z, seed).y).clamp(0.0, 1.0)
}

/// Biome colour in sRGB, derived from height and slope.
pub fn color_at(h: f32, slope: f32, seed: u32, x: f32, z: f32) -> [f32; 3] {
    let tint = noise::value_noise2(x * 0.05, z * 0.05, seed ^ 0x77aa_33bb) - 0.5;
    let sand = [0.80, 0.72, 0.48];
    let grass = [0.30, 0.52, 0.20];
    let forest = [0.17, 0.38, 0.16];
    let rock = [0.44, 0.42, 0.40];
    let snow = [0.92, 0.94, 0.97];

    let mut c = if h < WATER_LEVEL + 0.6 {
        sand
    } else if h < 1.6 {
        mix(sand, grass, ((h - 0.6) / 1.0).clamp(0.0, 1.0))
    } else if h < 14.0 {
        mix(grass, forest, ((h - 1.6) / 12.4).clamp(0.0, 1.0))
    } else if h < 26.0 {
        mix(forest, rock, ((h - 14.0) / 12.0).clamp(0.0, 1.0))
    } else {
        mix(rock, snow, ((h - 26.0) / 10.0).clamp(0.0, 1.0))
    };

    // Steep faces are bare rock.
    if slope > 0.45 {
        let t = ((slope - 0.45) / 0.35).clamp(0.0, 1.0);
        c = mix(c, rock, t);
    }
    // Underwater mud.
    if h < WATER_LEVEL - 0.2 {
        let t = ((WATER_LEVEL - 0.2 - h) / 6.0).clamp(0.0, 1.0) * 0.7;
        c = mix(c, [0.24, 0.22, 0.18], t);
    }
    // Cheap large scale variation so it does not look like flat paint.
    let v = 1.0 + tint * 0.16;
    [c[0] * v, c[1] * v, c[2] * v]
}

#[inline]
pub fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Chunk coordinate (in chunk units) for a world position.
#[inline]
pub fn chunk_of(p: Vec3) -> IVec2 {
    IVec2::new(
        (p.x / CHUNK_SIZE).floor() as i32,
        (p.z / CHUNK_SIZE).floor() as i32,
    )
}

/// sRGB -> linear, because `Mesh::ATTRIBUTE_COLOR` is expected in linear space.
#[inline]
pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
