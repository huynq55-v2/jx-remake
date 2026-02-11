use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufReader;
use std::time::{Duration, Instant};

use minifb::{Key, Window, WindowOptions};
use rand::RngCore; // Import RngCore để dùng next_u32
use serde::Deserialize;

use jx_remake::client::spr::SprFile;
use jx_remake::common::pak::PakReader;

// --- CẤU HÌNH MÀN HÌNH ---
const WIN_WIDTH: usize = 1024;
const WIN_HEIGHT: usize = 768;
const CENTER_X: i32 = (WIN_WIDTH / 2) as i32 + 100; // Dịch nhân vật sang phải để nhường chỗ cho Menu
const CENTER_Y: i32 = (WIN_HEIGHT / 2) as i32;

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
    hair: HashMap<String, PartData>,
    shoulder: HashMap<String, PartData>,
    hand_left: HashMap<String, PartData>,
    hand_right: HashMap<String, PartData>,
    weapon_left: HashMap<String, PartData>,
    weapon_right: HashMap<String, PartData>,
    horse_front: HashMap<String, PartData>,
    horse_middle: HashMap<String, PartData>,
    horse_back: HashMap<String, PartData>,
}

#[derive(Deserialize, Debug, Clone)]
struct PartData {
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
    selected_row: usize, // Dòng menu đang chọn

    // Cache Lists (Sorted IDs để duyệt Left/Right)
    lists: AssetLists,

    // Render Data
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

struct LoadedPart {
    name: String,
    spr: SprFile,
    info: (usize, usize, usize), // Frames, Dirs, Interval
}

// --- MAIN ---

fn main() {
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
        horse_idx: 0, // 0 = No Horse
        npc_idx: 0,
        selected_row: 0,
        lists,
        loaded_parts: Vec::new(),
    };

    // Load nhân vật lần đầu
    reload_character(&mut state, &assets, &mut pak_readers);

    // 2. Window
    let mut window = Window::new(
        "JX Viewer - Arrows to Control - ESC to Exit",
        WIN_WIDTH,
        WIN_HEIGHT,
        WindowOptions::default(),
    )
    .unwrap();
    window.limit_update_rate(Some(Duration::from_micros(16600)));

    let mut buffer: Vec<u32> = vec![0; WIN_WIDTH * WIN_HEIGHT];
    let mut last_tick = Instant::now();
    let mut current_frame = 0;
    let mut accum_time = 0;
    let dir = 0; // Hướng nhìn (0-7)

