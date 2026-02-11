use serde_json::Value;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};

fn main() {
    let input_path = "data/newdata/npcres.json";
    let output_path = "data/unpak_list/npcres.txt";

    println!("📂 Đang đọc file: {}", input_path);

    // 1. Mở file JSON
    let file = match File::open(input_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!(
                "❌ Lỗi: Không tìm thấy file '{}'.\n   Chi tiết: {}",
                input_path, e
            );
            return;
        }
    };
    let reader = BufReader::new(file);

    // 2. Parse JSON (Dùng Value generic để không cần khai báo Struct phức tạp)
    let root: Value = match serde_json::from_reader(reader) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("❌ Lỗi: JSON không hợp lệ.\n   Chi tiết: {}", e);
            return;
        }
    };

    // 3. Quét tìm full_path
    let mut paths = HashSet::new(); // Dùng HashSet để tự động loại bỏ trùng lặp
    collect_full_paths(&root, &mut paths);

    println!("🔍 Tìm thấy {} đường dẫn file duy nhất.", paths.len());

    // 4. Ghi ra file text
    let out_file = File::create(output_path).expect("Không thể tạo file output");
    let mut writer = BufWriter::new(out_file);

    for path in paths {
        // Ghi từng dòng
        writeln!(writer, "{}", path).unwrap();
    }

    println!("✅ Đã xuất danh sách ra file: {}", output_path);
    println!(
        "👉 Bây giờ bạn có thể dùng lệnh: ./unpak -f {} -p .",
        output_path
    );
}

// Hàm đệ quy tìm key "full_path"
fn collect_full_paths(v: &Value, paths: &mut HashSet<String>) {
    match v {
        Value::Object(map) => {
            // Nếu Object này có key "full_path", lấy giá trị
            if let Some(Value::String(path)) = map.get("full_path") {
                if !path.trim().is_empty() {
                    paths.insert(path.clone());
                }
            }
            // Tiếp tục đệ quy vào các con của Object
            for (_, val) in map {
                collect_full_paths(val, paths);
            }
        }
        Value::Array(arr) => {
            // Đệ quy vào các phần tử của Array
            for val in arr {
                collect_full_paths(val, paths);
            }
        }
        _ => {} // Bỏ qua String, Number, Null, Bool ở cấp cao
    }
}
