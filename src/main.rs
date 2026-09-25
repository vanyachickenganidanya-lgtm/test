//! ScrapForge - a tiny open source building / contraption sandbox.
//!
//! Everything (terrain, blocks, machines, UI) is generated at runtime: the game
//! ships as a single binary with no asset files at all.

mod blocks;
mod build;
mod logic;
mod meshgen;
mod noise;
mod physics;
mod player;
mod save;
mod terrain;
mod ui;
mod world;

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use bevy::window::{CursorGrabMode, PrimaryWindow, WindowPlugin};

use blocks::{def, Behavior, Block};
use build::{Editor, Projectile, Tool};
use logic::LogicClock;
use meshgen::build_block_mesh;
use physics::Controls;
use player::{Player, PlayerCamera, PlayerSettings};
use ui::{UiAction, UiEvents, UiState};
use world::{BlockWorld, HitKind};

// ---------------------------------------------------------------------------

#[derive(Resource)]
struct Materials {
    solid: Handle<StandardMaterial>,
    terrain: Handle<StandardMaterial>,
    water: Handle<StandardMaterial>,
    ghost: Handle<StandardMaterial>,
    ghost_mesh: Handle<Mesh>,
    link: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
struct TerrainChunks {
    map: HashMap<IVec2, Entity>,
    center: Option<IVec2>,
    distance: i32,
}

#[derive(Resource, Default)]
struct LinkViews {
    entities: Vec<Entity>,
}

#[derive(Resource, Default)]
struct CursorLocked {
    locked: bool,
}

#[derive(Component)]
struct Sun;

#[derive(Component)]
struct Water;

#[derive(Component)]
struct Ghost;

#[derive(Component)]
struct CreationView {
    #[allow(dead_code)]
    id: u32,
}

#[derive(Component)]
struct TerrainChunk;

#[derive(Component)]
struct LinkBeam {
    creation: Option<u32>,
    a: Vec3,
    b: Vec3,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "ScrapForge - open source contraption sandbox".to_string(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.52, 0.70, 0.92)))
        .insert_resource(AmbientLight {
            color: Color::srgb(0.62, 0.72, 0.9),
            brightness: 420.0,
            ..default()
        })
        .insert_resource(BlockWorld::new(20260925))
        .insert_resource(Editor::default())
        .insert_resource(PlayerSettings::default())
        .insert_resource(UiState::default())
        .insert_resource(UiEvents::default())
        .insert_resource(LogicClock::default())
        .insert_resource(TerrainChunks::default())
        .insert_resource(LinkViews::default())
        .insert_resource(CursorLocked::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                cursor_system,
                input_system,
                motion_system,
                physics_system,
                logic_system,
                projectile_system,
                mesh_update_system,
                terrain_stream_system,
                link_visual_system,
                ghost_system,
                follow_system,
                camera_system,
                ui::button_system,
                ui_action_system,
                ui::update_hud,
                ui::update_panels,
            )
                .chain(),
        )
        .run();
}

// ---------------------------------------------------------------------------
// Startup
// ---------------------------------------------------------------------------

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut world: ResMut<BlockWorld>,
) {
    let solid = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.72,
        metallic: 0.08,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    let terrain_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.96,
        metallic: 0.0,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    let water_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.13, 0.38, 0.55, 0.72),
        perceptual_roughness: 0.06,
        metallic: 0.25,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    let ghost_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 0.85, 0.25, 0.35),
        emissive: Color::srgb(0.5, 0.42, 0.1).into(),
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    let link_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 1.0, 0.4),
        emissive: Color::srgb(0.1, 0.7, 0.2).into(),
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    let ghost_mesh = meshes.add(meshgen::build_highlight_mesh());

    commands.insert_resource(Materials {
        solid: solid.clone(),
        terrain: terrain_mat.clone(),
        water: water_mat.clone(),
        ghost: ghost_mat.clone(),
        ghost_mesh: ghost_mesh.clone(),
        link: link_mat.clone(),
    });

    // ---- world ------------------------------------------------------------
    starter_base(&mut world);
    world.rebuild_creations();

    // ---- sun --------------------------------------------------------------
    commands.spawn((
        Sun,
        DirectionalLight {
            color: Color::srgb(1.0, 0.96, 0.88),
            illuminance: 14_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_translation(Vec3::new(60.0, 120.0, 40.0)),
    ));

    // ---- water ------------------------------------------------------------
    commands.spawn((
        Water,
        Mesh3d(meshes.add(meshgen::build_water_mesh(6000.0))),
        MeshMaterial3d(water_mat),
        Transform::from_translation(Vec3::new(0.0, terrain::WATER_LEVEL, 0.0)),
    ));

    // ---- player -----------------------------------------------------------
    let spawn = spawn_point(&world);
    commands.spawn((
        Player {
            cam_pos: spawn,
            ..default()
        },
        Transform::from_translation(spawn),
    ));

    // ---- camera -----------------------------------------------------------
    commands.spawn((
        PlayerCamera,
        Camera3d::default(),
        Camera::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 75.0_f32.to_radians(),
            near: 0.08,
            far: 3000.0,
            ..default()
        }),
        Msaa::Sample4,
        DistanceFog {
            color: Color::srgb(0.62, 0.74, 0.88),
            falloff: FogFalloff::Linear {
                start: 150.0,
                end: 600.0,
            },
            ..default()
        },
        Transform::from_translation(spawn),
    ));

    // ---- placement ghost --------------------------------------------------
    commands.spawn((
        Ghost,
        Mesh3d(ghost_mesh),
        MeshMaterial3d(ghost_mat),
        Transform::default(),
        Visibility::Hidden,
    ));

    // ---- ui ---------------------------------------------------------------
    let hud = ui::spawn_ui(&mut commands);
    commands.insert_resource(hud);
}

