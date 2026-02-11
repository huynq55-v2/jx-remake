use serde::Serialize;
use std::collections::HashMap;
use std::fs::{self};
use std::path::Path;

// --- JSON Structures ---

#[derive(Serialize, Default)]
struct GameAssets {
    meta: MetaData,
    male: PlayerParts,
    female: PlayerParts,
    npcs: HashMap<String, PartData>, // Thêm phần NPC
}

#[derive(Serialize, Default)]
struct MetaData {
    // Action Map để Client biết "stand" là cột nào (nếu cần debug)
    action_map_debug: HashMap<String, usize>,
}

#[derive(Serialize, Default)]
struct PlayerParts {
    // Phần cơ bản
    head: HashMap<String, PartData>, // 头部 - Đầu
    body: HashMap<String, PartData>, // 躯体 - Thân (Áo)

    // Phần chi tiết (Layering)
    hair: HashMap<String, PartData>, // 发型 - Tóc (Vẽ đè lên đầu hoặc sau đầu tùy mũ)
    shoulder: HashMap<String, PartData>, // 肩膀 - Vai (Giáp vai)
    hand_left: HashMap<String, PartData>, // 左手 - Tay trái (Cánh tay)
    hand_right: HashMap<String, PartData>, // 右手 - Tay phải (Cánh tay)

    // Vũ khí
    weapon_left: HashMap<String, PartData>, // 左手武器 - Vũ khí trái (Khiên/Song đao)
    weapon_right: HashMap<String, PartData>, // 右手武器 - Vũ khí phải (Kiếm/Đao/Thương...)

    // Ngựa (Chia 3 lớp để nhân vật ngồi ở giữa)
    horse_front: HashMap<String, PartData>,  // 马前 - Đầu ngựa
    horse_middle: HashMap<String, PartData>, // 马中 - Thân ngựa (Người ngồi lên đây)
    horse_back: HashMap<String, PartData>,   // 马后 - Đuôi ngựa
}

#[derive(Serialize)]
struct PartData {
    id: String, // ID có thể là số (0, 1) hoặc chuỗi (enemy003)
    original_name: String,
    root_path: String, // Lưu root path để tiện debug
    actions: HashMap<String, ActionData>,
}

#[derive(Serialize)]
struct ActionData {
    full_path: String, // Đường dẫn FULL: root + filename
    spr: String,       // Tên file gốc
    info: String,
}

