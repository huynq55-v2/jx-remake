use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufReader;
use std::time::Instant;

use macroquad::prelude::*;
use serde::Deserialize;

use jx_remake::client::spr::{SprColor, SprFile, SprFrameInfo};
use jx_remake::common::pak::PakReader;

// --- CẤU HÌNH MÀN HÌNH ---
const WIN_WIDTH: f32 = 1024.0;
const WIN_HEIGHT: f32 = 768.0;
const CENTER_X: f32 = WIN_WIDTH / 2.0 + 100.0;
const CENTER_Y: f32 = WIN_HEIGHT / 2.0;
const FONT_SIZE: u16 = 20;
const LINE_HEIGHT: f32 = 28.0;

// --- DATA STRUCTURES (JSON) ---
#[derive(Deserialize, Debug)]
struct GameAssets {
    meta: MetaData,
    male: PlayerParts,
    female: PlayerParts,
    npcs: HashMap<String, PartData>,
}

#[derive(Deserialize, Debug)]
struct MetaData {
    action_map_debug: HashMap<String, usize>,
}

#[derive(Deserialize, Debug, Default)]
struct PlayerParts {
    head: HashMap<String, PartData>,
    body: HashMap<String, PartData>,
    #[allow(dead_code)]
    hair: HashMap<String, PartData>,
    #[allow(dead_code)]
    shoulder: HashMap<String, PartData>,
    #[allow(dead_code)]
    hand_left: HashMap<String, PartData>,
    #[allow(dead_code)]
    hand_right: HashMap<String, PartData>,
    #[allow(dead_code)]
    weapon_left: HashMap<String, PartData>,
    weapon_right: HashMap<String, PartData>,
    #[allow(dead_code)]
    horse_front: HashMap<String, PartData>,
    horse_middle: HashMap<String, PartData>,
    #[allow(dead_code)]
    horse_back: HashMap<String, PartData>,
}

#[derive(Deserialize, Debug, Clone)]
struct PartData {
    #[allow(dead_code)]
    id: String,
    actions: HashMap<String, ActionData>,
}

#[derive(Deserialize, Debug, Clone)]
struct ActionData {
    full_path: String,
    info: String,
}

// --- APP STATE ---

struct AppState {
    // Selection Indices
    char_type_idx: usize, // 0: Male, 1: Female, 2: NPC
    action_idx: usize,

    // Player Parts Indices
    body_idx: usize,
    head_idx: usize,
    weapon_idx: usize,
    horse_idx: usize,

    // NPC Index
    npc_idx: usize,

    // UI State
    selected_row: usize,

    // Cache Lists (Sorted IDs)
    lists: AssetLists,

    // Render Data - Textures thay vì raw SprFile
    loaded_parts: Vec<LoadedPart>,
}

struct AssetLists {
    actions: Vec<String>,
    male_bodies: Vec<String>,
    male_heads: Vec<String>,
    male_weapons: Vec<String>,
    male_horses: Vec<String>,

    female_bodies: Vec<String>,
    female_heads: Vec<String>,
    female_weapons: Vec<String>,
    female_horses: Vec<String>,

    npcs: Vec<String>,
}

struct LoadedFrame {
    texture: Texture2D,
    offset_x: f32,
    offset_y: f32,
}

struct LoadedPart {
    name: String,
    frames: Vec<LoadedFrame>,
    info: (usize, usize, usize), // TotalFrames, Dirs, Interval
}

// --- Chuyển SPR frame thành Texture2D ---
fn spr_frame_to_texture(frame: &SprFrameInfo, palette: &[SprColor]) -> Texture2D {
    let w = frame.width as u32;
    let h = frame.height as u32;
    let mut rgba = vec![0u8; (w * h * 4) as usize];

    for y in 0..h {
        for x in 0..w {
            let pi = (y * w + x) as usize;
            if pi < frame.decoded_indices.len() {
                let alpha = frame.alpha_map[pi];
                if alpha > 0 {
                    let c = &palette[frame.decoded_indices[pi] as usize];
                    let offset = pi * 4;
                    rgba[offset] = c.r;
                    rgba[offset + 1] = c.g;
                    rgba[offset + 2] = c.b;
                    rgba[offset + 3] = alpha;
                }
            }
        }
    }

    let img = Image {
        bytes: rgba,
        width: w as u16,
        height: h as u16,
    };
    let tex = Texture2D::from_image(&img);
    tex.set_filter(FilterMode::Nearest); // Pixel art style
    tex
}

