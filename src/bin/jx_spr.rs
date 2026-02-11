use image::{Rgba, RgbaImage};
use jx_remake::client::spr::SprFile;
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path; // Import SPR logic

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("JX SPR Converter");
        println!("Cách dùng: jx_spr <duong_dan_file.spr>");
        return;
    }

    let file_path = &args[1];
    process_spr(file_path);
}

fn process_spr(path: &str) {
    println!("🎨 Đang xử lý: {}", path);

    match SprFile::load(path) {
        Ok(spr) => {
            println!("--- Thông tin SPR ---");
            println!("Kích thước gốc: {}x{}", spr.header.width, spr.header.height);
            println!(
                "Tâm (Center): {}, {}",
                spr.header.center_x, spr.header.center_y
            );
            println!("Tổng số Frames: {}", spr.header.frames);
            println!("Số hướng (Dir): {}", spr.header.directions);
            println!("Tốc độ (Interval): {}", spr.header.interval);

            // Tạo thư mục output cùng tên file
            let stem = Path::new(path).file_stem().unwrap().to_str().unwrap();
            let out_dir = format!("spr_output/{}", stem);
            fs::create_dir_all(&out_dir).unwrap();

            // Tính số frame cho mỗi hướng
            let total_frames = spr.frames.len();
            let directions = spr.header.directions as usize;

            // Validate dữ liệu để tránh chia cho 0
            if directions == 0 || total_frames == 0 {
                println!("⚠️ File SPR rỗng hoặc lỗi header.");
                return;
            }

            let frames_per_dir = total_frames / directions;
            println!("👉 Animation: {} frames/hướng", frames_per_dir);

            // Loop qua tất cả các frame
            for (i, frame) in spr.frames.iter().enumerate() {
                // Tính toán frame này thuộc hướng nào
                let current_dir = i / frames_per_dir;
                let current_frame_idx = i % frames_per_dir;

                // Tạo ảnh
                let mut img = RgbaImage::new(frame.width as u32, frame.height as u32);

                for y in 0..frame.height {
                    for x in 0..frame.width {
                        let pixel_idx = (y * frame.width + x) as usize;

                        // Kiểm tra bounds an toàn
                        if pixel_idx >= frame.decoded_indices.len() {
                            continue;
                        }

                        let color_idx = frame.decoded_indices[pixel_idx] as usize;
                        let alpha = frame.alpha_map[pixel_idx];

                        // Logic màu của JX:
                        // Nếu alpha > 0 thì vẽ màu từ palette.
                        // (Thực tế JX có Shadow mapping, nhưng cơ bản là vẽ pixel có màu)
                        if alpha > 0 && color_idx < spr.palette.len() {
                            let c = spr.palette[color_idx];
                            // Alpha 255 = rõ nét. Một số sprite dùng alpha map để làm bóng mờ.
                            // Ở đây ta cứ để 255 cho rõ, hoặc dùng chính giá trị `alpha` nếu muốn xem độ trong suốt gốc.
                            img.put_pixel(x as u32, y as u32, Rgba([c.r, c.g, c.b, 255]));
                        } else {
                            // Trong suốt
                            img.put_pixel(x as u32, y as u32, Rgba([0, 0, 0, 0]));
                        }
                    }
                }

                // Lưu file: dir_0_frame_001.png
                let out_name =
                    format!("{}/d{}_f{:03}.png", out_dir, current_dir, current_frame_idx);

                // Ghi thêm offset vào tên file (hoặc file json kèm theo) để sau này ghép game engine biết đường căn chỉnh
                // Ví dụ: d0_f001_offX_offY.png

                match img.save(&out_name) {
                    Ok(_) => {
                        // In tiến độ dạng .... để đỡ spam
                        if i % 10 == 0 {
                            print!(".");
                            std::io::stdout().flush().unwrap();
                        }
                    }
                    Err(e) => println!("\n❌ Lỗi lưu frame {}: {}", i, e),
                }
            }
            println!("\n✅ Hoàn tất! Đã lưu vào thư mục: {}", out_dir);

            // Gợi ý: Tạo thêm 1 file json meta data để Engine load
            save_meta_data(&out_dir, &spr);
        }
        Err(e) => eprintln!("❌ Lỗi đọc SPR: {}", e),
    }
}

// Hàm phụ để lưu thông tin Offset (Cực quan trọng để vẽ đúng vị trí)
fn save_meta_data(dir: &str, spr: &SprFile) {
    let json_path = format!("{}/meta.json", dir);
    let mut content = String::from("[\n");

    for (i, frame) in spr.frames.iter().enumerate() {
        let entry = format!(
            "  {{ \"id\": {}, \"w\": {}, \"h\": {}, \"off_x\": {}, \"off_y\": {} }},\n",
            i, frame.width, frame.height, frame.offset_x, frame.offset_y
        );
        content.push_str(&entry);
    }
    // Xóa dấu phẩy cuối
    if content.len() > 2 {
        content.truncate(content.len() - 2);
    }
    content.push_str("\n]");

    fs::write(json_path, content).unwrap_or_default();
}
