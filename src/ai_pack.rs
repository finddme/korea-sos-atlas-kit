// SPDX-License-Identifier: AGPL-3.0-only
//! AI pack install. The release ships the model and engine as two zips
//! (ai-pack-1, ai-pack-2) next to the Atlas zip. Instead of asking people to
//! unpack them into the right folder, the app finds the downloaded zips (kit
//! folder, the folders above it, the Downloads folder), unpacks them into
//! `data/` through a staging folder and checks every file against
//! `assets.json` before anything is moved into place.
//!
//! Only files the manifest lists (model shards, engine, engine libraries) and
//! plain licence/notice texts in the model and engine folders are accepted, so
//! a foreign zip with the same name cannot place other programs in the kit.
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{self, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

pub const PACKS: [&str; 2] = [
    "korea-sos-atlas-kit-ai-pack-1",
    "korea-sos-atlas-kit-ai-pack-2",
];
/// Every release zip has this top folder; the AI pack only fills `data/`.
const PREFIX: &str = "KoreaSOSAtlasKit/data/";
const STAGING: &str = ".ai-pack-install";

/// `name.zip`, or the copy a browser saves as `name (1).zip` / `name(1).zip`.
fn is_pack_file(file: &str, pack: &str) -> bool {
    let lower = file.to_lowercase();
    let Some(rest) = lower
        .strip_prefix(pack)
        .and_then(|r| r.strip_suffix(".zip"))
    else {
        return false;
    };
    let rest = rest.trim_start();
    rest.is_empty()
        || rest
            .strip_prefix('(')
            .and_then(|r| r.strip_suffix(')'))
            .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

/// Where downloaded zips usually are: the kit folder and up to three folders
/// above it (zips unpacked next to each other), then the Downloads folder.
pub fn search_dirs(data: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(kit) = data.parent() {
        dirs.extend(kit.ancestors().take(4).map(Path::to_path_buf));
    }
    if let Some(d) = downloads_dir() {
        dirs.push(d);
    }
    let mut seen = HashSet::new();
    dirs.retain(|d| !d.as_os_str().is_empty() && seen.insert(d.clone()));
    dirs
}

/// The two pack zips, each from the first folder that has it (newest copy).
pub fn find(data: &Path) -> [Option<PathBuf>; 2] {
    find_in(&search_dirs(data))
}
fn find_in(dirs: &[PathBuf]) -> [Option<PathBuf>; 2] {
    let mut found: [Option<PathBuf>; 2] = [None, None];
    for dir in dirs {
        let Ok(list) = fs::read_dir(dir) else {
            continue;
        };
        let files: Vec<(String, PathBuf, Option<std::time::SystemTime>)> = list
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
            .map(|e| {
                (
                    e.file_name().to_string_lossy().into_owned(),
                    e.path(),
                    e.metadata().and_then(|m| m.modified()).ok(),
                )
            })
            .collect();
        for (i, pack) in PACKS.iter().enumerate() {
            if found[i].is_some() {
                continue;
            }
            found[i] = files
                .iter()
                .filter(|(name, _, _)| is_pack_file(name, pack))
                .max_by_key(|(_, _, t)| *t)
                .map(|(_, p, _)| p.clone());
        }
    }
    found
}

#[cfg(windows)]
fn downloads_dir() -> Option<PathBuf> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStringExt;
    #[repr(C)]
    struct Guid(u32, u16, u16, [u8; 8]);
    #[link(name = "shell32")]
    extern "system" {
        fn SHGetKnownFolderPath(
            id: *const Guid,
            flags: u32,
            token: *mut c_void,
            path: *mut *mut u16,
        ) -> i32;
    }
    #[link(name = "ole32")]
    extern "system" {
        fn CoTaskMemFree(p: *mut c_void);
    }
    // FOLDERID_Downloads {374DE290-123F-4565-9164-39C4925E467B}: follows a
    // Downloads folder the user moved to another drive.
    let id = Guid(
        0x374D_E290,
        0x123F,
        0x4565,
        [0x91, 0x64, 0x39, 0xC4, 0x92, 0x5E, 0x46, 0x7B],
    );
    let mut p: *mut u16 = std::ptr::null_mut();
    let known = unsafe {
        if SHGetKnownFolderPath(&id, 0, std::ptr::null_mut(), &mut p) == 0 && !p.is_null() {
            let mut n = 0;
            while *p.add(n) != 0 {
                n += 1;
            }
            let s = std::ffi::OsString::from_wide(std::slice::from_raw_parts(p, n));
            CoTaskMemFree(p.cast());
            Some(PathBuf::from(s))
        } else {
            if !p.is_null() {
                CoTaskMemFree(p.cast());
            }
            None
        }
    };
    known.or_else(|| std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join("Downloads")))
}
#[cfg(not(windows))]
fn downloads_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Downloads"))
}

