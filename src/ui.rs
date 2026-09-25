//! HUD, part menu and pause menu. Everything is plain Bevy UI - no images,
//! no fonts loaded from disk (the built-in font is embedded in the binary).

use bevy::prelude::*;

use crate::blocks::{def, Category, PARTS, PALETTE};
use crate::build::{parts_in_category, Editor};
use crate::player::{Player, PlayerSettings};
use crate::world::BlockWorld;

#[derive(Resource, Default)]
pub struct UiState {
    pub pause: bool,
}

#[derive(Resource, Default)]
pub struct UiEvents(pub Vec<UiAction>);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiAction {
    Resume,
    Save,
    Load,
    Clear,
    Fly,
    Shadows,
    RdUp,
    RdDown,
    SensUp,
    SensDown,
    FovUp,
    FovDown,
    Quit,
}

#[derive(Resource)]
pub struct Hud {
    pub info: Entity,
    pub hint: Entity,
    pub toast: Entity,
    pub hotbar: Vec<Entity>,
    pub hotbar_bg: Vec<Entity>,
    pub menu: Entity,
    pub menu_text: Entity,
    pub pause: Entity,
    pub pause_text: Entity,
}

const PANEL: Color = Color::srgba(0.05, 0.06, 0.09, 0.92);
const ACCENT: Color = Color::srgb(1.0, 0.72, 0.18);

fn label(text: &str, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text.to_string()),
        TextFont { font_size: size, ..default() },
        TextColor(color),
    )
}