fn spawn_point(world: &BlockWorld) -> Vec3 {
    let y = terrain::height(0.0, 0.0, world.seed).max(terrain::WATER_LEVEL + 0.5) + 3.0;
    Vec3::new(0.5, y, 0.5)
}

/// A small starter platform so the player can build immediately.
fn starter_base(world: &mut BlockWorld) {
    let base_y = (terrain::height(0.0, 0.0, world.seed).max(terrain::WATER_LEVEL + 0.4) + 0.6).floor() as i32;
    for x in -6..=6 {
        for z in -6..=6 {
            let mut block = Block::new(blocks::P_BLOCK);
            block.color = if (x + z) % 2 == 0 { 0 } else { 11 };
            world.set_block(IVec3::new(x, base_y, z), block);
        }
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn cursor_system(
    ui: Res<UiState>,
    editor: Res<Editor>,
    mut locked: ResMut<CursorLocked>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let want_lock = !ui.pause && !editor.menu_open;
    if want_lock == locked.locked {
        return;
    }
    locked.locked = want_lock;
    for mut window in windows.iter_mut() {
        window.cursor_options.visible = !want_lock;
        window.cursor_options.grab_mode = if want_lock {
            CursorGrabMode::Locked
        } else {
            CursorGrabMode::None
        };
    }
}

#[allow(clippy::too_many_arguments)]
fn input_system(
    mut commands: Commands,
    mut world: ResMut<BlockWorld>,
    mut editor: ResMut<Editor>,
    mut ui_state: ResMut<UiState>,
    mut events: ResMut<UiEvents>,
    mut player_query: Query<&mut Player>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut motion: EventReader<bevy::input::mouse::MouseMotion>,
    mut wheel: EventReader<bevy::input::mouse::MouseWheel>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    settings: Res<PlayerSettings>,
    time: Res<Time>,
) {
    editor.message_timer = (editor.message_timer - time.delta_secs()).max(0.0);

    let Ok(mut player) = player_query.get_single_mut() else {
        return;
    };

    // ------------------------------------------------------------ escape etc
    if keys.just_pressed(KeyCode::Escape) {
        if ui_state.pause {
            ui_state.pause = false;
        } else if editor.menu_open {
            editor.menu_open = false;
        } else {
            ui_state.pause = true;
        }
        editor.connect_from = None;
    }
    if ui_state.pause {
        return;
    }

    // ------------------------------------------------------------ mouse look
    if !editor.menu_open {
        let mut delta = Vec2::ZERO;
        for ev in motion.read() {
            delta += ev.delta;
        }
        player.yaw -= delta.x * settings.sensitivity;
        player.pitch -= delta.y * settings.sensitivity;
        player.pitch = player.pitch.clamp(-1.55, 1.55);
    }

    // ---------------------------------------------------------------- wheels
    for ev in wheel.read() {
        let dir = if ev.y > 0.0 { -1 } else { 1 };
        if editor.menu_open {
            let count = build::parts_in_category(blocks::Category::ALL[editor.menu_category]).len() as i32;
            let max_page = (count / 10).max(0);
            let page = editor.menu_scroll as i32 + dir;
            editor.menu_scroll = page.clamp(0, max_page) as usize;
        } else {
            let len = editor.hotbar.len() as i32;
            let next = (editor.slot as i32 + dir).rem_euclid(len);
            editor.slot = next as usize;
        }
    }

    // ------------------------------------------------------------ menu / keys
    if keys.just_pressed(KeyCode::KeyB) {
        editor.menu_open = !editor.menu_open;
    }
    if editor.menu_open {
        if keys.just_pressed(KeyCode::Tab) {
            editor.menu_category = (editor.menu_category + 1) % blocks::Category::ALL.len();
            editor.menu_scroll = 0;
        }
        for i in 0..10u32 {
            if keys.just_pressed(digit_key(i)) {
                let parts = build::parts_in_category(blocks::Category::ALL[editor.menu_category]);
                let index = editor.menu_scroll * 10 + i as usize;
                if let Some(part) = parts.get(index).copied() {
                    editor.hotbar[editor.slot] = part;
                    editor.say(&format!("Selected {}", def(part).name));
                }
            }
        }
        return;
    }

    // ------------------------------------------------------------ hotbar keys
    for i in 0..10u32 {
        if keys.just_pressed(digit_key(i)) {
            editor.slot = i as usize;
        }
    }
    if keys.just_pressed(KeyCode::KeyR) {
        if editor.lifted.is_some() {
            build::rotate_lifted(&mut world, &editor);
        } else if keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight) {
            editor.tool = match editor.tool {
                Tool::Build => Tool::Connect,
                Tool::Connect => Tool::Paint,
                Tool::Paint => Tool::Remove,
                Tool::Remove => Tool::Build,
            };
            editor.say(editor.tool.name());
        } else {
            editor.rot = (editor.rot + 1) % 4;
        }
    }
    if keys.just_pressed(KeyCode::KeyF) && player.riding.is_none() {
        player.fly = !player.fly;
        editor.say(if player.fly { "Fly ON" } else { "Fly OFF" });
    }
    if keys.just_pressed(KeyCode::BracketRight) {
        editor.color = (editor.color + 1) % blocks::PALETTE.len() as u8;
    }
    if keys.just_pressed(KeyCode::BracketLeft) {
        editor.color = (editor.color + blocks::PALETTE.len() as u8 - 1) % blocks::PALETTE.len() as u8;
    }

    // ------------------------------------------------------------------ undo
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    if ctrl && keys.just_pressed(KeyCode::KeyZ) {
        build::undo(&mut world, &mut editor);
    }
    if ctrl && keys.just_pressed(KeyCode::KeyY) {
        build::redo(&mut world, &mut editor);
    }
    if ctrl && keys.just_pressed(KeyCode::KeyC) {
        if let Some(hit) = current_hit(&world, &player) {
            build::copy_creation(&world, &mut editor, &hit);
        }
    }
    if ctrl && keys.just_pressed(KeyCode::KeyV) {
        if let Some(hit) = current_hit(&world, &player) {
            build::paste_creation(&mut world, &mut editor, &hit);
        }
    }

    // ---------------------------------------------------------- save / load
    if keys.just_pressed(KeyCode::F5) {
        events.0.push(UiAction::Save);
    }
    if keys.just_pressed(KeyCode::F9) {
        events.0.push(UiAction::Load);
    }

    // ---------------------------------------------------------- interaction
    let eye = player::eye_position(&player, &world);
    let dir = player::look_direction(player.yaw, player.pitch);

    if keys.just_pressed(KeyCode::KeyE) {
        if let Some(hit) = current_hit(&world, &player) {
            if !try_enter_seat(&mut world, &mut editor, &mut player, &hit) {
                build::interact_block(&mut world, &mut editor, &hit);
            }
        }
    }
    if keys.just_pressed(KeyCode::KeyQ) && player.riding.is_some() {
        leave_seat(&mut world, &mut editor, &mut player);
    }
    if keys.just_pressed(KeyCode::KeyG) {
        build::toggle_lift(&mut world, &mut editor, &player, eye, dir);
    }
    if keys.just_pressed(KeyCode::KeyC) && !ctrl {
        editor.tool = if editor.tool == Tool::Connect {
            Tool::Build
        } else {
            Tool::Connect
        };
        editor.connect_from = None;
        editor.say(&format!("Tool: {}", editor.tool.name()));
    }

    // ------------------------------------------------------------ mouse uses
    if player.riding.is_some() {
        if mouse.just_pressed(MouseButton::Left) {
            let id = player.riding.unwrap();
            build::fire_spudgun(&world, id, &mut commands, &mut meshes, &mut materials);
        }
        return;
    }

    let Some(hit) = current_hit(&world, &player) else {
        return;
    };

    if mouse.just_pressed(MouseButton::Left) {
        match editor.tool {
            Tool::Build => {
                build::place_block(&mut world, &mut editor, &hit);
            }
            Tool::Remove => {
                build::remove_block(&mut world, &mut editor, &hit);
            }
            Tool::Paint => {
                build::paint_block(&mut world, &mut editor, &hit);
            }
            Tool::Connect => {
                build::connect_block(&mut world, &mut editor, &hit);
            }
        }
    }
    if mouse.just_pressed(MouseButton::Right) {
        build::remove_block(&mut world, &mut editor, &hit);
    }
    if mouse.just_pressed(MouseButton::Middle) {
        if let HitKind::StaticBlock { cell } = hit.kind {
            if let Some(block) = world.blocks.get(&cell).copied() {
                editor.hotbar[editor.slot] = block.part;
                editor.color = block.color;
                editor.say(&format!("Picked {}", def(block.part).name));
            }
        }
    }
}

fn digit_key(i: u32) -> KeyCode {
    match i {
        0 => KeyCode::Digit1,
        1 => KeyCode::Digit2,
        2 => KeyCode::Digit3,
        3 => KeyCode::Digit4,
        4 => KeyCode::Digit5,
        5 => KeyCode::Digit6,
        6 => KeyCode::Digit7,
        7 => KeyCode::Digit8,
        8 => KeyCode::Digit9,
        _ => KeyCode::Digit0,
    }
}

fn current_hit(world: &BlockWorld, player: &Player) -> Option<world::RayHit> {
    let eye = player::eye_position(player, world);
    let dir = player::look_direction(player.yaw, player.pitch);
    world::raycast(world, eye, dir, 8.0)
}

fn try_enter_seat(
    world: &mut BlockWorld,
    editor: &mut Editor,
    player: &mut Player,
    hit: &world::RayHit,
) -> bool {
    let (cell, creation_id) = match hit.kind {
        HitKind::StaticBlock { cell } => (cell, world.creation_of_cell(cell)),
        HitKind::DynamicBlock { creation, cell } => (cell, Some(creation)),
        HitKind::Terrain => return false,
    };
    let Some(id) = creation_id else { return false };

    let is_seat = world
        .creation_by_id(id)
        .and_then(|c| c.block_at(cell))
        .map(|b| matches!(def(b.part).behavior, Behavior::Seat))
        .unwrap_or(false);
    if !is_seat {
        return false;
    }

    if !world.creation_by_id(id).map(|c| c.dynamic).unwrap_or(false) {
        if !world.make_dynamic(id) {
            return false;
        }
    }
    let seat = world.creation_by_id(id).and_then(physics::first_seat);
    player.riding = Some(id);
    player.seat_cell = seat;
    player.fly = false;
    editor.say("Driving - WASD, Space brake, Shift boost, Q to get out");
    true
}

fn leave_seat(world: &mut BlockWorld, editor: &mut Editor, player: &mut Player) {
    let Some(id) = player.riding.take() else { return };
    if let Some(c) = world.creation_by_id(id) {
        let pos = match player.seat_cell {
            Some(cell) => physics::block_world_pos(c, cell),
            None => c.body.pos,
        };
        player.cam_pos = pos + Vec3::Y * 0.6;
    }
    if let Some(idx) = world.creations.iter().position(|c| c.id == id) {
        world.creations[idx].controls = Controls::default();
    }
    player.seat_cell = None;
    editor.say("Left the seat");
}

// ---------------------------------------------------------------------------

fn motion_system(
    mut world: ResMut<BlockWorld>,
    mut player_query: Query<&mut Player>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs().min(0.05);
    let Ok(mut player) = player_query.get_single_mut() else {
        return;
    };

    let forward = (keys.pressed(KeyCode::KeyW) as i32 - keys.pressed(KeyCode::KeyS) as i32) as f32;
    let strafe = (keys.pressed(KeyCode::KeyD) as i32 - keys.pressed(KeyCode::KeyA) as i32) as f32;
    let jump = keys.pressed(KeyCode::Space);
    let sprint = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let crouch = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);

    if let Some(id) = player.riding {
        let Some(idx) = world.creations.iter().position(|c| c.id == id) else {
            player.riding = None;
            return;
        };
        let wheels = world.creations[idx]
            .blocks
            .iter()
            .filter(|(_, b)| matches!(def(b.part).behavior, Behavior::Wheel { .. }))
            .count();
        let controls = Controls {
            throttle: forward,
            steer: strafe,
            lift: (jump as i32 - crouch as i32) as f32,
            brake: wheels > 0 && jump,
            boost: sprint,
            driven: true,
            ..default()
        };
        world.creations[idx].controls = controls;
        player.cam_pos = world.creations[idx].body.pos;
        return;
    }

    let yaw = player.yaw;
    let q = Quat::from_axis_angle(Vec3::Y, yaw);
    let fwd = q * Vec3::NEG_Z;
    let right = q * Vec3::X;
    let mut wish = Vec3::ZERO;
    wish += fwd * forward;
    wish += right * strafe;

    player::step_player(&mut player, &world, dt, wish, jump, sprint, crouch);
}

