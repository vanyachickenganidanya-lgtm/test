//! All geometry in ScrapForge is generated at runtime. There is not a single
//! texture, model or material file in the repository: shapes are built here and
//! colours are baked into `Mesh::ATTRIBUTE_COLOR`.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;

use crate::blocks::{block_color, def, Block, Shape};
use crate::terrain::{self, CHUNK_RES, CHUNK_SIZE, WATER_LEVEL};

pub struct MeshBuilder {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

impl MeshBuilder {
    pub fn new() -> Self {
        Self { pos: Vec::new(), nrm: Vec::new(), col: Vec::new(), idx: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.idx.is_empty()
    }

    fn push(&mut self, p: Vec3, n: Vec3, c: [f32; 3]) -> u32 {
        let i = self.pos.len() as u32;
        self.pos.push([p.x, p.y, p.z]);
        self.nrm.push([n.x, n.y, n.z]);
        self.col.push([
            terrain::srgb_to_linear(c[0].clamp(0.0, 1.0)),
            terrain::srgb_to_linear(c[1].clamp(0.0, 1.0)),
            terrain::srgb_to_linear(c[2].clamp(0.0, 1.0)),
            1.0,
        ]);
        i
    }

    pub fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3, color: [f32; 3]) {
        let n = (b - a).cross(c - a);
        if n.length_squared() < 1e-12 {
            return;
        }
        let n = n.normalize();
        let i0 = self.push(a, n, color);
        let i1 = self.push(b, n, color);
        let i2 = self.push(c, n, color);
        self.idx.extend_from_slice(&[i0, i1, i2]);
    }

    pub fn quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, color: [f32; 3]) {
        self.tri(a, b, c, color);
        self.tri(a, c, d, color);
    }

    /// Axis aligned box with per-face culling.
    pub fn box_aa(&mut self, min: Vec3, max: Vec3, color: [f32; 3], cull: [bool; 6]) {
        let (x0, y0, z0) = (min.x, min.y, min.z);
        let (x1, y1, z1) = (max.x, max.y, max.z);
        if !cull[0] {
            // -X
            self.quad(
                Vec3::new(x0, y0, z0),
                Vec3::new(x0, y0, z1),
                Vec3::new(x0, y1, z1),
                Vec3::new(x0, y1, z0),
                color,
            );
        }
        if !cull[1] {
            // +X
            self.quad(
                Vec3::new(x1, y0, z1),
                Vec3::new(x1, y0, z0),
                Vec3::new(x1, y1, z0),
                Vec3::new(x1, y1, z1),
                color,
            );
        }
        if !cull[2] {
            // -Y
            self.quad(
                Vec3::new(x0, y0, z0),
                Vec3::new(x1, y0, z0),
                Vec3::new(x1, y0, z1),
                Vec3::new(x0, y0, z1),
                color,
            );
        }
        if !cull[3] {
            // +Y
            self.quad(
                Vec3::new(x0, y1, z1),
                Vec3::new(x1, y1, z1),
                Vec3::new(x1, y1, z0),
                Vec3::new(x0, y1, z0),
                color,
            );
        }
        if !cull[4] {
            // -Z
            self.quad(
                Vec3::new(x1, y0, z0),
                Vec3::new(x0, y0, z0),
                Vec3::new(x0, y1, z0),
                Vec3::new(x1, y1, z0),
                color,
            );
        }
        if !cull[5] {
            // +Z
            self.quad(
                Vec3::new(x0, y0, z1),
                Vec3::new(x1, y0, z1),
                Vec3::new(x1, y1, z1),
                Vec3::new(x0, y1, z1),
                color,
            );
        }
    }