/// Free bytes on the volume that holds `dir`, if the OS tells us.
#[cfg(windows)]
pub fn free_space(dir: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetDiskFreeSpaceExW(
            dir: *const u16,
            avail: *mut u64,
            total: *mut u64,
            free: *mut u64,
        ) -> i32;
    }
    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain([0]).collect();
    let mut avail = 0u64;
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut avail,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (ok != 0).then_some(avail)
}
#[cfg(unix)]
pub fn free_space(dir: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    (unsafe { libc::statvfs(c.as_ptr(), &mut s) } == 0)
        .then(|| s.f_bavail as u64 * s.f_frsize as u64)
}
#[cfg(not(any(windows, unix)))]
pub fn free_space(_: &Path) -> Option<u64> {
    None
}

pub fn gb(bytes: u64) -> String {
    format!("{:.1}GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
}

// ---- zip reading (stored and deflate entries, zip64) ----

#[derive(Debug, Clone)]
struct Entry {
    name: String,
    method: u16,
    crc: u32,
    csize: u64,
    usize: u64,
    offset: u64,
}

fn u16le(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}
fn u32le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn u64le(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}
fn read_at(f: &mut fs::File, at: u64, len: usize) -> io::Result<Vec<u8>> {
    f.seek(SeekFrom::Start(at))?;
    let mut b = vec![0; len];
    f.read_exact(&mut b)?;
    Ok(b)
}

fn entries(f: &mut fs::File) -> Result<Vec<Entry>, String> {
    let bad = || "zip 형식이 아닙니다".to_string();
    let len = f.metadata().map_err(|e| e.to_string())?.len();
    let tail_len = len.min(22 + 65535) as usize;
    let tail = read_at(f, len - tail_len as u64, tail_len).map_err(|_| bad())?;
    let eocd = (0..=tail_len.saturating_sub(22))
        .rev()
        .find(|&i| u32le(&tail, i) == 0x0605_4b50)
        .ok_or_else(bad)?;
    let mut count = u16le(&tail, eocd + 10) as u64;
    let mut cd_size = u32le(&tail, eocd + 12) as u64;
    let mut cd_off = u32le(&tail, eocd + 16) as u64;
    if count == 0xFFFF || cd_size == 0xFFFF_FFFF || cd_off == 0xFFFF_FFFF {
        let loc = eocd.checked_sub(20).ok_or_else(bad)?;
        if u32le(&tail, loc) != 0x0706_4b50 {
            return Err(bad());
        }
        let z64 = read_at(f, u64le(&tail, loc + 8), 56).map_err(|_| bad())?;
        if u32le(&z64, 0) != 0x0606_4b50 {
            return Err(bad());
        }
        count = u64le(&z64, 32);
        cd_size = u64le(&z64, 40);
        cd_off = u64le(&z64, 48);
    }
    if cd_size > 64 << 20 || cd_off.checked_add(cd_size).is_none_or(|end| end > len) {
        return Err(bad());
    }
    let cd = read_at(f, cd_off, cd_size as usize).map_err(|_| bad())?;
    let mut out = Vec::new();
    let mut p = 0;
    for _ in 0..count {
        if p + 46 > cd.len() || u32le(&cd, p) != 0x0201_4b50 {
            return Err(bad());
        }
        let flags = u16le(&cd, p + 8);
        let (nlen, xlen, clen) = (
            u16le(&cd, p + 28) as usize,
            u16le(&cd, p + 30) as usize,
            u16le(&cd, p + 32) as usize,
        );
        if p + 46 + nlen + xlen + clen > cd.len() {
            return Err(bad());
        }
        if flags & 1 != 0 {
            return Err("암호가 걸린 zip은 풀 수 없습니다".into());
        }
        let mut e = Entry {
            name: String::from_utf8_lossy(&cd[p + 46..p + 46 + nlen]).into_owned(),
            method: u16le(&cd, p + 10),
            crc: u32le(&cd, p + 16),
            csize: u32le(&cd, p + 20) as u64,
            usize: u32le(&cd, p + 24) as u64,
            offset: u32le(&cd, p + 42) as u64,
        };
        // zip64 extra field: the 8-byte values follow in this order, only
        // for the fields that are 0xFFFFFFFF above.
        let extra = &cd[p + 46 + nlen..p + 46 + nlen + xlen];
        let mut x = 0;
        while x + 4 <= extra.len() {
            let (id, size) = (u16le(extra, x), u16le(extra, x + 2) as usize);
            let body = extra.get(x + 4..x + 4 + size).ok_or_else(bad)?;
            if id == 1 {
                let mut q = 0;
                for v in [&mut e.usize, &mut e.csize, &mut e.offset] {
                    if *v == 0xFFFF_FFFF {
                        *v = u64le(body.get(q..q + 8).ok_or_else(bad)?, 0);
                        q += 8;
                    }
                }
            }
            x += 4 + size;
        }
        out.push(e);
        p += 46 + nlen + xlen + clen;
    }
    Ok(out)
}

const CRC_TABLE: [u32; 256] = {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
};
fn crc_update(crc: u32, data: &[u8]) -> u32 {
    !data.iter().fold(!crc, |c, &b| {
        CRC_TABLE[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8)
    })
}

/// Writes one entry to `to`, returning its SHA-256. Checks CRC and size.
fn extract(
    f: &mut fs::File,
    e: &Entry,
    to: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> Result<String, String> {
    let local = read_at(f, e.offset, 30).map_err(|e| e.to_string())?;
    if u32le(&local, 0) != 0x0403_4b50 {
        return Err("zip 형식이 아닙니다".into());
    }
    let data = e.offset + 30 + u16le(&local, 26) as u64 + u16le(&local, 28) as u64;
    f.seek(SeekFrom::Start(data)).map_err(|e| e.to_string())?;
    let mut input = f.take(e.csize);
    let mut out =
        BufWriter::with_capacity(1 << 20, fs::File::create(to).map_err(|e| e.to_string())?);
    let mut sha = Sha256::new();
    let mut crc = 0u32;
    let mut written = 0u64;
    let mut emit = |chunk: &[u8], out: &mut BufWriter<fs::File>| -> Result<(), String> {
        out.write_all(chunk).map_err(|e| e.to_string())?;
        sha.update(chunk);
        crc = crc_update(crc, chunk);
        written += chunk.len() as u64;
        progress(chunk.len() as u64);
        Ok(())
    };
    let mut buf = vec![0u8; 1 << 20];
    match e.method {
        0 => loop {
            if cancel.load(Ordering::Acquire) {
                return Err("Cancelled".into());
            }
            let n = input.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            emit(&buf[..n], &mut out)?;
        },
        8 => {
            use miniz_oxide::inflate::stream::{inflate, InflateState};
            use miniz_oxide::{DataFormat, MZFlush, MZStatus};
            let mut state = InflateState::new_boxed(DataFormat::Raw);
            let mut outbuf = vec![0u8; 1 << 20];
            let (mut start, mut end, mut eof) = (0, 0, false);
            loop {
                if cancel.load(Ordering::Acquire) {
                    return Err("Cancelled".into());
                }
                if start == end && !eof {
                    end = input.read(&mut buf).map_err(|e| e.to_string())?;
                    start = 0;
                    eof = end == 0;
                }
                let flush = if eof { MZFlush::Finish } else { MZFlush::None };
                let r = inflate(&mut state, &buf[start..end], &mut outbuf, flush);
                start += r.bytes_consumed;
                emit(&outbuf[..r.bytes_written], &mut out)?;
                match r.status {
                    Ok(MZStatus::StreamEnd) => break,
                    Ok(_) if eof && r.bytes_written == 0 && r.bytes_consumed == 0 => {
                        return Err("zip 데이터가 중간에 끊겼습니다".into())
                    }
                    Ok(_) => {}
                    // BufError: needs more input or output space; loop again.
                    Err(miniz_oxide::MZError::Buf) if !eof || r.bytes_written > 0 => {}
                    Err(_) => return Err("zip 데이터가 손상되었습니다".into()),
                }
            }
        }
        m => return Err(format!("지원하지 않는 zip 압축 방식({m})입니다")),
    }
    out.flush().map_err(|e| e.to_string())?;
    if written != e.usize || crc != e.crc {
        return Err("zip 데이터가 손상되었습니다".into());
    }
    Ok(format!("{:x}", sha.finalize()))
}

// ---- install ----

/// A licence or build note (plain text), allowed in the model and engine
/// folders without being in the manifest.
fn is_notice(rel: &str) -> bool {
    let (dir, file) = rel.rsplit_once('/').unwrap_or(("", rel));
    // models/ or runtime/llama.cpp-<platform>/ itself, not a subfolder.
    let dir_ok =
        dir == "models" || (dir.starts_with("runtime/llama.cpp-") && dir.matches('/').count() == 1);
    let lower = file.to_lowercase();
    let program = [
        ".exe", ".dll", ".com", ".bat", ".cmd", ".ps1", ".vbs", ".js", ".scr", ".msi", ".lnk",
        ".so", ".dylib",
    ]
    .iter()
    .any(|x| lower.ends_with(x));
    dir_ok
        && !program
        && (lower.ends_with(".txt") || lower.ends_with(".md") || lower.starts_with("license"))
}
fn safe_rel(rel: &str) -> bool {
    !rel.is_empty()
        && !rel.contains('\\')
        && !rel.contains(':')
        && rel
            .split('/')
            .all(|c| !c.is_empty() && c != "." && c != "..")
}

/// Removes the staging folder however `install` ends.
struct Staging(PathBuf);
impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Unpacks both zips into `data` and returns the manifest files it placed
/// with their verified SHA-256, so the first AI start need not hash them
/// again. `progress(done, total)` reports unpacked bytes.
pub fn install(
    data: &Path,
    zips: &[PathBuf; 2],
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<Vec<(PathBuf, String)>, String> {
    let assets = crate::layout::ai_assets(data)?;
    let mut expected: HashMap<String, String> = HashMap::new();
    for a in std::iter::once(&assets.model)
        .chain(&assets.model_parts)
        .chain(std::iter::once(&assets.engine))
        .chain(&assets.engine_libs)
    {
        let rel = a
            .path
            .strip_prefix(data)
            .map_err(|_| "AI 파일 경로가 data 폴더 밖을 가리킵니다".to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        expected.insert(rel, a.sha256.to_lowercase());
    }

    // Read both central directories and check every name before writing.
    let mut plan: Vec<(usize, Entry, String)> = Vec::new();
    let mut files = Vec::new();
    let mut seen = HashSet::new();
    for (i, zip) in zips.iter().enumerate() {
        let name = zip
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut f = fs::File::open(zip).map_err(|e| format!("{name}을(를) 열 수 없습니다: {e}"))?;
        for e in entries(&mut f).map_err(|e| format!("{name}: {e}"))? {
            if e.name.ends_with('/') {
                continue;
            }
            let rel = e.name.strip_prefix(PREFIX).unwrap_or("").to_string();
            if !safe_rel(&rel) || !(expected.contains_key(&rel) || is_notice(&rel)) {
                return Err(format!(
                    "{name}에 AI 추가팩에 없어야 할 파일이 있습니다({}). 배포 페이지에서 받은 zip인지 확인하세요.",
                    e.name
                ));
            }
            if !seen.insert(rel.clone()) {
                return Err(format!("{name}: 같은 파일이 두 번 들어 있습니다({rel})"));
            }
            plan.push((i, e, rel));
        }
        files.push(f);
    }
    if let Some(missing) = expected.keys().find(|k| !seen.contains(*k)) {
        return Err(format!(
            "AI 추가팩 zip에 {missing}이(가) 없습니다. 이 앱과 같은 버전의 AI 추가팩 2개(ai-pack-1, ai-pack-2)인지 확인하세요."
        ));
    }

    let total: u64 = plan.iter().map(|(_, e, _)| e.usize).sum();
    if let Some(free) = free_space(data) {
        // Existing copies are replaced only after everything is checked,
        // so the new files need their full size first.
        let need = total + (64 << 20);
        if free < need {
            return Err(format!(
                "저장 공간이 부족합니다. 설치에 {}가 필요한데 남은 공간은 {}입니다. 공간을 비우거나 여유가 있는 드라이브로 키트 폴더를 옮긴 뒤 다시 p를 누르세요.",
                gb(need),
                gb(free)
            ));
        }
    }

    let staging = Staging(data.join(STAGING));
    let _ = fs::remove_dir_all(&staging.0);
    fs::create_dir_all(&staging.0).map_err(|e| format!("임시 폴더를 만들 수 없습니다: {e}"))?;
    let mut done = 0u64;
    progress(0, total);
    for (i, e, rel) in &plan {
        if cancel.load(Ordering::Acquire) {
            return Err("Cancelled".into());
        }
        let to = staging.0.join(rel);
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let zip_name = zips[*i]
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let sha = extract(&mut files[*i], e, &to, cancel, &mut |n| {
            done += n;
            progress(done, total);
        })
        .map_err(|err| {
            if err == "Cancelled" {
                err
            } else {
                format!(
                    "{zip_name}을(를) 푸는 중 문제가 생겼습니다: {err}. zip을 다시 받아 주세요."
                )
            }
        })?;
        if let Some(want) = expected.get(rel) {
            if &sha != want {
                return Err(format!(
                    "{zip_name}의 {rel} 해시가 이 앱의 목록(assets.json)과 다릅니다. 이 앱과 같은 버전의 AI 추가팩을 다시 받아 주세요."
                ));
            }
        }
    }

    // Everything checked: move into place.
    let mut placed = Vec::new();
    for (_, _, rel) in &plan {
        let target = data.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        if target.exists() {
            fs::remove_file(&target).map_err(|e| format!("{rel}을(를) 바꿀 수 없습니다: {e}"))?;
        }
        fs::rename(staging.0.join(rel), &target)
            .map_err(|e| format!("{rel}을(를) 옮길 수 없습니다: {e}"))?;
        if let Some(sha) = expected.get(rel) {
            placed.push((target, sha.clone()));
        }
    }
    Ok(placed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ksak-pack-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }
    fn sha(b: &[u8]) -> String {
        format!("{:x}", Sha256::digest(b))
    }
    /// Minimal zip writer: `deflate` entries are compressed with miniz_oxide.
    fn zip(path: &Path, files: &[(&str, &[u8], bool)]) {
        let (mut out, mut cd) = (Vec::new(), Vec::new());
        for (name, body, deflate) in files {
            let data = if *deflate {
                miniz_oxide::deflate::compress_to_vec(body, 6)
            } else {
                body.to_vec()
            };
            let crc = crc_update(0, body);
            let method: u16 = if *deflate { 8 } else { 0 };
            let off = out.len() as u32;
            let mut head = |sig: u32, central: bool| {
                let mut h = Vec::new();
                h.extend(sig.to_le_bytes());
                if central {
                    h.extend(20u16.to_le_bytes());
                }
                h.extend(20u16.to_le_bytes());
                h.extend(0u16.to_le_bytes());
                h.extend(method.to_le_bytes());
                h.extend([0u8; 4]);
                h.extend(crc.to_le_bytes());
                h.extend((data.len() as u32).to_le_bytes());
                h.extend((body.len() as u32).to_le_bytes());
                h.extend((name.len() as u16).to_le_bytes());
                h.extend(0u16.to_le_bytes());
                if central {
                    // comment length, disk, internal attributes, external attributes
                    h.extend([0u8; 6]);
                    h.extend(0u32.to_le_bytes());
                    h.extend(off.to_le_bytes());
                }
                h.extend(name.as_bytes());
                h
            };
            let local = head(0x0403_4b50, false);
            let central = head(0x0201_4b50, true);
            out.extend(local);
            out.extend(&data);
            cd.extend(central);
        }
        let cd_off = out.len() as u32;
        out.extend(&cd);
        out.extend(0x0605_4b50u32.to_le_bytes());
        out.extend([0u8; 4]);
        out.extend((files.len() as u16).to_le_bytes());
        out.extend((files.len() as u16).to_le_bytes());
        out.extend((cd.len() as u32).to_le_bytes());
        out.extend(cd_off.to_le_bytes());
        out.extend(0u16.to_le_bytes());
        fs::write(path, out).unwrap();
    }
    fn kit(base: &Path, model: &[u8], part: &[u8], engine: &[u8], lib: &[u8]) -> PathBuf {
        let data = base.join("KoreaSOSAtlasKit/data");
        fs::create_dir_all(&data).unwrap();
        let platform = crate::layout::platform();
        let m = serde_json::json!({
            "version": 1,
            "model": {"file": "models/m-1.gguf", "sha256": sha(model),
                      "parts": [{"file": "models/m-2.gguf", "sha256": sha(part)}]},
            "engine": {platform: {"file": "runtime/llama.cpp-x/server", "sha256": sha(engine),
                                  "libs": {"runtime/llama.cpp-x/lib.dll": sha(lib)}}}
        });
        fs::write(data.join("assets.json"), m.to_string()).unwrap();
        data
    }

    #[test]
    fn pack_names_and_browser_copies_match() {
        let p = PACKS[0];
        assert!(is_pack_file("korea-sos-atlas-kit-ai-pack-1.zip", p));
        assert!(is_pack_file("Korea-SOS-Atlas-Kit-AI-Pack-1 (2).zip", p));
        assert!(is_pack_file("korea-sos-atlas-kit-ai-pack-1(1).zip", p));
        assert!(!is_pack_file(
            "korea-sos-atlas-kit-ai-pack-1.zip.crdownload",
            p
        ));
        assert!(!is_pack_file("korea-sos-atlas-kit-ai-pack-12.zip", p));
        assert!(!is_pack_file("korea-sos-atlas-kit-ai-pack-2.zip", p));
    }

    #[test]
    fn downloads_folder_and_free_space_are_found() {
        let d = downloads_dir().expect("Downloads folder");
        eprintln!("downloads: {}", d.display());
        assert!(d.is_absolute());
        let free = free_space(&std::env::temp_dir()).expect("free space");
        eprintln!("free: {}", gb(free));
        assert!(free > 0);
    }

    #[test]
    fn notices_and_paths() {
        assert!(is_notice("models/MODELS.md"));
        assert!(is_notice("runtime/llama.cpp-win-x64/LICENSE-llama.cpp"));
        assert!(is_notice("runtime/llama.cpp-win-x64/BUILD-INFO.txt"));
        assert!(!is_notice("runtime/llama.cpp-win-x64/evil.dll"));
        assert!(!is_notice("runtime/llama.cpp-win-x64/LICENSE.exe"));
        assert!(!is_notice("content-library/approved.json"));
        assert!(!is_notice("runtime/llama.cpp-x/sub/a.txt"));
        assert!(!is_notice("README.md"));
        assert!(!safe_rel("models/../x"));
        assert!(!safe_rel("C:/x"));
        assert!(!safe_rel("models\\x"));
        assert!(safe_rel("models/x.gguf"));
    }

    #[test]
    fn finds_packs_above_the_kit_and_installs_them() {
        let base = tmp("install");
        let (model, part, engine, lib) = (
            vec![7u8; 3 << 20],
            b"part two".to_vec(),
            b"engine".to_vec(),
            b"library ".repeat(5000),
        );
        let data = kit(&base, &model, &part, &engine, &lib);
        let z1 = base.join("korea-sos-atlas-kit-ai-pack-1.zip");
        let z2 = base.join("korea-sos-atlas-kit-ai-pack-2 (1).zip");
        zip(
            &z1,
            &[
                ("KoreaSOSAtlasKit/data/models/MODELS.md", b"notice", true),
                ("KoreaSOSAtlasKit/data/models/m-1.gguf", &model, false),
                (
                    "KoreaSOSAtlasKit/data/runtime/llama.cpp-x/server",
                    &engine,
                    true,
                ),
                (
                    "KoreaSOSAtlasKit/data/runtime/llama.cpp-x/lib.dll",
                    &lib,
                    true,
                ),
            ],
        );
        zip(
            &z2,
            &[("KoreaSOSAtlasKit/data/models/m-2.gguf", &part, false)],
        );
        let dirs: Vec<PathBuf> = data
            .parent()
            .unwrap()
            .ancestors()
            .take(4)
            .map(Path::to_path_buf)
            .collect();
        let found = find_in(&dirs);
        assert_eq!(found, [Some(z1.clone()), Some(z2.clone())]);
        assert!(crate::layout::ai_unavailable_reason(&data).is_some());
        let mut last = (0, 0);
        let placed = install(&data, &[z1, z2], &AtomicBool::new(false), &mut |d, t| {
            last = (d, t)
        })
        .unwrap();
        assert_eq!(last.0, last.1);
        assert_eq!(placed.len(), 4);
        assert_eq!(fs::read(data.join("models/m-1.gguf")).unwrap(), model);
        assert_eq!(
            fs::read(data.join("runtime/llama.cpp-x/lib.dll")).unwrap(),
            lib
        );
        assert_eq!(fs::read(data.join("models/MODELS.md")).unwrap(), b"notice");
        assert!(!data.join(STAGING).exists());
        assert_eq!(crate::layout::ai_unavailable_reason(&data), None);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn wrong_or_foreign_zips_change_nothing() {
        let base = tmp("reject");
        let data = kit(&base, b"model", b"part", b"engine", b"lib");
        let z2 = base.join("korea-sos-atlas-kit-ai-pack-2.zip");
        zip(
            &z2,
            &[("KoreaSOSAtlasKit/data/models/m-2.gguf", b"part", false)],
        );
        let z1 = base.join("korea-sos-atlas-kit-ai-pack-1.zip");
        let no = AtomicBool::new(false);

        // A different model: hash check fails, nothing is placed.
        zip(
            &z1,
            &[
                ("KoreaSOSAtlasKit/data/models/m-1.gguf", b"other", true),
                (
                    "KoreaSOSAtlasKit/data/runtime/llama.cpp-x/server",
                    b"engine",
                    true,
                ),
                (
                    "KoreaSOSAtlasKit/data/runtime/llama.cpp-x/lib.dll",
                    b"lib",
                    true,
                ),
            ],
        );
        let err = install(&data, &[z1.clone(), z2.clone()], &no, &mut |_, _| {}).unwrap_err();
        assert!(err.contains("해시"), "{err}");
        assert!(!data.join("models").exists() && !data.join(STAGING).exists());

        // An extra program next to the engine is refused before writing.
        zip(
            &z1,
            &[
                ("KoreaSOSAtlasKit/data/models/m-1.gguf", b"model", true),
                (
                    "KoreaSOSAtlasKit/data/runtime/llama.cpp-x/server",
                    b"engine",
                    true,
                ),
                (
                    "KoreaSOSAtlasKit/data/runtime/llama.cpp-x/lib.dll",
                    b"lib",
                    true,
                ),
                (
                    "KoreaSOSAtlasKit/data/runtime/llama.cpp-x/extra.dll",
                    b"x",
                    true,
                ),
            ],
        );
        let err = install(&data, &[z1.clone(), z2.clone()], &no, &mut |_, _| {}).unwrap_err();
        assert!(err.contains("extra.dll"), "{err}");
        assert!(!data.join("runtime").exists());

        // A missing file is reported by name.
        zip(
            &z1,
            &[("KoreaSOSAtlasKit/data/models/m-1.gguf", b"model", true)],
        );
        let err = install(&data, &[z1.clone(), z2.clone()], &no, &mut |_, _| {}).unwrap_err();
        assert!(err.contains("runtime/llama.cpp-x/"), "{err}");

        // Cancel leaves no staging folder.
        zip(
            &z1,
            &[
                ("KoreaSOSAtlasKit/data/models/m-1.gguf", b"model", true),
                (
                    "KoreaSOSAtlasKit/data/runtime/llama.cpp-x/server",
                    b"engine",
                    true,
                ),
                (
                    "KoreaSOSAtlasKit/data/runtime/llama.cpp-x/lib.dll",
                    b"lib",
                    true,
                ),
            ],
        );
        let err = install(&data, &[z1, z2], &AtomicBool::new(true), &mut |_, _| {}).unwrap_err();
        assert_eq!(err, "Cancelled");
        assert!(!data.join(STAGING).exists() && !data.join("models").exists());
        fs::remove_dir_all(base).unwrap();
    }

    /// The real release zips: `KSAK_PACK_DIR=dist/release cargo test --release
    /// real_release_packs -- --ignored`. Uses the release manifest with its
    /// engine entry moved to this platform.
    #[test]
    #[ignore]
    fn real_release_packs_install() {
        let dir = PathBuf::from(std::env::var("KSAK_PACK_DIR").expect("KSAK_PACK_DIR"));
        let base = tmp("real");
        let data = base.join("KoreaSOSAtlasKit/data");
        fs::create_dir_all(&data).unwrap();
        let mut m: serde_json::Value = serde_json::from_slice(
            &fs::read(dir.join("stage/atlas/KoreaSOSAtlasKit/data/assets.json")).unwrap(),
        )
        .unwrap();
        let engine = m["engine"]["windows-x86_64"].take();
        m["engine"] = serde_json::json!({crate::layout::platform(): engine});
        fs::write(data.join("assets.json"), m.to_string()).unwrap();
        let zips = [
            dir.join(format!("{}.zip", PACKS[0])),
            dir.join(format!("{}.zip", PACKS[1])),
        ];
        let t = std::time::Instant::now();
        let placed = install(&data, &zips, &AtomicBool::new(false), &mut |_, _| {}).unwrap();
        eprintln!("installed {} files in {:?}", placed.len(), t.elapsed());
        assert!(crate::layout::ai_unavailable_reason(&data).is_none());
        assert!(data.join("models/MODELS.md").is_file());
        fs::remove_dir_all(base).unwrap();
    }
}
