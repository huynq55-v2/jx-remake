// --- 1. IMPORT & FIX LỖI XUNG ĐỘT ---
use ::rand as rand_crate;
use rand_crate::seq::SliceRandom;

use byteorder::{LittleEndian, ReadBytesExt};
use encoding_rs::GBK;
use macroquad::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::Path;

// --- MODULE PAK READER ---
pub const PACK_SIGNATURE: u32 = 0x4b434150;

#[derive(Debug, Clone, Copy)]
pub struct PakHeader {
    pub signature: u32,
    pub count: u32,
    pub index_offset: u32,
    pub data_offset: u32,
    pub _crc32: u32,
    pub _reserved: [u8; 12],
}

#[derive(Debug, Clone, Copy)]
pub struct PakEntry {
    pub id: u32,
    pub offset: u32,
    pub original_size: u32,
    pub compress_flag: u32,
}

impl PakEntry {
    pub fn get_stored_size(&self) -> u32 {
        self.compress_flag & 0x00FFFFFF
    }
    pub fn get_compression_type(&self) -> u8 {
        ((self.compress_flag >> 24) & 0xFF) as u8
    }
}

pub fn read_u32_le(buf: &[u8]) -> u32 {
    u32::from_le_bytes(buf.try_into().unwrap())
}

// --- SỬA LỖI 1: Hàm Hash không tự ý cắt bỏ ký tự đầu nữa ---
pub fn jx_file_name_hash(path: &str) -> u32 {
    // Chỉ chuẩn hóa: Chuyển hết / thành \ (Backslash)
    let normalized_path = path.replace('/', "\\");

    // Encode GBK (Cho hỗ trợ tiếng Trung/Việt nếu có)
    let (cow, _, _) = GBK.encode(&normalized_path);
    let gbk_bytes: &[u8] = &cow;

    let mut id: u32 = 0;
    let mut index: i32 = 0;
    for &byte in gbk_bytes {
        index += 1;
        let mut char_code = byte as i8 as i32;
        if byte >= b'A' && byte <= b'Z' {
            char_code = (byte + (b'a' - b'A')) as i8 as i32;
        }
        let term1 = index.wrapping_mul(char_code) as u32;
        let sum = id.wrapping_add(term1);
        let modded = sum % 0x8000000b;
        id = modded.wrapping_mul(0xffffffef);
    }
    id ^ 0x12345678
}

pub struct PakReader {
    pub file_path: String,
    file: File,
    pub header: PakHeader,
    index_map: HashMap<u32, PakEntry>,
}

impl PakReader {
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        let mut f = File::open(path)?;
        let mut header_buf = [0u8; 24];
        f.read_exact(&mut header_buf)?;
        let header = PakHeader {
            signature: read_u32_le(&header_buf[0..4]),
            count: read_u32_le(&header_buf[4..8]),
            index_offset: read_u32_le(&header_buf[8..12]),
            data_offset: read_u32_le(&header_buf[12..16]),
            _crc32: read_u32_le(&header_buf[16..20]),
            _reserved: [0; 12],
        };
        if header.signature != PACK_SIGNATURE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid PAK: {}", path_str),
            ));
        }
        let mut index_map = HashMap::new();
        f.seek(SeekFrom::Start(header.index_offset as u64))?;
        let mut entry_buf = [0u8; 16];
        for _ in 0..header.count {
            f.read_exact(&mut entry_buf)?;
            let id = read_u32_le(&entry_buf[0..4]);
            index_map.insert(
                id,
                PakEntry {
                    id,
                    offset: read_u32_le(&entry_buf[4..8]),
                    original_size: read_u32_le(&entry_buf[8..12]),
                    compress_flag: read_u32_le(&entry_buf[12..16]),
                },
            );
        }
        Ok(PakReader {
            file_path: path_str,
            file: f,
            header,
            index_map,
        })
    }

    pub fn find_file(&self, path: &str) -> Option<&PakEntry> {
        let target_hash = jx_file_name_hash(path);
        self.index_map.get(&target_hash)
    }

    pub fn read_entry_data(&mut self, entry: &PakEntry) -> io::Result<Vec<u8>> {
        let stored_size = entry.get_stored_size();
        self.file.seek(SeekFrom::Start(entry.offset as u64))?;
        let mut buffer = vec![0u8; stored_size as usize];
        self.file.read_exact(&mut buffer)?;
        match entry.get_compression_type() {
            0 => Ok(buffer),
            1 => nrv2b_decompress_8(&buffer, entry.original_size as usize)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
            _ => Ok(buffer),
        }
    }
}

