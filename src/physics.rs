//! Custom arcade rigid-body physics for player-built creations.
//!
//! Scrap Mechanic uses Bullet, which is where most of the "my car jiggles
//! itself apart" complaints on r/ScrapMechanic come from. ScrapForge uses a
//! small, very damped solver: suspension raycasts for wheels, penalty contacts
//! sampled from the block grid, and a box-approximated inertia tensor. It is
//! not a general purpose engine - it is tuned so that contraptions behave.

use bevy::prelude::*;

use crate::blocks::{def, forward, Behavior, Block};
use crate::terrain::{self, WATER_LEVEL};
use crate::world::{raycast, BlockWorld, Creation, RayHit};

pub const GRAVITY: f32 = -18.0;
/// Contact samples per body (big builds are sub-sampled, which keeps it cheap).
const MAX_CONTACT_SAMPLES: usize = 192;
const CONTACT_RADIUS: f32 = 0.42;

#[derive(Clone, Copy, Debug, Default)]
pub struct Controls {
    /// -1 .. 1, forward/backward
    pub throttle: f32,
    /// -1 .. 1, left/right
    pub steer: f32,
    /// -1 .. 1, up/down (thrusters, balloons, aircraft pitch)
    pub lift: f32,
    /// -1 .. 1, pitch
    pub pitch: f32,
    /// -1 .. 1, yaw
    pub yaw: f32,
    pub brake: bool,
    pub boost: bool,
    /// True while a player sits in a seat of this creation.
    pub driven: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Body {
    /// World position of the centre of mass.
    pub pos: Vec3,
    pub rot: Quat,
    pub vel: Vec3,
    pub ang_vel: Vec3,
    pub mass: f32,
    /// Diagonal inertia in body space.
    pub inertia: Vec3,
    /// Centre of mass expressed in "anchor local" block space.
    pub com_local: Vec3,
    pub grounded: bool,
    /// 0..1 how submerged the body is.
    pub wet: f32,
}

impl Default for Body {
    fn default() -> Self {
        Self {
            pos: Vec3::ZERO,
            rot: Quat::IDENTITY,
            vel: Vec3::ZERO,
            ang_vel: Vec3::ZERO,
            mass: 1.0,
            inertia: Vec3::ONE,
            com_local: Vec3::new(0.5, 0.5, 0.5),
            grounded: false,
            wet: 0.0,
        }
    }
}

/// Velocity of a point on a rigid body.
#[inline]
pub fn point_velocity(vel: Vec3, ang_vel: Vec3, r: Vec3) -> Vec3 {
    vel + ang_vel.cross(r)
}

/// Runs `substeps` physics steps of size `dt` for one dynamic creation.
pub fn step_creation(c: &mut Creation, world: &BlockWorld, dt: f32, substeps: u32) {
    let substeps = substeps.max(1);
    let h = dt / substeps as f32;
    for _ in 0..substeps {
        integrate(c, world, h);
    }
}

fn integrate(c: &mut Creation, world: &BlockWorld, dt: f32) {
    let rot = c.body.rot;
    let com = c.body.pos;
    let mass = c.body.mass.max(2.0);
    let com_local = c.body.com_local;
    let controls = c.controls;

    let mut force = Vec3::new(0.0, GRAVITY * mass, 0.0);
    let mut torque = Vec3::ZERO;
    let mut grounded = false;

    let wheel_count = c
        .blocks
        .iter()
        .filter(|(_, b)| matches!(def(b.part).behavior, Behavior::Wheel { .. }))
        .count()
        .max(1) as f32;

    // ---------------------------------------------------------------- parts
    let blocks = std::mem::take(&mut c.blocks);
    for (local, block) in blocks.iter() {
        let r = local.as_vec3() + Vec3::splat(0.5) - com_local;
        let wp = com + rot * r;
        let d = def(block.part);
        let powered = block.powered();

        match d.behavior {
            Behavior::Wheel { radius, grip, power } => {
                let up = rot * Vec3::Y;
                let steer = controls.steer * 0.55;
                let fwd0 = rot * forward(block.rot);
                let fwd = Quat::from_axis_angle(up, steer) * fwd0;
                let right = fwd.cross(up).normalize_or_zero();
                let rest = 0.32;
                if let Some(hit) = raycast(world, wp, -up, rest + radius) {
                    let compression = (rest + radius - hit.dist).max(0.0);
                    let stiffness = mass * 160.0 / wheel_count;
                    let damping = 2.0 * (stiffness * mass / wheel_count).sqrt() * 0.45;
                    let cv = point_velocity(c.body.vel, c.body.ang_vel, wp - com);
                    let vn = cv.dot(up);
                    let f = up * (compression * stiffness - vn * damping).max(0.0);
                    force += f;
                    torque += (wp - com).cross(f);
                    grounded = true;

                    // lateral grip
                    let v_lat = cv.dot(right);
                    let grip_force = mass * 14.0 * grip / wheel_count;
                    let mut f_lat = -right * v_lat * grip_force;
                    f_lat = f_lat.clamp_length_max(mass * 30.0 / wheel_count);
                    force += f_lat;
                    torque += (wp - com).cross(f_lat);

                    // drive / brake
                    if controls.driven {
                        let boost = if controls.boost { 1.8 } else { 1.0 };
                        let f_drive =
                            fwd * controls.throttle * power * mass * 7.5 * boost / wheel_count;
                        force += f_drive;
                        torque += (wp - com).cross(f_drive);
                    }

                    // rolling resistance
                    let v_fwd = cv.dot(fwd);
                    let roll_k = if controls.brake { 8.0 } else { 0.7 };
                    let f_roll = -fwd * v_fwd * mass * roll_k / wheel_count;
                    force += f_roll;
                    torque += (wp - com).cross(f_roll);
                }
            }
            Behavior::Engine { force: f0 } => {
                // an engine is driven by the throttle when someone sits in a
                // seat, and runs at full power when it is switched by logic
                let on = powered || controls.driven;
                if on {
                    let scale = if controls.driven {
                        controls.throttle
                    } else {
                        1.0
                    };
                    let f = (rot * forward(block.rot)) * f0 * scale;
                    force += f;
                    torque += (wp - com).cross(f);
                }
            }
            Behavior::Thruster { force: f0 } => {
                if powered {
                    let f = (rot * forward(block.rot)) * f0;
                    force += f;
                    torque += (wp - com).cross(f);
                }
            }
            Behavior::Propeller { force: f0 } => {
                if powered {
                    let dir = (rot * forward(block.rot)).normalize_or_zero();
                    let speed = c.body.vel.dot(dir);
                    let fade = (1.0 - speed / 55.0).clamp(0.0, 1.0);
                    let f = dir * f0 * fade;
                    force += f;
                    torque += (wp - com).cross(f);
                }
            }
            Behavior::Piston { force: f0 } => {
                if powered {
                    let f = (rot * forward(block.rot)) * f0;
                    force += f;
                    torque += (wp - com).cross(f);
                }
            }
            Behavior::Motor { torque: t0 } => {
                if powered {
                    torque += (rot * forward(block.rot)) * t0;
                }
            }
            Behavior::Balloon { lift } => {
                let f = Vec3::Y * (mass * -GRAVITY * lift);
                force += f;
                torque += (wp - com).cross(f);
            }
            Behavior::Buoyancy { lift } => {
                let depth = (WATER_LEVEL - wp.y).max(0.0).min(1.2);
                if depth > 0.0 {
                    let f = Vec3::Y * (mass * -GRAVITY * lift * depth);
                    force += f;
                    torque += (wp - com).cross(f);
                }
            }
            Behavior::Gyro { strength } => {
                let up = rot * Vec3::Y;
                let align = up.cross(Vec3::Y);
                torque += align * mass * 22.0 * strength;
                torque -= c.body.ang_vel * mass * 3.5 * strength;
            }
            _ => {}
        }
    }
    c.blocks = blocks;

    // ------------------------------------------------------------- contacts
    let blocks = std::mem::take(&mut c.blocks);
    let len = blocks.len().max(1);
    let stride = (len / MAX_CONTACT_SAMPLES).max(1);
    let samples = (len / stride).max(1) as f32;
    let mut submerged = 0;
    for (i, (local, _block)) in blocks.iter().enumerate() {
        if i % stride != 0 {
            continue;
        }
        let r = local.as_vec3() + Vec3::splat(0.5) - com_local;
        let wp = com + rot * r;
        if wp.y < WATER_LEVEL {
            submerged += 1;
        }

        // terrain
        let h = terrain::height(wp.x, wp.z, world.seed);
        let pen = h + CONTACT_RADIUS - wp.y;
        if pen > 0.0 {
            apply_contact(
                &mut force,
                &mut torque,
                &mut grounded,
                com,
                c.body.vel,
                c.body.ang_vel,
                wp,
                Vec3::Y,
                pen,
                mass,
                samples,
            );
        }

        // static blocks (sphere vs voxel)
        let cell = world_to_cell(wp);
        if world.blocks.contains_key(&cell) {
            let center = cell.as_vec3() + Vec3::splat(0.5);
            let dv = wp - center;
            let half = 0.5 + CONTACT_RADIUS;
            let px = half - dv.x.abs();
            let py = half - dv.y.abs();
            let pz = half - dv.z.abs();
            let (pen, normal) = if px <= py && px <= pz {
                (px, Vec3::X * dv.x.signum())
            } else if py <= pz {
                (py, Vec3::Y * dv.y.signum())
            } else {
                (pz, Vec3::Z * dv.z.signum())
            };
            if pen > 0.0 {
                apply_contact(
                    &mut force,
                    &mut torque,
                    &mut grounded,
                    com,
                    c.body.vel,
                    c.body.ang_vel,
                    wp,
                    normal,
                    pen,
                    mass,
                    samples,
                );
            }
        }
    }
    c.blocks = blocks;
    c.body.wet = submerged as f32 / samples;

    // ------------------------------------------------------------- integrate
    if !force.is_finite() {
        force = Vec3::ZERO;
    }
    if !torque.is_finite() {
        torque = Vec3::ZERO;
    }

    c.body.vel += force / mass * dt;
    // water drag
    if c.body.wet > 0.0 {
        let drag = 1.0 - (1.6 * c.body.wet * dt).min(0.9);
        c.body.vel *= drag;
        c.body.ang_vel *= drag;
    }
    let lin_damp = if grounded { 0.35 } else { 0.12 };
    let ang_damp = if grounded { 1.2 } else { 0.35 };
    c.body.vel *= (1.0 - lin_damp * dt).max(0.0);
    c.body.pos += c.body.vel * dt;

    let inv = Vec3::new(
        1.0 / c.body.inertia.x.max(1.0),
        1.0 / c.body.inertia.y.max(1.0),
        1.0 / c.body.inertia.z.max(1.0),
    );
    let t_local = rot.inverse() * torque;
    let acc = rot * Vec3::new(t_local.x * inv.x, t_local.y * inv.y, t_local.z * inv.z);
    c.body.ang_vel += acc * dt;
    c.body.ang_vel *= (1.0 - ang_damp * dt).max(0.0);
    c.body.ang_vel = c.body.ang_vel.clamp_length_max(14.0);

    let wq = Quat::from_xyzw(c.body.ang_vel.x, c.body.ang_vel.y, c.body.ang_vel.z, 0.0);
    c.body.rot = (c.body.rot + wq * c.body.rot * (0.5 * dt)).normalize();

    // world bounds: nothing ever escapes
    if c.body.pos.y < -120.0 {
        c.body.pos.y = -120.0;
        c.body.vel.y = c.body.vel.y.max(0.0);
    }
    if c.body.pos.y > 1500.0 {
        c.body.pos.y = 1500.0;
        c.body.vel.y = c.body.vel.y.min(0.0);
    }
    c.body.vel = c.body.vel.clamp_length_max(400.0);
    c.body.grounded = grounded;

    if !c.body.pos.is_finite() {
        c.body.pos = Vec3::new(0.0, terrain::height(0.0, 0.0, world.seed) + 20.0, 0.0);
        c.body.vel = Vec3::ZERO;
        c.body.ang_vel = Vec3::ZERO;
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_contact(
    force: &mut Vec3,
    torque: &mut Vec3,
    grounded: &mut bool,
    com: Vec3,
    vel: Vec3,
    ang_vel: Vec3,
    point: Vec3,
    normal: Vec3,
    depth: f32,
    mass: f32,
    samples: f32,
) {
    let r = point - com;
    let cv = point_velocity(vel, ang_vel, r);
    let vn = cv.dot(normal);
    let k = mass * 220.0 / samples;
    let c = 2.0 * (k * mass / samples).sqrt() * 0.55;
    let mut f = normal * (depth.min(0.6) * k - vn * c).max(0.0);

    // Coulomb-ish friction
    let v_tan = cv - normal * vn;
    let f_max = f.length() * 0.9;
    let f_fric = -v_tan * (mass * 6.0 / samples);
    let f_fric = f_fric.clamp_length_max(f_max);
    f += f_fric;

    *force += f;
    *torque += r.cross(f);
    if normal.y > 0.5 {
        *grounded = true;
    }
}

/// World position -> grid cell.
#[inline]
pub fn world_to_cell(p: Vec3) -> IVec3 {
    IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)
}

/// Rebuilds mass / centre of mass / inertia from the block list.
pub fn recompute_mass_properties(c: &mut Creation) {
    let mut total = 0.0;
    let mut com = Vec3::ZERO;
    for (cell, block) in c.blocks.iter() {
        let m = def(block.part).mass.max(0.5);
        total += m;
        com += (cell.as_vec3() + Vec3::splat(0.5)) * m;
    }
    if total <= 0.0 {
        total = 1.0;
    }
    let com = com / total;
    c.body.mass = total;
    c.body.com_local = com;

    let mut min = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
    let mut max = Vec3::new(f32::MIN, f32::MIN, f32::MIN);
    for (cell, _) in c.blocks.iter() {
        let p = cell.as_vec3();
        min = min.min(p);
        max = max.max(p + Vec3::ONE);
    }
    let s = (max - min).max(Vec3::ONE);
    let m = total;
    c.body.inertia = Vec3::new(
        (m / 12.0) * (s.y * s.y + s.z * s.z),
        (m / 12.0) * (s.x * s.x + s.z * s.z),
        (m / 12.0) * (s.x * s.x + s.y * s.y),
    )
    .max(Vec3::splat(1.0));
}

/// Convenience: world space centre of a block of a (possibly dynamic) creation.
pub fn block_world_pos(c: &Creation, local: IVec3) -> Vec3 {
    c.body.pos + c.body.rot * (local.as_vec3() + Vec3::splat(0.5) - c.body.com_local)
}

/// Finds the first seat of a creation, returning its block position.
pub fn first_seat(c: &Creation) -> Option<IVec3> {
    c.blocks
        .iter()
        .find(|(_, b)| matches!(def(b.part).behavior, Behavior::Seat))
        .map(|(cell, _)| *cell)
}

/// Ray-cast helper used by wheels and the spud-gun.
pub fn cast_ray(world: &BlockWorld, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<RayHit> {
    raycast(world, origin, dir, max_dist)
}

/// True if a block is a "device" that reacts to logic signals.
pub fn is_actuator(block: &Block) -> bool {
    matches!(
        def(block.part).behavior,
        Behavior::Engine { .. }
            | Behavior::Thruster { .. }
            | Behavior::Propeller { .. }
            | Behavior::Piston { .. }
            | Behavior::Motor { .. }
            | Behavior::Light
            | Behavior::Spudgun { .. }
    )
}
