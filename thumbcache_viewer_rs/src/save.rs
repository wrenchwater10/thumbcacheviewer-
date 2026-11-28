// Saving extracted images, exporting the list as CSV and copying list rows.

use crate::thumbcache::Entry;
use crate::win::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn sanitize(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if "\\/:*?\"<>|".contains(c) || (c as u32) < 32 {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    out.trim().trim_matches('.').to_string()
}

fn unique_name(used: &mut HashMap<String, u32>, name: &str) -> String {
    let key = name.to_lowercase();
    let count = used.entry(key).or_insert(0);
    *count += 1;
    if *count == 1 {
        return name.to_string();
    }
    let p = Path::new(name);
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string());
    let ext = p
        .extension()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if ext.is_empty() {
        format!("{} ({})", stem, count)
    } else {
        format!("{} ({}).{}", stem, count, ext)
    }
}

/// Writes the raw image of every entry into `dir`. Returns how many were written.
pub fn save_entries(entries: &[Entry], dir: &Path) -> Result<usize, String> {
    fs::create_dir_all(dir).map_err(|e| format!("Unable to create {}: {}", dir.display(), e))?;

    let mut used: HashMap<String, u32> = HashMap::new();
    let mut saved = 0usize;

    for entry in entries {
        if entry.data_size == 0 {
            continue;
        }
        let mut name = sanitize(&entry.filename);
        if name.is_empty() {
            name = format!("entry_{:08X}", entry.entry_hash as u32);
        }
        if Path::new(&name).extension().is_none() {
            name.push('.');
            name.push_str(entry.extension());
        }
        let name = unique_name(&mut used, &name);
        let data = entry
            .data()
            .map_err(|e| format!("{}: {}", entry.filename, e))?;
        let path = dir.join(&name);
        fs::write(&path, data).map_err(|e| format!("{}: {}", path.display(), e))?;
        saved += 1;
    }

    Ok(saved)
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Writes the entry list as CSV. Returns how many rows were written.
pub fn save_csv(path: &Path, entries: &[Entry]) -> Result<usize, String> {
    let mut out = String::new();
    out.push_str("Filename,Cache Entry Offset,Cache Entry Size,Data Offset,Data Size,Data Checksum,Header Checksum,Cache Entry Hash,System,Location\r\n");

    for e in entries {
        let row = [
            e.filename.clone(),
            e.header_offset.to_string(),
            e.cache_entry_size.to_string(),
            e.data_offset.to_string(),
            e.data_size.to_string(),
            format!("{:016X}", e.data_checksum),
            format!("{:016X}", e.header_checksum),
            format!("{:016X}", e.entry_hash),
            e.system_name().to_string(),
            e.db_path.to_string_lossy().into_owned(),
        ];
        let fields: Vec<String> = row.iter().map(|f| csv_field(f)).collect();
        out.push_str(&fields.join(","));
        out.push_str("\r\n");
    }

    fs::write(path, out.as_bytes()).map_err(|e| format!("{}: {}", path.display(), e))?;
    Ok(entries.len())
}

pub unsafe fn copy_text_to_clipboard(owner: HWND, text: &str) {
    let mut wtext = wide(text);
    let bytes = wtext.len() * 2;

    if OpenClipboard(owner) == 0 {
        return;
    }
    EmptyClipboard();

    let hmem = GlobalAlloc(GMEM_MOVEABLE, bytes);
    if !hmem.is_null() {
        let ptr = GlobalLock(hmem) as *mut u8;
        if !ptr.is_null() {
            std::ptr::copy_nonoverlapping(wtext.as_mut_ptr() as *const u8, ptr, bytes);
            GlobalUnlock(hmem);
            if SetClipboardData(CLIPBOARD_CF_UNICODETEXT, hmem).is_null() {
                GlobalFree(hmem);
            }
        } else {
            GlobalFree(hmem);
        }
    }

    CloseClipboard();
}