    /// Cylinder with `segments` sides, centred on the cell, axis along Y.
    pub fn cylinder(&mut self, center: Vec3, radius: f32, height: f32, segments: u32, color: [f32; 3], shade_top: bool) {
        let half = height * 0.5;
        let top_c = if shade_top { shade(color, 1.12) } else { color };
        for s in 0..segments {
            let a0 = s as f32 / segments as f32 * std::f32::consts::TAU;
            let a1 = (s + 1) as f32 / segments as f32 * std::f32::consts::TAU;
            let (s0, c0) = a0.sin_cos();
            let (s1, c1) = a1.sin_cos();
            let p0 = center + Vec3::new(c0 * radius, -half, s0 * radius);
            let p1 = center + Vec3::new(c1 * radius, -half, s1 * radius);
            let p2 = center + Vec3::new(c1 * radius, half, s1 * radius);
            let p3 = center + Vec3::new(c0 * radius, half, s0 * radius);
            let n = Vec3::new(c0, 0.0, s0);
            let i0 = self.push(p0, n, color);
            let i1 = self.push(p1, n, color);
            let i2 = self.push(p2, n, color);
            let i3 = self.push(p3, n, color);
            self.idx.extend_from_slice(&[i0, i1, i2, i0, i2, i3]);
        }
        // caps
        let top = center + Vec3::Y * half;
        let bottom = center - Vec3::Y * half;
        for s in 0..segments {
            let a0 = s as f32 / segments as f32 * std::f32::consts::TAU;
            let a1 = (s + 1) as f32 / segments as f32 * std::f32::consts::TAU;
            let (s0, c0) = a0.sin_cos();
            let (s1, c1) = a1.sin_cos();
            self.tri(
                top,
                top + Vec3::new(c0 * radius, 0.0, s0 * radius),
                top + Vec3::new(c1 * radius, 0.0, s1 * radius),
                top_c,
            );
            self.tri(
                bottom,
                bottom + Vec3::new(c1 * radius, 0.0, s1 * radius),
                bottom + Vec3::new(c0 * radius, 0.0, s0 * radius),
                shade(color, 0.75),
            );
        }
    }

    pub fn sphere(&mut self, center: Vec3, radius: f32, rings: u32, sectors: u32, color: [f32; 3]) {
        for r in 0..rings {
            let phi0 = r as f32 / rings as f32 * std::f32::consts::PI;
            let phi1 = (r + 1) as f32 / rings as f32 * std::f32::consts::PI;
            for s in 0..sectors {
                let th0 = s as f32 / sectors as f32 * std::f32::consts::TAU;
                let th1 = (s + 1) as f32 / sectors as f32 * std::f32::consts::TAU;
                let v = |phi: f32, th: f32| {
                    let (sp, cp) = phi.sin_cos();
                    Vec3::new(sp * th.cos(), cp, sp * th.sin()) * radius
                };
                let a = center + v(phi0, th0);
                let b = center + v(phi1, th0);
                let c = center + v(phi1, th1);
                let d = center + v(phi0, th1);
                let na = (a - center).normalize();
                let i0 = self.push(a, na, color);
                let i1 = self.push(b, (b - center).normalize(), color);
                let i2 = self.push(c, (c - center).normalize(), color);
                let i3 = self.push(d, (d - center).normalize(), color);
                self.idx.extend_from_slice(&[i0, i1, i2, i0, i2, i3]);
                let _ = na;
            }
        }
    }

    /// Thin frame: four vertical beams (looks like a scaffolding cube).
    pub fn frame(&mut self, min: Vec3, color: [f32; 3]) {
        let t = 0.09;
        for dx in [0.0, 1.0 - t] {
            for dz in [0.0, 1.0 - t] {
                self.box_aa(
                    min + Vec3::new(dx, 0.0, dz),
                    min + Vec3::new(dx + t, 1.0, dz + t),
                    color,
                    [false; 6],
                );
            }
        }
        // top ring
        for dx in [0.0, 1.0 - t] {
            self.box_aa(
                min + Vec3::new(dx, 1.0 - t, 0.0),
                min + Vec3::new(dx + t, 1.0, 1.0),
                color,
                [false; 6],
            );
        }
        for dz in [0.0, 1.0 - t] {
            self.box_aa(
                min + Vec3::new(0.0, 1.0 - t, dz),
                min + Vec3::new(1.0, 1.0, dz + t),
                color,
                [false; 6],
            );
        }
    }

