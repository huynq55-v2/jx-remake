// src/common/spr.rs
use byteorder::{LittleEndian, ReadBytesExt};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

// --- CẤU TRÚC DỮ LIỆU ---

#[derive(Debug, Clone)]
pub struct SprHeader {
    pub signature: [u8; 4], // "SPR\0"
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
    pub offset_x: i16, // C++ là WORD (u16) nhưng thực tế offset có thể âm hoặc dùng để trừ
    pub offset_y: i16,
    // Dữ liệu pixel thô đã giải nén (Index vào Palette)
    pub decoded_indices: Vec<u8>,
    // Kênh Alpha (Transparency)
    pub alpha_map: Vec<u8>,
}

// --- SPR PARSER CLASS ---

pub struct SprFile {
    pub header: SprHeader,
    pub palette: Vec<SprColor>,
    pub frames: Vec<SprFrameInfo>,
}

impl SprFile {
    pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut f = File::open(path)?;

        // 1. Đọc Header (Size = 24 bytes + Reserved)
        // Lưu ý: Cấu trúc C++ có padding alignment, ta đọc từng field cho chắc
        let mut sig = [0u8; 4];
        f.read_exact(&mut sig)?;

        // Kiểm tra chữ ký "SPR" (C++: g_MemComp(..., "SPR", 3))
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

        // 2. Đọc Palette (header.colors * 3 bytes)
        // C++: KPAL24 (Red, Green, Blue)
        let mut palette = Vec::with_capacity(header.colors as usize);
        for _ in 0..header.colors {
            let r = f.read_u8()?;
            let g = f.read_u8()?;
            let b = f.read_u8()?;
            palette.push(SprColor { r, g, b });
        }

        // 3. Đọc Offset Table (header.frames * 8 bytes)
        // Struct SPROFFS { Offset: DWORD, Length: DWORD }
        let mut offsets = Vec::with_capacity(header.frames as usize);
        for _ in 0..header.frames {
            let offset = f.read_u32::<LittleEndian>()?;
            let length = f.read_u32::<LittleEndian>()?;
            offsets.push((offset, length));
        }

        // 4. Đọc và giải mã từng Frame
        let mut frames = Vec::with_capacity(header.frames as usize);

        // Vị trí bắt đầu vùng data sprite (Header + Pal + OffsetTable)
        // Nhưng trong file SPR, Offset trong bảng Offset là tính từ ĐẦU VÙNG SPRITE DATA
        // chứ không phải đầu file.
        // Logic C++: m_pSprite = (LPBYTE)pTemp (sau khi cộng offset header/pal/offset_table)
        // => Absolute File Offset = Current Pos + FrameOffset
        let data_start_pos = f.stream_position()?;

        for (offset, _length) in offsets {
            f.seek(SeekFrom::Start(data_start_pos + offset as u64))?;

            // Đọc Frame Header
            let f_width = f.read_u16::<LittleEndian>()?;
            let f_height = f.read_u16::<LittleEndian>()?;
            let f_off_x = f.read_i16::<LittleEndian>()?; // Cast sang i16 cho dễ tính toán
            let f_off_y = f.read_i16::<LittleEndian>()?;

            // Giải mã RLE (Run-Length Encoding)
            // Logic dựa trên KSpriteCodec::ConvertLine
            // Data structure: [Count][Alpha][Data...]

            let total_pixels = (f_width as usize) * (f_height as usize);
            let mut indices = vec![0u8; total_pixels];
            let mut alphas = vec![0u8; total_pixels];

            let mut pixel_idx = 0;

            // Loop cho đến khi điền đủ pixel của frame đó
            // Lưu ý: Frame SPR được mã hóa theo dòng (row by row)
            while pixel_idx < total_pixels {
                // Đọc Count và Alpha
                let count = f.read_u8()?;
                let alpha = f.read_u8()?;

                let count_usize = count as usize;

                if alpha > 0 {
                    // Có màu (Solid hoặc Translucent) -> Đọc tiếp `count` bytes index màu
                    for _ in 0..count_usize {
                        if pixel_idx >= total_pixels {
                            break;
                        }
                        let color_index = f.read_u8()?;
                        indices[pixel_idx] = color_index;
                        alphas[pixel_idx] = alpha; // Alpha 255 = Solid, < 255 = Translucent
                        pixel_idx += 1;
                    }
                } else {
                    // Trong suốt (Transparent) -> Bỏ qua `count` pixel
                    // Không cần đọc color index, chỉ tịnh tiến con trỏ
                    // indices[pixel_idx..] mặc định là 0
                    // alphas[pixel_idx..] mặc định là 0
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
