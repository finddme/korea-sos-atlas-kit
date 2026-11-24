// SPDX-License-Identifier: AGPL-3.0-only
//! Copies text to the system clipboard without any network or extra install.
//! The app captures the mouse for its buttons, so the terminal's own
//! drag-to-select does not work inside it; `c` copies the answer instead.
use std::io::{self, Write};

/// Returns how the text was copied, or an error when no method worked.
pub fn copy(text: &str) -> Result<&'static str, String> {
    if text.trim().is_empty() {
        return Err("복사할 내용이 없습니다".into());
    }
    match native(text) {
        Ok(how) => Ok(how),
        Err(e) => {
            // Last resort: ask the terminal itself (OSC 52). Many terminals
            // (Windows Terminal, iTerm2, kitty, xterm with allowWindowOps)
            // honour it; others ignore it silently.
            osc52(text).map_err(|o| format!("{e}; {o}"))?;
            Ok("터미널 클립보드 요청(OSC 52)")
        }
    }
}

fn osc52(text: &str) -> Result<(), String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let mut out = io::stdout();
    write!(out, "\x1b]52;c;{}\x07", STANDARD.encode(text.as_bytes()))
        .and_then(|_| out.flush())
        .map_err(|e| e.to_string())
}

#[cfg(windows)]
fn native(text: &str) -> Result<&'static str, String> {
    use std::ffi::c_void;
    #[link(name = "user32")]
    extern "system" {
        fn OpenClipboard(owner: *mut c_void) -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(format: u32, mem: *mut c_void) -> *mut c_void;
        fn CloseClipboard() -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
        fn GlobalLock(mem: *mut c_void) -> *mut c_void;
        fn GlobalUnlock(mem: *mut c_void) -> i32;
        fn GlobalFree(mem: *mut c_void) -> *mut c_void;
    }
    const CF_UNICODETEXT: u32 = 13;
    const GMEM_MOVEABLE: u32 = 2;
    // Windows text uses CRLF line ends.
    let wide: Vec<u16> = text
        .replace("\r\n", "\n")
        .replace('\n', "\r\n")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return Err("클립보드를 열 수 없습니다".into());
        }
        let result = (|| {
            EmptyClipboard();
            let mem = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2);
            if mem.is_null() {
                return Err("메모리 할당 실패".to_string());
            }
            let dst = GlobalLock(mem) as *mut u16;
            if dst.is_null() {
                GlobalFree(mem);
                return Err("메모리 잠금 실패".to_string());
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), dst, wide.len());
            GlobalUnlock(mem);
            if SetClipboardData(CF_UNICODETEXT, mem).is_null() {
                GlobalFree(mem);
                return Err("클립보드에 쓰지 못했습니다".to_string());
            }
            Ok("Windows 클립보드")
        })();
        CloseClipboard();
        result
    }
}

#[cfg(not(windows))]
fn native(text: &str) -> Result<&'static str, String> {
    use std::process::{Command, Stdio};
    fn pipe(program: &str, args: &[&str], input: &[u8]) -> bool {
        let Ok(mut child) = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            return false;
        };
        let wrote = child
            .stdin
            .take()
            .map(|mut s| s.write_all(input).is_ok())
            .unwrap_or(false);
        child.wait().map(|s| s.success()).unwrap_or(false) && wrote
    }
    if cfg!(target_os = "macos") {
        return if pipe("pbcopy", &[], text.as_bytes()) {
            Ok("macOS 클립보드")
        } else {
            Err("pbcopy 실패".into())
        };
    }
    if std::env::var_os("WSL_DISTRO_NAME").is_some() {
        // clip.exe would keep the UTF-16 BOM as a character, so read UTF-8
        // from stdin in PowerShell instead.
        let script = "[Console]::InputEncoding = [Text.UTF8Encoding]::new($false); \
                      Set-Clipboard -Value ([Console]::In.ReadToEnd())";
        let input = text.replace("\r\n", "\n").replace('\n', "\r\n");
        if pipe(
            "powershell.exe",
            &["-NoProfile", "-NonInteractive", "-Command", script],
            input.as_bytes(),
        ) {
            return Ok("Windows 클립보드(WSL)");
        }
    }
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    if wayland && pipe("wl-copy", &[], text.as_bytes()) {
        return Ok("클립보드(wl-copy)");
    }
    if std::env::var_os("DISPLAY").is_some() {
        if pipe("xclip", &["-selection", "clipboard"], text.as_bytes()) {
            return Ok("클립보드(xclip)");
        }
        if pipe("xsel", &["--clipboard", "--input"], text.as_bytes()) {
            return Ok("클립보드(xsel)");
        }
    }
    Err("이 환경의 클립보드 도구를 찾지 못했습니다".into())
}

#[cfg(test)]
mod tests {
    #[test]
    fn empty_text_is_not_copied() {
        assert!(super::copy("  \n").is_err());
    }
}