    pub fn build(self) -> Mesh {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        mesh = mesh
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.pos)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.nrm)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.col)
            .with_inserted_indices(Indices::U32(self.idx));
        mesh
    }
}

fn shade(c: [f32; 3], k: f32) -> [f32; 3] {
    [c[0] * k, c[1] * k, c[2] * k]
}

/// Adds one block to the builder. `occupied` is used for face culling and
/// should return true when the neighbouring cell is a solid full cube.
pub fn add_block(
    mb: &mut MeshBuilder,
    cell: IVec3,
    block: &Block,
    occupied: &dyn Fn(IVec3) -> bool,
) {
    let min = cell.as_vec3();
    let mut color = block_color(block);
    if block.powered() {
        color = shade(color, 1.55);
    }
    let yaw = Quat::from_axis_angle(Vec3::Y, (block.rot % 4) as f32 * std::f32::consts::FRAC_PI_2);
    let around = |v: Vec3| min + Vec3::new(0.5, 0.5, 0.5) + yaw * (v - Vec3::new(0.5, 0.5, 0.5));

    match def(block.part).shape {
        Shape::Cube => {
            let cull = [
                occupied(cell - IVec3::X),
                occupied(cell + IVec3::X),
                occupied(cell - IVec3::Y),
                occupied(cell + IVec3::Y),
                occupied(cell - IVec3::Z),
                occupied(cell + IVec3::Z),
            ];
            mb.box_aa(min, min + Vec3::ONE, color, cull);
            // subtle inset panel so big walls are not visually flat
            if !cull[3] {
                mb.quad(
                    min + Vec3::new(0.12, 1.001, 0.12),
                    min + Vec3::new(0.88, 1.001, 0.12),
                    min + Vec3::new(0.88, 1.001, 0.88),
                    min + Vec3::new(0.12, 1.001, 0.88),
                    shade(color, 1.06),
                );
            }
        }
        Shape::Plate => {
            mb.box_aa(min, min + Vec3::new(1.0, 0.25, 1.0), color, [false; 6]);
        }
        Shape::Wedge => {
            let p = |x: f32, y: f32, z: f32| around(Vec3::new(x, y, z));
            // bottom (-Y)
            mb.quad(p(0.0, 0.0, 0.0), p(1.0, 0.0, 0.0), p(1.0, 0.0, 1.0), p(0.0, 0.0, 1.0), shade(color, 0.8));
            // back (-Z)
            mb.quad(p(1.0, 0.0, 0.0), p(0.0, 0.0, 0.0), p(0.0, 1.0, 0.0), p(1.0, 1.0, 0.0), color);
            // sides
            mb.tri(p(0.0, 0.0, 0.0), p(0.0, 0.0, 1.0), p(0.0, 1.0, 0.0), shade(color, 0.9));
            mb.tri(p(1.0, 0.0, 1.0), p(1.0, 0.0, 0.0), p(1.0, 1.0, 0.0), shade(color, 0.9));
            // slope
            mb.quad(p(0.0, 0.0, 1.0), p(1.0, 0.0, 1.0), p(1.0, 1.0, 0.0), p(0.0, 1.0, 0.0), shade(color, 1.08));
        }
        Shape::Cylinder => {
            let center = min + Vec3::new(0.5, 0.5, 0.5);
            mb.cylinder(center, 0.45, 1.0, 14, color, true);
        }
        Shape::Sphere => {
            let center = min + Vec3::new(0.5, 0.5, 0.5);
            mb.sphere(center, 0.47, 8, 12, color);
        }
        Shape::Frame => {
            mb.frame(min, color);
        }
        Shape::Wheel => {
            let radius = match def(block.part).behavior {
                crate::blocks::Behavior::Wheel { radius, .. } => radius,
                _ => 0.45,
            };
            let center = min + Vec3::new(0.5, 0.5, 0.5);
            // Wheel axis lies along the block's local X (rotated by yaw).
            let axis = yaw * Vec3::X;
            let up = yaw * Vec3::Y;
            let fwd = yaw * Vec3::Z;
            let segs = 16u32;
            let half_w = 0.16;
            for s in 0..segs {
                let a0 = s as f32 / segs as f32 * std::f32::consts::TAU;
                let a1 = (s + 1) as f32 / segs as f32 * std::f32::consts::TAU;
                let dir0 = up * a0.cos() + fwd * a0.sin();
                let dir1 = up * a1.cos() + fwd * a1.sin();
                let p0 = center + dir0 * radius - axis * half_w;
                let p1 = center + dir1 * radius - axis * half_w;
                let p2 = center + dir1 * radius + axis * half_w;
                let p3 = center + dir0 * radius + axis * half_w;
                let n = dir0;
                let i0 = mb.push(p0, n, color);
                let i1 = mb.push(p1, n, color);
                let i2 = mb.push(p2, n, color);
                let i3 = mb.push(p3, n, color);
                mb.idx.extend_from_slice(&[i0, i1, i2, i0, i2, i3]);
            }
            // hub + rim highlight
            mb.box_aa(
                center - Vec3::ONE * 0.12,
                center + Vec3::ONE * 0.12,
                shade(color, 2.2),
                [false; 6],
            );
        }
        Shape::Seat => {
            // seat pan + backrest, oriented by yaw
            let c = min + Vec3::new(0.5, 0.5, 0.5);
            let f = yaw * Vec3::Z;
            let r = yaw * Vec3::X;
            mb.box_aa(c - Vec3::Y * 0.2 - r * 0.4 - f * 0.4, c + Vec3::Y * 0.05 + r * 0.4 + f * 0.35, color, [false; 6]);
            mb.box_aa(
                c - Vec3::Y * 0.2 - r * 0.4 + f * 0.30,
                c + Vec3::Y * 0.75 + r * 0.4 + f * 0.45,
                shade(color, 0.9),
                [false; 6],
            );
        }
    }
}

