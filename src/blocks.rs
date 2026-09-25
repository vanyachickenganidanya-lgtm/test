//! Part catalogue. Every part is described by data - geometry is generated in
//! `meshgen.rs`, colour comes from the palette below. Nothing is loaded from
//! disk, which is why the whole game fits in a single ~10 MB binary.

use bevy::prelude::*;

/// A single occupied cell of the building grid.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Block {
    pub part: u16,
    /// Yaw step: 0 = north, 1 = east, 2 = south, 3 = west.
    pub rot: u8,
    /// Index into `PALETTE`.
    pub color: u8,
    /// Bit 0: powered/logic high. Bit 1: latched (switch/timer phase).
    /// Bits 2..8: counter value / timer progress.
    pub state: u8,
}

impl Block {
    pub fn new(part: u16) -> Self {
        Self { part, rot: 0, color: 0, state: 0 }
    }
    pub fn powered(self) -> bool {
        self.state & 1 != 0
    }
    pub fn set_powered(&mut self, value: bool) {
        self.state = (self.state & !1) | (value as u8);
    }
    pub fn latched(self) -> bool {
        self.state & 2 != 0
    }
    pub fn set_latched(&mut self, value: bool) {
        self.state = (self.state & !2) | ((value as u8) << 1);
    }
    /// Generic 6 bit payload (counter, timer phase, ...).
    pub fn data(self) -> u8 {
        self.state >> 2
    }
    pub fn set_data(&mut self, value: u8) {
        self.state = (self.state & 3) | ((value & 0x3f) << 2);
    }
}

// ---------------------------------------------------------------------------
// Parts
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Structure,
    Movement,
    Power,
    Logic,
    Utility,
}