// --- MAIN ---
fn window_conf() -> Conf {
    Conf {
        window_title: "JX Viewer - Arrows to Control - ESC to Exit".to_string(),
        window_width: WIN_WIDTH as i32,
        window_height: WIN_HEIGHT as i32,
        window_resizable: false,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    // 1. Init Data
    println!("⏳ Loading Assets...");
    let assets = load_assets();
    let mut pak_readers = load_paks();
    let lists = build_asset_lists(&assets);

    // Init State mặc định
    let mut state = AppState {
        char_type_idx: 0, // Male
        action_idx: lists.actions.iter().position(|r| r == "run").unwrap_or(0),
        body_idx: 0,
        head_idx: 0,
        weapon_idx: 0,
        horse_idx: 0,
        npc_idx: 0,
        selected_row: 0,
        lists,
        loaded_parts: Vec::new(),
    };

    // Load nhân vật lần đầu
    reload_character(&mut state, &assets, &mut pak_readers);

    let mut current_frame: usize = 0;
    let mut last_tick = Instant::now();
    let mut accum_time: u64 = 0;
    let dir: usize = 0; // Hướng nhìn (0-7)

    // Key repeat timer
    let mut key_repeat_timer: f64 = 0.0;
    let key_repeat_delay = 0.25; // Delay đầu tiên
    let key_repeat_rate = 0.08; // Tốc độ lặp
    let mut key_held_time: f64 = 0.0;

    loop {
        // --- EXIT ---
        if is_key_pressed(KeyCode::Escape) {
            break;
        }

        let dt = get_frame_time() as f64;

        // --- INPUT ---
        let mut changed = false;

        // Navigation Up/Down
        if is_key_pressed(KeyCode::Up) {
            if state.selected_row > 0 {
                state.selected_row -= 1;
            }
        }
        if is_key_pressed(KeyCode::Down) {
            let max_row = if state.char_type_idx == 2 { 2 } else { 5 };
            if state.selected_row < max_row {
                state.selected_row += 1;
            }
        }

        // Left/Right with key repeat
        let left_pressed = is_key_pressed(KeyCode::Left);
        let right_pressed = is_key_pressed(KeyCode::Right);
        let left_held = is_key_down(KeyCode::Left);
        let right_held = is_key_down(KeyCode::Right);

        let mut delta: i32 = 0;

        if left_pressed {
            delta = -1;
            key_held_time = 0.0;
            key_repeat_timer = 0.0;
        } else if right_pressed {
            delta = 1;
            key_held_time = 0.0;
            key_repeat_timer = 0.0;
        } else if left_held || right_held {
            key_held_time += dt;
            if key_held_time > key_repeat_delay {
                key_repeat_timer += dt;
                if key_repeat_timer >= key_repeat_rate {
                    key_repeat_timer -= key_repeat_rate;
                    delta = if left_held { -1 } else { 1 };
                }
            }
        } else {
            key_held_time = 0.0;
            key_repeat_timer = 0.0;
        }

        if delta != 0 {
            changed = true;
            match state.selected_row {
                0 => {
                    state.char_type_idx = wrap_idx(state.char_type_idx, delta, 3);
                    // Reset sub-indices khi đổi type
                    state.body_idx = 0;
                    state.head_idx = 0;
                    state.weapon_idx = 0;
                    state.horse_idx = 0;
                    state.npc_idx = 0;
                    state.selected_row = 0;
                }
                1 => {
                    state.action_idx = wrap_idx(state.action_idx, delta, state.lists.actions.len());
                }
                _ => {
                    if state.char_type_idx == 2 {
                        // NPC MODE
                        if state.selected_row == 2 {
                            state.npc_idx = wrap_idx(state.npc_idx, delta, state.lists.npcs.len());
                        }
                    } else {
                        // PLAYER MODE
                        match state.selected_row {
                            2 => {
                                state.head_idx =
                                    wrap_idx(state.head_idx, delta, get_head_list(&state).len())
                            }
                            3 => {
                                state.body_idx =
                                    wrap_idx(state.body_idx, delta, get_body_list(&state).len())
                            }
                            4 => {
                                state.weapon_idx =
                                    wrap_idx(state.weapon_idx, delta, get_weapon_list(&state).len())
                            }
                            5 => {
                                state.horse_idx =
                                    wrap_idx(state.horse_idx, delta, get_horse_list(&state).len())
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        if changed {
            reload_character(&mut state, &assets, &mut pak_readers);
            current_frame = 0;
        }

        // --- ANIMATION TIMER ---
        let interval = if !state.loaded_parts.is_empty() {
            let (_, _, iv) = state.loaded_parts[0].info;
            if iv > 0 { iv as u64 } else { 4 }
        } else {
            4
        };

        accum_time += last_tick.elapsed().as_millis() as u64;
        last_tick = Instant::now();

        if accum_time >= interval * 55 {
            current_frame += 1;
            accum_time = 0;
        }

        // --- RENDER ---
        clear_background(Color::from_rgba(32, 32, 32, 255));

        // 1. Draw Character (Textures)
        draw_character(&state, dir, current_frame);

        // 2. Draw UI
        draw_ui(&state);

        next_frame().await;
    }
}

// --- HELPER FUNCTIONS ---

fn wrap_idx(current: usize, delta: i32, max: usize) -> usize {
    if max == 0 {
        return 0;
    }
    let res = (current as i32) + delta;
    if res < 0 {
        max - 1
    } else if res >= max as i32 {
        0
    } else {
        res as usize
    }
}

fn get_body_list(state: &AppState) -> &Vec<String> {
    if state.char_type_idx == 0 {
        &state.lists.male_bodies
    } else {
        &state.lists.female_bodies
    }
}
fn get_head_list(state: &AppState) -> &Vec<String> {
    if state.char_type_idx == 0 {
        &state.lists.male_heads
    } else {
        &state.lists.female_heads
    }
}
fn get_weapon_list(state: &AppState) -> &Vec<String> {
    if state.char_type_idx == 0 {
        &state.lists.male_weapons
    } else {
        &state.lists.female_weapons
    }
}
fn get_horse_list(state: &AppState) -> &Vec<String> {
    if state.char_type_idx == 0 {
        &state.lists.male_horses
    } else {
        &state.lists.female_horses
    }
}

fn load_assets() -> GameAssets {
    let file = File::open("data/newdata/npcres.json").expect("Missing JSON");
    serde_json::from_reader(BufReader::new(file)).expect("Bad JSON")
}

fn load_paks() -> Vec<PakReader> {
    let mut readers = Vec::new();
    if let Ok(entries) = fs::read_dir("data/pak") {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .map_or(false, |e| e.eq_ignore_ascii_case("pak"))
            {
                if let Ok(r) = PakReader::new(&path) {
                    println!("📦 Loaded PAK: {}", path.display());
                    readers.push(r);
                }
            }
        }
    }
    if readers.is_empty() {
        println!("⚠️ WARNING: No .pak files found in 'data/pak/' directory!");
    } else {
        println!("✅ Loaded {} PAK files.", readers.len());
    }
    readers
}

fn build_asset_lists(assets: &GameAssets) -> AssetLists {
    let mut actions: Vec<String> = assets.meta.action_map_debug.keys().cloned().collect();
    actions.sort();

    let sort_keys = |map: &HashMap<String, PartData>| -> Vec<String> {
        let mut v: Vec<String> = map.keys().cloned().collect();
        v.sort();
        v
    };

    AssetLists {
        actions,
        male_bodies: sort_keys(&assets.male.body),
        male_heads: sort_keys(&assets.male.head),
        male_weapons: sort_keys(&assets.male.weapon_right),
        male_horses: sort_keys(&assets.male.horse_middle),

        female_bodies: sort_keys(&assets.female.body),
        female_heads: sort_keys(&assets.female.head),
        female_weapons: sort_keys(&assets.female.weapon_right),
        female_horses: sort_keys(&assets.female.horse_middle),

        npcs: sort_keys(&assets.npcs),
    }
}

// --- RELOAD LOGIC ---

fn reload_character(state: &mut AppState, assets: &GameAssets, readers: &mut [PakReader]) {
    state.loaded_parts.clear();

    let action = &state.lists.actions[state.action_idx];
    let mut parts_to_load: Vec<(&str, &PartData, &str)> = Vec::new();

    if state.char_type_idx == 2 {
        // NPC
        if let Some(id) = state.lists.npcs.get(state.npc_idx) {
            if let Some(data) = assets.npcs.get(id) {
                let act = if data.actions.contains_key(action) {
                    action.as_str()
                } else {
                    "stand"
                };
                parts_to_load.push(("npc_body", data, act));
            }
        }
    } else {
        // Player
        let is_male = state.char_type_idx == 0;
        let (p_body, p_head, p_wep, _p_horse) = if is_male {
            (
                &assets.male.body,
                &assets.male.head,
                &assets.male.weapon_right,
                &assets.male.horse_middle,
            )
        } else {
            (
                &assets.female.body,
                &assets.female.head,
                &assets.female.weapon_right,
                &assets.female.horse_middle,
            )
        };

        // Body
        let body_list = get_body_list(state);
        if let Some(id) = body_list.get(state.body_idx) {
            if let Some(data) = p_body.get(id) {
                if data.actions.contains_key(action) {
                    parts_to_load.push(("body", data, action.as_str()));
                }
            }
        }

        // Head
        let head_list = get_head_list(state);
        if let Some(id) = head_list.get(state.head_idx) {
            if let Some(data) = p_head.get(id) {
                if data.actions.contains_key(action) {
                    parts_to_load.push(("head", data, action.as_str()));
                }
            }
        }

        // Weapon
        let weapon_list = get_weapon_list(state);
        if let Some(id) = weapon_list.get(state.weapon_idx) {
            if let Some(data) = p_wep.get(id) {
                if data.actions.contains_key(action) {
                    parts_to_load.push(("weapon_right", data, action.as_str()));
                }
            }
        }
    }

    // LOAD FILES
    let mut counter: u32 = 0;
    for (p_name, data, act) in parts_to_load {
        if let Some(act_data) = data.actions.get(act) {
            let mut found = false;
            for reader in readers.iter_mut() {
                if let Some(entry) = reader.find_file(&act_data.full_path) {
                    found = true;
                    let entry_copy = *entry;
                    if let Ok(bytes) = reader.read_entry_data(&entry_copy) {
                        // Temp file workaround (SprFile::load cần file path)
                        counter += 1;
                        let tmp = format!("tmp_{}_{}.spr", p_name, counter);
                        fs::write(&tmp, &bytes).unwrap();
                        if let Ok(spr) = SprFile::load(&tmp) {
                            // Parse Info
                            let info_parts: Vec<&str> = act_data.info.split(',').collect();
                            let frames: usize =
                                info_parts.first().unwrap_or(&"0").parse().unwrap_or(0);
                            let dirs: usize =
                                info_parts.get(1).unwrap_or(&"0").parse().unwrap_or(0);
                            let interval: usize =
                                info_parts.get(2).unwrap_or(&"0").parse().unwrap_or(0);

                            // Chuyển tất cả frames thành Texture2D
                            let loaded_frames: Vec<LoadedFrame> = spr
                                .frames
                                .iter()
                                .map(|f| LoadedFrame {
                                    texture: spr_frame_to_texture(f, &spr.palette),
                                    offset_x: f.offset_x as f32,
                                    offset_y: f.offset_y as f32,
                                })
                                .collect();

                            state.loaded_parts.push(LoadedPart {
                                name: p_name.to_string(),
                                frames: loaded_frames,
                                info: (frames, dirs, interval),
                            });
                        }
                        let _ = fs::remove_file(tmp);
                        break;
                    }
                }
            }
            if !found {
                println!("❌ Missing file in PAKs: {}", act_data.full_path);
            }
        }
    }
}

// --- DRAWING ---

fn draw_character(state: &AppState, dir: usize, frame_tick: usize) {
    if state.loaded_parts.is_empty() {
        // Hiển thị thông báo khi chưa có gì
        draw_text(
            "No character loaded",
            CENTER_X - 100.0,
            CENTER_Y,
            24.0,
            Color::from_rgba(150, 150, 150, 255),
        );
        return;
    }

    // Z-Order
    let z_order = ["body", "npc_body", "head", "weapon_right"];

    let mut map: HashMap<&str, &LoadedPart> = HashMap::new();
    for p in &state.loaded_parts {
        map.insert(&p.name, p);
    }

    for name in z_order {
        if let Some(part) = map.get(name) {
            let (total_frames, dirs, _) = part.info;
            if total_frames == 0 || dirs == 0 {
                continue;
            }
            let fpd = total_frames / dirs;
            if fpd == 0 {
                continue;
            }
            let idx = (dir * fpd) + (frame_tick % fpd);

            if let Some(frame) = part.frames.get(idx) {
                let x = CENTER_X + frame.offset_x;
                let y = CENTER_Y + frame.offset_y;
                draw_texture(&frame.texture, x, y, WHITE);
            }
        }
    }
}

// --- UI DRAWING ---
fn draw_ui(state: &AppState) {
    let yellow = Color::from_rgba(255, 220, 50, 255);
    let white = Color::from_rgba(240, 240, 240, 255);
    let gray = Color::from_rgba(160, 160, 160, 255);
    let bg_color = Color::from_rgba(0, 0, 0, 180);
    let highlight_bg = Color::from_rgba(255, 200, 0, 40);

    let types = ["Male", "Female", "NPC"];
    let font_size = FONT_SIZE as f32;

    // Panel background
    let panel_x = 10.0;
    let panel_y = 10.0;
    let panel_w = 340.0;
    let panel_h = if state.char_type_idx == 2 {
        210.0
    } else {
        300.0
    };
    draw_rectangle(panel_x, panel_y, panel_w, panel_h, bg_color);
    draw_rectangle_lines(
        panel_x,
        panel_y,
        panel_w,
        panel_h,
        1.0,
        Color::from_rgba(80, 80, 80, 255),
    );

    let mut y = 35.0;
    let x = 25.0;

    // Title
    draw_text("JX VIEWER", x, y, font_size + 4.0, yellow);
    y += LINE_HEIGHT;
    draw_text("Up/Down: Select  |  Left/Right: Change", x, y, 14.0, gray);
    y += LINE_HEIGHT + 4.0;

    // --- Separator ---
    draw_line(
        panel_x + 5.0,
        y - 10.0,
        panel_x + panel_w - 5.0,
        y - 10.0,
        1.0,
        Color::from_rgba(80, 80, 80, 255),
    );

    // Menu Items
    let mut row: usize = 0;

    // Row 0: Type
    let label = format!("  << {} >>", types[state.char_type_idx]);
    draw_menu_row(
        x,
        y,
        panel_x,
        panel_w,
        &label,
        "Type:",
        row == state.selected_row,
        yellow,
        white,
        highlight_bg,
        font_size,
    );
    y += LINE_HEIGHT;
    row += 1;

    // Row 1: Action
    let action_name = state
        .lists
        .actions
        .get(state.action_idx)
        .map(|s| s.as_str())
        .unwrap_or("???");
    let label = format!("  << {} >>", action_name);
    draw_menu_row(
        x,
        y,
        panel_x,
        panel_w,
        &label,
        "Action:",
        row == state.selected_row,
        yellow,
        white,
        highlight_bg,
        font_size,
    );
    y += LINE_HEIGHT;
    row += 1;

    if state.char_type_idx == 2 {
        // NPC Menu
        let npc_id = state
            .lists
            .npcs
            .get(state.npc_idx)
            .map(|s| s.as_str())
            .unwrap_or("None");
        let label = format!("  << {} >>", npc_id);
        draw_menu_row(
            x,
            y,
            panel_x,
            panel_w,
            &label,
            "NPC:",
            row == state.selected_row,
            yellow,
            white,
            highlight_bg,
            font_size,
        );
    } else {
        // Player Menu
        let h_list = get_head_list(state);
        let b_list = get_body_list(state);
        let w_list = get_weapon_list(state);
        let horse_list = get_horse_list(state);

        let parts = [
            (
                "Head:",
                h_list
                    .get(state.head_idx)
                    .map(|s| s.as_str())
                    .unwrap_or("-"),
            ),
            (
                "Body:",
                b_list
                    .get(state.body_idx)
                    .map(|s| s.as_str())
                    .unwrap_or("-"),
            ),
            (
                "Weapon:",
                w_list
                    .get(state.weapon_idx)
                    .map(|s| s.as_str())
                    .unwrap_or("-"),
            ),
            (
                "Horse:",
                horse_list
                    .get(state.horse_idx)
                    .map(|s| s.as_str())
                    .unwrap_or("-"),
            ),
        ];

        for (i, (name, value)) in parts.iter().enumerate() {
            let label = format!("  << {} >>", value);
            draw_menu_row(
                x,
                y,
                panel_x,
                panel_w,
                &label,
                name,
                (row + i) == state.selected_row,
                yellow,
                white,
                highlight_bg,
                font_size,
            );
            y += LINE_HEIGHT;
        }
    }

    // Footer info
    let fps = get_fps();
    let info = format!("FPS: {} | Parts: {}", fps, state.loaded_parts.len());
    draw_text(&info, 10.0, WIN_HEIGHT - 10.0, 16.0, gray);
}

fn draw_menu_row(
    x: f32,
    y: f32,
    panel_x: f32,
    panel_w: f32,
    value: &str,
    label: &str,
    selected: bool,
    sel_color: Color,
    normal_color: Color,
    highlight_bg: Color,
    font_size: f32,
) {
    if selected {
        // Highlight background
        draw_rectangle(
            panel_x + 2.0,
            y - font_size + 4.0,
            panel_w - 4.0,
            LINE_HEIGHT,
            highlight_bg,
        );
        // Arrow indicator
        draw_text(">", x - 12.0, y, font_size, sel_color);
    }

    let color = if selected { sel_color } else { normal_color };
    draw_text(label, x, y, font_size, color);

    // Value (right-aligned)
    draw_text(value, x + 80.0, y, font_size, color);
}
