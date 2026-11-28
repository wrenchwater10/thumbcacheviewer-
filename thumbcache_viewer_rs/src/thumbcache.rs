// Parser for Windows thumbnail cache databases (thumbcache_*.db).
//
// Layout (all little endian):
//   header: "CMMM", u32 version, u32 type, then padding up to offset 24 (28 on 8v2)
//   entry : "CMMM", u32 cache_entry_size, u64 entry_hash, ...
//           followed by the UTF-16 filename, padding and the image data.

use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub const WINDOWS_VISTA: u32 = 0x14;
pub const WINDOWS_7: u32 = 0x15;
pub const WINDOWS_8: u32 = 0x1A;
pub const WINDOWS_8V2: u32 = 0x1C;
pub const WINDOWS_8V3: u32 = 0x1E;
pub const WINDOWS_8_1: u32 = 0x1F;
pub const WINDOWS_10: u32 = 0x20;

pub const FIF_TYPE_BMP: u8 = 1;
pub const FIF_TYPE_JPG: u8 = 2;
pub const FIF_TYPE_PNG: u8 = 4;

#[derive(Clone, Debug)]
pub struct Entry {
    pub filename: String,
    pub header_offset: u32,
    pub cache_entry_size: u32,
    pub data_offset: u32,
    pub data_size: u32,
    pub entry_hash: u64,
    pub data_checksum: u64,
    pub header_checksum: u64,
    pub flag: u8,
    pub system: u32,
    // Only present in Windows 8 and later entries; kept for completeness.
    #[allow(dead_code)]
    pub width: u32,
    #[allow(dead_code)]
    pub height: u32,
    pub db_path: PathBuf,
}

impl Entry {
    pub fn system_name(&self) -> &'static str {
        match self.system {
            WINDOWS_VISTA => "Windows Vista",
            WINDOWS_7 => "Windows 7",
            WINDOWS_8 | WINDOWS_8V2 => "Windows 8",
            WINDOWS_8V3 => "Windows 8.1 preview",
            WINDOWS_8_1 => "Windows 8.1",
            WINDOWS_10 => "Windows 10",
            _ => "Unknown",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self.flag {
            FIF_TYPE_BMP => "bmp",
            FIF_TYPE_JPG => "jpg",
            FIF_TYPE_PNG => "png",
            _ => "bin",
        }
    }

    /// Reads this entry's image data out of its database file.
    pub fn data(&self) -> std::io::Result<Vec<u8>> {
        let mut f = fs::File::open(&self.db_path)?;
        f.seek(SeekFrom::Start(self.data_offset as u64))?;
        let mut buf = vec![0u8; self.data_size as usize];
        f.read_exact(&mut buf)?;
        Ok(buf)
    }
}

