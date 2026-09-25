//! The block world: a sparse voxel grid plus the set of "creations"
//! (connected components) built out of it. Creations can be frozen into the
//! grid (static, zero cost) or promoted to dynamic rigid bodies.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::blocks::{def, Behavior, Block};
use crate::physics::{self, Controls, Body};
use crate::terrain;

pub const SECTION_SIZE: i32 = 16;

#[derive(Clone, Copy, Debug)]
pub struct SectionView {
    pub entity: Entity,
    pub mesh: Handle<Mesh>,
}

/// Marker for the mesh entity of one 16^3 block section.
#[derive(Component)]
pub struct SectionMarker;

/// Euclidean division (glam does not provide it for integer vectors).
#[inline]
pub fn floor_div(a: i32, b: i32) -> i32 {
    let q = a / b;
    let r = a % b;
    if r != 0 && ((r > 0) != (b > 0)) {
        q - 1
    } else {
        q
    }
}

#[derive(Clone, Debug)]
pub struct Creation {
    pub id: u32,
    pub dynamic: bool,
    /// Local block coordinates. For static creations these are world cells.
    pub blocks: Vec<(IVec3, Block)>,
    pub block_set: HashSet<IVec3>,
    pub body: Body,
    /// Logic links, in the same coordinate space as `blocks`.
    pub links: Vec<(IVec3, IVec3)>,
    pub controls: Controls,
    pub entity: Option<Entity>,
    /// Handle of the merged mesh used to render dynamic creations.
    pub mesh: Option<Handle<Mesh>>,
    pub mesh_dirty: bool,
}

impl Creation {
    pub fn new(id: u32, blocks: Vec<(IVec3, Block)>) -> Self {
        let mut c = Self {
            id,
            dynamic: false,
            blocks,
            block_set: HashSet::new(),
            body: Body::default(),
            links: Vec::new(),
            controls: Controls::default(),
            entity: None,
            mesh: None,
            mesh_dirty: true,
        };
        c.rebuild_set();
        c
    }

    pub fn rebuild_set(&mut self) {
        self.block_set = self.blocks.iter().map(|(c, _)| *c).collect();
    }

    pub fn block_at(&self, cell: IVec3) -> Option<&Block> {
        self.blocks.iter().find(|(c, _)| *c == cell).map(|(_, b)| b)
    }

    pub fn block_at_mut(&mut self, cell: IVec3) -> Option<&mut Block> {
        self.blocks.iter_mut().find(|(c, _)| *c == cell).map(|(_, b)| b)
    }

    pub fn has_seat(&self) -> bool {
        self.blocks
            .iter()
            .any(|(_, b)| matches!(def(b.part).behavior, Behavior::Seat))
    }

    pub fn center(&self) -> Vec3 {
        if self.dynamic {
            self.body.pos
        } else {
            let mut acc = Vec3::ZERO;
            for (c, _) in self.blocks.iter() {
                acc += c.as_vec3() + Vec3::splat(0.5);
            }
            acc / self.blocks.len().max(1) as f32
        }
    }
}

// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct BlockWorld {
    pub seed: u32,
    /// Static (frozen into the grid) blocks.
    pub blocks: HashMap<IVec3, Block>,
    pub section_cells: HashMap<IVec3, HashSet<IVec3>>,
    pub sections: HashMap<IVec3, SectionView>,
    pub creations: Vec<Creation>,
    /// Logic links for static blocks, in world coordinates.
    pub links: Vec<(IVec3, IVec3)>,
    pub cell_creation: HashMap<IVec3, u32>,
    pub next_id: u32,
    pub dirty_sections: HashSet<IVec3>,
    pub creations_dirty: bool,
    /// Logic links changed: their little beams must be rebuilt.
    pub links_dirty: bool,
}

impl BlockWorld {
    pub fn new(seed: u32) -> Self {
        Self {
            seed,
            blocks: HashMap::new(),
            section_cells: HashMap::new(),
            sections: HashMap::new(),
            creations: Vec::new(),
            links: Vec::new(),
            cell_creation: HashMap::new(),
            next_id: 1,
            dirty_sections: HashSet::new(),
            creations_dirty: true,
            links_dirty: true,
        }
    }

    pub fn section_of(cell: IVec3) -> IVec3 {
        IVec3::new(
            floor_div(cell.x, SECTION_SIZE),
            floor_div(cell.y, SECTION_SIZE),
            floor_div(cell.z, SECTION_SIZE),
        )
    }