impl Category {
    pub const ALL: [Category; 5] = [
        Category::Structure,
        Category::Movement,
        Category::Power,
        Category::Logic,
        Category::Utility,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Category::Structure => "Structure",
            Category::Movement => "Movement",
            Category::Power => "Power",
            Category::Logic => "Logic",
            Category::Utility => "Utility",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    Cube,
    Plate,
    Wedge,
    Cylinder,
    Sphere,
    Frame,
    Wheel,
    Seat,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Behavior {
    None,
    Wheel { radius: f32, grip: f32, power: f32 },
    Seat,
    Engine { force: f32 },
    Thruster { force: f32 },
    Propeller { force: f32 },
    Balloon { lift: f32 },
    Buoyancy { lift: f32 },
    Gyro { strength: f32 },
    Motor { torque: f32 },
    Piston { force: f32 },
    Light,
    Sensor,
    Switch,
    Button,
    Gate(GateKind),
    Spudgun { speed: f32 },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GateKind {
    And,
    Or,
    Xor,
    Not,
    Timer,
    Counter,
    Memory,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PartDef {
    pub name: &'static str,
    /// Base colour (sRGB). Multiplied with the palette colour of the block.
    pub color: [f32; 3],
    pub category: Category,
    pub mass: f32,
    pub shape: Shape,
    pub behavior: Behavior,
    pub hint: &'static str,
}

impl PartDef {
    const fn new(
        name: &'static str,
        color: [f32; 3],
        category: Category,
        mass: f32,
        shape: Shape,
        behavior: Behavior,
        hint: &'static str,
    ) -> Self {
        Self { name, color, category, mass, shape, behavior, hint }
    }
}

pub const P_BLOCK: u16 = 0;
pub const P_PLATE: u16 = 1;
pub const P_WEDGE: u16 = 2;
pub const P_CYLINDER: u16 = 3;
pub const P_SPHERE: u16 = 4;
pub const P_FRAME: u16 = 5;
pub const P_WEIGHT: u16 = 6;
pub const P_WHEEL: u16 = 7;
pub const P_WHEEL_SMALL: u16 = 8;
pub const P_WHEEL_OFFROAD: u16 = 9;
pub const P_SEAT: u16 = 10;
pub const P_GYRO: u16 = 11;
pub const P_THRUSTER: u16 = 12;
pub const P_PROPELLER: u16 = 13;
pub const P_BALLOON: u16 = 14;
pub const P_BUOYANCY: u16 = 15;
pub const P_PISTON: u16 = 16;
pub const P_BEARING: u16 = 17;
pub const P_ENGINE: u16 = 18;
pub const P_MOTOR_ENGINE: u16 = 19;
pub const P_ROCKET: u16 = 20;
pub const P_SWITCH: u16 = 21;
pub const P_BUTTON: u16 = 22;
pub const P_SENSOR: u16 = 23;
pub const P_AND: u16 = 24;
pub const P_OR: u16 = 25;
pub const P_XOR: u16 = 26;
pub const P_NOT: u16 = 27;
pub const P_TIMER: u16 = 28;
pub const P_COUNTER: u16 = 29;
pub const P_MEMORY: u16 = 30;
pub const P_LIGHT: u16 = 31;
pub const P_SPUDGUN: u16 = 32;

#[rustfmt::skip]
pub static PARTS: &[PartDef] = &[
    // --- Structure ---------------------------------------------------------
    PartDef::new("Block",        [0.62, 0.63, 0.66], Category::Structure, 20.0,  Shape::Cube,     Behavior::None, "The brick everything is made of"),
    PartDef::new("Plate",        [0.55, 0.57, 0.60], Category::Structure, 8.0,   Shape::Plate,    Behavior::None, "Thin 1/4 block panel"),
    PartDef::new("Wedge",        [0.60, 0.62, 0.65], Category::Structure, 14.0,  Shape::Wedge,    Behavior::None, "Ramp, walkable slope"),
    PartDef::new("Cylinder",     [0.58, 0.60, 0.64], Category::Structure, 16.0,  Shape::Cylinder, Behavior::None, "Round pillar"),
    PartDef::new("Sphere",       [0.58, 0.60, 0.64], Category::Structure, 16.0,  Shape::Sphere,   Behavior::None, "Ball, rolls nicely (visually)"),
    PartDef::new("Frame",        [0.45, 0.47, 0.52], Category::Structure, 6.0,   Shape::Frame,    Behavior::None, "Lightweight beam"),
    PartDef::new("Weight",       [0.20, 0.21, 0.24], Category::Structure, 160.0, Shape::Cube,     Behavior::None, "Heavy ballast, lowers centre of mass"),
    // --- Movement ----------------------------------------------------------
    PartDef::new("Wheel",        [0.13, 0.14, 0.16], Category::Movement, 12.0,  Shape::Wheel,     Behavior::Wheel { radius: 0.45, grip: 1.0, power: 1.0 }, "Suspended drive wheel"),
    PartDef::new("Small Wheel",  [0.13, 0.14, 0.16], Category::Movement, 7.0,   Shape::Wheel,     Behavior::Wheel { radius: 0.30, grip: 0.8, power: 0.7 }, "Light caster wheel"),
    PartDef::new("Offroad Wheel",[0.16, 0.16, 0.16], Category::Movement, 22.0,  Shape::Wheel,     Behavior::Wheel { radius: 0.62, grip: 1.5, power: 1.4 }, "Big grippy wheel"),
    PartDef::new("Seat",         [0.75, 0.30, 0.22], Category::Movement, 25.0,  Shape::Seat,      Behavior::Seat, "Sit here to drive the creation"),
    PartDef::new("Gyroscope",    [0.30, 0.55, 0.85], Category::Movement, 30.0,  Shape::Cube,      Behavior::Gyro { strength: 1.0 }, "Auto-levels the creation (what Scrap Mechanic lacks)"),
    PartDef::new("Thruster",     [0.85, 0.45, 0.10], Category::Movement, 18.0,  Shape::Cylinder,  Behavior::Thruster { force: 14_000.0 }, "Rocket thrust along its arrow"),
    PartDef::new("Propeller",    [0.80, 0.80, 0.30], Category::Movement, 14.0,  Shape::Frame,     Behavior::Propeller { force: 9_000.0 }, "Air propulsion, loses push at speed"),
    PartDef::new("Balloon",      [0.90, 0.35, 0.45], Category::Movement, 5.0,   Shape::Sphere,    Behavior::Balloon { lift: 0.92 }, "Cancels most of the gravity"),
    PartDef::new("Buoyancy Tank",[0.25, 0.70, 0.90], Category::Movement, 15.0,  Shape::Cube,      Behavior::Buoyancy { lift: 3.0 }, "Floats in water"),
    PartDef::new("Piston",       [0.70, 0.70, 0.75], Category::Movement, 18.0,  Shape::Cylinder,  Behavior::Piston { force: 6_000.0 }, "Linear actuator along its axis"),
    PartDef::new("Bearing Motor",[0.55, 0.35, 0.75], Category::Movement, 16.0,  Shape::Cylinder,  Behavior::Motor { torque: 9_000.0 }, "Spins the creation around its axis"),
    // --- Power -------------------------------------------------------------
    PartDef::new("Gas Engine",   [0.85, 0.25, 0.20], Category::Power, 60.0,  Shape::Cube, Behavior::Engine { force: 9_000.0 },  "Push force when powered"),
    PartDef::new("Electric Motor",[0.25, 0.60, 0.85],Category::Power, 40.0,  Shape::Cube, Behavior::Engine { force: 5_500.0 },  "Smooth push force"),
    PartDef::new("Rocket Engine",[0.95, 0.60, 0.10], Category::Power, 35.0,  Shape::Cylinder, Behavior::Engine { force: 22_000.0 }, "Serious push force"),
    // --- Logic -------------------------------------------------------------
    PartDef::new("Switch",       [0.30, 0.85, 0.40], Category::Logic, 4.0, Shape::Plate, Behavior::Switch, "Click / press E to toggle"),
    PartDef::new("Button",       [0.95, 0.75, 0.20], Category::Logic, 4.0, Shape::Plate, Behavior::Button, "Momentary signal"),
    PartDef::new("Sensor",       [0.20, 0.85, 0.90], Category::Logic, 6.0, Shape::Plate, Behavior::Sensor, "Detects players/creations in front"),
    PartDef::new("AND Gate",     [0.35, 0.45, 0.75], Category::Logic, 4.0, Shape::Plate, Behavior::Gate(GateKind::And), "All inputs must be on"),
    PartDef::new("OR Gate",      [0.45, 0.35, 0.75], Category::Logic, 4.0, Shape::Plate, Behavior::Gate(GateKind::Or), "Any input turns it on"),
    PartDef::new("XOR Gate",     [0.55, 0.35, 0.70], Category::Logic, 4.0, Shape::Plate, Behavior::Gate(GateKind::Xor), "Odd number of inputs"),
    PartDef::new("NOT Gate",     [0.70, 0.35, 0.55], Category::Logic, 4.0, Shape::Plate, Behavior::Gate(GateKind::Not), "Inverts the signal"),
    PartDef::new("Timer",        [0.40, 0.75, 0.70], Category::Logic, 4.0, Shape::Plate, Behavior::Gate(GateKind::Timer), "Blinks every ~1 s"),
    PartDef::new("Counter",      [0.75, 0.60, 0.35], Category::Logic, 4.0, Shape::Plate, Behavior::Gate(GateKind::Counter), "Fires every 8 pulses"),
    PartDef::new("Memory Cell",  [0.60, 0.60, 0.30], Category::Logic, 4.0, Shape::Plate, Behavior::Gate(GateKind::Memory), "Set/reset latch"),
    // --- Utility -----------------------------------------------------------
    PartDef::new("Light",        [1.00, 0.95, 0.70], Category::Utility, 5.0, Shape::Plate, Behavior::Light, "Glows when powered"),
    PartDef::new("Spudgun",      [0.35, 0.80, 0.35], Category::Utility, 20.0, Shape::Cylinder, Behavior::Spudgun { speed: 60.0 }, "Fires potatoes, pushes creations"),
];

#[inline]
pub fn def(part: u16) -> &'static PartDef {
    &PARTS[(part as usize).min(PARTS.len() - 1)]
}

#[inline]
pub fn mass_of(block: &Block) -> f32 {
    def(block.part).mass
}

pub fn is_logic_gate(part: u16) -> bool {
    matches!(def(part).behavior, Behavior::Gate(_))
}

pub fn is_input_part(part: u16) -> bool {
    matches!(
        def(part).behavior,
        Behavior::Switch | Behavior::Button | Behavior::Sensor
    )
}

/// Parts whose colour the player can change with the palette.
pub fn paintable(part: u16) -> bool {
    matches!(def(part).shape, Shape::Cube | Shape::Plate | Shape::Wedge | Shape::Cylinder | Shape::Sphere | Shape::Frame)
}

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

#[rustfmt::skip]
pub const PALETTE: [[f32; 3]; 14] = [
    [0.72, 0.73, 0.76], // 0 white
    [0.85, 0.25, 0.22], // 1 red
    [0.90, 0.55, 0.12], // 2 orange
    [0.88, 0.82, 0.20], // 3 yellow
    [0.30, 0.72, 0.30], // 4 green
    [0.18, 0.60, 0.55], // 5 teal
    [0.20, 0.45, 0.85], // 6 blue
    [0.45, 0.28, 0.75], // 7 purple
    [0.85, 0.45, 0.70], // 8 pink
    [0.42, 0.30, 0.22], // 9 brown
    [0.16, 0.17, 0.19], // 10 black
    [0.55, 0.56, 0.60], // 11 grey
    [0.95, 0.95, 0.35], // 12 signal yellow
    [0.20, 0.85, 0.35], // 13 signal green
];

/// Final sRGB colour of a block: part colour blended with the palette colour.
pub fn block_color(block: &Block) -> [f32; 3] {
    let base = def(block.part).color;
    let tint = PALETTE[(block.color as usize).min(PALETTE.len() - 1)];
    // Structural parts take the palette fully, functional parts keep their
    // identity colour and only get a slight tint.
    let t = if paintable(block.part) { 0.92 } else { 0.25 };
    [
        base[0] + (tint[0] - base[0]) * t,
        base[1] + (tint[1] - base[1]) * t,
        base[2] + (tint[2] - base[2]) * t,
    ]
}

/// Yaw rotation (radians) for a rotation step.
#[inline]
pub fn rot_yaw(step: u8) -> f32 {
    (step % 4) as f32 * std::f32::consts::FRAC_PI_2
}

/// Forward direction (in block space) for a rotation step.
pub fn forward(step: u8) -> Vec3 {
    Quat::from_axis_angle(Vec3::Y, rot_yaw(step)) * Vec3::Z
}

/// Right direction (in block space) for a rotation step.
pub fn right(step: u8) -> Vec3 {
    Quat::from_axis_angle(Vec3::Y, rot_yaw(step)) * Vec3::X
}
