use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::Path;

// Cấu hình đường dẫn
const INPUT_PATH: &str = "data/newdata/npcres.json";
const OUTPUT_PATH: &str = "data/unpak_list/npcres.txt";

fn main() -> io::Result<()> {
    println!("📂 Đang quét file (Chế độ text): {}", INPUT_PATH);

    // 1. Mở file Input
    let file = File::open(INPUT_PATH).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("Không tìm thấy file '{}': {}", INPUT_PATH, e),
        )
    })?;
    let reader = BufReader::new(file);

    // 2. Duyệt từng dòng để tìm chuỗi ".spr"
    let mut paths = HashSet::new(); // Dùng HashSet để khử trùng lặp

    for line_result in reader.lines() {
        let line = line_result?;

        // Logic tìm kiếm thủ công (nhanh & nhẹ)
        if let Some(path) = extract_spr_from_line(&line) {
            paths.insert(path);
        }
    }

    println!("🔍 Tìm thấy {} file .spr duy nhất.", paths.len());

    // 3. Tạo thư mục Output (nếu chưa có)
    if let Some(parent) = Path::new(OUTPUT_PATH).parent() {
        fs::create_dir_all(parent)?;
    }

    // 4. Ghi ra file Text
    let out_file = File::create(OUTPUT_PATH)?;
    let mut writer = BufWriter::new(out_file);

    // Sắp xếp lại cho đẹp trước khi ghi (Optional, nhưng nên làm để dễ check)
    let mut sorted_paths: Vec<String> = paths.into_iter().collect();
    sorted_paths.sort();

    for path in sorted_paths {
        writeln!(writer, "{}", path)?;
    }

    println!("✅ Đã xuất danh sách ra file: {}", OUTPUT_PATH);
    Ok(())
}

// --- HÀM LOGIC CỐT LÕI ---
// Tìm chuỗi nằm trong ngoặc kép "..." và kết thúc bằng .spr
fn extract_spr_from_line(line: &str) -> Option<String> {
    // 1. Tìm vị trí của cụm ".spr"" (đuôi file + dấu đóng ngoặc kép)
    // Lưu ý: Dấu ngoặc kép sau .spr là dấu hiệu kết thúc chuỗi trong JSON
    let marker = ".spr\"";

    if let Some(end_idx) = line.find(marker) {
        // end_idx đang trỏ vào dấu chấm (.) của .spr
        // Ta cần tìm dấu ngoặc kép mở (") gần nhất phía trước nó

        // Cắt lấy đoạn text đứng trước dấu chấm
        let prefix = &line[..end_idx];

        if let Some(start_idx) = prefix.rfind('"') {
            // Lấy nội dung từ sau dấu " mở đến hết chữ r (end_idx + 4)
            // .spr có độ dài là 4 ký tự
            let path_content = &line[start_idx + 1..end_idx + 4];

            // Lọc rác: Đảm bảo path không quá ngắn hoặc chứa ký tự lạ nếu cần
            if path_content.len() > 4 {
                return Some(path_content.to_string());
            }
        }
    }

    None
}
