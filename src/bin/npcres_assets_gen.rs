use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::Path;

// Cấu hình đường dẫn
const INPUT_BASE_DIR: &str = "data/settings/npcres";
const OUTPUT_FILE: &str = "data/newdata/npcres_full.json";

fn main() -> io::Result<()> {
    println!("Log: Bắt đầu xử lý từ thư mục '{}'...", INPUT_BASE_DIR);

    // 1. Load bảng gốc
    let char_rows = parse_tab_file_dynamic("人物类型.txt")?;
    let mut json_root = Map::new();

    for row in char_rows {
        let char_name = match row.get("CharacterName") {
            Some(name) if !name.is_empty() => name.clone(),
            _ => continue,
        };

        // Lấy root_path và chuẩn hóa
        let raw_root = row
            .get("ResFilePath")
            .unwrap_or(&String::new())
            .replace("\\", "/");
        let char_type = row.get("CharacterType").map(|s| s.as_str()).unwrap_or("");

        let mut char_data = Map::new();
        char_data.insert("type".to_string(), json!(char_type));
        char_data.insert("root_path".to_string(), json!(&raw_root));

        if char_type == "SpecialNpc" {
            // --- XỬ LÝ SPECIAL NPC ---

            // A. Weapon Logic (Deep Parse Mount & Unmount Matrix)
            // Thay vì chỉ lưu tên file, ta đọc luôn nội dung bảng mapping vào JSON
            let mut weapon_logic = Map::new();

            // 1. Unmounted (Đi bộ)
            if let Some(filename) = row.get("WeaponActionTab1") {
                if !filename.trim().is_empty() {
                    match parse_weapon_matrix(filename) {
                        Ok(data) => {
                            weapon_logic.insert("unmounted".to_string(), Value::Object(data));
                        }
                        Err(e) => println!("Warning: Lỗi đọc Unmounted Logic {} - {}", filename, e),
                    }
                }
            }

            // 2. Mounted (Cưỡi ngựa)
            if let Some(filename) = row.get("WeaponActionTab2") {
                if !filename.trim().is_empty() {
                    match parse_weapon_matrix(filename) {
                        Ok(data) => {
                            weapon_logic.insert("mounted".to_string(), Value::Object(data));
                        }
                        Err(e) => println!("Warning: Lỗi đọc Mounted Logic {} - {}", filename, e),
                    }
                }
            }

            if !weapon_logic.is_empty() {
                char_data.insert("weapon_logic".to_string(), Value::Object(weapon_logic));
            }

            // B. Render Order (Deep Parse)
            if let Some(filename) = row.get("ActionRenderOrderTab") {
                if !filename.trim().is_empty() {
                    match parse_render_order(filename) {
                        Ok(data) => {
                            char_data.insert("render_order".to_string(), Value::Object(data));
                        }
                        Err(e) => println!("Warning: Lỗi đọc RenderOrder {} - {}", filename, e),
                    }
                }
            }

            // C. Components (Deep Parse & Full Path Generation)
            let component_cols = vec![
                "Head",
                "Hair",
                "Shoulder",
                "Body",
                "LeftHand",
                "RightHead",
                "LeftWeapon",
                "RightWeapon",
                "HorseFront",
                "HorseMiddle",
                "HorseBack",
            ];

            let mut components_data = Map::new();

            for col in component_cols {
                if let Some(filename) = row.get(col) {
                    if !filename.trim().is_empty() {
                        match parse_component_config(filename, &raw_root) {
                            Ok(data) => {
                                components_data.insert(col.to_lowercase(), Value::Object(data));
                            }
                            Err(e) => println!("Warning: Lỗi đọc Component {} - {}", filename, e),
                        }
                    }
                }
            }

            if !components_data.is_empty() {
                char_data.insert("components".to_string(), Value::Object(components_data));
            }
        } else if char_type == "NormalNpc" {
            // Logic NormalNPC (Giữ nguyên hoặc mở rộng)
        }

        json_root.insert(char_name, Value::Object(char_data));
    }

    // Xuất file
    let path = Path::new(OUTPUT_FILE);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let final_json = serde_json::to_string_pretty(&json_root)?;
    fs::write(path, final_json)?;

    println!(
        "Success: Toàn bộ dữ liệu (bao gồm Logic Ngựa) đã lưu tại '{}'",
        OUTPUT_FILE
    );
    Ok(())
}

// --- CÁC HÀM HELPER ---

