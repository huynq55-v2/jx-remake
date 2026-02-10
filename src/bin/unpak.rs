use jx_remake::common::pak::{PakReader, jx_file_name_hash};
use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() >= 2 && args[1] == "--hash" {
        if args.len() < 3 {
            println!("Usage: --hash <string>");
            return;
        }
        let input = &args[2];
        let hash = jx_file_name_hash(input);
        println!("String: {}", input);
        println!("Hash  : {:08X}", hash);
        return;
    }

    let file_target_path = &args[1];
    let pak_path = &args[2];

    println!("--- JX PAK UNPACKER ---");
    println!("Target: {}", file_target_path);
    println!("Source: {}", pak_path);

    // 1. Khởi tạo Reader
    let mut reader = match PakReader::new(pak_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("❌ Lỗi mở file PAK: {}", e);
            return;
        }
    };

    println!("PAK Info: {} files found.", reader.header.count);

    // 2. Tìm file
    match reader.find_file(file_target_path) {
        Ok(Some(entry)) => {
            println!("✅ Đã tìm thấy file!");
            println!(
                "   Hash ID: {:08X} (Check: {:08X})",
                entry.id,
                jx_file_name_hash(file_target_path)
            );
            println!("   Size gốc: {} bytes", entry.original_size);
            println!("   Size nén: {} bytes", entry.get_stored_size());

            let comp_type = entry.get_compression_type();
            match comp_type {
                0 => println!("   Compression: None"),
                1 => println!("   Compression: UCL (Cần giải nén!)"),
                2 => println!("   Compression: BZIP2"),
                _ => println!("   Compression: Unknown ({})", comp_type),
            }

            // 3. Đọc dữ liệu
            match reader.read_entry_data(&entry) {
                Ok(data) => {
                    // 4. Lưu ra đĩa
                    if let Err(e) = save_file_to_disk(file_target_path, &data) {
                        eprintln!("❌ Lỗi khi lưu file: {}", e);
                    }
                }
                Err(e) => eprintln!("❌ Lỗi khi đọc dữ liệu: {}", e),
            }
        }
        Ok(None) => {
            println!("❌ Không tìm thấy file trong PAK.");
            println!(
                "   Hash ID đã thử: {:08X}",
                jx_file_name_hash(file_target_path)
            );
        }
        Err(e) => eprintln!("❌ Lỗi khi tìm kiếm: {}", e),
    }
}

fn save_file_to_disk(path_str: &str, data: &[u8]) -> std::io::Result<()> {
    // Xử lý đường dẫn để an toàn trên Linux/Windows
    let clean_path = path_str.replace('\\', "/");
    let clean_path = clean_path.trim_start_matches('/');

    let output_path = PathBuf::from(clean_path);

    // Tạo thư mục cha nếu chưa có
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
        println!("📁 Đã tạo thư mục: {:?}", parent);
    }

    // Ghi file
    let mut f = fs::File::create(&output_path)?;
    f.write_all(data)?;
    println!("💾 Đã lưu file tại: {:?}", output_path);
    Ok(())
}
