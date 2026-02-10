// src/common/pak.rs
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

// --- CONSTANTS ---
pub const PACK_SIGNATURE: u32 = 0x4b434150; // 'PACK'

// --- STRUCTS ---

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

// --- HELPER FUNCTIONS ---

pub fn read_u32_le(buf: &[u8]) -> u32 {
    u32::from_le_bytes(buf.try_into().unwrap())
}

/// Thuật toán Hash tên file của Kingsoft
pub fn jx_file_name_hash(path: &str) -> u32 {
    // 1. Chuẩn hóa đường dẫn: thay / bằng \
    let normalized_path = path.replace('/', "\\");

    // Đảm bảo path bắt đầu bằng '\\' nếu chưa có (theo logic g_GetPackPath của C++)
    // Lưu ý: Input của bạn là "\settings\serverlist.ini" đã có \ nên OK.
    let final_path = if !normalized_path.starts_with('\\') {
        format!("\\{}", normalized_path)
    } else {
        normalized_path
    };

    let mut id: u32 = 0; // BẮT BUỘC dùng u32 để mô phỏng unsigned long của C++
    let mut index: i32 = 0; // C++ dùng int

    for byte in final_path.bytes() {
        index += 1;

        // Xử lý Uppercase -> Lowercase
        let char_code = if byte >= b'A' && byte <= b'Z' {
            byte + (b'a' - b'A')
        } else {
            byte
        } as i32;

        // Logic C++: id = (id + (++index) * char_code) % 0x8000000b * 0xffffffef;
        // Phân tích từng bước để ép kiểu chuẩn xác:

        // 1. (index * char_code)
        let term1 = (index.wrapping_mul(char_code)) as u32;

        // 2. (id + term1)
        // Lưu ý: C++ unsigned long tự động wrap khi tràn
        let sum = id.wrapping_add(term1);

        // 3. % 0x8000000b
        let modded = sum % 0x8000000b;

        // 4. * 0xffffffef
        // ĐÂY LÀ CHỖ SAI CŨ: Phép nhân này phải wrap trong 32-bit.
        // 0xffffffef tương đương -17 (signed) nhưng là số lớn unsigned.
        id = modded.wrapping_mul(0xffffffef);
    }

    // XOR cuối cùng
    id ^ 0x12345678
}

// --- PAK READER CLASS ---

pub struct PakReader {
    file: File,
    pub header: PakHeader,
}

impl PakReader {
    /// Mở file PAK và đọc Header
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut f = File::open(path)?;

        // Đọc 24 bytes header
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
                "Invalid PAK signature",
            ));
        }

        Ok(PakReader { file: f, header })
    }

    /// Tìm file trong PAK dựa trên đường dẫn (tự tính hash)
    pub fn find_file(&mut self, path: &str) -> io::Result<Option<PakEntry>> {
        let target_hash = jx_file_name_hash(path);

        // Seek đến bảng Index
        self.file
            .seek(SeekFrom::Start(self.header.index_offset as u64))?;

        let mut entry_buf = [0u8; 16];
        for _ in 0..self.header.count {
            self.file.read_exact(&mut entry_buf)?;
            let id = read_u32_le(&entry_buf[0..4]);

            if id == target_hash {
                return Ok(Some(PakEntry {
                    id,
                    offset: read_u32_le(&entry_buf[4..8]),
                    original_size: read_u32_le(&entry_buf[8..12]),
                    compress_flag: read_u32_le(&entry_buf[12..16]),
                }));
            }
        }

        Ok(None)
    }

    /// Đọc dữ liệu raw của một entry
    pub fn read_entry_data(&mut self, entry: &PakEntry) -> io::Result<Vec<u8>> {
        let stored_size = entry.get_stored_size();
        self.file.seek(SeekFrom::Start(entry.offset as u64))?;

        let mut buffer = vec![0u8; stored_size as usize];
        self.file.read_exact(&mut buffer)?;

        Ok(buffer)
    }
}
