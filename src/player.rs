//! First person player: free flight, walking with voxel collision, driving and
//! ride-along camera for dynamic creations.

use bevy::prelude::*;

use crate::blocks::{def, Shape};
use crate::physics::{self, GRAVITY};
use crate::terrain;
use crate::world::{raycast, BlockWorld, HitKind};

pub const PLAYER_RADIUS: f32 = 0.34;
pub const PLAYER_HEIGHT: f32 = 1.8;
pub const EYE_HEIGHT: f32 = 1.62;
pub const WALK_SPEED: f32 = 6.2;
pub const SPRINT_SPEED: f32 = 11.0;
pub const FLY_SPEED: f32 = 22.0;

#[derive(Component)]
pub struct Player {
    pub yaw: f32,
    pub pitch: f32,
    pub vel: Vec3,
    pub on_ground: bool,
    pub fly: bool,
    /// Creation the player is sitting in.
    pub riding: Option<u32>,
    pub seat_cell: Option<IVec3>,
    /// Smoothed camera position ( ride-along feels nicer with a little lag ).
    pub cam_pos: Vec3,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: -0.15,
            vel: Vec3::ZERO,
            on_ground: false,
            fly: false,
            riding: None,
            seat_cell: None,
            cam_pos: Vec3::ZERO,
        }
    }
}

#[derive(Component)]
pub struct PlayerCamera;

#[derive(Resource)]
pub struct PlayerSettings {
    pub sensitivity: f32,
    pub fov: f32,
    pub render_distance: i32,
    pub shadows: bool,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            sensitivity: 0.0022,
            fov: 75.0,
            render_distance: 10,
            shadows: true,
        }
    }
}

/// Where the player's eyes are right now.
pub fn eye_position(player: &Player, world: &BlockWorld) -> Vec3 {
    if let Some(id) = player.riding {
        if let Some(c) = world.creation_by_id(id) {
            if let Some(cell) = player.seat_cell {
                return physics::block_world_pos(c, cell) + Vec3::Y * 0.55;
            }
            return c.body.pos + Vec3::Y * 1.0;
        }
    }
    player.cam_pos + Vec3::Y * EYE_HEIGHT
}

pub fn look_direction(yaw: f32, pitch: f32) -> Vec3 {
    let q = Quat::from_axis_angle(Vec3::Y, yaw) * Quat::from_axis_angle(Vec3::X, pitch);
    q * Vec3::NEG_Z
}

pub fn look_quat(yaw: f32, pitch: f32) -> Quat {
    Quat::from_axis_angle(Vec3::Y, yaw) * Quat::from_axis_angle(Vec3::X, pitch)
}

pub fn move_axis(world: &BlockWorld, pos: &mut Vec3, axis: usize, delta: f32) -> bool {
    if delta == 0.0 {
        return false;
    }
    let old = pos[axis];
    pos[axis] = old + delta;
    if blocked(world, *pos) {
        pos[axis] = old;
        return true;
    }
    false
}

/// Is the player AABB intersecting anything solid?
pub fn blocked(world: &BlockWorld, pos: Vec3) -> bool {
    let r = PLAYER_RADIUS - 0.02;
    let min = IVec3::new(
        (pos.x - r).floor() as i32,
        (pos.y + 0.05).floor() as i32,
        (pos.z - r).floor() as i32,
    );
    let max = IVec3::new(
        (pos.x + r).floor() as i32,
        (pos.y + PLAYER_HEIGHT - 0.05).floor() as i32,
        (pos.z + r).floor() as i32,
    );
    for x in min.x..=max.x {
        for y in min.y..=max.y {
            for z in min.z..=max.z {
                if world.blocks.contains_key(&IVec3::new(x, y, z)) {
                    return true;
                }
            }
        }
    }
    // dynamic creations
    for c in world.creations.iter() {
        if !c.dynamic {
            continue;
        }
        if (c.body.pos - pos).length() > 40.0 {
            continue;
        }
        for (cell, _) in c.blocks.iter() {
            let wp = physics::block_world_pos(c, *cell);
            if (wp.y - pos.y).abs() > 2.0 {
                continue;
            }
            if (wp.x - pos.x).abs() > r + 0.5 || (wp.z - pos.z).abs() > r + 0.5 {
                continue;
            }
            if wp.y > pos.y + PLAYER_HEIGHT || wp.y + 1.0 < pos.y {
                continue;
            }
            return true;
        }
    }
    false
}