// ---------------------------------------------------------------------------

fn physics_system(time: Res<Time>, mut world: ResMut<BlockWorld>, editor: Res<Editor>) {
    let dt = time.delta_secs().min(0.05);
    let ids: Vec<u32> = world
        .creations
        .iter()
        .filter(|c| c.dynamic)
        .map(|c| c.id)
        .collect();

    for id in ids {
        if editor.lifted == Some(id) {
            continue;
        }
        let Some(idx) = world.creations.iter().position(|c| c.id == id) else {
            continue;
        };
        let mut creation = world.creations.remove(idx);
        physics::step_creation(&mut creation, &world, dt, 4);
        world.creations.insert(idx, creation);
    }
}

// ---------------------------------------------------------------------------

fn logic_system(
    mut world: ResMut<BlockWorld>,
    mut clock: ResMut<LogicClock>,
    time: Res<Time>,
    player_query: Query<&Player>,
) {
    clock.acc += time.delta_secs();
    if clock.acc < logic::TICK {
        return;
    }
    clock.acc = 0.0;
    let player_pos = player_query
        .iter()
        .next()
        .map(|p| p.cam_pos + Vec3::Y * 0.9)
        .unwrap_or(Vec3::ZERO);
    logic::tick_logic(&mut world, player_pos);
}