pub fn nrv2b_decompress_8(src: &[u8], dst_len: usize) -> Result<Vec<u8>, String> {
    let mut dst = Vec::with_capacity(dst_len);
    let mut bb: u32 = 0;
    let mut ilen: usize = 0;
    let mut olen: usize = 0;
    let mut last_m_off: u32 = 1;
    let src_len = src.len();

    let mut getbit = |bb: &mut u32, src: &[u8], ilen: &mut usize| -> Result<u32, String> {
        if (*bb & 0x7f) != 0 {
            *bb = bb.wrapping_mul(2);
        } else {
            if *ilen >= src.len() {
                return Err("INPUT_OVERRUN".into());
            }
            *bb = (src[*ilen] as u32).wrapping_mul(2).wrapping_add(1);
            *ilen += 1;
        }
        Ok((*bb >> 8) & 1)
    };

    loop {
        while getbit(&mut bb, src, &mut ilen)? != 0 {
            if ilen >= src_len || olen >= dst_len {
                return Err("OVERRUN".into());
            }
            dst.push(src[ilen]);
            ilen += 1;
            olen += 1;
        }
        let mut m_off: u32 = 1;
        loop {
            m_off = m_off
                .wrapping_mul(2)
                .wrapping_add(getbit(&mut bb, src, &mut ilen)?);
            if getbit(&mut bb, src, &mut ilen)? != 0 {
                break;
            }
        }
        if m_off == 2 {
            m_off = last_m_off;
        } else {
            if ilen >= src_len {
                return Err("INPUT_OVERRUN".into());
            }
            m_off = m_off
                .wrapping_sub(3)
                .wrapping_mul(256)
                .wrapping_add(src[ilen] as u32);
            ilen += 1;
            if m_off == u32::MAX {
                break;
            }
            m_off = m_off.wrapping_add(1);
            last_m_off = m_off;
        }
        let mut m_len = getbit(&mut bb, src, &mut ilen)?;
        m_len = m_len
            .wrapping_mul(2)
            .wrapping_add(getbit(&mut bb, src, &mut ilen)?);
        if m_len == 0 {
            m_len = 1;
            loop {
                m_len = m_len
                    .wrapping_mul(2)
                    .wrapping_add(getbit(&mut bb, src, &mut ilen)?);
                if getbit(&mut bb, src, &mut ilen)? != 0 {
                    break;
                }
            }
            m_len += 2;
        }
        if m_off > 0x0d00 {
            m_len += 1;
        }
        if olen + (m_len as usize) + 1 > dst_len {
            return Err("OUTPUT_OVERRUN".into());
        }
        let start = olen - m_off as usize;
        for i in 0..=m_len as usize {
            dst.push(dst[start + i]);
            olen += 1;
        }
    }
    Ok(dst)
}

// --- MODULE SPR PARSER ---
#[derive(Debug, Clone)]
pub struct SprHeader {
    pub signature: [u8; 4], // Đã thêm trường signature
    pub width: u16,
    pub height: u16,
    pub center_x: u16,
    pub center_y: u16,
    pub frames: u16,
    pub colors: u16,
    pub directions: u16,
    pub interval: u16,
    pub reserved: [u16; 6],
}

#[derive(Debug, Clone, Copy)]
pub struct SprColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone)]
pub struct SprFrameInfo {
    pub width: u16,
    pub height: u16,
    pub offset_x: i16,
    pub offset_y: i16,
    pub decoded_indices: Vec<u8>,
    pub alpha_map: Vec<u8>,
}

pub struct SprFile {
    pub header: SprHeader,
    pub palette: Vec<SprColor>,
    pub frames: Vec<SprFrameInfo>,
}