pub fn spawn_ui(commands: &mut Commands) -> Hud {
    // ---------------------------------------------------------------- crosshair
    commands.spawn((
        label("+", 26.0, Color::srgba(1.0, 1.0, 1.0, 0.85)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(50.0),
            margin: UiRect::px(-10.0, 0.0, -16.0, 0.0),
            ..default()
        },
    ));

    // -------------------------------------------------------------------- info
    let info = commands
        .spawn((
            label("", 15.0, Color::srgb(0.92, 0.94, 0.98)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(10.0),
                ..default()
            },
        ))
        .id();

    // ----------------------------------------------------------------- hotbar
    let mut hotbar: Vec<Entity> = Vec::new();
    let mut hotbar_bg: Vec<Entity> = Vec::new();
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(12.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            column_gap: Val::Px(5.0),
            ..default()
        })
        .with_children(|parent| {
            for _ in 0..10 {
                let mut text_entity: Option<Entity> = None;
                let bg = parent
                    .spawn((
                        Node {
                            width: Val::Px(104.0),
                            height: Val::Px(32.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.05, 0.06, 0.09, 0.75)),
                        BorderColor(Color::srgba(0.5, 0.55, 0.65, 0.7)),
                    ))
                    .with_children(|slot| {
                        text_entity = Some(slot.spawn(label("", 14.0, Color::srgb(0.9, 0.92, 0.96))).id());
                    })
                    .id();
                hotbar_bg.push(bg);
                hotbar.push(text_entity.unwrap_or(bg));
            }
        });

    // ------------------------------------------------------------------- hint
    let hint = commands
        .spawn((
            label("", 14.0, Color::srgba(0.85, 0.88, 0.95, 0.75)),
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(52.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .id();

    // ------------------------------------------------------------------ toast
    let toast = commands
        .spawn((
            label("", 20.0, ACCENT),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(28.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .id();

    // -------------------------------------------------------------- part menu
    let mut menu_text: Option<Entity> = None;
    let menu = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(12.0),
                top: Val::Percent(10.0),
                width: Val::Percent(76.0),
                height: Val::Percent(70.0),
                padding: UiRect::all(Val::Px(18.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(PANEL),
            Visibility::Hidden,
        ))
        .with_children(|parent| {
            parent.spawn(label(
                "BUILD MENU    [Tab] next category    [1..9,0] pick part    [B] close",
                20.0,
                ACCENT,
            ));
            menu_text = Some(parent.spawn(label("", 16.0, Color::srgb(0.88, 0.9, 0.95))).id());
        })
        .id();
    let menu_text = menu_text.unwrap_or(menu);

    // ------------------------------------------------------------- pause menu
    let rows: Vec<Vec<(&'static str, UiAction)>> = vec![
        vec![
            ("Resume", UiAction::Resume),
            ("Save (F5)", UiAction::Save),
            ("Load (F9)", UiAction::Load),
            ("New world", UiAction::Clear),
        ],
        vec![
            ("Fly", UiAction::Fly),
            ("Shadows", UiAction::Shadows),
            ("Render -", UiAction::RdDown),
            ("Render +", UiAction::RdUp),
        ],
        vec![
            ("Sens -", UiAction::SensDown),
            ("Sens +", UiAction::SensUp),
            ("FOV -", UiAction::FovDown),
            ("FOV +", UiAction::FovUp),
        ],
        vec![("Quit", UiAction::Quit)],
    ];

    let mut pause_text: Option<Entity> = None;
    let pause = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.04, 0.82)),
            Visibility::Hidden,
        ))
        .with_children(|parent| {
            parent.spawn(label("SCRAPFORGE", 42.0, ACCENT));
            pause_text = Some(parent.spawn(label("", 15.0, Color::srgb(0.8, 0.85, 0.92))).id());
            parent.spawn(label(
                "LMB place  RMB remove  R rotate  G lift  C wire  V paste  E use  F fly  B build menu",
                14.0,
                Color::srgba(0.8, 0.85, 0.92, 0.7),
            ));
            for row in rows {
                parent
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(10.0),
                        ..default()
                    })
                    .with_children(|row_parent| {
                        for (text, action) in row {
                            row_parent
                                .spawn((
                                    Node {
                                        width: Val::Px(168.0),
                                        height: Val::Px(40.0),
                                        justify_content: JustifyContent::Center,
                                        align_items: AlignItems::Center,
                                        border: UiRect::all(Val::Px(2.0)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::srgba(0.12, 0.14, 0.2, 0.95)),
                                    BorderColor(Color::srgba(0.6, 0.65, 0.75, 0.8)),
                                    Interaction::None,
                                    ButtonAction { action },
                                ))
                                .with_children(|btn| {
                                    btn.spawn(label(text, 16.0, Color::srgb(0.95, 0.96, 1.0)));
                                });
                        }
                    });
            }
        })
        .id();
    let pause_text = pause_text.unwrap_or(pause);

    Hud {
        info,
        hint,
        toast,
        hotbar,
        hotbar_bg,
        menu,
        menu_text,
        pause,
        pause_text,
    }
}

#[derive(Component)]
pub struct ButtonAction {
    pub action: UiAction,
}

/// Feeds clicks into `UiEvents`.
pub fn button_system(
    mut events: ResMut<UiEvents>,
    query: Query<(&Interaction, &ButtonAction), Changed<Interaction>>,
) {
    for (interaction, action) in query.iter() {
        if *interaction == Interaction::Pressed {
            events.0.push(action.action);
        }
    }
}