// ---------------------------------------------------------------------------

fn projectile_system(
    mut commands: Commands,
    mut projectiles: Query<(Entity, &mut Transform, &mut Projectile)>,
    mut world: ResMut<BlockWorld>,
    time: Res<Time>,
) {
    let dt = time.delta_secs().min(0.05);
    let mut impulses: Vec<(u32, Vec3, Vec3)> = Vec::new();
    let mut to_despawn: Vec<Entity> = Vec::new();

    for (entity, mut transform, mut projectile) in projectiles.iter_mut() {
        projectile.life -= dt;
        projectile.vel.y += physics::GRAVITY * dt;
        let next = transform.translation + projectile.vel * dt;
        transform.translation = next;
        if projectile.life <= 0.0 {
            to_despawn.push(entity);
            continue;
        }
        if next.y < terrain::height(next.x, next.z, world.seed) {
            to_despawn.push(entity);
            continue;
        }
        if world.blocks.contains_key(&physics::world_to_cell(next)) {
            to_despawn.push(entity);
            continue;
        }
        for c in world.creations.iter() {
            if !c.dynamic || (c.body.pos - next).length() > 60.0 {
                continue;
            }
            let local = c.body.rot.inverse() * (next - c.body.pos) + c.body.com_local;
            let cell = physics::world_to_cell(local);
            if c.block_set.contains(&cell) {
                impulses.push((c.id, next, projectile.vel * 0.3));
                to_despawn.push(entity);
                break;
            }
        }
    }

    for (id, point, impulse) in impulses {
        if let Some(idx) = world.creations.iter().position(|c| c.id == id) {
            let mut creation = world.creations.remove(idx);
            creation.body.vel += impulse;
            let arm = point - creation.body.pos;
            creation.body.ang_vel += arm.cross(impulse) / creation.body.inertia.x.max(1.0);
            world.creations.insert(idx, creation);
        }
    }

    for entity in to_despawn {
        commands.entity(entity).despawn();
    }
}