    while window.is_open() && !window.is_key_down(Key::Escape) {
        // --- LOGIC INPUT ---
        let mut changed = false;

        // Navigation (Up/Down)
        if window.is_key_pressed(Key::Up, minifb::KeyRepeat::Yes) {
            if state.selected_row > 0 {
                state.selected_row -= 1;
            }
        }
        if window.is_key_pressed(Key::Down, minifb::KeyRepeat::Yes) {
            let max_row = if state.char_type_idx == 2 { 2 } else { 5 }; // NPC menu it dong hon
            if state.selected_row < max_row {
                state.selected_row += 1;
            }
        }

        // Modification (Left/Right)
        let delta: i32 = if window.is_key_pressed(Key::Left, minifb::KeyRepeat::Yes) {
            -1
        } else if window.is_key_pressed(Key::Right, minifb::KeyRepeat::Yes) {
            1
        } else {
            0
        };

        if delta != 0 {
            changed = true;
            match state.selected_row {
                0 => {
                    // TYPE
                    state.char_type_idx = wrap_idx(state.char_type_idx, delta, 3);
                }
                1 => {
                    // ACTION
                    state.action_idx = wrap_idx(state.action_idx, delta, state.lists.actions.len());
                }
                _ => {
                    if state.char_type_idx == 2 {
                        // NPC MODE
                        if state.selected_row == 2 {
                            // NPC ID
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
            current_frame = 0; // Reset animation
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

        if accum_time >= (interval * 55) {
            current_frame += 1;
            accum_time = 0;
        }

        // --- RENDER ---
        // 1. Clear Screen
        for p in buffer.iter_mut() {
            *p = 0xFF202020;
        } // Dark Grey

        // 2. Draw Character
        draw_character(&mut buffer, &state, dir, current_frame);

        // 3. Draw UI
        draw_ui(&mut buffer, &state);

        window
            .update_with_buffer(&buffer, WIN_WIDTH, WIN_HEIGHT)
            .unwrap();
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
        println!("⚠️ WARNING: No .pak files found in 'data/' directory!");
        println!("   The demo needs PAK files (e.g. settings.pak, spr.pak) to load assets.");
    } else {
        println!("✅ Loaded {} PAK files.", readers.len());
    }
    readers
}

fn build_asset_lists(assets: &GameAssets) -> AssetLists {
    let mut actions: Vec<String> = assets.meta.action_map_debug.keys().cloned().collect();
    actions.sort(); // Sort alpha
    // Ensure "run" or "stand" is prioritized if needed, but sort is fine.

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
    let mut parts_to_load = Vec::new();

    if state.char_type_idx == 2 {
        // NPC
        if let Some(id) = state.lists.npcs.get(state.npc_idx) {
            if let Some(data) = assets.npcs.get(id) {
                // NPC fallback action logic
                let act = if data.actions.contains_key(action) {
                    action
                } else {
                    "stand"
                };
                parts_to_load.push(("npc_body", data, act));
            }
        }
    } else {
        // Player
        let is_male = state.char_type_idx == 0;
        let (p_body, p_head, p_wep, p_horse) = if is_male {
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

        // Helper
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

        // Horse (Multi-part) logic omitted for brevity, adding just body if horse_idx > 0
        // Trong thực tế cần load horse_front, horse_back từ cùng ID horse_middle
    }

    // LOAD FILES
    let mut rng = rand::thread_rng();
    for (p_name, data, act) in parts_to_load {
        if let Some(act_data) = data.actions.get(act) {
            let mut found = false;
            for reader in readers.iter_mut() {
                if let Some(entry) = reader.find_file(&act_data.full_path) {
                    found = true;
                    // println!("   Found {} in {}", act_data.full_path, reader.file_path);
                    let entry_copy = *entry;
                    if let Ok(bytes) = reader.read_entry_data(&entry_copy) {
                        // Temp file fix
                        let tmp = format!("tmp_{}_{}.spr", p_name, rng.next_u32());
                        fs::write(&tmp, &bytes).unwrap();
                        if let Ok(spr) = SprFile::load(&tmp) {
                            // Parse Info
                            let info_parts: Vec<&str> = act_data.info.split(',').collect();
                            let frames = info_parts.get(0).unwrap_or(&"0").parse().unwrap_or(0);
                            let dirs = info_parts.get(1).unwrap_or(&"0").parse().unwrap_or(0);
                            let interval = info_parts.get(2).unwrap_or(&"0").parse().unwrap_or(0);

                            state.loaded_parts.push(LoadedPart {
                                name: p_name.to_string(),
                                spr,
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

fn draw_character(buffer: &mut [u32], state: &AppState, dir: usize, frame_tick: usize) {
    if state.loaded_parts.is_empty() {
        return;
    }

    // Z-Order Simple
    let z_order = vec!["body", "npc_body", "head", "weapon_right"];

    let mut map: HashMap<&str, &LoadedPart> = HashMap::new();
    for p in &state.loaded_parts {
        map.insert(&p.name, p);
    }

    for name in z_order {
        if let Some(part) = map.get(name) {
            let (frames, dirs, _) = part.info;
            if frames == 0 || dirs == 0 {
                continue;
            }
            let fpd = frames / dirs;
            let idx = (dir * fpd) + (frame_tick % fpd);

            if let Some(frame) = part.spr.frames.get(idx) {
                let w = frame.width as i32;
                let h = frame.height as i32;
                let off_x = CENTER_X + frame.offset_x as i32;
                let off_y = CENTER_Y + frame.offset_y as i32;

                for y in 0..h {
                    for x in 0..w {
                        let sx = off_x + x;
                        let sy = off_y + y;
                        if sx < 0 || sx >= WIN_WIDTH as i32 || sy < 0 || sy >= WIN_HEIGHT as i32 {
                            continue;
                        }

                        let pi = (y * w + x) as usize;
                        if pi < frame.decoded_indices.len() {
                            let alpha = frame.alpha_map[pi];
                            if alpha > 0 {
                                let c = part.spr.palette[frame.decoded_indices[pi] as usize];
                                // Draw pixel (No blending for perf)
                                buffer[(sy as usize) * WIN_WIDTH + (sx as usize)] =
                                    ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32);
                            }
                        }
                    }
                }
            }
        }
    }
}

// --- UI DRAWING (SIMPLE BITMAP FONT) ---
fn draw_ui(buffer: &mut [u32], state: &AppState) {
    let yellow = 0xFFFF00;
    let white = 0xFFFFFF;
    let gray = 0xAAAAAA;

    let types = vec!["Male", "Female", "NPC"];

    // Helper draw text
    let mut y = 20;
    let x = 20;

    draw_string(buffer, x, y, "JX VIEWER CONTROLS:", yellow);
    y += 15;
    draw_string(buffer, x, y, "Up/Down : Select Item", gray);
    y += 10;
    draw_string(buffer, x, y, "Left/Right: Change Value", gray);
    y += 20;

    // Menu Items
    let items = vec![
        format!("Type:   <{}>", types[state.char_type_idx]),
        format!(
            "Action: <{}>",
            state
                .lists
                .actions
                .get(state.action_idx)
                .unwrap_or(&"???".to_string())
        ),
    ];

    // Common items
    for (i, text) in items.iter().enumerate() {
        let color = if state.selected_row == i {
            yellow
        } else {
            white
        };
        draw_string(buffer, x, y, text, color);
        y += 15;
    }

    if state.char_type_idx == 2 {
        // NPC Menu
        let npc_id = state
            .lists
            .npcs
            .get(state.npc_idx)
            .map(|s| s.as_str())
            .unwrap_or("None");
        let color = if state.selected_row == 2 {
            yellow
        } else {
            white
        };
        draw_string(buffer, x, y, &format!("NPC ID: <{}>", npc_id), color);
    } else {
        // Player Menu
        let h_list = get_head_list(state);
        let b_list = get_body_list(state);
        let w_list = get_weapon_list(state);
        let horse_list = get_horse_list(state);

        let p_items = vec![
            format!(
                "Head:   <{}>",
                h_list.get(state.head_idx).unwrap_or(&"-".to_string())
            ),
            format!(
                "Body:   <{}>",
                b_list.get(state.body_idx).unwrap_or(&"-".to_string())
            ),
            format!(
                "Weapon: <{}>",
                w_list.get(state.weapon_idx).unwrap_or(&"-".to_string())
            ),
            format!(
                "Horse:  <{}>",
                horse_list.get(state.horse_idx).unwrap_or(&"-".to_string())
            ),
        ];

        for (i, text) in p_items.iter().enumerate() {
            let color = if state.selected_row == (i + 2) {
                yellow
            } else {
                white
            };
            draw_string(buffer, x, y, text, color);
            y += 15;
        }
    }
}

// Minimal 5x7 ASCII Font Bitmap (Hardcoded A-Z, 0-9 for demo)
// Để tiết kiệm dòng code, mình dùng thuật toán vẽ pixel đơn giản.
// Thực tế bạn nên load file font bitmap. Ở đây mình vẽ ô vuông đại diện text.
fn draw_string(buffer: &mut [u32], x: usize, y: usize, text: &str, color: u32) {
    // Demo: Chỉ vẽ các ký tự cơ bản bằng các chấm pixel nếu không có font
    // Vì không thể paste 200 dòng hex font vào đây, mình dùng minifb_fonts nếu có,
    // hoặc vẽ text "giả" (placeholder).

    // TẠM THỜI: Dùng thư viện có sẵn hoặc vẽ pixel art đơn giản
    // Để code chạy được ngay mà không cần file font:
    let mut cursor_x = x;
    for c in text.chars() {
        if c == ' ' {
            cursor_x += 6;
            continue;
        }
        // Vẽ 1 ô vuông đại diện ký tự (Thay bằng font thật sau)
        for dy in 0..7 {
            for dx in 0..5 {
                if should_draw_pixel(c, dx, dy) {
                    let px = cursor_x + dx;
                    let py = y + dy;
                    if px < WIN_WIDTH && py < WIN_HEIGHT {
                        buffer[py * WIN_WIDTH + px] = color;
                    }
                }
            }
        }
        cursor_x += 7;
    }
}

// Hàm giả lập font (Rất cơ bản)
fn should_draw_pixel(_c: char, dx: usize, dy: usize) -> bool {
    // Vẽ khung chữ nhật rỗng 5x7
    dx == 0 || dx == 4 || dy == 0 || dy == 6
}
