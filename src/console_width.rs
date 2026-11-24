// SPDX-License-Identifier: AGPL-3.0-only
//! East Asian "ambiguous width" characters (· ‘ ’ → ° ½ ① …) are one cell in
//! the app's layout (unicode-width), but the classic Windows console with a
//! Korean/CJK font draws many of them two cells wide. The next character is
//! then overwritten ("응급·건강" shows as "응급· 강"). Windows Terminal and
//! Linux terminals draw them one cell wide and are unaffected.
//!
//! At startup (Windows only) each candidate is written to a hidden console
//! buffer to measure how wide the console really draws it. Only measured-wide
//! characters are touched:
//! - document and AI text: replaced with forms that keep the meaning
//!   (½ → 1/2, ① → (1), ℃ → 도C, · → ㆍ), so quantities are never lost;
//! - anything still left in a frame (UI strings): drawn over two cells when a
//!   space follows, otherwise replaced with a one-cell look-alike.
use ratatui::buffer::Buffer;
use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::OnceLock;
use unicode_width::UnicodeWidthChar;

static WIDE: OnceLock<HashSet<char>> = OnceLock::new();

/// One cell in the layout but two cells in a CJK-locale terminal.
pub fn is_ambiguous(c: char) -> bool {
    // Box drawing and block elements stay narrow even in the CJK console.
    !('\u{2500}'..='\u{259F}').contains(&c) && c.width() == Some(1) && c.width_cjk() == Some(2)
}

/// Measures the candidates once; later calls keep the first result.
pub fn init(candidates: impl IntoIterator<Item = char>) -> &'static HashSet<char> {
    WIDE.get_or_init(|| {
        let mut list: Vec<char> = candidates
            .into_iter()
            .filter(|c| is_ambiguous(*c))
            .collect();
        list.extend(UI_CHARS.chars());
        list.sort_unstable();
        list.dedup();
        measure(&list)
    })
}

fn wide() -> Option<&'static HashSet<char>> {
    WIDE.get().filter(|w| !w.is_empty())
}

/// Characters used in the app's own strings.
const UI_CHARS: &str = "·…↑↓←→×÷‘’“”∼";

#[cfg(windows)]
fn measure(chars: &[char]) -> HashSet<char> {
    use std::ffi::c_void;
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Coord {
        x: i16,
        y: i16,
    }
    #[repr(C)]
    #[derive(Default)]
    struct Info {
        size: Coord,
        cursor: Coord,
        attributes: u16,
        window: [i16; 4],
        max: Coord,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateConsoleScreenBuffer(
            access: u32,
            share: u32,
            security: *const c_void,
            flags: u32,
            data: *const c_void,
        ) -> *mut c_void;
        fn SetConsoleCursorPosition(h: *mut c_void, at: Coord) -> i32;
        fn WriteConsoleW(
            h: *mut c_void,
            text: *const u16,
            len: u32,
            written: *mut u32,
            reserved: *const c_void,
        ) -> i32;
        fn GetConsoleScreenBufferInfo(h: *mut c_void, info: *mut Info) -> i32;
        fn CloseHandle(h: *mut c_void) -> i32;
    }
    const GENERIC_READ_WRITE: u32 = 0xC000_0000;
    const SHARE_READ_WRITE: u32 = 3;
    const TEXTMODE_BUFFER: u32 = 1;
    let mut wide = HashSet::new();
    unsafe {
        // A buffer that is never made active: nothing appears on screen.
        let h = CreateConsoleScreenBuffer(
            GENERIC_READ_WRITE,
            SHARE_READ_WRITE,
            std::ptr::null(),
            TEXTMODE_BUFFER,
            std::ptr::null(),
        );
        if h.is_null() || h as isize == -1 {
            return wide; // not attached to a console
        }
        for &c in chars {
            let mut units = [0u16; 2];
            let text = c.encode_utf16(&mut units);
            let mut info = Info::default();
            let mut written = 0;
            if SetConsoleCursorPosition(h, Coord::default()) != 0
                && WriteConsoleW(
                    h,
                    text.as_ptr(),
                    text.len() as u32,
                    &mut written,
                    std::ptr::null(),
                ) != 0
                && GetConsoleScreenBufferInfo(h, &mut info) != 0
                && info.cursor.x >= 2
            {
                wide.insert(c);
            }
        }
        CloseHandle(h);
    }
    wide
}

#[cfg(not(windows))]
fn measure(_chars: &[char]) -> HashSet<char> {
    HashSet::new()
}

/// Meaning-preserving replacement for text the reader relies on.
fn spelled(c: char) -> Option<Cow<'static, str>> {
    const CIRCLED: [&str; 20] = [
        "(1)", "(2)", "(3)", "(4)", "(5)", "(6)", "(7)", "(8)", "(9)", "(10)", "(11)", "(12)",
        "(13)", "(14)", "(15)", "(16)", "(17)", "(18)", "(19)", "(20)",
    ];
    const ROMAN: [&str; 12] = [
        "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII",
    ];
    const ROMAN_LOWER: [&str; 12] = [
        "i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x", "xi", "xii",
    ];
    let s: &'static str = match c {
        '·' => "ㆍ",
        '‘' | '’' => "'",
        '“' | '”' => "\"",
        '→' => "->",
        '←' => "<-",
        '↔' => "<->",
        '↑' => "^",
        '↓' => "v",
        '↕' => "^v",
        '∼' | '～' => "~",
        '×' => "x",
        '÷' => "/",
        '∥' => "||",
        '★' | '☆' => "*",
        '…' => "...",
        '°' => "도",
        '℃' => "도C",
        '℉' => "도F",
        '½' => "1/2",
        '¼' => "1/4",
        '¾' => "3/4",
        '⅓' => "1/3",
        '⅔' => "2/3",
        '⅛' => "1/8",
        '§' => "S",
        '±' => "+-",
        '¹' => "1",
        '²' => "2",
        '³' => "3",
        '\u{2460}'..='\u{2473}' => CIRCLED[c as usize - 0x2460],
        '\u{2160}'..='\u{216B}' => ROMAN[c as usize - 0x2160],
        '\u{2170}'..='\u{217B}' => ROMAN_LOWER[c as usize - 0x2170],
        '\u{2080}'..='\u{2089}' => {
            return Some(
                char::from(b'0' + (c as u32 - 0x2080) as u8)
                    .to_string()
                    .into(),
            )
        }
        _ => return None,
    };
    Some(Cow::Borrowed(s))
}