fn u32_at(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

fn u64_at(d: &[u8], o: usize) -> u64 {
    u64::from_le_bytes([
        d[o],
        d[o + 1],
        d[o + 2],
        d[o + 3],
        d[o + 4],
        d[o + 5],
        d[o + 6],
        d[o + 7],
    ])
}

fn utf16_string(bytes: &[u8]) -> String {
    let mut chars: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    if let Some(p) = chars.iter().position(|&c| c == 0) {
        chars.truncate(p);
    }
    String::from_utf16_lossy(&chars)
}

fn find_magic(data: &[u8], from: usize) -> Option<usize> {
    if from + 4 > data.len() {
        return None;
    }
    data[from..]
        .windows(4)
        .position(|w| w == b"CMMM")
        .map(|p| from + p)
}

/// Reads every entry in a thumbcache database.
pub fn read_database(path: &Path, hide_blank: bool) -> Result<Vec<Entry>, String> {
    let data = fs::read(path).map_err(|e| format!("{}: {}", path.display(), e))?;

    if data.len() < 12 || &data[0..4] != b"CMMM" {
        return Err(format!(
            "{} is not a thumbcache database.",
            path.display()
        ));
    }

    let version = u32_at(&data, 4);
    let header_len: usize = match version {
        WINDOWS_7 => 48,
        WINDOWS_VISTA | WINDOWS_8 | WINDOWS_8V2 | WINDOWS_8V3 | WINDOWS_8_1 | WINDOWS_10 => 56,
        _ => {
            return Err(format!(
                "{} is not supported by this program (version 0x{:X}).",
                path.display(),
                version
            ))
        }
    };

    let mut entries: Vec<Entry> = Vec::new();
    let mut pos: usize = if version == WINDOWS_8V2 { 28 } else { 24 };

    loop {
        // Recover from corrupt or padded areas by scanning for the next entry.
        let header_offset = match find_magic(&data, pos) {
            Some(p) => p,
            None => break,
        };
        if header_offset + header_len > data.len() {
            break;
        }

        let cache_entry_size = u32_at(&data, header_offset + 4);
        let entry_hash = u64_at(&data, header_offset + 8);

        // The Vista entry stores the file extension before the filename length.
        let (filename_length, padding_size, data_size, width, height, data_checksum, header_checksum) =
            match version {
                WINDOWS_7 => (
                    u32_at(&data, header_offset + 16),
                    u32_at(&data, header_offset + 20),
                    u32_at(&data, header_offset + 24),
                    0,
                    0,
                    u64_at(&data, header_offset + 32),
                    u64_at(&data, header_offset + 40),
                ),
                WINDOWS_VISTA => (
                    u32_at(&data, header_offset + 24),
                    u32_at(&data, header_offset + 28),
                    u32_at(&data, header_offset + 32),
                    0,
                    0,
                    u64_at(&data, header_offset + 40),
                    u64_at(&data, header_offset + 48),
                ),
                _ => (
                    u32_at(&data, header_offset + 16),
                    u32_at(&data, header_offset + 20),
                    u32_at(&data, header_offset + 24),
                    u32_at(&data, header_offset + 28),
                    u32_at(&data, header_offset + 32),
                    u64_at(&data, header_offset + 40),
                    u64_at(&data, header_offset + 48),
                ),
            };

        if entry_hash == 0 {
            // End of the valid area; skip this header and keep scanning.
            pos = header_offset + header_len;
            continue;
        }

        let next_entry = header_offset.saturating_add(cache_entry_size as usize);
        let name_offset = header_offset + header_len;
        let name_len = (filename_length as usize).min(0xFFFE);
        let name_end = name_offset.saturating_add(name_len);

        if cache_entry_size < header_len as u32 || name_end > data.len() {
            pos = header_offset + header_len;
            continue;
        }

        let mut filename = utf16_string(&data[name_offset..name_end]);
        if filename.is_empty() {
            filename = format!("Entry_{:08X}", entry_hash as u32);
        }

        // The data begins after the filename and the padding that follows it.
        let data_offset = name_end
            .saturating_add(filename_length as usize - name_len)
            .saturating_add(padding_size as usize);
        let data_end = data_offset.saturating_add(data_size as usize);
        if data_end > data.len() || data_offset > next_entry.max(name_end) + 0x10000 {
            pos = header_offset + header_len;
            continue;
        }

        if data_size == 0 && hide_blank {
            pos = next_entry.max(header_offset + header_len);
            continue;
        }

        // Identify the image type from its signature.
        let mut flag = 0u8;
        let mut ext_name = String::new();
        if data_size >= 2 {
            let s = &data[data_offset..data.len().min(data_offset + 8)];
            if s.len() >= 8 && s[0..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
                flag = FIF_TYPE_PNG;
                ext_name = "png".to_string();
            } else if s.len() >= 3 && s[0..3] == [0xFF, 0xD8, 0xFF] {
                flag = FIF_TYPE_JPG;
                ext_name = "jpg".to_string();
            } else if s.len() >= 2 && s[0..2] == *b"BM" {
                flag = FIF_TYPE_BMP;
                ext_name = "bmp".to_string();
            }
        }

        if version == WINDOWS_VISTA {
            // Vista keeps the extension in the entry itself.
            let ext = utf16_string(&data[header_offset + 16..header_offset + 24]);
            let ext = ext.trim_matches(|c: char| c == '\0').to_string();
            if !ext.is_empty() {
                filename.push('.');
                filename.push_str(&ext);
            }
        } else if !ext_name.is_empty() {
            let lower = filename.to_lowercase();
            if !lower.ends_with(&format!(".{}", ext_name)) {
                filename.push('.');
                filename.push_str(&ext_name);
            }
        }

        entries.push(Entry {
            filename,
            header_offset: header_offset as u32,
            cache_entry_size,
            data_offset: data_offset as u32,
            data_size,
            entry_hash,
            data_checksum,
            header_checksum,
            flag,
            system: version,
            width,
            height,
            db_path: path.to_path_buf(),
        });

        pos = next_entry.max(header_offset + header_len);
    }

    if entries.is_empty() {
        return Err(format!(
            "{} contains no cache entries.",
            path.display()
        ));
    }

    Ok(entries)
}
