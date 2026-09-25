//! Logic network: switches, buttons, sensors, gates, timers, counters.
//!
//! Scrap Mechanic logic is boolean-only and notoriously painful to automate
//! with. ScrapForge keeps the simple boolean model (it is fun and readable) but
//! adds the blocks people keep asking for on the subreddit: a proper timer, a
//! counter, a memory cell and a configurable sensor cone.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::blocks::{def, forward, Behavior, Block, GateKind};
use crate::physics;
use crate::world::{BlockWorld, Creation};

/// Logic tick period (seconds).
pub const TICK: f32 = 0.08;
const SENSOR_RANGE: f32 = 7.0;
const SENSOR_HALF_WIDTH: f32 = 1.6;

#[derive(Resource, Default)]
pub struct LogicClock {
    pub acc: f32,
}

/// Runs one logic tick over every creation that contains logic parts.
/// Returns true when something changed (so meshes can be refreshed).
pub fn tick_logic(world: &mut BlockWorld, player_pos: Vec3) -> bool {
    let mut changed = false;
    let dynamic_positions: Vec<Vec3> = world
        .creations
        .iter()
        .filter(|c| c.dynamic)
        .map(|c| c.body.pos)
        .collect();

    for idx in 0..world.creations.len() {
        let has_logic = world.creations[idx]
            .blocks
            .iter()
            .any(|(_, b)| is_network_part(b.part));
        if !has_logic {
            continue;
        }

        let c = &world.creations[idx];
        let prev: HashMap<IVec3, bool> = c.blocks.iter().map(|(cell, b)| (*cell, b.powered())).collect();
        let links = c.links.clone();
        let base = if c.dynamic {
            c.body.pos - c.body.rot * c.body.com_local
        } else {
            Vec3::ZERO
        };
        let rot = c.body.rot;

        // incoming links per target cell
        let mut incoming: HashMap<IVec3, Vec<IVec3>> = HashMap::new();
        for (a, b) in links.iter() {
            incoming.entry(*b).or_default().push(*a);
            // links are undirected for evaluation purposes: a gate feeding a
            // lamp still shows the correct colour on the wire
            incoming.entry(*a).or_default().push(*b);
        }

        let mut next: HashMap<IVec3, bool> = HashMap::new();
        let mut new_blocks: Vec<(IVec3, Block)> = Vec::new();

        for (cell, block) in c.blocks.iter() {
            let d = def(block.part);
            let mut b = *block;
            match d.behavior {
                Behavior::Switch => {
                    // state is driven by the player, nothing to compute
                    next.insert(*cell, block.powered());
                }
                Behavior::Button => {
                    if block.powered() {
                        let countdown = block.data().saturating_sub(1);
                        if countdown == 0 {
                            b.set_powered(false);
                        }
                        b.set_data(countdown);
                    }
                    next.insert(*cell, b.powered());
                }
                Behavior::Sensor => {
                    let world_pos = if c.dynamic {
                        physics::block_world_pos(c, *cell)
                    } else {
                        base + cell.as_vec3() + Vec3::splat(0.5)
                    };
                    let fwd = rot * forward(block.rot);
                    let mut active = sensor_hits(world_pos, fwd, player_pos);
                    if !active {
                        for p in dynamic_positions.iter() {
                            if sensor_hits(world_pos, fwd, *p) {
                                active = true;
                                break;
                            }
                        }
                    }
                    b.set_powered(active);
                    next.insert(*cell, active);
                }
                Behavior::Gate(kind) => {
                    let inputs: Vec<bool> = incoming
                        .get(cell)
                        .map(|srcs| {
                            srcs.iter()
                                .filter_map(|s| prev.get(s).copied())
                                .collect()
                        })
                        .unwrap_or_default();
                    let out = match kind {
                        GateKind::And => !inputs.is_empty() && inputs.iter().all(|v| *v),
                        GateKind::Or => inputs.iter().any(|v| *v),
                        GateKind::Xor => inputs.iter().filter(|v| **v).count() % 2 == 1,
                        GateKind::Not => !inputs.first().copied().unwrap_or(false),
                        GateKind::Timer => {
                            let count = block.data().wrapping_add(1) % 16;
                            b.set_data(count);
                            count < 8
                        }
                        GateKind::Counter => {
                            let rising = inputs.iter().any(|v| *v) && !block.latched();
                            b.set_latched(inputs.iter().any(|v| *v));
                            let mut count = block.data();
                            if rising {
                                count = count.saturating_add(1);
                            }
                            if count >= 8 {
                                count = 0;
                                b.set_data(0);
                                true
                            } else {
                                b.set_data(count);
                                false
                            }
                        }
                        GateKind::Memory => {
                            let rising = inputs.iter().any(|v| *v) && !block.latched();
                            b.set_latched(inputs.iter().any(|v| *v));
                            if rising {
                                !block.powered()
                            } else {
                                block.powered()
                            }
                        }
                    };
                    b.set_powered(out);
                    next.insert(*cell, out);
                }
                _ => {
                    // actuator / plain part: powered comes from incoming links
                    if let Some(srcs) = incoming.get(cell) {
                        let any = srcs.iter().filter_map(|s| prev.get(s).copied()).any(|v| v);
                        b.set_powered(any);
                        next.insert(*cell, any);
                    }
                }
            }
            if b.powered() != block.powered() || b.data() != block.data() {
                changed = true;
            }
            new_blocks.push((*cell, b));
        }

        let c = &mut world.creations[idx];
        for (new_cell, new_block) in new_blocks {
            if let Some(existing) = c.blocks.iter_mut().find(|(cell, _)| *cell == new_cell) {
                existing.1 = new_block;
            }
        }
        if changed {
            c.mesh_dirty = true;
        }
    }

    if changed {
        world.links_dirty = true;
    }
    changed
}