// 1. Đọc file Weapon Matrix (Bảng liên kết hành động)
// Input: Tên file (ví dụ: 男主角骑马关联表.txt)
// Output: Map<WeaponName, Map<GeneralAction, ActionID>>
fn parse_weapon_matrix(filename: &str) -> io::Result<Map<String, Value>> {
    let full_path = format!("{}/{}", INPUT_BASE_DIR, filename);
    let file = File::open(&full_path)?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    // Dòng 1: Header (EqName, FightStand, Attack1...)
    let header_line = match lines.next() {
        Some(Ok(s)) => s,
        _ => return Ok(Map::new()),
    };

    // Tách header, bỏ cột đầu tiên (EqName)
    let headers: Vec<String> = header_line
        .split('\t')
        .skip(1)
        .map(|s| s.trim().to_string())
        .collect();

    let mut matrix = Map::new();

    for line in lines {
        let s = line?;
        if s.trim().is_empty() {
            continue;
        }

        let parts: Vec<&str> = s.split('\t').collect();
        if parts.is_empty() {
            continue;
        }

        let weapon_name = parts[0].trim().to_string(); // "Tay không", "Kiếm 1"...
        let mut action_map = Map::new();

        for (i, col_name) in headers.iter().enumerate() {
            // Giá trị bắt đầu từ index 1
            if i + 1 < parts.len() {
                let action_id = parts[i + 1].trim();
                if !action_id.is_empty() {
                    action_map.insert(col_name.clone(), json!(action_id));
                }
            }
        }
        matrix.insert(weapon_name, Value::Object(action_map));
    }
    Ok(matrix)
}

// 2. Đọc file Component và ghép Full Path
fn parse_component_config(filename: &str, root_path: &str) -> io::Result<Map<String, Value>> {
    let full_path = format!("{}/{}", INPUT_BASE_DIR, filename);
    let file = File::open(&full_path)?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let header_line = match lines.next() {
        Some(Ok(s)) => s,
        _ => return Ok(Map::new()),
    };

    let headers: Vec<String> = header_line
        .split('\t')
        .skip(1)
        .map(|s| s.trim().to_string())
        .collect();
    let mut items_map = Map::new();

    for line in lines {
        let s = line?;
        if s.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = s.split('\t').collect();
        if parts.is_empty() {
            continue;
        }

        let item_id = parts[0].trim().to_string();
        let mut actions = Map::new();

        for (i, header) in headers.iter().enumerate() {
            if i + 1 < parts.len() {
                let spr_name = parts[i + 1].trim();
                if !spr_name.is_empty() {
                    let full_spr_path = format!("/{}/{}", root_path, spr_name);
                    actions.insert(header.clone(), json!(full_spr_path));
                }
            }
        }
        items_map.insert(item_id, Value::Object(actions));
    }
    Ok(items_map)
}

// 3. Đọc file Render Order (INI style)
fn parse_render_order(filename: &str) -> io::Result<Map<String, Value>> {
    let full_path = format!("{}/{}", INPUT_BASE_DIR, filename);
    let file = File::open(&full_path)?;
    let reader = BufReader::new(file);

    let mut sections = Map::new();
    let mut current_section_name = String::new();
    let mut current_section_data = Map::new();

    for line in reader.lines() {
        let s = line?;
        let trimmed = s.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if !current_section_name.is_empty() {
                sections.insert(
                    current_section_name.clone(),
                    Value::Object(current_section_data),
                );
                current_section_data = Map::new();
            }
            current_section_name = trimmed[1..trimmed.len() - 1].to_string();
        } else if let Some(idx) = trimmed.find('=') {
            let key = trimmed[..idx].trim().to_string();
            let val = trimmed[idx + 1..].trim().to_string();
            current_section_data.insert(key, json!(val));
        }
    }
    if !current_section_name.is_empty() {
        sections.insert(current_section_name, Value::Object(current_section_data));
    }
    Ok(sections)
}

// 4. Đọc file Tab cơ bản
fn parse_tab_file_dynamic(filename: &str) -> io::Result<Vec<HashMap<String, String>>> {
    let full_path = format!("{}/{}", INPUT_BASE_DIR, filename);
    let file = File::open(&full_path)?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let header_line = lines
        .next()
        .ok_or(io::Error::new(io::ErrorKind::InvalidData, "Empty"))??;
    let headers: Vec<String> = header_line
        .split('\t')
        .map(|s| s.trim().to_string())
        .collect();

    let mut result = Vec::new();
    for line in lines {
        let s = line?;
        if s.trim().is_empty() {
            continue;
        }
        let values: Vec<&str> = s.split('\t').collect();
        let mut map = HashMap::new();
        for (i, header) in headers.iter().enumerate() {
            if i < values.len() {
                map.insert(header.clone(), values[i].trim().to_string());
            }
        }
        result.push(map);
    }
    Ok(result)
}