/// Updates every HUD label.
#[allow(clippy::too_many_arguments)]
pub fn update_hud(
    hud: Res<Hud>,
    mut texts: Query<&mut Text>,
    mut bgs: Query<&mut BackgroundColor>,
    world: Res<BlockWorld>,
    editor: Res<Editor>,
    player_query: Query<&Player>,
    settings: Res<PlayerSettings>,
    time: Res<Time>,
) {
    let player = player_query.iter().next();
    let fps = 1.0 / time.delta_secs().max(0.0001);

    if let Ok(mut t) = texts.get_mut(hud.info) {
        let (pos, mode) = match player {
            Some(p) => (
                p.cam_pos,
                if p.riding.is_some() {
                    "driving"
                } else if p.fly {
                    "flying"
                } else if p.on_ground {
                    "walking"
                } else {
                    "airborne"
                },
            ),
            None => (Vec3::ZERO, "-"),
        };
        t.0 = format!(
            "ScrapForge 0.1.0   {:.0} fps\n\
             xyz {:.1} {:.1} {:.1}   {}\n\
             blocks {}   creations {}   seed {}\n\
             tool {}   part {}   rot {}   colour {}\n\
             aim: {}",
            fps,
            pos.x,
            pos.y,
            pos.z,
            mode,
            world.block_count(),
            world.creation_count(),
            world.seed,
            editor.tool.name(),
            def(editor.current_part()).name,
            editor.rot,
            editor.color,
            if editor.hint.is_empty() { "-" } else { &editor.hint },
        );
    }

    if let Ok(mut t) = texts.get_mut(hud.hint) {
        t.0 = match player {
            Some(p) if p.riding.is_some() => {
                "WASD drive   Space brake   Shift boost   Q leave seat".to_string()
            }
            _ => "LMB place  RMB remove  MMB pick  R rotate  G lift  C wire  V paste  E use  F fly  B menu  Esc pause"
                .to_string(),
        };
    }

    if let Ok(mut t) = texts.get_mut(hud.toast) {
        t.0 = if editor.message_timer > 0.0 {
            editor.message.clone()
        } else {
            String::new()
        };
    }

    // hotbar
    for (i, entity) in hud.hotbar.iter().enumerate() {
        if let Ok(mut t) = texts.get_mut(*entity) {
            let part = editor
                .hotbar
                .get(i)
                .copied()
                .unwrap_or(0)
                .min(PARTS.len() as u16 - 1);
            let key = if i == 9 { "0".to_string() } else { format!("{}", i + 1) };
            t.0 = format!("[{}] {}", key, def(part).name);
        }
        if let Some(bg_entity) = hud.hotbar_bg.get(i) {
            if let Ok(mut bg) = bgs.get_mut(*bg_entity) {
                bg.0 = if i == editor.slot {
                    Color::srgba(0.95, 0.62, 0.12, 0.95)
                } else {
                    Color::srgba(0.05, 0.06, 0.09, 0.75)
                };
            }
        }
    }

    // part menu
    if let Ok(mut t) = texts.get_mut(hud.menu_text) {
        let category = Category::ALL[editor.menu_category % Category::ALL.len()];
        let parts = parts_in_category(category);
        let mut lines = Vec::new();
        for (i, id) in parts.iter().enumerate() {
            let key = match i % 10 {
                9 => "0".to_string(),
                n => format!("{}", n + 1),
            };
            let marker = if *id == editor.current_part() { ">" } else { " " };
            lines.push(format!(
                "{} [{}] {:<16} {:>6.0} kg   {}",
                marker,
                key,
                def(*id).name,
                def(*id).mass,
                def(*id).hint
            ));
        }
        t.0 = format!("{}\n\n{}", category.name(), lines.join("\n"));
    }

    // pause panel
    if let Ok(mut t) = texts.get_mut(hud.pause_text) {
        t.0 = format!(
            "render distance {} chunks   sensitivity {:.4}   fov {:.0}   shadows {}",
            settings.render_distance,
            settings.sensitivity,
            settings.fov,
            if settings.shadows { "on" } else { "off" }
        );
    }
}

/// Shows / hides the two overlay panels.
pub fn update_panels(
    hud: Res<Hud>,
    editor: Res<Editor>,
    ui: Res<UiState>,
    mut visibility: Query<&mut Visibility>,
) {
    if let Ok(mut v) = visibility.get_mut(hud.menu) {
        *v = if editor.menu_open && !ui.pause {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if let Ok(mut v) = visibility.get_mut(hud.pause) {
        *v = if ui.pause {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Colour swatch helper (keeps the palette in one place for UI and renderer).
pub fn palette_color(index: u8) -> Color {
    let c = PALETTE[(index as usize).min(PALETTE.len() - 1)];
    Color::srgb(c[0], c[1], c[2])
}