// ---------------------------------------------------------------------------

fn mesh_update_system(
    mut commands: Commands,
    mut world: ResMut<BlockWorld>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<Materials>,
    mut transforms: Query<&mut Transform>,
) {
    // ---- static sections ---------------------------------------------------
    let dirty_sections: Vec<IVec3> = world.dirty_sections.iter().copied().collect();
    world.dirty_sections.clear();

    for section in dirty_sections {
        let cells: Vec<IVec3> = match world.section_cells.get(&section) {
            Some(set) => set.iter().copied().collect(),
            None => Vec::new(),
        };
        let blocks: Vec<(IVec3, Block)> = cells
            .iter()
            .filter_map(|c| world.blocks.get(c).map(|b| (*c, *b)))
            .collect();

        if blocks.is_empty() {
            if let Some(view) = world.sections.remove(&section) {
                commands.entity(view.entity).despawn();
            }
            continue;
        }

        let mesh = build_block_mesh(&blocks, Some(&world.blocks), IVec3::ZERO);
        match world.sections.get(&section) {
            Some(view) => {
                if let Some(existing) = meshes.get_mut(&view.mesh) {
                    *existing = mesh;
                }
            }
            None => {
                let handle = meshes.add(mesh);
                let entity = commands
                    .spawn((
                        Mesh3d(handle.clone()),
                        MeshMaterial3d(materials.solid.clone()),
                        Transform::default(),
                        world::SectionMarker,
                    ))
                    .id();
                world
                    .sections
                    .insert(section, world::SectionView { entity, mesh: handle });
            }
        }
    }

    // ---- creation views of creations that are static again -----------------
    let mut to_despawn: Vec<Entity> = Vec::new();
    for c in world.creations.iter_mut() {
        if !c.dynamic {
            if let Some(entity) = c.entity.take() {
                to_despawn.push(entity);
            }
            c.mesh = None;
        }
    }
    for entity in to_despawn {
        commands.entity(entity).despawn();
    }

    // ---- dynamic creations -------------------------------------------------
    let ids: Vec<u32> = world
        .creations
        .iter()
        .filter(|c| c.dynamic)
        .map(|c| c.id)
        .collect();

    for id in ids {
        let Some(idx) = world.creations.iter().position(|c| c.id == id) else {
            continue;
        };
        if world.creations[idx].mesh_dirty {
            let blocks = world.creations[idx].blocks.clone();
            let mesh = build_block_mesh(&blocks, None, IVec3::ZERO);
            match world.creations[idx].mesh.clone() {
                Some(handle) => {
                    if let Some(existing) = meshes.get_mut(&handle) {
                        *existing = mesh;
                    }
                }
                None => {
                    let handle = meshes.add(mesh);
                    let entity = commands
                        .spawn((
                            Mesh3d(handle.clone()),
                            MeshMaterial3d(materials.solid.clone()),
                            Transform::default(),
                            CreationView { id },
                        ))
                        .id();
                    world.creations[idx].mesh = Some(handle);
                    world.creations[idx].entity = Some(entity);
                }
            }
            world.creations[idx].mesh_dirty = false;
        }

        let body_pos = world.creations[idx].body.pos;
        let body_rot = world.creations[idx].body.rot;
        let com_local = world.creations[idx].body.com_local;
        if let Some(entity) = world.creations[idx].entity {
            if let Ok(mut transform) = transforms.get_mut(entity) {
                transform.translation = body_pos - body_rot * com_local;
                transform.rotation = body_rot;
            }
        }
    }
}