fn main() {
    // Đường dẫn folder npcres
    let base_path = "data/settings/npcres";

    // Load Mapping
    let action_map = load_action_map();
    // Load Root Paths từ 人物类型.txt
    let root_paths = load_root_paths(format!("{}/人物类型.txt", base_path));

    let mut assets = GameAssets::default();
    assets.meta.action_map_debug = action_map.iter().map(|(k, v)| (v.clone(), *k)).collect();

    // ---------------------------------------------------------
    // 2. Xử lý Nhân vật NAM (Male) - Load đủ 11 bộ phận
    // ---------------------------------------------------------
    println!("⚙️ Processing Male Assets...");

    // Cơ thể & Đầu tóc
    assets.male.body = process_pair(base_path, "男主角躯体", &action_map, &root_paths);
    assets.male.head = process_pair(base_path, "男主角头部", &action_map, &root_paths);
    assets.male.hair = process_pair(base_path, "男主角发型", &action_map, &root_paths);
    assets.male.shoulder = process_pair(base_path, "男主角肩膀", &action_map, &root_paths);

    // Tay chân
    assets.male.hand_left = process_pair(base_path, "男主角左手", &action_map, &root_paths);
    assets.male.hand_right = process_pair(base_path, "男主角右手", &action_map, &root_paths);

    // Vũ khí
    assets.male.weapon_left = process_pair(base_path, "男主角左手武器", &action_map, &root_paths);
    assets.male.weapon_right = process_pair(base_path, "男主角右手武器", &action_map, &root_paths);

    // Ngựa (3 phần)
    assets.male.horse_front = process_pair(base_path, "男主角马前", &action_map, &root_paths);
    assets.male.horse_middle = process_pair(base_path, "男主角马中", &action_map, &root_paths);
    assets.male.horse_back = process_pair(base_path, "男主角马后", &action_map, &root_paths);

    // ---------------------------------------------------------
    // 3. Xử lý Nhân vật NỮ (Female) - Load đủ 11 bộ phận
    // ---------------------------------------------------------
    println!("⚙️ Processing Female Assets...");
    // Cơ thể & Đầu tóc
    assets.female.body = process_pair(base_path, "女主角躯体", &action_map, &root_paths);
    assets.female.head = process_pair(base_path, "女主角头部", &action_map, &root_paths);
    assets.female.hair = process_pair(base_path, "女主角发型", &action_map, &root_paths);
    assets.female.shoulder = process_pair(base_path, "女主角肩膀", &action_map, &root_paths);

    // Tay chân
    assets.female.hand_left = process_pair(base_path, "女主角左手", &action_map, &root_paths);
    assets.female.hand_right = process_pair(base_path, "女主角右手", &action_map, &root_paths);

    // Vũ khí
    assets.female.weapon_left = process_pair(base_path, "女主角左手武器", &action_map, &root_paths);
    assets.female.weapon_right =
        process_pair(base_path, "女主角右手武器", &action_map, &root_paths);

    // Ngựa (3 phần)
    assets.female.horse_front = process_pair(base_path, "女主角马前", &action_map, &root_paths);
    assets.female.horse_middle = process_pair(base_path, "女主角马中", &action_map, &root_paths);
    assets.female.horse_back = process_pair(base_path, "女主角马后", &action_map, &root_paths);

    // ---------------------------------------------------------
    // 4. Xử lý NPC & Quái vật
    // ---------------------------------------------------------
    println!("⚙️ Processing NPC Assets...");
    assets.npcs = process_pair(base_path, "普通npc资源", &action_map, &root_paths);

    // 5. Save
    let output_path = "data/newdata/assets_full.json";

    // Tạo thư mục nếu chưa có
    if let Some(parent) = Path::new(output_path).parent() {
        fs::create_dir_all(parent).unwrap_or_default();
    }

    let json_str = serde_json::to_string_pretty(&assets).unwrap();
    fs::write(output_path, json_str).expect("Unable to write file");

    println!(
        "✅ Hoàn tất! File data siêu to khổng lồ đã sẵn sàng tại: {}",
        output_path
    );
}

// --- Hàm đọc file GBK ---
fn read_utf8_file<P: AsRef<Path>>(path: P) -> String {
    // Đọc toàn bộ nội dung file thành String (mặc định Rust coi là UTF-8)
    fs::read_to_string(path).expect("Không thể đọc file hoặc file không phải chuẩn UTF-8")
}

// Load CharacterType.txt -> HashMap<String, String>
// Key: "男主角头部" -> Value: "spr/npcres/man"
fn load_root_paths<P: AsRef<Path>>(path: P) -> HashMap<String, String> {
    let content = read_utf8_file(path);
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .has_headers(false) // File này header tiếng Trung thường ở dòng 1 nhưng csv reader skip lỗi
        .from_reader(content.as_bytes());

    let mut map = HashMap::new();

    for result in rdr.records() {
        if let Ok(record) = result {
            // Cột 0: Code Name (ví dụ: enemy003, 男主角头部)
            // Cột 2: Path (ví dụ: \spr\npcres\enemy\enemy003)
            if record.len() >= 3 {
                let code = record[0].trim().to_string();
                let raw_path = record[2].trim().to_string();

                // Chuẩn hóa path: Xóa dấu \ ở cuối, thay \ thành /
                let clean_path = raw_path
                    .replace("\\", "/")
                    .trim_end_matches('/')
                    .to_string();

                if !code.is_empty() && !clean_path.is_empty() {
                    map.insert(code, clean_path);
                }
            }
        }
    }
    map
}

fn load_action_map() -> HashMap<usize, String> {
    // Mapping chuẩn dựa trên '动作编号表.txt'
    // Cần kiểm tra kỹ file gốc để map đúng index cột
    let mut map = HashMap::new();
    // Lưu ý: Cột trong file txt thường bắt đầu chứa data SPR từ cột số 1 hoặc 2
    // Giả sử cột 0 là Tên, thì:
    map.insert(0, "stand".to_string()); // Cột 1
    map.insert(1, "walk".to_string()); // Cột 2
    map.insert(2, "run".to_string());
    map.insert(3, "fight_run".to_string()); // Chiến đấu chạy
    map.insert(4, "fight_stand".to_string()); // Chiến đấu đứng
    map.insert(5, "attack1".to_string());
    map.insert(6, "attack2".to_string());
    map.insert(7, "magic".to_string());
    map.insert(8, "sit".to_string());
    map.insert(9, "die".to_string());
    // ... map tiếp tùy file action
    map
}

