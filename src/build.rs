//! Building tools: place / remove / paint / rotate, the lift tool (pick a whole
//! contraption up and move it), the connect tool (wire logic together),
//! copy & paste blueprints and a full undo/redo stack.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::blocks::{def, paintable, Block, Category, Behavior, P_BLOCK, P_PLATE, P_WEDGE, P_CYLINDER, P_SPHERE, P_FRAME, P_WHEEL, P_SEAT, P_ENGINE, P_THRUSTER, P_GYRO, P_SWITCH, P_LIGHT, P_SPUDGUN};
use crate::meshgen;
use crate::physics;
use crate::player::{self, Player, EYE_HEIGHT};
use crate::world::{BlockWorld, Creation, HitKind, RayHit};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Build,
    Remove,
    Paint,
    Connect,
}

impl Tool {
    pub fn name(self) -> &'static str {
        match self {
            Tool::Build => "Build",
            Tool::Remove => "Remove",
            Tool::Paint => "Paint",
            Tool::Connect => "Connect",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Edit {
    pub before: Vec<(IVec3, Option<Block>)>,
    pub after: Vec<(IVec3, Option<Block>)>,
}

#[derive(Resource)]
pub struct Editor {
    pub hotbar: Vec<u16>,
    pub slot: usize,
    pub rot: u8,
    pub color: u8,
    pub tool: Tool,
    pub menu_open: bool,
    pub menu_category: usize,
    pub menu_scroll: usize,
    pub connect_from: Option<(u32, IVec3)>,
    pub lifted: Option<u32>,
    pub clipboard: Option<Vec<(IVec3, Block)>>,
    pub undo: Vec<Edit>,
    pub redo: Vec<Edit>,
    pub ghost_part: u16,
    pub last_place: f32,
    pub message: String,
    pub message_timer: f32,
}

impl Default for Editor {
    fn default() -> Self {
        Self {
            hotbar: vec![
                P_BLOCK, P_PLATE, P_WEDGE, P_CYLINDER, P_FRAME, P_WHEEL, P_SEAT, P_ENGINE, P_THRUSTER, P_LIGHT,
            ],
            slot: 0,
            rot: 0,
            color: 0,
            tool: Tool::Build,
            menu_open: false,
            menu_category: 0,
            menu_scroll: 0,
            connect_from: None,
            lifted: None,
            clipboard: None,
            undo: Vec::new(),
            redo: Vec::new(),
            ghost_part: u16::MAX,
            last_place: 0.0,
            message: String::new(),
            message_timer: 0.0,
        }
    }
}

impl Editor {
    pub fn current_part(&self) -> u16 {
        self.hotbar
            .get(self.slot)
            .copied()
            .unwrap_or(P_BLOCK)
    }

    pub fn say(&mut self, text: &str) {
        self.message = text.to_string();
        self.message_timer = 3.5;
    }
}

// ---------------------------------------------------------------------------
// Editing operations
// ---------------------------------------------------------------------------

fn snapshot(world: &BlockWorld, cells: &[IVec3]) -> Vec<(IVec3, Option<Block>)> {
    cells.iter().map(|c| (*c, world.blocks.get(c).copied())).collect()
}

fn apply_snapshot(world: &mut BlockWorld, snap: &[(IVec3, Option<Block>)]) {
    for (cell, block) in snap {
        match block {
            Some(b) => world.set_block(*cell, *b),
            None => {
                world.remove_block(*cell);
            }
        }
    }
    world.rebuild_creations();
}

pub fn place_block(world: &mut BlockWorld, editor: &mut Editor, hit: &RayHit) -> bool {
    let (cell, creation) = match hit.kind {
        HitKind::Terrain => {
            let p = hit.point + hit.normal * 0.001;
            let cell = IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
            if world.blocks.contains_key(&cell) {
                return false;
            }
            (cell, None)
        }
        HitKind::StaticBlock { cell } => {
            let n = round_normal(hit.normal);
            (cell + n, None)
        }
        HitKind::DynamicBlock { creation, cell } => (cell, Some(creation)),
    };

    match creation {
        None => {
            if world.blocks.contains_key(&cell) {
                return false;
            }
            let before = snapshot(world, &[cell]);
            let mut block = Block::new(editor.current_part());
            block.rot = editor.rot;
            block.color = editor.color;
            world.set_block(cell, block);
            let after = snapshot(world, &[cell]);
            world.rebuild_creations();
            editor.undo.push(Edit { before, after });
            editor.redo.clear();
            true
        }
        Some(id) => {
            // placing onto a moving contraption
            let Some(idx) = world.creations.iter().position(|c| c.id == id) else {
                return false;
            };
            let n_local = round_normal(world.creations[idx].body.rot.inverse() * hit.normal);
            let target = cell + n_local;
            if world.creations[idx].block_set.contains(&target) {
                return false;
            }
            let mut block = Block::new(editor.current_part());
            block.rot = editor.rot;
            block.color = editor.color;
            world.creations[idx].blocks.push((target, block));
            world.creations[idx].block_set.insert(target);
            world.creations[idx].mesh_dirty = true;
            physics::recompute_mass_properties(&mut world.creations[idx]);
            true
        }
    }
}

pub fn remove_block(world: &mut BlockWorld, editor: &mut Editor, hit: &RayHit) -> bool {
    match hit.kind {
        HitKind::Terrain => false,
        HitKind::StaticBlock { cell } => {
            if !world.blocks.contains_key(&cell) {
                return false;
            }
            let before = snapshot(world, &[cell]);
            world.remove_block(cell);
            let after = snapshot(world, &[cell]);
            world.rebuild_creations();
            editor.undo.push(Edit { before, after });
            editor.redo.clear();
            true
        }
        HitKind::DynamicBlock { creation, cell } => {
            let Some(idx) = world.creations.iter().position(|c| c.id == creation) else {
                return false;
            };
            if world.creations[idx].blocks.len() <= 1 {
                return false;
            }
            world.creations[idx].blocks.retain(|(c, _)| *c != cell);
            world.creations[idx].rebuild_set();
            world.creations[idx].mesh_dirty = true;
            physics::recompute_mass_properties(&mut world.creations[idx]);
            true
        }
    }
}

pub fn paint_block(world: &mut BlockWorld, editor: &mut Editor, hit: &RayHit) -> bool {
    let (cell, creation) = match hit.kind {
        HitKind::StaticBlock { cell } => (cell, None),
        HitKind::DynamicBlock { creation, cell } => (cell, Some(creation)),
        HitKind::Terrain => return false,
    };
    match creation {
        None => {
            let Some(block) = world.blocks.get(&cell).copied() else {
                return false;
            };
            if !paintable(block.part) {
                editor.say("This part cannot be painted");
                return false;
            }
            let before = snapshot(world, &[cell]);
            let mut block = block;
            block.color = editor.color;
            world.set_block(cell, block);
            let after = snapshot(world, &[cell]);
            world.rebuild_creations();
            editor.undo.push(Edit { before, after });
            editor.redo.clear();
            true
        }
        Some(id) => {
            let Some(idx) = world.creations.iter().position(|c| c.id == id) else {
                return false;
            };
            let Some(pos) = world.creations[idx].blocks.iter().position(|(c, _)| *c == cell) else {
                return false;
            };
            if !paintable(world.creations[idx].blocks[pos].1.part) {
                return false;
            }
            world.creations[idx].blocks[pos].1.color = editor.color;
            world.creations[idx].mesh_dirty = true;
            true
        }
    }
}

/// Toggles a switch / button / sensor block (pressing E).
pub fn interact_block(world: &mut BlockWorld, editor: &mut Editor, hit: &RayHit) -> bool {
    let (cell, creation) = match hit.kind {
        HitKind::StaticBlock { cell } => (cell, None),
        HitKind::DynamicBlock { creation, cell } => (cell, Some(creation)),
        HitKind::Terrain => return false,
    };
    let read = match creation {
        None => world.blocks.get(&cell).copied(),
        Some(id) => world
            .creations
            .iter()
            .find(|c| c.id == id)
            .and_then(|c| c.block_at(cell).copied()),
    };
    let Some(block) = read else { return false };
    if !matches!(
        def(block.part).behavior,
        Behavior::Switch | Behavior::Button
    ) {
        return false;
    }

    let mut new_block = block;
    match def(block.part).behavior {
        Behavior::Switch => new_block.set_powered(!block.powered()),
        Behavior::Button => new_block.set_powered(true),
        _ => {}
    }
    new_block.set_latched(new_block.powered());

    match creation {
        None => {
            world.set_block(cell, new_block);
            world.rebuild_creations();
        }
        Some(id) => {
            let Some(idx) = world.creations.iter().position(|c| c.id == id) else {
                return false;
            };
            if let Some(b) = world.creations[idx].block_at_mut(cell) {
                *b = new_block;
            }
            world.creations[idx].mesh_dirty = true;
        }
    }
    editor.say(if new_block.powered() { "ON" } else { "OFF" });
    true
}

/// Connect tool: creates a logic link between two blocks of one creation.
pub fn connect_block(world: &mut BlockWorld, editor: &mut Editor, hit: &RayHit) -> bool {
    let (cell, creation) = match hit.kind {
        HitKind::StaticBlock { cell } => (cell, match world.creation_of_cell(cell) {
            Some(id) => Some(id),
            None => {
                editor.say("Select two blocks of the same creation");
                return false;
            }
        }),
        HitKind::DynamicBlock { creation, cell } => (cell, Some(creation)),
        HitKind::Terrain => return false,
    };

    match editor.connect_from {
        None => {
            editor.connect_from = Some((creation.unwrap_or(0), cell));
            editor.say("Source selected - click the target block");
            true
        }
        Some((src_creation, src_cell)) => {
            if src_creation != creation.unwrap_or(0) {
                editor.say("Links only work inside one creation");
                editor.connect_from = None;
                return false;
            }
            if src_cell == cell {
                editor.connect_from = None;
                return false;
            }
            let Some(idx) = world.creations.iter().position(|c| c.id == creation.unwrap_or(0)) else {
                editor.connect_from = None;
                return false;
            };
            let c = &mut world.creations[idx];
            let pair = (src_cell, cell);
            if c.links.contains(&pair) || c.links.contains(&(cell, src_cell)) {
                c.links.retain(|l| *l != pair && *l != (cell, src_cell));
                editor.say("Link removed");
            } else {
                c.links.push(pair);
                editor.say("Linked");
            }
            c.mesh_dirty = true;
            world.links_dirty = true;
            editor.connect_from = None;
            true
        }
    }
}

pub fn undo(world: &mut BlockWorld, editor: &mut Editor) {
    if let Some(edit) = editor.undo.pop() {
        apply_snapshot(world, &edit.before);
        editor.redo.push(edit);
        editor.say("Undo");
    }
}

pub fn redo(world: &mut BlockWorld, editor: &mut Editor) {
    if let Some(edit) = editor.redo.pop() {
        apply_snapshot(world, &edit.after);
        editor.undo.push(edit);
        editor.say("Redo");
    }
}

/// Copies the hovered creation into the clipboard (relative coordinates).
pub fn copy_creation(world: &BlockWorld, editor: &mut Editor, hit: &RayHit) {
    let (cell, creation) = match hit.kind {
        HitKind::StaticBlock { cell } => (cell, match world.creation_of_cell(cell) {
            Some(id) => Some(id),
            None => return,
        }),
        HitKind::DynamicBlock { creation, cell } => (cell, Some(creation)),
        HitKind::Terrain => return,
    };
    let Some(id) = creation else { return };
    let Some(c) = world.creation_by_id(id) else { return };
    let origin = if c.dynamic {
        c.blocks
            .iter()
            .map(|(cc, _)| *cc)
            .fold(IVec3::new(i32::MAX, i32::MAX, i32::MAX), |a, b| a.min(b))
    } else {
        cell
    };
    let blocks: Vec<(IVec3, Block)> = c
        .blocks
        .iter()
        .map(|(cc, b)| (*cc - origin, *b))
        .collect();
    editor.clipboard = Some(blocks);
    editor.say("Copied - press V-place (Ctrl+V) to paste");
}

pub fn paste_creation(world: &mut BlockWorld, editor: &mut Editor, hit: &RayHit) {
    let Some(clip) = editor.clipboard.clone() else {
        editor.say("Clipboard is empty");
        return;
    };
    let base = match hit.kind {
        HitKind::Terrain => {
            let p = hit.point + hit.normal * 0.6;
            IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)
        }
        HitKind::StaticBlock { cell } => cell + round_normal(hit.normal),
        HitKind::DynamicBlock { .. } => {
            editor.say("Paste onto terrain or a static build");
            return;
        }
    };
    let rot = editor.rot % 4;
    let rotate_offset = |c: IVec3| -> IVec3 {
        // 90 degree steps around Y: (x, z) -> (z, -x)
        let mut v = c;
        for _ in 0..rot {
            v = IVec3::new(v.z, v.y, -v.x);
        }
        v
    };
    let cells: Vec<IVec3> = clip.iter().map(|(c, _)| base + rotate_offset(*c)).collect();
    let before = snapshot(world, &cells);
    let mut placed = 0;
    for (offset, block) in clip.iter() {
        let cell = base + rotate_offset(*offset);
        if world.blocks.contains_key(&cell) {
            continue;
        }
        let mut b = *block;
        b.rot = (b.rot + rot) % 4;
        world.set_block(cell, b);
        placed += 1;
    }
    let after = snapshot(world, &cells);
    world.rebuild_creations();
    editor.undo.push(Edit { before, after });
    editor.redo.clear();
    editor.say(&format!("Pasted {} blocks", placed));
}

// ---------------------------------------------------------------------------
// Lift tool
// ---------------------------------------------------------------------------

/// Picks up the hovered creation (or drops the one being carried).
pub fn toggle_lift(world: &mut BlockWorld, editor: &mut Editor, player: &Player, eye: Vec3, dir: Vec3) {
    if let Some(id) = editor.lifted.take() {
        world.freeze(id);
        editor.say("Dropped");
        return;
    }
    let Some(hit) = crate::world::raycast(world, eye, dir, 10.0) else {
        return;
    };
    let creation_id = match hit.kind {
        HitKind::StaticBlock { cell } => world.creation_of_cell(cell),
        HitKind::DynamicBlock { creation, .. } => Some(creation),
        HitKind::Terrain => None,
    };
    let Some(id) = creation_id else { return };
    if world.make_dynamic(id) {
        editor.lifted = Some(id);
        editor.say("Lifted - move with the mouse, R rotates, G drops");
    }
}

/// Keeps the lifted creation in front of the player.
pub fn update_lifted(world: &mut BlockWorld, editor: &Editor, player: &Player, eye: Vec3, dir: Vec3) {
    let Some(id) = editor.lifted else { return };
    let Some(idx) = world.creations.iter().position(|c| c.id == id) else {
        return;
    };
    let c = &mut world.creations[idx];
    if !c.dynamic {
        return;
    }
    let target = eye + dir * 4.5;
    c.body.pos = target;
    c.body.rot = Quat::IDENTITY;
    c.body.vel = Vec3::ZERO;
    c.body.ang_vel = Vec3::ZERO;
    let _ = player;
}

/// Rotates the lifted creation by 90 degrees.
pub fn rotate_lifted(world: &mut BlockWorld, editor: &Editor) {
    let Some(id) = editor.lifted else { return };
    let Some(idx) = world.creations.iter().position(|c| c.id == id) else {
        return;
    };
    let c = &mut world.creations[idx];
    let yaw = Quat::from_axis_angle(Vec3::Y, std::f32::consts::FRAC_PI_2);
    let rot = yaw * c.body.rot;
    // rotate block coordinates so that freezing snaps to a clean grid
    let com = c.body.com_local;
    let blocks: Vec<(IVec3, Block)> = c
        .blocks
        .iter()
        .map(|(cell, b)| {
            let p = cell.as_vec3() + Vec3::splat(0.5) - com;
            let p = yaw * p;
            let new_cell = IVec3::new(
                (p.x + com.x - 0.5).round() as i32,
                (p.y + com.y - 0.5).round() as i32,
                (p.z + com.z - 0.5).round() as i32,
            );
            let mut nb = *b;
            nb.rot = (nb.rot + 1) % 4;
            (new_cell, nb)
        })
        .collect();
    c.blocks = blocks;
    c.rebuild_set();
    c.body.rot = rot;
    c.mesh_dirty = true;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn round_normal(n: Vec3) -> IVec3 {
    let a = [n.x.abs(), n.y.abs(), n.z.abs()];
    let max = a[0].max(a[1]).max(a[2]);
    if max == a[0] {
        IVec3::new(n.x.signum() as i32, 0, 0)
    } else if max == a[1] {
        IVec3::new(0, n.y.signum() as i32, 0)
    } else {
        IVec3::new(0, 0, n.z.signum() as i32)
    }
}

/// Returns the part list of a category, used by the part menu.
pub fn parts_in_category(category: Category) -> Vec<u16> {
    (0..crate::blocks::PARTS.len() as u16)
        .filter(|id| def(*id).category == category)
        .collect()
}

/// Builds the ghost mesh of the currently selected part.
pub fn ghost_mesh(part: u16, rot: u8, color: u8) -> Mesh {
    let block = Block { part, rot, color, state: 0 };
    meshgen::build_block_mesh(&[(IVec3::ZERO, block)], None, IVec3::ZERO)
}

pub const fn default_hotbar() -> [u16; 10] {
    [
        P_BLOCK, P_PLATE, P_WEDGE, P_CYLINDER, P_FRAME, P_WHEEL, P_SEAT, P_ENGINE, P_THRUSTER, P_LIGHT,
    ]
}

#[allow(dead_code)]
pub fn creation_summary(world: &BlockWorld, id: u32) -> String {
    match world.creation_by_id(id) {
        Some(c) => format!(
            "{} blocks, {:.0} kg{}",
            c.blocks.len(),
            c.body.mass,
            if c.dynamic { " (dynamic)" } else { "" }
        ),
        None => "none".to_string(),
    }
}

/// Used by the HUD: describes what the player is aiming at.
pub fn describe_hit(world: &BlockWorld, hit: Option<&RayHit>) -> String {
    match hit {
        None => String::new(),
        Some(hit) => match hit.kind {
            HitKind::Terrain => "terrain".to_string(),
            HitKind::StaticBlock { cell } => match world.blocks.get(&cell) {
                Some(b) => format!("{} @ {}", def(b.part).name, cell),
                None => String::new(),
            },
            HitKind::DynamicBlock { cell, .. } => format!("moving part @ {}", cell),
        },
    }
}

/// Spawns a projectile from a spudgun of a creation.
pub fn fire_spudgun(
    world: &BlockWorld,
    id: u32,
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let Some(c) = world.creation_by_id(id) else { return };
    for (cell, block) in c.blocks.iter() {
        if let Behavior::Spudgun { speed } = def(block.part).behavior {
            if !block.powered() {
                continue;
            }
            let origin = physics::block_world_pos(c, *cell);
            let dir = (c.body.rot * crate::blocks::forward(block.rot)).normalize_or_zero();
            spawn_projectile(commands, meshes, materials, origin + dir * 0.6, dir * speed + c.body.vel);
        }
    }
}

pub fn spawn_projectile(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    pos: Vec3,
    vel: Vec3,
) {
    let mesh = meshes.add(meshgen::build_marker_mesh(0.14, [0.55, 0.45, 0.25]));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.75, 0.45),
        emissive: Color::srgb(0.25, 0.2, 0.05).into(),
        ..default()
    });
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(material),
        Transform::from_translation(pos),
        Projectile { vel, life: 6.0 },
    ));
}

#[derive(Component)]
pub struct Projectile {
    pub vel: Vec3,
    pub life: f32,
}

/// Map of cell -> powered state, used to colour logic links.
pub fn powered_map(c: &Creation) -> HashMap<IVec3, bool> {
    c.blocks.iter().map(|(cell, b)| (*cell, b.powered())).collect()
}