/// Builds the mesh of a set of blocks (a creation or a world section).
/// `neighbors` supplies occupancy for face culling; pass `None` to disable.
pub fn build_block_mesh(
    blocks: &[(IVec3, Block)],
    neighbors: Option<&HashMap<IVec3, Block>>,
    offset: IVec3,
) -> Mesh {
    let mut mb = MeshBuilder::new();
    let mut solid: HashMap<IVec3, bool> = HashMap::new();
    if let Some(map) = neighbors {
        for (c, b) in blocks {
            let cell = *c + offset;
            solid.insert(cell, matches!(def(b.part).shape, Shape::Cube));
        }
    }
    let occupied = |cell: IVec3| -> bool {
        match neighbors {
            Some(map) => map
                .get(&cell)
                .map(|b| matches!(def(b.part).shape, Shape::Cube))
                .unwrap_or(false),
            None => solid.get(&cell).copied().unwrap_or(false),
        }
    };
    for (cell, block) in blocks {
        add_block(&mut mb, *cell, block, &occupied);
    }
    mb.build()
}

/// Builds the mesh of one terrain chunk.
pub fn build_terrain_chunk(chunk: IVec2, seed: u32) -> Mesh {
    let mut mb = MeshBuilder::new();
    let origin = Vec2::new(chunk.x as f32 * CHUNK_SIZE, chunk.y as f32 * CHUNK_SIZE);
    let step = CHUNK_SIZE / CHUNK_RES as f32;

    let mut heights = vec![0f32; (CHUNK_RES + 1) * (CHUNK_RES + 1)];
    for j in 0..=CHUNK_RES {
        for i in 0..=CHUNK_RES {
            let x = origin.x + i as f32 * step;
            let z = origin.y + j as f32 * step;
            heights[j * (CHUNK_RES + 1) + i] = terrain::height(x, z, seed);
        }
    }

    for j in 0..CHUNK_RES {
        for i in 0..CHUNK_RES {
            let x = origin.x + i as f32 * step;
            let z = origin.y + j as f32 * step;
            let h00 = heights[j * (CHUNK_RES + 1) + i];
            let h10 = heights[j * (CHUNK_RES + 1) + i + 1];
            let h01 = heights[(j + 1) * (CHUNK_RES + 1) + i];
            let h11 = heights[(j + 1) * (CHUNK_RES + 1) + i + 1];

            let a = Vec3::new(x, h00, z);
            let b = Vec3::new(x + step, h10, z);
            let c = Vec3::new(x + step, h11, z + step);
            let d = Vec3::new(x, h01, z + step);

            let n = terrain::normal_at(x + step * 0.5, z + step * 0.5, seed);
            let slope = terrain::slope_at(x + step * 0.5, z + step * 0.5, seed);
            let avg_h = (h00 + h10 + h01 + h11) * 0.25;
            let mut col = terrain::color_at(avg_h, slope, seed, x, z);

            // Two triangles, shaded with the analytic normal so the terrain
            // looks smooth even at 2 m resolution.
            let i0 = mb.push(a, n, col);
            let i1 = mb.push(b, n, col);
            let i2 = mb.push(c, n, col);
            let i3 = mb.push(d, n, col);
            mb.idx.extend_from_slice(&[i0, i1, i2, i0, i2, i3]);
            let _ = &mut col;
        }
    }
    mb.build()
}

