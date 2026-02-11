use jx_remake::common::pak::{PakReader, jx_file_name_hash};
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = env::args().collect();

    // --- Parse Arguments ---
    let mut filelist_path: Option<String> = None;
    let mut pak_paths: Vec<String> = Vec::new();
    let mut mode = ""; // "" | "f" | "p"

    // Parse tay để linh hoạt hơn (support --p dir pak1 pak2...)
    for arg in args.iter().skip(1) {
        if arg == "-l" || arg == "--l" {
            mode = "l";
            continue;
        }
        if arg == "-p" || arg == "--p" {
            mode = "p";
            continue;
        }

        match mode {
            "l" => filelist_path = Some(arg.clone()),
            "p" => pak_paths.push(arg.clone()),
            _ => {}
        }
    }

    if filelist_path.is_none() || pak_paths.is_empty() {
        print_usage();
        return;
    }

    // --- 1. Load danh sách PAK ---
    let mut readers: Vec<PakReader> = Vec::new();
    println!("📦 Đang load các file PAK...");

    for path_str in pak_paths {
        let path = Path::new(&path_str);
        if path.is_dir() {
            // Nếu là folder -> Load tất cả *.pak trong đó
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if let Some(ext) = p.extension() {
                        if ext.to_string_lossy().eq_ignore_ascii_case("pak") {
                            add_pak_reader(&mut readers, &p);
                        }
                    }
                }
            }
        } else if path.is_file() {
            // Nếu là file -> Load trực tiếp
            add_pak_reader(&mut readers, path);
        } else {
            eprintln!("⚠️ Cảnh báo: Đường dẫn không tồn tại: {}", path_str);
        }
    }

    if readers.is_empty() {
        eprintln!("❌ Không tìm thấy file PAK nào hợp lệ!");
        return;
    }
    println!("✅ Đã load {} file PAK.", readers.len());

    // --- 2. Đọc Filelist ---
    let list_file = match fs::File::open(filelist_path.as_ref().unwrap()) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("❌ Không thể mở file list: {}", e);
            return;
        }
    };
    let lines: Vec<String> = io::BufReader::new(list_file)
        .lines()
        .filter_map(|l| l.ok())
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    println!("📄 Tìm thấy {} file cần giải nén.", lines.len());

    // --- 3. Thực thi giải nén ---
    let mut success_count = 0;
    let mut fail_count = 0;

    for target_path in lines {
        let mut found = false;

        for reader in &mut readers {
            if let Some(entry) = reader.find_file(&target_path) {
                let entry_copy = *entry;

                match reader.read_entry_data(&entry_copy) {
                    Ok(data) => {
                        if let Err(e) = save_file_to_disk(&target_path, &data) {
                            eprintln!("   ❌ Lỗi ghi đĩa '{}': {}", target_path, e);
                            fail_count += 1; // FIX: Tăng lỗi khi ghi file hỏng
                        } else {
                            println!("✅ Extracted: {} (from {})", target_path, reader.file_path);
                            success_count += 1;
                        }
                    }
                    Err(e) => {
                        eprintln!("   ❌ Lỗi đọc/giải nén '{}': {}", target_path, e);
                        fail_count += 1; // FIX: Tăng lỗi khi giải nén hỏng
                    }
                }
                found = true;
                break;
            }
        }

        if !found {
            eprintln!(
                "❌ Missing: {} (Hash: {:08X})",
                target_path,
                jx_file_name_hash(&target_path)
            );
            fail_count += 1;
        }
    }

    println!("\n--- HOÀN TẤT ---");
    println!("Thành công: {}", success_count);
    println!("Thất bại  : {}", fail_count);
}

fn add_pak_reader(readers: &mut Vec<PakReader>, path: &Path) {
    match PakReader::new(path) {
        Ok(r) => {
            println!("   Loaded: {} ({} files)", path.display(), r.header.count);
            readers.push(r);
        }
        Err(e) => eprintln!("   ❌ Lỗi load {}: {}", path.display(), e),
    }
}

fn save_file_to_disk(path_str: &str, data: &[u8]) -> io::Result<()> {
    // Xử lý đường dẫn
    let clean_path = path_str.replace('\\', "/");
    let clean_path = clean_path.trim_start_matches('/');

    // Lưu vào thư mục output mặc định là "extracted" để không lộn xộn
    let output_path = PathBuf::from("extracted").join(clean_path);

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut f = fs::File::create(&output_path)?;
    f.write_all(data)?;
    Ok(())
}

fn print_usage() {
    println!("Usage:");
    println!("  jx_unpack -l <filelist.txt> -p <pak_path_or_dir> [pak_path_2 ...]");
    println!("\nExamples:");
    println!("  jx_unpack -l list.txt -p settings.pak");
    println!("  jx_unpack -l list.txt -p update01.pak update02.pak");
    println!("  jx_unpack -l list.txt -p C:\\Game\\Client\\");
}