/// Ground height under a horizontal position, including wedges.
pub fn ground_height(world: &BlockWorld, pos: Vec3) -> f32 {
    let mut best = terrain::height(pos.x, pos.z, world.seed);
    // blocks directly under the player
    let base = IVec3::new(pos.x.floor() as i32, (pos.y + 0.4).floor() as i32, pos.z.floor() as i32);
    for dy in -2..=1 {
        for dx in -1..=1 {
            for dz in -1..=1 {
                let cell = base + IVec3::new(dx, dy, dz);
                let Some(block) = world.blocks.get(&cell) else {
                    continue;
                };
                if (pos.x - (cell.x as f32 + 0.5)).abs() > 0.55 + PLAYER_RADIUS
                    || (pos.z - (cell.z as f32 + 0.5)).abs() > 0.55 + PLAYER_RADIUS
                {
                    continue;
                }
                let top = match def(block.part).shape {
                    Shape::Wedge => {
                        let lx = (pos.x - cell.x as f32).clamp(0.0, 1.0);
                        let lz = (pos.z - cell.z as f32).clamp(0.0, 1.0);
                        let t = match block.rot % 4 {
                            0 => lz,
                            1 => lx,
                            2 => 1.0 - lz,
                            _ => 1.0 - lx,
                        };
                        cell.y as f32 + t
                    }
                    Shape::Plate => cell.y as f32 + 0.25,
                    Shape::Frame => cell.y as f32 + 1.0,
                    _ => cell.y as f32 + 1.0,
                };
                if top <= pos.y + 0.6 && top > best {
                    best = top;
                }
            }
        }
    }
    // dynamic creations: allow standing on them
    for c in world.creations.iter() {
        if !c.dynamic {
            continue;
        }
        if (c.body.pos - pos).length() > 40.0 {
            continue;
        }
        for (cell, _) in c.blocks.iter() {
            let wp = physics::block_world_pos(c, *cell);
            if (wp.x - pos.x).abs() > 0.9 || (wp.z - pos.z).abs() > 0.9 {
                continue;
            }
            let top = wp.y + 0.5;
            if top <= pos.y + 0.6 && top > best {
                best = top;
            }
        }
    }
    best
}

/// What the player is currently aiming at.
pub fn aim_target(world: &BlockWorld, origin: Vec3, dir: Vec3) -> Option<crate::world::RayHit> {
    raycast(world, origin, dir, 8.0)
}

pub fn hit_is_block(hit: &crate::world::RayHit) -> bool {
    !matches!(hit.kind, HitKind::Terrain)
}

/// Player physics + input. Returns nothing, mutates the player and the world.
pub fn step_player(
    player: &mut Player,
    world: &BlockWorld,
    dt: f32,
    wish: Vec3,
    jump: bool,
    sprint: bool,
    crouch: bool,
) {
    if player.riding.is_some() {
        return;
    }

    if player.fly {
        let speed = if sprint { FLY_SPEED * 2.4 } else { FLY_SPEED };
        let mut v = wish * speed;
        if jump {
            v.y += speed;
        }
        if crouch {
            v.y -= speed;
        }
        player.vel = v;
        let mut pos = player.cam_pos;
        move_axis(world, &mut pos, 0, v.x * dt);
        move_axis(world, &mut pos, 1, v.y * dt);
        move_axis(world, &mut pos, 2, v.z * dt);
        player.cam_pos = pos;
        player.on_ground = false;
        return;
    }

    let speed = if sprint { SPRINT_SPEED } else { WALK_SPEED };
    let wish = if wish.length_squared() > 1.0 {
        wish.normalize()
    } else {
        wish
    };

    // horizontal acceleration with different ground/air control
    let accel = if player.on_ground { 60.0 } else { 14.0 };
    let target = wish * speed;
    let delta = target - Vec3::new(player.vel.x, 0.0, player.vel.z);
    let max_delta = accel * dt;
    if delta.length() > max_delta {
        let d = delta.normalize() * max_delta;
        player.vel.x += d.x;
        player.vel.z += d.z;
    } else {
        player.vel.x = target.x;
        player.vel.z = target.z;
    }
    if wish.length_squared() < 0.01 && player.on_ground {
        let friction = (1.0 - 12.0 * dt).max(0.0);
        player.vel.x *= friction;
        player.vel.z *= friction;
    }

    player.vel.y += GRAVITY * dt;

    let mut pos = player.cam_pos;
    move_axis(world, &mut pos, 0, player.vel.x * dt);
    move_axis(world, &mut pos, 2, player.vel.z * dt);
    let hit_y = move_axis(world, &mut pos, 1, player.vel.y * dt);

    let ground = ground_height(world, pos);
    if hit_y && player.vel.y <= 0.0 {
        player.vel.y = 0.0;
        player.on_ground = true;
    } else if pos.y <= ground {
        pos.y = ground;
        player.vel.y = 0.0;
        player.on_ground = true;
    } else {
        player.on_ground = false;
    }

    if jump && player.on_ground {
        player.vel.y = (-GRAVITY * 1.35).sqrt() * 1.35;
        player.on_ground = false;
    }

    // drowning guard: never fall out of the world
    if pos.y < -100.0 {
        pos.y = terrain::height(pos.x, pos.z, world.seed) + 3.0;
        player.vel = Vec3::ZERO;
    }
    player.cam_pos = pos;
}