/// A big flat quad used for the water surface.
pub fn build_water_mesh(size: f32) -> Mesh {
    let mut mb = MeshBuilder::new();
    let h = size * 0.5;
    let color = [0.10, 0.32, 0.52];
    let n = Vec3::Y;
    let i0 = mb.push(Vec3::new(-h, WATER_LEVEL, -h), n, color);
    let i1 = mb.push(Vec3::new(h, WATER_LEVEL, -h), n, color);
    let i2 = mb.push(Vec3::new(h, WATER_LEVEL, h), n, color);
    let i3 = mb.push(Vec3::new(-h, WATER_LEVEL, h), n, color);
    mb.idx.extend_from_slice(&[i0, i1, i2, i0, i2, i3]);
    mb.build()
}

/// Unit cube wireframe-ish box used for the placement highlight.
pub fn build_highlight_mesh() -> Mesh {
    let mut mb = MeshBuilder::new();
    let color = [1.0, 0.9, 0.2];
    mb.box_aa(-Vec3::ONE * 0.52, Vec3::ONE * 0.52, color, [false; 6]);
    mb.build()
}

/// Small box used for link lines / projectiles.
pub fn build_marker_mesh(size: f32, color: [f32; 3]) -> Mesh {
    let mut mb = MeshBuilder::new();
    mb.box_aa(-Vec3::ONE * size, Vec3::ONE * size, color, [false; 6]);
    mb.build()
}

/// Thin beam between two local points (logic link visualisation).
pub fn build_link_mesh(a: Vec3, b: Vec3, color: [f32; 3]) -> Mesh {
    let mut mb = MeshBuilder::new();
    let dir = (b - a).normalize_or_zero();
    if dir.length_squared() < 0.5 {
        return mb.build();
    }
    let up = if dir.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
    let side = dir.cross(up).normalize() * 0.035;
    let side2 = dir.cross(side).normalize() * 0.035;
    let corners = |p: Vec3| [p + side + side2, p + side - side2, p - side - side2, p - side + side2];
    let ca = corners(a);
    let cb = corners(b);
    for k in 0..4 {
        let k2 = (k + 1) % 4;
        mb.quad(ca[k], ca[k2], cb[k2], cb[k], color);
    }
    mb.build()
}
