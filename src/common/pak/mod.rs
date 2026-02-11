use std::collections::HashMap;
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

/// Thuật toán Hash tên file của Kingsoft (Phiên bản Fix chuẩn 32-bit)
pub fn jx_file_name_hash(path: &str) -> u32 {
    let normalized_path = path.replace('/', "\\");

    // Đảm bảo path bắt đầu bằng '\\'
    let final_path = if !normalized_path.starts_with('\\') {
        format!("\\{}", normalized_path)
    } else {
        normalized_path
    };

    let mut id: u32 = 0;
    let mut index: i32 = 0;

    for byte in final_path.bytes() {
        index += 1;
        let char_code = if byte >= b'A' && byte <= b'Z' {
            byte + (b'a' - b'A')
        } else {
            byte
        } as i32;

        let term1 = (index.wrapping_mul(char_code)) as u32;
        let sum = id.wrapping_add(term1);
        let modded = sum % 0x8000000b;
        id = modded.wrapping_mul(0xffffffef);
    }

    id ^ 0x12345678
}

// --- PAK READER CLASS ---

pub struct PakReader {
    pub file_path: String, // Lưu đường dẫn để log
    file: File,
    pub header: PakHeader,
    // Cache Index để tìm kiếm O(1) thay vì O(n)
    index_map: HashMap<u32, PakEntry>,
}

impl PakReader {
    /// Mở file PAK và load toàn bộ Index vào RAM
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        let mut f = File::open(path)?;

        // 1. Đọc Header
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
                format!("Invalid PAK signature in {}", path_str),
            ));
        }

        // 2. Load Index Table
        let mut index_map = HashMap::new();
        f.seek(SeekFrom::Start(header.index_offset as u64))?;

        let mut entry_buf = [0u8; 16];
        for _ in 0..header.count {
            f.read_exact(&mut entry_buf)?;
            let id = read_u32_le(&entry_buf[0..4]);

            // Nếu có ID trùng (lỗi packer), file sau sẽ đè file trước (theo logic C++ cũng vậy)
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

    /// Tìm file trong Index đã cache
    pub fn find_file(&self, path: &str) -> Option<&PakEntry> {
        let target_hash = jx_file_name_hash(path);
        self.index_map.get(&target_hash)
    }

    /// Đọc dữ liệu raw
    pub fn read_entry_data(&mut self, entry: &PakEntry) -> io::Result<Vec<u8>> {
        let stored_size = entry.get_stored_size();
        self.file.seek(SeekFrom::Start(entry.offset as u64))?;

        let mut buffer = vec![0u8; stored_size as usize];
        self.file.read_exact(&mut buffer)?;

        // Kiểm tra loại nén
        match entry.get_compression_type() {
            0 => {
                // Không nén: Trả về luôn
                Ok(buffer)
            }
            1 => {
                // UCL/NRV2B: Gọi hàm giải nén
                // println!("   ... Decompressing (UCL) {} -> {} bytes", stored_size, entry.original_size);

                nrv2b_decompress_8(&buffer, entry.original_size as usize).map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidData, format!("UCL Error: {}", e))
                })
            }
            2 => {
                // BZIP2 (Hiếm gặp trong Client, thường ở Server)
                // Hiện tại chưa implement, trả về raw
                println!("⚠️ Warning: BZIP2 compression not supported yet.");
                Ok(buffer)
            }
            _ => {
                println!("⚠️ Warning: Unknown compression type.");
                Ok(buffer)
            }
        }
    }
}

/// Thuật toán giải nén NRV2B 8-bit (Port từ ucl/n2b_d.c)

pub fn nrv2b_decompress_8(src: &[u8], dst_len: usize) -> Result<Vec<u8>, String> {
    let mut dst = Vec::with_capacity(dst_len);

    let mut bb: u32 = 0;
    let mut ilen: usize = 0;
    let mut olen: usize = 0;
    let mut last_m_off: u32 = 1;

    let src_len = src.len();

    // Exact port of:
    // #define getbit_8(bb, src, ilen)
    // (((bb = bb & 0x7f ? bb*2 : ((unsigned)src[ilen++]*2+1)) >> 8) & 1)
    #[inline(always)]
    fn getbit(bb: &mut u32, src: &[u8], ilen: &mut usize) -> Result<u32, String> {
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
    }

    loop {
        /* -------- literal copy phase -------- */
        while getbit(&mut bb, src, &mut ilen)? != 0 {
            if ilen >= src_len {
                return Err("INPUT_OVERRUN".into());
            }
            if olen >= dst_len {
                return Err("OUTPUT_OVERRUN".into());
            }

            dst.push(src[ilen]);
            ilen += 1;
            olen += 1;
        }

        /* -------- match offset decode -------- */
        let mut m_off: u32 = 1;
        loop {
            m_off = m_off
                .wrapping_mul(2)
                .wrapping_add(getbit(&mut bb, src, &mut ilen)?);

            if m_off > 0x00ff_ffff + 3 {
                return Err("LOOKBEHIND_OVERRUN".into());
            }

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

            // (m_off-3)*256 + src[ilen++]
            m_off = m_off
                .wrapping_sub(3)
                .wrapping_mul(256)
                .wrapping_add(src[ilen] as u32);
            ilen += 1;

            // sentinel
            if m_off == u32::MAX {
                break;
            }

            m_off = m_off.wrapping_add(1);
            last_m_off = m_off;
        }

        /* -------- match length decode -------- */
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

        /* -------- match copy -------- */
        if olen + (m_len as usize) + 1 > dst_len {
            return Err("OUTPUT_OVERRUN".into());
        }
        if m_off as usize > olen {
            return Err("LOOKBEHIND_OVERRUN".into());
        }

        let start = olen - m_off as usize;
        for i in 0..=m_len as usize {
            let b = dst[start + i];
            dst.push(b);
            olen += 1;
        }
    }

    Ok(dst)
}