impl SprFile {
    pub fn from_reader<R: Read + Seek>(mut f: R) -> io::Result<Self> {
        let mut sig = [0u8; 4];
        f.read_exact(&mut sig)?;
        if &sig[0..3] != b"SPR" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid SPR signature",
            ));
        }

        let header = SprHeader {
            signature: sig,
            width: f.read_u16::<LittleEndian>()?,
            height: f.read_u16::<LittleEndian>()?,
            center_x: f.read_u16::<LittleEndian>()?,
            center_y: f.read_u16::<LittleEndian>()?,
            frames: f.read_u16::<LittleEndian>()?,
            colors: f.read_u16::<LittleEndian>()?,
            directions: f.read_u16::<LittleEndian>()?,
            interval: f.read_u16::<LittleEndian>()?,
            reserved: [
                f.read_u16::<LittleEndian>()?,
                f.read_u16::<LittleEndian>()?,
                f.read_u16::<LittleEndian>()?,
                f.read_u16::<LittleEndian>()?,
                f.read_u16::<LittleEndian>()?,
                f.read_u16::<LittleEndian>()?,
            ],
        };

        let mut palette = Vec::with_capacity(header.colors as usize);
        for _ in 0..header.colors {
            let r = f.read_u8()?;
            let g = f.read_u8()?;
            let b = f.read_u8()?;
            palette.push(SprColor { r, g, b });
        }

        let mut offsets = Vec::with_capacity(header.frames as usize);
        for _ in 0..header.frames {
            let offset = f.read_u32::<LittleEndian>()?;
            let length = f.read_u32::<LittleEndian>()?;
            offsets.push((offset, length));
        }

        let mut frames = Vec::with_capacity(header.frames as usize);
        let data_start_pos = f.stream_position()?;

        for (offset, _) in offsets {
            f.seek(SeekFrom::Start(data_start_pos + offset as u64))?;
            let f_width = f.read_u16::<LittleEndian>()?;
            let f_height = f.read_u16::<LittleEndian>()?;
            let f_off_x = f.read_i16::<LittleEndian>()?;
            let f_off_y = f.read_i16::<LittleEndian>()?;

            let total_pixels = (f_width as usize) * (f_height as usize);
            let mut indices = vec![0u8; total_pixels];
            let mut alphas = vec![0u8; total_pixels];
            let mut pixel_idx = 0;

            while pixel_idx < total_pixels {
                let count = f.read_u8()?;
                let alpha = f.read_u8()?;
                let count_usize = count as usize;

                if alpha > 0 {
                    for _ in 0..count_usize {
                        if pixel_idx >= total_pixels {
                            break;
                        }
                        let color_index = f.read_u8()?;
                        indices[pixel_idx] = color_index;
                        alphas[pixel_idx] = alpha;
                        pixel_idx += 1;
                    }
                } else {
                    pixel_idx += count_usize;
                }
            }

            frames.push(SprFrameInfo {
                width: f_width,
                height: f_height,
                offset_x: f_off_x,
                offset_y: f_off_y,
                decoded_indices: indices,
                alpha_map: alphas,
            });
        }

        Ok(SprFile {
            header,
            palette,
            frames,
        })
    }
}

// --- HELPER MACROQUAD ---
struct SprTexture {
    textures: Vec<Texture2D>,
    offsets: Vec<(f32, f32)>,
    interval: f32,
    total_frames: usize,
}

impl SprTexture {
    fn from_spr_file(spr: &SprFile) -> Self {
        let mut textures = Vec::new();
        let mut offsets = Vec::new();

        for frame in &spr.frames {
            let mut image_data = vec![0u8; frame.width as usize * frame.height as usize * 4];
            for (i, &color_idx) in frame.decoded_indices.iter().enumerate() {
                let r_idx = i * 4;
                if frame.alpha_map[i] > 0 {
                    let color = spr.palette[color_idx as usize];
                    image_data[r_idx] = color.r;
                    image_data[r_idx + 1] = color.g;
                    image_data[r_idx + 2] = color.b;
                    image_data[r_idx + 3] = 255;
                } else {
                    image_data[r_idx + 3] = 0;
                }
            }
            let image = Image {
                bytes: image_data,
                width: frame.width,
                height: frame.height,
            };
            let texture = Texture2D::from_image(&image);
            texture.set_filter(FilterMode::Nearest);
            textures.push(texture);
            offsets.push((frame.offset_x as f32, frame.offset_y as f32));
        }

        let interval_sec = if spr.header.interval > 0 {
            spr.header.interval as f32 / 18.0
        } else {
            0.05
        };

        SprTexture {
            textures,
            offsets,
            interval: interval_sec,
            total_frames: spr.header.frames as usize,
        }
    }
}

