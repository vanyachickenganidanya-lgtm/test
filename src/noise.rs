//! Deterministic hash-based noise. No dependencies, no lookup tables, no assets:
//! everything the world needs is generated from a 32 bit seed.

/// Integer hash (a variant of the lowbias32 mixer).
#[inline]
pub fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

#[inline]
pub fn hash2(x: i32, z: i32, seed: u32) -> u32 {
    hash_u32(x as u32 ^ hash_u32((z as u32).wrapping_add(0x9e37_79b9) ^ hash_u32(seed)))
}

/// Pseudo random value in `[0, 1)` for a lattice point.
#[inline]
pub fn rand01(x: i32, z: i32, seed: u32) -> f32 {
    (hash2(x, z, seed) >> 8) as f32 / 16_777_216.0
}

#[inline]
fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Bilinear value noise in `[0, 1)`.
pub fn value_noise2(x: f32, z: f32, seed: u32) -> f32 {
    let x0 = x.floor();
    let z0 = z.floor();
    let fx = smooth(x - x0);
    let fz = smooth(z - z0);
    let i = x0 as i32;
    let j = z0 as i32;
    let a = rand01(i, j, seed);
    let b = rand01(i + 1, j, seed);
    let c = rand01(i, j + 1, seed);
    let d = rand01(i + 1, j + 1, seed);
    let ab = a + (b - a) * fx;
    let cd = c + (d - c) * fx;
    ab + (cd - ab) * fz
}

/// Fractal brownian motion in `[0, 1)`.
pub fn fbm(x: f32, z: f32, seed: u32, octaves: u32) -> f32 {
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for o in 0..octaves {
        sum += amp * value_noise2(x * freq, z * freq, seed.wrapping_add(o * 0x9e37_79b1));
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm.max(0.0001)
}

/// Ridge noise, used for mountain ranges.
pub fn ridge(x: f32, z: f32, seed: u32, octaves: u32) -> f32 {
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for o in 0..octaves {
        let n = (value_noise2(x * freq, z * freq, seed.wrapping_add(o * 0x85eb_ca6b)) - 0.5).abs();
        sum += amp * (1.0 - n * 2.0);
        norm += amp;
        amp *= 0.5;
        freq *= 2.03;
    }
    (sum / norm.max(0.0001)).clamp(0.0, 1.0)
}
