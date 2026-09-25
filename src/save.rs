//! Save / load. The whole world is a JSON file - no binary blobs, no bundles:
//! you can read it, edit it by hand and share it as a text file.

use bevy::prelude::*;

use crate::blocks::Block;
use crate::world::BlockWorld;

pub const SAVE_PATH: &str = "scrapforge_save.json";
pub const FORMAT_VERSION: u32 = 1;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SavedBlock {
    c: [i32; 3],
    p: u16,
    r: u8,
    col: u8,
    s: u8,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SavedLink {
    a: [i32; 3],
    b: [i32; 3],
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SaveFile {
    pub version: u32,
    pub seed: u32,
    pub player: [f32; 3],
    pub blocks: Vec<SavedBlock>,
    pub links: Vec<SavedLink>,
}

fn to_saved(world: &BlockWorld, player: Vec3) -> SaveFile {
    let mut blocks: Vec<SavedBlock> = world
        .blocks
        .iter()
        .map(|(c, b)| SavedBlock {
            c: [c.x, c.y, c.z],
            p: b.part,
            r: b.rot,
            col: b.color,
            s: b.state,
        })
        .collect();
    blocks.sort_by_key(|b| (b.c[1], b.c[0], b.c[2]));
    SaveFile {
        version: FORMAT_VERSION,
        seed: world.seed,
        player: [player.x, player.y, player.z],
        links: world
            .links
            .iter()
            .map(|(a, b)| SavedLink {
                a: [a.x, a.y, a.z],
                b: [b.x, b.y, b.z],
            })
            .collect(),
        blocks,
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save(world: &BlockWorld, player: Vec3) -> Result<usize, String> {
    let data = to_saved(world, player);
    let json = serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?;
    std::fs::write(SAVE_PATH, json).map_err(|e| e.to_string())?;
    Ok(data.blocks.len())
}

#[cfg(target_arch = "wasm32")]
pub fn save(_world: &BlockWorld, _player: Vec3) -> Result<usize, String> {
    Err("the web build has no file system - use the desktop build to save".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load(world: &mut BlockWorld) -> Result<(Vec3, usize), String> {
    let json = std::fs::read_to_string(SAVE_PATH).map_err(|e| e.to_string())?;
    let data: SaveFile = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    apply(world, data)
}

#[cfg(target_arch = "wasm32")]
pub fn load(_world: &mut BlockWorld) -> Result<(Vec3, usize), String> {
    Err("the web build has no file system".to_string())
}

fn apply(world: &mut BlockWorld, data: SaveFile) -> Result<(Vec3, usize), String> {
    world.blocks.clear();
    world.section_cells.clear();
    for section in world.sections.values() {
        // meshes are rebuilt by the streaming system
        let _ = section;
    }
    world.links.clear();
    world.seed = data.seed;
    for b in data.blocks.iter() {
        let cell = IVec3::new(b.c[0], b.c[1], b.c[2]);
        world.set_block(
            cell,
            Block {
                part: b.p.min(crate::blocks::PARTS.len() as u16 - 1),
                rot: b.r % 4,
                color: b.col,
                state: b.s,
            },
        );
    }
    for l in data.links.iter() {
        world.links.push((
            IVec3::new(l.a[0], l.a[1], l.a[2]),
            IVec3::new(l.b[0], l.b[1], l.b[2]),
        ));
    }
    world.rebuild_creations();
    world.links_dirty = true;
    Ok((
        Vec3::new(data.player[0], data.player[1], data.player[2]),
        data.blocks.len(),
    ))
}

/// Loads a world from a JSON string (used by the in-game import box).
pub fn from_json(world: &mut BlockWorld, json: &str) -> Result<usize, String> {
    let data: SaveFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let (_, n) = apply(world, data)?;
    Ok(n)
}

/// Exports the world as a JSON string (clipboard / share / modding).
pub fn to_json(world: &BlockWorld, player: Vec3) -> String {
    serde_json::to_string_pretty(&to_saved(world, player)).unwrap_or_default()
}

/// Serialises a blueprint (a standalone group of blocks, relative coords).
pub fn blueprint_to_json(blocks: &[(IVec3, Block)]) -> String {
    let file = SaveFile {
        version: FORMAT_VERSION,
        seed: 0,
        player: [0.0, 0.0, 0.0],
        blocks: blocks
            .iter()
            .map(|(c, b)| SavedBlock {
                c: [c.x, c.y, c.z],
                p: b.part,
                r: b.rot,
                col: b.color,
                s: b.state,
            })
            .collect(),
        links: Vec::new(),
    };
    serde_json::to_string(&file).unwrap_or_default()
}

pub fn blueprint_from_json(json: &str) -> Result<Vec<(IVec3, Block)>, String> {
    let data: SaveFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(data
        .blocks
        .iter()
        .map(|b| {
            (
                IVec3::new(b.c[0], b.c[1], b.c[2]),
                Block {
                    part: b.p.min(crate::blocks::PARTS.len() as u16 - 1),
                    rot: b.r % 4,
                    color: b.col,
                    state: b.s,
                },
            )
        })
        .collect())
}