// --- MAIN SIMULATION ---

#[derive(Deserialize, Debug)]
struct NpcResJson {
    #[serde(rename = "MainMan")]
    main_man: SpecialNpcData,
    #[serde(rename = "MainLady")]
    main_lady: SpecialNpcData,
}

#[derive(Deserialize, Debug)]
struct SpecialNpcData {
    #[serde(default)]
    weapon_logic: HashMap<String, HashMap<String, HashMap<String, String>>>,
    #[serde(default)]
    components: HashMap<String, HashMap<String, HashMap<String, String>>>,
    #[serde(default)]
    render_order: HashMap<String, HashMap<String, String>>,
}

struct RenderLayer {
    #[allow(dead_code)]
    z_index: i32,
    #[allow(dead_code)]
    slot_name: String,
    spr_data: Option<SprTexture>,
}

#[macroquad::main("JX1 Character Viewer")]
async fn main() {
    println!("🚀 Đang khởi động Engine...");

    // A. Load PAKs
    let mut pak_readers = Vec::new();
    if let Ok(entries) = fs::read_dir("data/pak") {
        for entry in entries.flatten() {
            if entry.path().extension().map_or(false, |e| e == "pak") {
                if let Ok(reader) = PakReader::new(&entry.path()) {
                    println!("   Loaded PAK: {}", entry.path().display());
                    pak_readers.push(reader);
                }
            }
        }
    }

    // B. Load Config & Random
    let file = File::open("data/newdata/npcres.json").expect("Thiếu npcres.json");
    let db: NpcResJson = serde_json::from_reader(BufReader::new(file)).unwrap();

    let mut rng = rand_crate::thread_rng();
    let is_male = rand_crate::random::<bool>();

    let (char_id, char_data) = if is_male {
        ("MainMan", &db.main_man)
    } else {
        ("MainLady", &db.main_lady)
    };

    let states: Vec<&String> = char_data.weapon_logic.keys().collect();
    if states.is_empty() {
        return;
    }
    let state = *states.choose(&mut rng).unwrap();
    let valid_weapons = char_data.weapon_logic.get(state).unwrap();
    let weapon_name = *valid_weapons
        .keys()
        .collect::<Vec<_>>()
        .choose(&mut rng)
        .unwrap();
    let command = *valid_weapons
        .get(weapon_name)
        .unwrap()
        .keys()
        .collect::<Vec<_>>()
        .choose(&mut rng)
        .unwrap();
    let action_id = valid_weapons
        .get(weapon_name)
        .unwrap()
        .get(command)
        .unwrap();

    println!(
        "🎭 KỊCH BẢN: {} | {} | {} | Hành động: {}",
        char_id, state, weapon_name, action_id
    );

    // C. Prepare Layers
    let mut render_layers: Vec<RenderLayer> = Vec::new();
    let mut equipment = HashMap::new();
    for (slot, item_list) in &char_data.components {
        if slot == "rightweapon" {
            if item_list.contains_key(weapon_name) {
                equipment.insert(slot, weapon_name);
            }
        } else {
            if let Some(item) = item_list.keys().collect::<Vec<_>>().choose(&mut rng) {
                equipment.insert(slot, *item);
            }
        }
    }

    let default_order = HashMap::new();
    let order_config = char_data
        .render_order
        .get(action_id)
        .unwrap_or(&default_order);
    let z_order_str = order_config
        .get("Dir1")
        .map(|s: &String| s.as_str())
        .unwrap_or("-1,14,13,1,4,9,7,6,5,12,8,0");

    let layer_ids: Vec<i32> = z_order_str
        .split(',')
        .filter_map(|s: &str| s.trim().parse::<i32>().ok())
        .filter(|&id| id >= 0)
        .collect();

    // --- SỬA LỖI 2: LOGIC DÒ TÌM ĐƯỜNG DẪN THÔNG MINH ---
    for (idx, layer_id) in layer_ids.iter().enumerate() {
        let slot_name = get_slot_name(*layer_id);
        let mut spr_texture = None;
        let mut debug_msg = "No Item".to_string();

        if let Some(item_name) = equipment.get(&slot_name) {
            if let Some(spr_path_raw) = char_data
                .components
                .get(&slot_name)
                .and_then(|i: &HashMap<String, HashMap<String, String>>| i.get(*item_name))
                .and_then(|a: &HashMap<String, String>| a.get(action_id))
            {
                // Chuẩn hóa input ban đầu
                let raw = spr_path_raw.replace('/', "\\"); // Đổi / -> \

                // Tạo 3 ứng viên đường dẫn để thử
                let candidates = vec![
                    format!("\\{}", raw.trim_start_matches('\\')), // 1. Có \ ở đầu: \spr\npcres...
                    raw.trim_start_matches('\\').to_string(), // 2. Không \ ở đầu: spr\npcres...
                    raw.replace("\\spr\\", "\\")
                        .trim_start_matches('\\')
                        .to_string(), // 3. Bỏ spr: npcres...
                ];

                let mut found = false;
                'pak_loop: for pak in &mut pak_readers {
                    for try_path in &candidates {
                        if let Some(entry) = pak.find_file(try_path) {
                            if let Ok(data) = pak.read_entry_data(&entry.clone()) {
                                let cursor = Cursor::new(data);
                                if let Ok(spr_file) = SprFile::from_reader(cursor) {
                                    spr_texture = Some(SprTexture::from_spr_file(&spr_file));
                                    debug_msg = format!("✅ OK ({})", try_path);
                                    found = true;
                                    break 'pak_loop;
                                }
                            }
                        }
                    }
                }

                if !found {
                    debug_msg = format!("❌ Failed (Input: {})", spr_path_raw);
                }
            } else {
                debug_msg = "No Action Logic".to_string();
            }
        }

        render_layers.push(RenderLayer {
            z_index: *layer_id,
            slot_name: slot_name.clone(),
            spr_data: spr_texture,
        });

        println!(
            "   Layer {:02} | Slot: {:<12} | {}",
            idx, slot_name, debug_msg
        );
    }

    // D. Game Loop
    let mut global_timer = 0.0f32;
    let center_x = screen_width() / 2.0;
    let center_y = screen_height() / 2.0;

    loop {
        clear_background(DARKGRAY); // Đổi màu nền tối để dễ nhìn
        global_timer += get_frame_time();

        for layer in &render_layers {
            if let Some(anim) = &layer.spr_data {
                if anim.total_frames == 0 {
                    continue;
                }
                let frame_idx = (global_timer / anim.interval) as usize % anim.total_frames;
                let texture = &anim.textures[frame_idx];
                let (off_x, off_y) = anim.offsets[frame_idx];

                // Công thức vẽ: Center - Offset
                let draw_x = center_x - off_x;
                let draw_y = center_y - off_y;

                draw_texture(texture, draw_x, draw_y, WHITE);
            }
        }

        draw_text(
            format!("Action: {}", action_id).as_str(),
            20.0,
            20.0,
            30.0,
            WHITE,
        );
        draw_text(
            format!("State: {}", state).as_str(),
            20.0,
            50.0,
            30.0,
            WHITE,
        );
        next_frame().await
    }
}

fn get_slot_name(id: i32) -> String {
    match id {
        0 => "head",
        1 => "body",
        2 => "lefthand",
        4 => "leftweapon",
        5 => "shoulder",
        6 => "horsemiddle",
        7 => "horsefront",
        8 => "horseback",
        12 => "hair",
        13 => "lefthand",
        14 => "rightweapon",
        _ => "unknown",
    }
    .to_string()
}
