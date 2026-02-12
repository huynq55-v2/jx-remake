use std::{collections::HashMap, hash::Hash};

use serde::Deserialize;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ActionId {
    FreeStand1 = 0,
    FreeStand2 = 1,
    FreeStand3 = 2,
    MeleeWStand = 3,
    RangeWStand = 4,
    DoubleWStand = 5,
    FreeWalk = 6,
    NormalWalk = 7,
    MeleeWWalk = 8,
    RangeWWalk = 9,
    DoubleWWalk = 10,
    FreeRun = 11,
    NormalRun = 12,
    MeleeWRun = 13,
    RangeWRun = 14,
    DoubleWRun = 15,
    FreeWound = 16,
    MeleeWWound = 17,
    RangeWWound = 18,
    DoubleWWound = 19,
    FreeDie = 20,
    MeleeWDie = 21,
    RangeWDie = 22,
    DoubleWDie = 23,
    FreeAttack = 24,
    MeleeWPuncture = 25,
    MeleeWCut = 26,
    RangeWPuncture = 27,
    RangeWCut = 28,
    DoubleWPull = 29,
    DoubleWPound = 30,
    DartThrow = 31,
    FreeMagic = 32,
    MeleeWMagic = 33,
    RangeWMagic = 34,
    DoubleWMagic = 35,
    SitDown = 36,
    JumpFly = 37,
    RideStand = 38,
    RideWalk = 39,
    RideRun = 40,
    RideCut = 41,
    RidePuncture = 42,
    RideMagic = 43,
    RideWound = 44,
    RideDie = 45,
    RideStand1 = 46,
    RideStand2 = 47,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Sex {
    Man,
    Lady,
}

#[derive(Debug)]
pub enum NpcKind {
    // Loại 1: Special (Main Man/Lady - Người chơi)
    // Đặc điểm: Render phức tạp (nhiều layers), Logic phức tạp (Input)
    Special(Sex),
    // Loại 2: Normal (Quái, NPC Bán hàng, NPC Nhiệm vụ)
    // Đặc điểm: Render đơn giản (thường là 1-2 layers), Logic tự động (AI/Script)
    Normal,
}

#[derive(Debug)]
pub struct Npc {
    pub id: u64,
    pub name: String,
    // pub pos: WorldPos,
    pub dir: u8,
    pub state: ActionId,
    pub kind: NpcKind,
}

impl Npc {}

#[derive(Deserialize, Debug, Clone)]
pub struct PartData {
    pub id: String,
}

pub struct VisualSlots {
    pub head: HashMap<String, PartData>,
    pub body: HashMap<String, PartData>,
    pub weapon: HashMap<String, PartData>,
    pub horse: HashMap<String, PartData>,
}

pub struct NpcAssets {
    pub action: ActionId,
    pub male: VisualSlots,
    pub female: VisualSlots,
    pub npcs: HashMap<String, PartData>,
}