// --- Hàm "Zip" thần thánh ---
// Hàm xử lý cặp file Res + Info
fn process_pair(
    base_dir: &str,
    file_prefix: &str, // Ví dụ: "男主角躯体" hoặc "普通npc资源"
    action_map: &HashMap<usize, String>,
    root_map: &HashMap<String, String>, // Bảng tra cứu đường dẫn gốc
) -> HashMap<String, PartData> {
    let res_path = format!("{}/{}.txt", base_dir, file_prefix);
    let info_path = format!("{}/{}信息.txt", base_dir, file_prefix);

    // Check file exists
    if !Path::new(&res_path).exists() {
        println!("⚠️ Skip missing: {}", file_prefix);
        return HashMap::new();
    }

    let res_content = read_utf8_file(&res_path);
    let info_content = read_utf8_file(&info_path);

    let mut res_rdr = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(res_content.as_bytes());
    let mut info_rdr = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(info_content.as_bytes());

    let mut res_iter = res_rdr.records();
    let mut info_iter = info_rdr.records();

    let mut result = HashMap::new();

    // SỬA TẠI ĐÂY: Xác định Key tổng quát để tra cứu root path
    // Nếu prefix chứa "男主角" -> dùng key "男主角"
    // Nếu prefix chứa "女主角" -> dùng key "女主角"
    // Nếu là "普通npc资源" -> mỗi dòng sẽ có root riêng (xử lý trong vòng lặp)
    let lookup_key = if file_prefix.contains("男主角") {
        "男主角"
    } else if file_prefix.contains("女主角") {
        "女主角"
    } else {
        file_prefix // Trường hợp khác giữ nguyên
    };

    // Xác định Root Path mặc định cho file này (nếu có)
    // Ví dụ file "男主角躯体" -> Root là "\spr\npcres\man"
    let default_root = root_map.get(lookup_key).cloned().unwrap_or_default();

    loop {
        match (res_iter.next(), info_iter.next()) {
            (Some(Ok(res_rec)), Some(Ok(info_rec))) => {
                let code_name = res_rec.get(0).unwrap_or("Unknown").trim().to_string();

                // Logic cho NPC: Nếu là file "普通npc资源", root path nằm ở CharacterType theo từng NPC ID
                let current_root = if file_prefix == "普通npc资源" {
                    root_map.get(&code_name).cloned().unwrap_or_default()
                } else {
                    default_root.clone()
                };

                let mut actions = HashMap::new();

                for (col_idx, action_name) in action_map {
                    let data_idx = col_idx + 1;
                    if data_idx < res_rec.len() && data_idx < info_rec.len() {
                        let spr_name = res_rec[data_idx].trim().to_string();
                        let info_data = info_rec[data_idx].trim().to_string();

                        if !spr_name.is_empty() && spr_name != "-" {
                            // Chuẩn hóa đường dẫn
                            let clean_spr = spr_name.replace("/", "\\");
                            let mut clean_root = current_root.replace("/", "\\");

                            if !clean_root.starts_with('\\') && !clean_root.is_empty() {
                                clean_root = format!("\\{}", clean_root);
                            }
                            let clean_root = clean_root.trim_end_matches('\\');

                            // Ghép full path
                            let full_path = if clean_root.is_empty() {
                                if clean_spr.starts_with('\\') {
                                    clean_spr.clone()
                                } else {
                                    format!("\\{}", clean_spr)
                                }
                            } else {
                                format!("{}\\{}", clean_root, clean_spr)
                            };

                            actions.insert(
                                action_name.clone(),
                                ActionData {
                                    full_path,
                                    spr: clean_spr,
                                    info: info_data,
                                },
                            );
                        }
                    }
                }

                if !actions.is_empty() {
                    result.insert(
                        code_name.clone(),
                        PartData {
                            id: code_name,
                            original_name: String::new(),
                            root_path: current_root, // Bây giờ sẽ có giá trị: \spr\npcres\man
                            actions,
                        },
                    );
                }
            }
            _ => break,
        }
    }
    result
}