    pub fn set_block(&mut self, cell: IVec3, block: Block) {
        let section = Self::section_of(cell);
        self.blocks.insert(cell, block);
        self.section_cells.entry(section).or_default().insert(cell);
        self.dirty_sections.insert(section);
        self.creations_dirty = true;
    }

    pub fn remove_block(&mut self, cell: IVec3) -> Option<Block> {
        let removed = self.blocks.remove(&cell);
        if removed.is_some() {
            let section = Self::section_of(cell);
            if let Some(set) = self.section_cells.get_mut(&section) {
                set.remove(&cell);
            }
            self.dirty_sections.insert(section);
            self.creations_dirty = true;
        }
        removed
    }

    pub fn creation_by_id(&self, id: u32) -> Option<&Creation> {
        self.creations.iter().find(|c| c.id == id)
    }

    pub fn creation_by_id_mut(&mut self, id: u32) -> Option<&mut Creation> {
        self.creations.iter_mut().find(|c| c.id == id)
    }

    pub fn creation_of_cell(&self, cell: IVec3) -> Option<u32> {
        self.cell_creation.get(&cell).copied()
    }

    /// Rebuilds the connected-component analysis. Cheap enough to run after
    /// every edit for the sizes of build we target.
    pub fn rebuild_creations(&mut self) {
        let mut kept: Vec<Creation> = self.creations.drain(..).filter(|c| c.dynamic).collect();
        let mut visited: HashSet<IVec3> = HashSet::new();
        let cells: Vec<IVec3> = self.blocks.keys().copied().collect();

        for start in cells {
            if visited.contains(&start) {
                continue;
            }
            let mut stack = vec![start];
            visited.insert(start);
            let mut blocks: Vec<(IVec3, Block)> = Vec::new();
            while let Some(cur) = stack.pop() {
                if let Some(b) = self.blocks.get(&cur) {
                    blocks.push((cur, *b));
                }
                for n in [
                    cur + IVec3::X,
                    cur - IVec3::X,
                    cur + IVec3::Y,
                    cur - IVec3::Y,
                    cur + IVec3::Z,
                    cur - IVec3::Z,
                ] {
                    if !visited.contains(&n) && self.blocks.contains_key(&n) {
                        visited.insert(n);
                        stack.push(n);
                    }
                }
            }
            let set: HashSet<IVec3> = blocks.iter().map(|(c, _)| *c).collect();
            let links: Vec<(IVec3, IVec3)> = self
                .links
                .iter()
                .filter(|(a, b)| set.contains(a) && set.contains(b))
                .copied()
                .collect();
            let mut creation = Creation::new(self.next_id, blocks);
            creation.links = links;
            self.next_id += 1;
            kept.push(creation);
        }

        self.creations = kept;
        self.links_dirty = true;
        self.cell_creation.clear();
        for c in self.creations.iter() {
            if !c.dynamic {
                for (cell, _) in c.blocks.iter() {
                    self.cell_creation.insert(*cell, c.id);
                }
            }
        }
        self.creations_dirty = false;
    }

    /// Promotes a static creation to a dynamic rigid body.
    pub fn make_dynamic(&mut self, id: u32) -> bool {
        let Some(idx) = self.creations.iter().position(|c| c.id == id) else {
            return false;
        };
        if self.creations[idx].dynamic {
            return false;
        }
        let mut c = self.creations.remove(idx);

        // pull the blocks out of the static grid
        let cells: Vec<IVec3> = c.blocks.iter().map(|(cell, _)| *cell).collect();
        for cell in cells {
            self.blocks.remove(&cell);
            let section = Self::section_of(cell);
            if let Some(set) = self.section_cells.get_mut(&section) {
                set.remove(&cell);
            }
            self.dirty_sections.insert(section);
        }

        physics::recompute_mass_properties(&mut c);
        let com_world = c.body.com_local;
        let anchor = IVec3::new(
            com_world.x.floor() as i32,
            com_world.y.floor() as i32,
            com_world.z.floor() as i32,
        );
        for (cell, _) in c.blocks.iter_mut() {
            *cell -= anchor;
        }
        for (a, b) in c.links.iter_mut() {
            *a -= anchor;
            *b -= anchor;
        }
        c.dynamic = true;
        physics::recompute_mass_properties(&mut c);
        c.body.pos = anchor.as_vec3() + c.body.com_local;
        c.body.rot = Quat::IDENTITY;
        c.body.vel = Vec3::ZERO;
        c.body.ang_vel = Vec3::ZERO;
        c.rebuild_set();
        c.mesh_dirty = true;
        self.creations.push(c);
        self.rebuild_creations();
        true
    }