// ---------------------------------------------------------------------------

fn terrain_stream_system(
    mut commands: Commands,
    mut chunks: ResMut<TerrainChunks>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<Materials>,
    world: Res<BlockWorld>,
    settings: Res<PlayerSettings>,
    player_query: Query<&Player>,
) {
    let Ok(player) = player_query.get_single() else {
        return;
    };
    let center = terrain::chunk_of(player.cam_pos);
    let distance = settings.render_distance.clamp(3, 24);
    if chunks.center == Some(center) && chunks.distance == distance && !chunks.map.is_empty() {
        return;
    }
    chunks.center = Some(center);
    chunks.distance = distance;

    let mut needed: HashSet<IVec2> = HashSet::new();
    for dz in -distance..=distance {
        for dx in -distance..=distance {
            if dx * dx + dz * dz > distance * distance {
                continue;
            }
            let coord = center + IVec2::new(dx, dz);
            needed.insert(coord);
            if !chunks.map.contains_key(&coord) {
                let mesh = meshgen::build_terrain_chunk(coord, world.seed);
                let handle = meshes.add(mesh);
                let entity = commands
                    .spawn((
                        Mesh3d(handle),
                        MeshMaterial3d(materials.terrain.clone()),
                        Transform::default(),
                        TerrainChunk,
                    ))
                    .id();
                chunks.map.insert(coord, entity);
            }
        }
    }

    let mut far: Vec<IVec2> = Vec::new();
    for coord in chunks.map.keys().copied() {
        if !needed.contains(&coord) {
            far.push(coord);
        }
    }
    for coord in far {
        if let Some(entity) = chunks.map.remove(&coord) {
            commands.entity(entity).despawn();
        }
    }
}