/// One-cell stand-in for whatever is left in a frame.
fn single_cell(c: char, wide: &HashSet<char>) -> char {
    let options: &[char] = match c {
        '·' => &['•', '.'],
        '‘' | '’' => &['\''],
        '“' | '”' => &['"'],
        '→' => &['>'],
        '←' => &['<'],
        '↑' => &['^'],
        '↓' => &['v'],
        '↔' => &['-'],
        '↕' => &['|'],
        '∼' => &['~'],
        '×' => &['x'],
        '÷' => &['/'],
        '∥' => &['|'],
        '★' => &['*'],
        '…' => &['.'],
        '°' => &['o'],
        _ => &['?'],
    };
    options
        .iter()
        .copied()
        .find(|o| !wide.contains(o))
        .unwrap_or('?')
}

fn spell_with<'a>(text: &'a str, wide: &HashSet<char>) -> Cow<'a, str> {
    if !text.chars().any(|c| wide.contains(&c)) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + 16);
    for c in text.chars() {
        match wide.contains(&c).then(|| spelled(c)).flatten() {
            Some(s) => out.push_str(&s),
            None => out.push(c),
        }
    }
    Cow::Owned(out)
}

/// Rewrites measured-wide characters in text the reader relies on.
pub fn text(text: &str) -> Cow<'_, str> {
    match wide() {
        Some(w) => spell_with(text, w),
        None => Cow::Borrowed(text),
    }
}

fn fix_with(buf: &mut Buffer, wide: &HashSet<char>) {
    let width = buf.area.width as usize;
    if width == 0 {
        return;
    }
    for row in buf.content.chunks_mut(width) {
        let mut i = 0;
        while i < row.len() {
            let mut chars = row[i].symbol().chars();
            if let (Some(c), None) = (chars.next(), chars.next()) {
                if wide.contains(&c) {
                    if i + 1 < row.len() && row[i + 1].symbol() == " " && !row[i + 1].skip {
                        // The console draws the glyph over this space.
                        row[i + 1].set_skip(true);
                        i += 2;
                        continue;
                    }
                    row[i].set_char(single_cell(c, wide));
                }
            }
            i += 1;
        }
    }
}

/// Last pass over a frame before it is sent to the terminal.
pub fn fix_buffer(buf: &mut Buffer) {
    if let Some(w) = wide() {
        fix_with(buf, w);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    fn set(s: &str) -> HashSet<char> {
        s.chars().collect()
    }

    #[test]
    fn ambiguous_detection_excludes_box_drawing_and_hangul() {
        assert!(is_ambiguous('·') && is_ambiguous('½') && is_ambiguous('→'));
        assert!(!is_ambiguous('─') && !is_ambiguous('┊') && !is_ambiguous('가'));
        assert!(!is_ambiguous('a'));
    }

    #[test]
    fn quantities_and_numbering_keep_their_meaning() {
        let w = set("·½①℃°→Ⅱ₂");
        assert_eq!(
            spell_with("물 ½컵·소금 ①번, 38℃ 25° A→B Ⅱ단계 CO₂", &w),
            "물 1/2컵ㆍ소금 (1)번, 38도C 25도 A->B II단계 CO2"
        );
        // Characters the console draws narrow are left alone.
        assert_eq!(spell_with("A–B •", &w), "A–B •");
        assert!(matches!(spell_with("변화 없음", &w), Cow::Borrowed(_)));
    }

    #[test]
    fn every_spelled_form_is_itself_safe() {
        let all: Vec<char> = ('\u{00A0}'..='\u{2BFF}')
            .filter(|c| is_ambiguous(*c))
            .collect();
        for c in all {
            if let Some(s) = spelled(c) {
                assert!(!s.chars().any(|x| is_ambiguous(x)), "{c} -> {s}");
            }
        }
    }

    #[test]
    fn frame_keeps_glyph_over_a_following_space_or_substitutes() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 8, 1));
        buf.set_string(0, 0, "a · b·c", ratatui::style::Style::default());
        fix_with(&mut buf, &set("·"));
        let cells: Vec<(&str, bool)> = buf.content.iter().map(|c| (c.symbol(), c.skip)).collect();
        assert_eq!(cells[2], ("·", false));
        assert_eq!(cells[3], (" ", true));
        assert_eq!(cells[5], ("•", false));
        assert_eq!(cells[6], ("c", false));
        // '•' is used only while the console draws it narrow.
        let mut buf = Buffer::empty(Rect::new(0, 0, 3, 1));
        buf.set_string(0, 0, "a·b", ratatui::style::Style::default());
        fix_with(&mut buf, &set("·•"));
        assert_eq!(buf.content[1].symbol(), ".");
    }

    #[test]
    fn off_windows_nothing_is_measured_wide() {
        if !cfg!(windows) {
            assert!(measure(&['·', '½']).is_empty());
        }
    }
}
