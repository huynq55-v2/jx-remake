use encoding_rs::GBK;
use serde::Serialize;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
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
    // Đổi u32 thành String cho tất cả các bộ phận
    head: HashMap<String, PartData>,
    body: HashMap<String, PartData>,
    weapon_left: HashMap<String, PartData>,
    weapon_right: HashMap<String, PartData>,
    horse: HashMap<String, PartData>,
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
    // Đường dẫn folder npcres (Bạn sửa lại cho đúng máy bạn)
    let base_path = "data/settings/npcres";

    let action_map = load_action_map(); // Load cứng hoặc đọc file

    // QUAN TRỌNG: Load bảng đường dẫn gốc từ "人物类型.txt"
    let root_paths = load_root_paths(format!("{}/人物类型.txt", base_path));

    let mut assets = GameAssets::default();
    assets.meta.action_map_debug = action_map.iter().map(|(k, v)| (v.clone(), *k)).collect();

    // 2. Xử lý nhân vật Nam (Male) - "Zip" các cặp file
    println!("⚙️ Processing Male Assets...");
    assets.male.body = process_pair(base_path, "男主角躯体", &action_map, &root_paths);
    assets.male.head = process_pair(base_path, "男主角头部", &action_map, &root_paths);
    assets.male.weapon_right = process_pair(base_path, "男主角右手武器", &action_map, &root_paths);
    // ... làm tiếp cho các bộ phận khác

    // 3. Xử lý nhân vật Nữ (Female)
    println!("⚙️ Processing Female Assets...");
    assets.female.body = process_pair(base_path, "女主角躯体", &action_map, &root_paths);
    // ...

    // 4. Xử lý NPC & Quái vật (NEW)
    // File: 普通npc资源.txt (NormalNpcRes.txt)
    println!("⚙️ Processing NPC Assets...");
    assets.npcs = process_pair(base_path, "普通npc资源", &action_map, &root_paths);

    // 5. Save
    let json_str = serde_json::to_string_pretty(&assets).unwrap();
    fs::write("assets_full.json", json_str).expect("Unable to write file");
    println!("✅ Done! Check assets_full.json");
}

// --- Hàm đọc file GBK ---
fn read_gbk_file<P: AsRef<Path>>(path: P) -> String {
    let mut file = File::open(path).expect("File not found");
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).expect("Read error");
    let (cow, _encoding_used, _had_errors) = GBK.decode(&buffer);
    cow.into_owned()
}

// Load CharacterType.txt -> HashMap<String, String>
// Key: "男主角头部" -> Value: "spr/npcres/man"
fn load_root_paths<P: AsRef<Path>>(path: P) -> HashMap<String, String> {
    let content = read_gbk_file(path);
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

    let res_content = read_gbk_file(&res_path);
    let info_content = read_gbk_file(&info_path);

    let mut res_rdr = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(res_content.as_bytes());
    let mut info_rdr = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .from_reader(info_content.as_bytes());

    let mut res_iter = res_rdr.records();
    let mut info_iter = info_rdr.records();

    let mut result = HashMap::new();

    // Xác định Root Path mặc định cho file này (nếu có)
    // Ví dụ file "男主角躯体" -> Root là "\spr\npcres\man"
    let default_root = root_map.get(file_prefix).cloned().unwrap_or_default();

    loop {
        match (res_iter.next(), info_iter.next()) {
            (Some(Ok(res_rec)), Some(Ok(info_rec))) => {
                // Cột 0 là tên định danh (ví dụ: "enemy003" hoặc "躯体01")
                let code_name = res_rec.get(0).unwrap_or("Unknown").trim().to_string();

                // QUAN TRỌNG: Tìm root path cho dòng này.
                // - Với Player: Root nằm ở default_root ("\spr\npcres\man")
                // - Với NPC: Root path phụ thuộc vào từng con NPC (enemy003 có path riêng, enemy004 path riêng)
                // => Logic: Thử tra code_name trong root_map trước (cho NPC), nếu ko có thì dùng default_root (cho Player)
                let current_root = root_map
                    .get(&code_name)
                    .cloned()
                    .unwrap_or_else(|| default_root.clone());

                let mut actions = HashMap::new();

                for (col_idx, action_name) in action_map {
                    // +1 vì cột 0 là Name
                    let data_idx = col_idx + 1;

                    if data_idx < res_rec.len() && data_idx < info_rec.len() {
                        let spr_name = res_rec[data_idx].trim().to_string();
                        let info_data = info_rec[data_idx].trim().to_string();

                        if !spr_name.is_empty() && spr_name != "-" {
                            let clean_spr = spr_name.replace("\\", "/");

                            // GHÉP ĐƯỜNG DẪN: root + / + filename
                            let full_path = if current_root.is_empty() {
                                clean_spr.clone()
                            } else {
                                format!("{}/{}", current_root, clean_spr)
                            };

                            actions.insert(
                                action_name.clone(),
                                ActionData {
                                    full_path, // Đây là cái bạn cần!
                                    spr: clean_spr,
                                    info: info_data,
                                },
                            );
                        }
                    }
                }

                // Nếu có data hành động thì mới add vào list
                if !actions.is_empty() {
                    // Dùng code_name làm KEY của HashMap
                    result.insert(
                        code_name.clone(),
                        PartData {
                            id: code_name,
                            original_name: String::new(),
                            root_path: current_root,
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