// ---------------------------------------------------------------------------

fn link_visual_system(
    mut commands: Commands,
    mut world: ResMut<BlockWorld>,
    mut views: ResMut<LinkViews>,
    mut beams: Query<(&LinkBeam, &mut Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<Materials>,
) {
    if world.links_dirty {
        world.links_dirty = false;
        rebuild_link_beams(&mut commands, &world, &mut views, &mut meshes, &materials);
        return;
    }

    // keep beams of moving creations glued to their body
    for (beam, mut transform) in beams.iter_mut() {
        let Some(id) = beam.creation else { continue };
        let Some(c) = world.creation_by_id(id) else { continue };
        if !c.dynamic {
            continue;
        }
        let dir_local = (beam.b - beam.a).normalize_or_zero();
        let dir_world = (c.body.rot * dir_local).normalize_or_zero();
        transform.translation = c.body.pos + c.body.rot * (beam.a - c.body.com_local);
        if dir_local.length_squared() > 0.5 && dir_world.length_squared() > 0.5 {
            transform.rotation = Quat::from_rotation_arc(dir_local, dir_world);
        }
    }
}

fn rebuild_link_beams(
    commands: &mut Commands,
    world: &BlockWorld,
    views: &mut ResMut<LinkViews>,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &Res<Materials>,
) {
    for entity in views.entities.drain(..) {
        commands.entity(entity).despawn();
    }

    let mut jobs: Vec<(Vec3, Vec3, bool, Option<u32>)> = Vec::new();
    for (a, b) in world.links.iter() {
        let powered = world.blocks.get(a).map(|blk| blk.powered()).unwrap_or(false);
        jobs.push((
            a.as_vec3() + Vec3::splat(0.5),
            b.as_vec3() + Vec3::splat(0.5),
            powered,
            None,
        ));
    }
    for c in world.creations.iter() {
        for (a, b) in c.links.iter() {
            let on = c.block_at(*a).map(|blk| blk.powered()).unwrap_or(false);
            jobs.push((
                a.as_vec3() + Vec3::splat(0.5),
                b.as_vec3() + Vec3::splat(0.5),
                on,
                Some(c.id),
            ));
        }
    }

    for (a, b, powered, creation) in jobs {
        if (b - a).length_squared() < 0.001 {
            continue;
        }
        let mesh = meshgen::build_link_mesh(Vec3::ZERO, b - a, logic::link_color(powered));
        let handle = meshes.add(mesh);
        let entity = commands
            .spawn((
                Mesh3d(handle),
                MeshMaterial3d(materials.link.clone()),
                Transform::from_translation(a),
                LinkBeam { creation, a, b },
            ))
            .id();
        views.entities.push(entity);
    }
}

// ---------------------------------------------------------------------------

fn ghost_system(
    world: Res<BlockWorld>,
    mut editor: ResMut<Editor>,
    ui: Res<UiState>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<Materials>,
    player_query: Query<&Player>,
    mut ghost_query: Query<(&mut Transform, &mut Visibility), With<Ghost>>,
) {
    let Ok((mut transform, mut visibility)) = ghost_query.get_single_mut() else {
        return;
    };
    let Ok(player) = player_query.get_single() else {
        return;
    };

    if ui.pause || editor.menu_open || player.riding.is_some() || editor.lifted.is_some() {
        *visibility = Visibility::Hidden;
        return;
    }

    if editor.ghost_part != editor.current_part() {
        if let Some(mesh) = meshes.get_mut(&materials.ghost_mesh) {
            *mesh = build::ghost_mesh(editor.current_part(), editor.rot, editor.color);
        }
        editor.ghost_part = editor.current_part();
    }

    let Some(hit) = current_hit(&world, &player) else {
        *visibility = Visibility::Hidden;
        return;
    };

    let cell = match hit.kind {
        HitKind::Terrain => {
            let p = hit.point + hit.normal * 0.001;
            IVec3::new(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32)
        }
        HitKind::StaticBlock { cell } => match editor.tool {
            Tool::Build => cell + build::round_normal(hit.normal),
            _ => cell,
        },
        HitKind::DynamicBlock { .. } => {
            *visibility = Visibility::Hidden;
            return;
        }
    };

    *visibility = Visibility::Visible;
    transform.translation = cell.as_vec3() + Vec3::splat(0.5);
    transform.rotation =
        Quat::from_axis_angle(Vec3::Y, (editor.rot % 4) as f32 * std::f32::consts::FRAC_PI_2);
}

// ---------------------------------------------------------------------------

fn follow_system(
    player_query: Query<&Player>,
    mut sun_query: Query<&mut Transform, (With<Sun>, Without<PlayerCamera>)>,
    mut water_query: Query<&mut Transform, (With<Water>, Without<Sun>, Without<PlayerCamera>)>,
    settings: Res<PlayerSettings>,
    mut lights: Query<&mut DirectionalLight>,
) {
    let Ok(player) = player_query.get_single() else {
        return;
    };
    let focus = player.cam_pos;

    for mut transform in sun_query.iter_mut() {
        transform.translation = focus + Vec3::new(70.0, 130.0, 45.0);
        transform.look_at(focus, Vec3::Y);
    }
    for mut transform in water_query.iter_mut() {
        transform.translation = Vec3::new(focus.x, terrain::WATER_LEVEL, focus.z);
    }
    for mut light in lights.iter_mut() {
        light.shadows_enabled = settings.shadows;
    }
}

// ---------------------------------------------------------------------------

fn camera_system(
    world: Res<BlockWorld>,
    settings: Res<PlayerSettings>,
    player_query: Query<&Player>,
    mut camera_query: Query<(&mut Transform, &mut Projection), With<PlayerCamera>>,
) {
    let Ok(player) = player_query.get_single() else {
        return;
    };
    let Ok((mut transform, mut projection)) = camera_query.get_single_mut() else {
        return;
    };

    transform.translation = player::eye_position(&player, &world);
    transform.rotation = player::look_quat(player.yaw, player.pitch);

    if let Projection::Perspective(perspective) = projection.as_mut() {
        perspective.fov = settings.fov.to_radians();
    }
}

// ---------------------------------------------------------------------------

fn ui_action_system(
    mut events: ResMut<UiEvents>,
    mut world: ResMut<BlockWorld>,
    mut editor: ResMut<Editor>,
    mut settings: ResMut<PlayerSettings>,
    mut ui_state: ResMut<UiState>,
    mut player_query: Query<&mut Player>,
    mut exit: EventWriter<AppExit>,
) {
    if events.0.is_empty() {
        return;
    }
    let actions = std::mem::take(&mut events.0);

    for action in actions {
        match action {
            UiAction::Resume => ui_state.pause = false,
            UiAction::Save => {
                world.freeze_all();
                let pos = player_query
                    .iter()
                    .next()
                    .map(|p| p.cam_pos)
                    .unwrap_or(Vec3::ZERO);
                match save::save(&world, pos) {
                    Ok(n) => editor.say(&format!("Saved {} blocks", n)),
                    Err(e) => editor.say(&format!("Save failed: {}", e)),
                }
            }
            UiAction::Load => match save::load(&mut world) {
                Ok((pos, n)) => {
                    if let Ok(mut player) = player_query.get_single_mut() {
                        player.cam_pos = pos;
                        player.riding = None;
                    }
                    editor.say(&format!("Loaded {} blocks", n));
                }
                Err(e) => editor.say(&format!("Load failed: {}", e)),
            },
            UiAction::Clear => {
                world.blocks.clear();
                world.section_cells.clear();
                world.links.clear();
                world.creations.clear();
                world.next_id = 1;
                let sections: Vec<IVec3> = world.sections.keys().copied().collect();
                for section in sections {
                    world.dirty_sections.insert(section);
                }
                starter_base(&mut world);
                world.rebuild_creations();
                world.links_dirty = true;
                editor.say("New world");
            }
            UiAction::Fly => {
                if let Ok(mut player) = player_query.get_single_mut() {
                    player.fly = !player.fly;
                    editor.say(if player.fly { "Fly ON" } else { "Fly OFF" });
                }
            }
            UiAction::Shadows => {
                settings.shadows = !settings.shadows;
            }
            UiAction::RdUp => settings.render_distance = (settings.render_distance + 2).min(24),
            UiAction::RdDown => settings.render_distance = (settings.render_distance - 2).max(4),
            UiAction::SensUp => settings.sensitivity = (settings.sensitivity * 1.25).min(0.02),
            UiAction::SensDown => settings.sensitivity = (settings.sensitivity / 1.25).max(0.0002),
            UiAction::FovUp => settings.fov = (settings.fov + 5.0).min(120.0),
            UiAction::FovDown => settings.fov = (settings.fov - 5.0).max(40.0),
            UiAction::Quit => {
                exit.send(AppExit::Success);
            }
        }
    }
}