    /// Freezes a dynamic creation back into the grid, snapped to cells.
    pub fn freeze(&mut self, id: u32) -> bool {
        let Some(idx) = self.creations.iter().position(|c| c.id == id) else {
            return false;
        };
        if !self.creations[idx].dynamic {
            return false;
        }
        let c = self.creations.remove(idx);

        let mut new_blocks: HashMap<IVec3, Block> = HashMap::new();
        let mut mapping: HashMap<IVec3, IVec3> = HashMap::new();
        for (cell, block) in c.blocks.iter() {
            let wp = physics::block_world_pos(&c, *cell);
            let wc = physics::world_to_cell(wp);
            mapping.insert(*cell, wc);
            new_blocks.insert(wc, *block);
        }
        for (a, b) in c.links.iter() {
            if let (Some(wa), Some(wb)) = (mapping.get(a), mapping.get(b)) {
                if wa != wb {
                    let pair = (*wa, *wb);
                    if !self.links.contains(&pair) {
                        self.links.push(pair);
                    }
                }
            }
        }
        for (cell, block) in new_blocks {
            self.set_block(cell, block);
        }
        self.rebuild_creations();
        true
    }

    /// Freezes everything (used before saving and when the world is unloaded).
    pub fn freeze_all(&mut self) {
        let ids: Vec<u32> = self.creations.iter().filter(|c| c.dynamic).map(|c| c.id).collect();
        for id in ids {
            self.freeze(id);
        }
    }

    pub fn creation_count(&self) -> usize {
        self.creations.len()
    }

    pub fn block_count(&self) -> usize {
        let mut n = self.blocks.len();
        for c in self.creations.iter() {
            if c.dynamic {
                n += c.blocks.len();
            }
        }
        n
    }
}

// ---------------------------------------------------------------------------
// Ray casting
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
pub enum HitKind {
    Terrain,
    StaticBlock { cell: IVec3 },
    DynamicBlock { creation: u32, cell: IVec3 },
}

#[derive(Clone, Copy, Debug)]
pub struct RayHit {
    pub dist: f32,
    pub point: Vec3,
    pub normal: Vec3,
    pub kind: HitKind,
}

/// Marches a ray through terrain, the static grid and every dynamic creation.
pub fn raycast(world: &BlockWorld, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<RayHit> {
    let dir = dir.normalize_or_zero();
    if dir.length_squared() < 0.25 {
        return None;
    }
    let step = 0.05;
    let steps = (max_dist / step).max(1.0) as i32;
    let mut prev = origin;
    let mut prev_cell = physics::world_to_cell(origin);

    for i in 1..=steps {
        let dist = i as f32 * step;
        let p = origin + dir * dist;
        let cell = physics::world_to_cell(p);

        if world.blocks.contains_key(&cell) {
            let normal = axis_normal(prev_cell, cell, dir);
            return Some(RayHit {
                dist,
                point: p,
                normal,
                kind: HitKind::StaticBlock { cell },
            });
        }

        // dynamic creations (few of them, so a linear scan is fine)
        for c in world.creations.iter() {
            if !c.dynamic {
                continue;
            }
            let lp = c.body.rot.inverse() * (p - c.body.pos) + c.body.com_local;
            let lc = physics::world_to_cell(lp);
            if c.block_set.contains(&lc) {
                let lp_prev = c.body.rot.inverse() * (prev - c.body.pos) + c.body.com_local;
                let pc = physics::world_to_cell(lp_prev);
                let n_local = axis_normal(pc, lc, c.body.rot.inverse() * dir);
                return Some(RayHit {
                    dist,
                    point: p,
                    normal: (c.body.rot * n_local).normalize_or_zero(),
                    kind: HitKind::DynamicBlock { creation: c.id, cell: lc },
                });
            }
        }

        if p.y < terrain::height(p.x, p.z, world.seed) {
            return Some(RayHit {
                dist,
                point: p,
                normal: Vec3::Y,
                kind: HitKind::Terrain,
            });
        }

        prev = p;
        prev_cell = cell;
    }
    None
}

fn axis_normal(prev: IVec3, cell: IVec3, dir: Vec3) -> Vec3 {
    let d = cell - prev;
    if d.x != 0 {
        Vec3::new(-d.x as f32, 0.0, 0.0)
    } else if d.y != 0 {
        Vec3::new(0.0, -d.y as f32, 0.0)
    } else if d.z != 0 {
        Vec3::new(0.0, 0.0, -d.z as f32)
    } else {
        -dir.normalize_or_zero()
    }
    .normalize_or_zero()
}