fn is_network_part(part: u16) -> bool {
    matches!(
        def(part).behavior,
        Behavior::Switch
            | Behavior::Button
            | Behavior::Sensor
            | Behavior::Gate(_)
            | Behavior::Light
            | Behavior::Engine { .. }
            | Behavior::Thruster { .. }
            | Behavior::Propeller { .. }
            | Behavior::Piston { .. }
            | Behavior::Motor { .. }
            | Behavior::Spudgun { .. }
    )
}

fn sensor_hits(origin: Vec3, fwd: Vec3, point: Vec3) -> bool {
    let fwd = fwd.normalize_or_zero();
    if fwd.length_squared() < 0.5 {
        return false;
    }
    let mut right = fwd.cross(Vec3::Y);
    if right.length_squared() < 1e-4 {
        right = Vec3::X;
    }
    let right = right.normalize();
    let up = right.cross(fwd).normalize();
    let d = point - origin;
    let along = d.dot(fwd);
    let side = d.dot(right);
    let vert = d.dot(up);
    along > -0.5 && along < SENSOR_RANGE && side.abs() < SENSOR_HALF_WIDTH && vert.abs() < SENSOR_HALF_WIDTH
}

/// Called when the player presses a button: keeps it on for a few ticks.
pub fn press_button(block: &mut Block) {
    block.set_powered(true);
    block.set_data(4);
}

/// Colours used for the little beams between linked blocks.
pub fn link_color(powered: bool) -> [f32; 3] {
    if powered {
        [0.25, 1.0, 0.35]
    } else {
        [0.35, 0.35, 0.45]
    }
}

#[allow(dead_code)]
pub fn describe_network(c: &Creation) -> String {
    let gates = c
        .blocks
        .iter()
        .filter(|(_, b)| matches!(def(b.part).behavior, Behavior::Gate(_)))
        .count();
    let links = c.links.len();
    format!("{} gates, {} links", gates, links)
}
