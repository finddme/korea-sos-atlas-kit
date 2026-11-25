// SPDX-License-Identifier: AGPL-3.0-only
// Korea SOS Atlas Kit terminal application.
use base64::{engine::general_purpose::STANDARD, Engine};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
        MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use percent_encoding::{utf8_percent_encode, CONTROLS};
use ratatui::{
    backend::CrosstermBackend,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame, Terminal,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use unicode_casefold::UnicodeCaseFold;
use unicode_width::UnicodeWidthChar;
mod agent_prompts;
mod agentic_policy;
mod ai_policy;
mod ai_runtime;
mod ai_stream;
mod clipboard;
mod console_width;
mod energy_calc;
mod evidence_bundles;
mod facility_view;
mod grounding;
mod layout;
mod library;
mod maps;
mod medical_facilities;
mod search_processing;
mod shelters;
mod status;
mod topic_hints;
mod water_facilities;
const URI_ESC: &percent_encoding::AsciiSet = &CONTROLS.add(b' ').add(b'%').add(b'#').add(b'?');
const CATS: [&str; 12] = [
    "전체",
    "식수",
    "식량·조리",
    "응급·건강",
    "대피·안전",
    "피난처·체온",
    "야외기술",
    "전력·설비",
    "위생",
    "지도",
    "통신",
    "대피시설",
];
#[derive(Deserialize)]
struct Pack {
    original: String,
    original_sha256: String,
    #[serde(default)]
    source_label: Option<String>,
    documents: Vec<Meta>,
}
#[derive(Deserialize)]
struct Meta {
    id: String,
    title: String,
    categories: Vec<String>,
    file: String,
    sha256: String,
    sources: Vec<Source>,
}
#[derive(Deserialize)]
struct Source {
    spread: u64,
    printed_page: Option<serde_json::Value>,
}
#[derive(Serialize)]
struct Doc {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    location: Option<shelters::Location>,
    id: String,
    title: String,
    categories: Vec<String>,
    file: String,
    body: String,
    display_body: String,
    record: String,
    source: String,
    original_uri: String,
    spread: u64,
    #[serde(skip)]
    original_path: PathBuf,
}
fn hash(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn load(root: &Path) -> Result<(Vec<Doc>, PathBuf), Box<dyn std::error::Error>> {
    if root.join("approved.json").exists() {
        let (mut docs, path) = library::load_approved(root)?;
        docs.extend(library::load_public_manuals(root)?);
        docs.extend(shelters::load(root)?);
        docs.extend(medical_facilities::load(root)?);
        docs.extend(water_facilities::load(root)?);
        docs.extend(library::load_default_local(root)?);
        return Ok((docs, path));
    }
    if !cfg!(test) {
        return Err(
            "An approved.json content library is required; legacy packs are test fixtures only"
                .into(),
        );
    }
    let p: Pack = serde_json::from_slice(&fs::read(root.join("pack.json"))?)?;
    let pdf = root.join(&p.original).canonicalize()?;
    if hash(&fs::read(&pdf)?) != p.original_sha256 {
        return Err("Original PDF hash mismatch".into());
    }
    let source_label = p
        .source_label
        .as_deref()
        .unwrap_or("행정안전부 · 부산 서구청");
    let mut docs = Vec::new();
    for m in p.documents {
        let text = fs::read_to_string(root.join(&m.file))?;
        if hash(text.as_bytes()) != m.sha256 {
            return Err(format!("Document hash mismatch: {}", m.id).into());
        }
        if text.chars().any(|c| c < ' ' && c != '\n' && c != '\t') {
            return Err("Unreviewed control characters".into());
        }
        let (content, record) = text
            .split_once("\n## 출처·편집 기록\n")
            .ok_or("Missing source record")?;
        let body = content
            .split_once('\n')
            .ok_or("Missing title")?
            .1
            .trim()
            .to_string();
        let mut lines: Vec<String> = Vec::new();
        for l in body.lines().filter(|l| !l.trim().is_empty()) {
            if let Some(t) = l.strip_prefix("## ") {
                if !lines.is_empty() {
                    lines.push(String::new())
                }
                lines.push(t.into())
            } else if let Some(t) = l.strip_prefix("- ") {
                lines.push(format!("• {t}"))
            } else {
                lines.push(l.into())
            }
        }
        let mut pages = Vec::new();
        let mut printed = Vec::new();
        for x in &m.sources {
            if !pages.contains(&x.spread) {
                pages.push(x.spread)
            }
            if let Some(v) = &x.printed_page {
                let t = if let Some(s) = v.as_str() {
                    s.to_owned()
                } else {
                    v.to_string()
                };
                if t != "null" && t != "0" && !t.is_empty() && !printed.contains(&t) {
                    printed.push(t)
                }
            }
        }
        let page = *pages.first().ok_or("Missing PDF page")?;
        let source = format!(
            "{} / {}",
            source_label,
            if printed.is_empty() {
                format!(
                    "펼침면 {}",
                    pages
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            } else {
                format!("인쇄 {}쪽", printed.join(", "))
            }
        );
        let uri = format!(
            "file://{}#page={page}",
            utf8_percent_encode(&pdf.to_string_lossy(), URI_ESC)
        );
        docs.push(Doc {
            location: None,
            id: m.id,
            title: m.title,
            categories: m
                .categories
                .into_iter()
                .flat_map(|c| match c.as_str() {
                    "물·식량" => vec!["식수".into(), "식량·조리".into()],
                    "응급처치" => vec!["응급·건강".into()],
                    "대피·주거" => vec!["대피·안전".into()],
                    _ => vec![c],
                })
                .collect(),
            file: m.file,
            body,
            display_body: lines.join("\n"),
            record: record.trim().into(),
            source,
            original_uri: uri,
            spread: page,
            original_path: pdf.clone(),
        });
    }
    Ok((docs, pdf))
}
fn search(docs: &[Doc], query: &str, cat: &str) -> Vec<usize> {
    let near = shelters::near(query);
    let q = query.case_fold().collect::<String>();
    let words = q
        .split_whitespace()
        .filter(|s| near.is_none() || !s.starts_with('@'))
        .collect::<Vec<_>>();
    let mut result: Vec<_> = docs
        .iter()
        .enumerate()
        .filter(|(_, d)| {
            if near.is_some() && d.location.is_none() {
                return false;
            }
            if cat != "전체" && !d.categories.iter().any(|c| c == cat) {
                return false;
            }
            if words.is_empty() {
                return true;
            }
            let s = format!("{} {}", d.title, d.body)
                .case_fold()
                .collect::<String>();
            (cat == "전체" || d.categories.iter().any(|c| c == cat))
                && words.iter().all(|w| s.contains(w))
        })
        .map(|(i, _)| i)
        .collect();
    if let Some(origin) = near {
        result.sort_by(|a, b| {
            shelters::distance(origin, docs[*a].location.as_ref().unwrap()).total_cmp(
                &shelters::distance(origin, docs[*b].location.as_ref().unwrap()),
            )
        });
    }
    result
}
// TUI browsing has explicit category routing; --dump retains the full catalog contract.
fn browse(docs: &[Doc], query: &str, cat: &str) -> Vec<usize> {
    let facilities = cat == "대피시설";
    search(docs, query, cat)
        .into_iter()
        .filter(|i| docs[*i].id.starts_with("shelter:") == facilities)
        .collect()
}
fn wrap(s: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    for line in s.lines() {
        let mut part = String::new();
        let mut used = 0;
        for c in line.chars() {
            let w = c.width().unwrap_or(0);
            if used + w > width && !part.is_empty() {
                out.push(part);
                part = String::new();
                used = 0
            }
            part.push(c);
            used += w
        }
        out.push(part)
    }
    out
}
fn clipped(s: &str, width: usize) -> String {
    let mut used = 0;
    s.chars()
        .take_while(|c| {
            used += c.width().unwrap_or(0);
            used <= width
        })
        .collect()
}
fn style(bg: Option<u8>) -> Style {
    let s = Style::default().fg(Color::Indexed(231)).bg(Color::Black);
    if let Some(b) = bg {
        s.bg(Color::Indexed(b)).fg(if b == 21 || b == 160 {
            Color::White
        } else {
            Color::Black
        })
    } else {
        s
    }
}
/// Model answers use light Markdown. Keep `**emphasis**` (drawn bold by
/// `put_rich`) but show `*` list markers as `-`.
fn answer_display(text: &str) -> String {
    text.lines()
        .map(|l| {
            let body = l.trim_start();
            let indent = (l.len() - body.len()) / 2;
            match body.strip_prefix("* ").or_else(|| body.strip_prefix("- ")) {
                Some(rest) => format!("{}- {}", " ".repeat(indent), rest.trim_start()),
                None => l.to_owned(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
/// Like `put`, drawing text between `**` markers in bold. `bold` carries the
/// state across wrapped lines and resets at blank lines.
fn put_rich(f: &mut Frame, y: u16, x: u16, s: &str, width: u16, st: Style, bold: &mut bool) {
    if s.trim().is_empty() {
        *bold = false;
    }
    let a = f.area();
    let n = if x < a.width.saturating_sub(1) && y < a.height {
        width.min(a.width - x - 1) as usize
    } else {
        0
    };
    let mut spans = Vec::new();
    let mut used = 0;
    for (k, part) in s.split("**").enumerate() {
        if k > 0 {
            *bold = !*bold;
        }
        let mut piece = String::new();
        for c in part.chars() {
            let w = c.width().unwrap_or(0);
            if used + w > n {
                break;
            }
            used += w;
            piece.push(c);
        }
        if !piece.is_empty() {
            let style = if *bold {
                st.add_modifier(Modifier::BOLD)
            } else {
                st
            };
            spans.push(Span::styled(piece, style));
        }
    }
    if n > 0 {
        f.render_widget(
            Paragraph::new(Line::from(spans)).style(st),
            Rect::new(x, y, n as u16, 1),
        );
    }
}
fn put(f: &mut Frame, y: u16, x: u16, s: &str, width: u16, st: Style) {
    let a = f.area();
    if x < a.width.saturating_sub(1) && y < a.height {
        let n = width.min(a.width - x - 1);
        f.render_widget(
            Paragraph::new(clipped(s, n as usize)).style(st),
            Rect::new(x, y, n, 1),
        );
    }
}
#[derive(Default)]
struct App {
    ai_source_count: usize,
    ai_draft: String,
    ai_editing: bool,
    ai_pending: Option<String>,
    /// Items in the current category without the search filter (list header).
    cat_total: usize,
    ai_phase: String,
    ai_asked: bool,
    // Which linked source is open in the left pane (1-based, 0 = none yet).
    ai_source_shown: usize,
    // AI sources shown together in the left pane (doc indices), the first
    // wrapped line of each, and the doc the view last put in `opened`.
    collection: Vec<usize>,
    collection_starts: Vec<usize>,
    collection_opened: Option<usize>,
    ai_offset: usize,
    query: String,
    draft: String,
    editing: bool,
    cat: usize,
    selected: usize,
    opened: Option<usize>,
    offset: usize,
    info: bool,
    view: u8,
    notice: String,
    hover: Option<(u8, usize)>,
}
#[derive(Default, Deserialize, PartialEq)]
struct Ai {
    #[serde(default)]
    phase: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    seq: u64,
}
const AI_OFF_TEXT: &str = "AI가 꺼져 있습니다. 아래 질문칸에 질문을 쓰고 Enter를 누르면 AI를 켜고, 준비되는 대로 답합니다. 자료 검색과 열람에는 AI가 필요 없습니다.";
fn str_width(s: &str) -> u16 {
    s.chars().map(|c| c.width().unwrap_or(0) as u16).sum()
}
/// AI pane buttons on the row above the question box: (x offset, label, hit kind).
fn ai_buttons(phase: &str, sources: usize, asked: bool, shown: usize) -> Vec<(u16, String, u8)> {
    let mut labels = if phase == "CONFIRM_MEMORY" {
        // Two real buttons: the AI on/off button would cancel this question.
        vec![
            ("[Enter 그래도 켜기]".to_string(), 12u8),
            ("[Esc 취소]".to_string(), 13),
        ]
    } else {
        vec![(ai_button(phase).to_string(), 8u8)]
    };
    if sources > 0 {
        // One source opens per press; the label shows which one is open.
        let _ = shown;
        labels.push((format!("[s 근거 원문 보기 · {sources}건]"), 10));
    }
    if asked && matches!(phase, "READY" | "OFF" | "ERROR") {
        labels.push(("[c 답변 복사]".into(), 11));
    }
    let mut x = 0;
    labels
        .into_iter()
        .map(|(l, k)| {
            let at = x;
            x += str_width(&l) + 1;
            (at, l, k)
        })
        .collect()
}
fn ai_button(phase: &str) -> &'static str {
    match phase {
        "" | "OFF" | "ERROR" => "[p AI 켜기]",
        "UNAVAILABLE" => "[p AI 사용 불가 · 이유]",
        "STARTING" => "[p 켜는 중 · 취소]",
        "STOPPING" => "[끄는 중…]",
        _ => "[p AI 끄기]",
    }
}
/// Sends the typed question, starting the AI first when it is off. The
/// question then waits until the model is READY (see the main loop).
fn send_question(a: &mut App, engine: &mut ai_runtime::Controller) {
    let q = a.ai_draft.trim().to_owned();
    if q.is_empty() {
        return;
    }
    a.ai_editing = false;
    a.ai_offset = 0;
    a.ai_asked = true;
    match engine.status.phase.as_str() {
        "READY" => engine.submit(q),
        "" | "OFF" | "ERROR" | "STARTING" => {
            engine.start();
            match engine.status.phase.as_str() {
                "STARTING" => {
                    a.ai_pending = Some(q);
                    a.notice = "AI를 켜는 중입니다. 준비되면 질문을 보냅니다.".into();
                }
                "UNAVAILABLE" => {
                    a.notice = "AI를 쓸 수 없습니다. AI 답변 칸의 이유를 확인하세요.".into()
                }
                "CONFIRM_MEMORY" => {
                    a.ai_pending = Some(q);
                    a.notice =
                        "메모리가 부족합니다. y를 누르면 그래도 켜고 질문을 보냅니다.".into();
                }
                "ERROR" => {
                    a.notice = "AI를 켜지 못했습니다. AI 답변 칸의 안내를 확인하세요.".into()
                }
                _ => a.notice = "AI를 정리하는 중입니다. 잠시 후 다시 보내세요.".into(),
            }
        }
        "UNAVAILABLE" => a.notice = "AI를 쓸 수 없습니다. AI 답변 칸의 이유를 확인하세요.".into(),
        "CONFIRM_MEMORY" => {
            a.ai_pending = Some(q);
            a.notice = "메모리가 부족합니다. y를 누르면 그래도 켜고 질문을 보냅니다.".into();
        }
        _ => a.notice = "이전 질문을 처리하는 중입니다. 끝난 뒤 다시 보내세요.".into(),
    }
}
/// `c`: the AI answer in the AI pane, otherwise the open document.
fn copy_current(a: &mut App, docs: &[Doc], ai_raw: &str) {
    let text = if a.view == 1 && a.ai_asked && !ai_raw.trim().is_empty() {
        ai_raw.to_owned()
    } else if let Some(i) = a.opened {
        let d = &docs[i];
        format!(
            "{}\n{}\n\n{}\n\n{}",
            d.title, d.source, d.display_body, d.record
        )
    } else {
        a.notice = "복사할 답변이나 열린 문서가 없습니다.".into();
        return;
    };
    a.notice = match clipboard::copy(&text) {
        Ok(how) => format!("복사했습니다 · {how}"),
        Err(e) => format!("복사하지 못했습니다: {e}"),
    };
}
fn tabs(view: u8) -> Vec<(u16, String)> {
    let labels = [
        "[1 Library]",
        if view == 1 {
            "[2 Hide AI]"
        } else {
            "[2 Show AI]"
        },
        "[3 Settings]",
        "[4 Maps]",
    ];
    let mut x = 1;
    labels
        .iter()
        .map(|s| {
            let a = (x, s.to_string());
            x += s.len() as u16 + 1;
            a
        })
        .collect()
}
/// The key hint line. Every entry with a key can also be clicked; the click is
/// handled as that key press, so keyboard and mouse share one code path.
fn hints(editing: bool) -> Vec<(&'static str, Option<KeyCode>)> {
    if editing {
        vec![
            ("Search:", None),
            ("[Enter] Apply", Some(KeyCode::Enter)),
            ("[Esc] Cancel", Some(KeyCode::Esc)),
            ("[Backspace] Delete", Some(KeyCode::Backspace)),
            ("· Empty input clears filter", None),
        ]
    } else {
        vec![
            ("[/] Search", Some(KeyCode::Char('/'))),
            ("[Tab] Category", Some(KeyCode::Tab)),
            ("[↑↓] Select", None),
            ("[Enter] Open", Some(KeyCode::Enter)),
            ("[PgUp/Dn] Read", Some(KeyCode::PageDown)),
            ("[o] Source", Some(KeyCode::Char('o'))),
            ("[i] Info", Some(KeyCode::Char('i'))),
            ("[c] Copy", Some(KeyCode::Char('c'))),
            ("[q] Quit", Some(KeyCode::Char('q'))),
        ]
    }
}
fn hint_text(editing: bool) -> String {
    hints(editing)
        .iter()
        .map(|(t, _)| *t)
        .collect::<Vec<_>>()
        .join("  ")
}
fn hint_row(w: u16) -> u16 {
    if w >= 115 {
        2
    } else {
        3
    }
}
/// The key a click on the hint line stands for.
fn hint_hit(x: u16, y: u16, w: u16, h: u16, editing: bool) -> Option<KeyCode> {
    if w < 90 || h < 24 || y != hint_row(w) || x < 1 {
        return None;
    }
    let mut at = 1;
    for (t, key) in hints(editing) {
        let width = str_width(t);
        if x >= at && x < at + width {
            return key;
        }
        at += width + 2;
    }
    None
}
/// Numbered tabs for the AI source collection, right-aligned on the left
/// pane's bottom row: (x, label).
fn source_tabs(n: usize, w: u16, view: u8) -> Vec<(u16, String)> {
    let (_, half, _, _) = geometry(w, 30, view);
    // Left pane bottom row: [Close document] [Source file] [Info] end at 42.
    let (left, right) = (43, 2 + (if view == 1 { half } else { w }) - 5);
    let labels: Vec<String> = (1..=n).map(|k| format!("[{k}]")).collect();
    let width: u16 = labels.iter().map(|l| str_width(l)).sum();
    let gap = if width + n as u16 <= right.saturating_sub(left) {
        1
    } else {
        0
    };
    let mut x = right.saturating_sub(width + gap * n as u16).max(left);
    let mut out = Vec::new();
    for l in labels {
        if x + str_width(&l) > right {
            break;
        }
        let at = x;
        x += str_width(&l) + gap;
        out.push((at, l));
    }
    out
}
/// Show source k of the AI source collection.
fn show_source(a: &mut App, docs: &[Doc], k: usize) {
    let n = a.collection.len();
    if n == 0 {
        return;
    }
    let k = k.min(n - 1);
    a.offset = a.collection_starts.get(k).copied().unwrap_or(0);
    a.ai_source_shown = k + 1;
    a.opened = Some(a.collection[k]);
    a.collection_opened = a.opened;
    a.notice = format!(
        "근거 원문 [{}]/{} · {}",
        k + 1,
        n,
        docs[a.collection[k]].title
    );
}
fn geometry(w: u16, h: u16, view: u8) -> (u16, u16, u16, u16) {
    (
        (w as f64 * 0.35) as u16,
        w / 2,
        (6 + CATS.len().div_ceil(2) as u16).max((h as f64 * 0.35) as u16),
        h.saturating_sub(if view == 1 { 4 } else { 2 }),
    )
}
fn draw(f: &mut Frame, a: &mut App, docs: &[Doc], rows: &[usize], ai: &Ai, bench: bool) {
    let (w, h) = (f.area().width, f.area().height);
    let (edge, half, split, bottom) = geometry(w, h, a.view);
    let normal = style(None);
    let muted = normal.fg(Color::Indexed(244));
    f.render_widget(Paragraph::new("").style(normal), f.area());
    if w < 90 || h < 24 {
        put(
            f,
            0,
            0,
            "Resize to at least 90 columns x 24 rows. q: Quit",
            w,
            normal,
        );
        return;
    }
    put(
        f,
        0,
        1,
        "KOREA SOS ATLAS KIT / OFFLINE LIBRARY",
        w,
        style(Some(46)).add_modifier(Modifier::BOLD),
    );
    put(
        f,
        0,
        w - 24,
        &if bench {
            format!(" AI {}", ai.phase)
        } else {
            if ai.phase == "UNAVAILABLE" {
                " AI 사용 불가 [p: 이유]".into()
            } else if ai.phase.is_empty() || ai.phase == "OFF" {
                " AI OFF [p: AI ON]".into()
            } else if ai.phase == "ERROR" {
                " AI ERROR [p: AI ON]".into()
            } else if ai.phase == "CONFIRM_MEMORY" {
                " LOW MEMORY [Enter/Esc]".into()
            } else {
                format!(" {} [p: AI OFF]", ai.phase)
            }
        },
        24,
        style(Some(160)),
    );
    for (i, (x, t)) in tabs(a.view).iter().enumerate() {
        put(
            f,
            1,
            *x,
            t,
            t.len() as u16,
            if a.view as usize == i {
                style(Some(254))
            } else if a.hover == Some((0, i)) {
                normal.add_modifier(Modifier::UNDERLINED)
            } else {
                normal
            },
        )
    }
    if a.view == 2 {
        let left = 20;
        let right = left + 30.max((w - left) / 2);
        put(
            f,
            2,
            1,
            "UI preview · No inference or system changes",
            w - 2,
            muted,
        );
        for y in [4, h - 3] {
            put(f, y, 0, &"─".repeat(w as usize - 1), w, muted)
        }
        for x in [left, right] {
            for y in 5..h - 3 {
                put(f, y, x, "┊", 1, muted)
            }
        }
        put(f, 4, 2, " Settings ", 17, style(Some(51)));
        put(
            f,
            4,
            left + 2,
            " AI policy ",
            right - left - 4,
            style(Some(226)),
        );
        put(
            f,
            4,
            right + 2,
            " Content / Device ",
            w - right - 4,
            style(Some(244)),
        );
        for (i, s) in ["AI policy", "Content packs", "Device status", "Display"]
            .iter()
            .enumerate()
        {
            put(f, 6 + i as u16, 2, s, left - 4, muted)
        }
        for (y, s) in [
            (6, "AI on boot: OFF · Planned"),
            (8, "Start / stop: Manual · Planned"),
            (10, "Idle shutdown: Optional · Planned"),
        ] {
            put(f, y, left + 2, s, right - left - 4, normal)
        }
        for (y, s) in [
            (6, "Approved content library"),
            (8, "Pack management: Unavailable"),
            (10, "Battery / Storage / RAM"),
            (12, "Device monitoring: Not connected"),
        ] {
            put(f, y, right + 2, s, w - right - 5, normal)
        }
        return;
    }
    let sy = if w >= 115 { 1 } else { 2 };
    let sx = if w >= 115 {
        tabs(a.view)
            .last()
            .map(|(x, t)| x + t.len() as u16 + 1)
            .unwrap()
    } else {
        1
    };
    put(
        f,
        if w >= 115 { 2 } else { 3 },
        1,
        &hint_text(a.editing),
        w - 3,
        muted,
    );
    if a.editing {
        put(
            f,
            sy,
            sx,
            &format!("[ Search: {} ▏ ]", a.draft),
            w - sx,
            style(Some(244)),
        )
    } else if !a.query.is_empty() {
        put(f, sy, sx, &format!("Filter: {}", a.query), w - sx, normal)
    }
    for y in [4, split, bottom] {
        put(f, y, 0, &"─".repeat(w as usize - 1), w, muted)
    }
    for y in 5..split {
        put(f, y, edge, "┊", 1, muted)
    }
    if a.view == 1 {
        for y in split + 1..bottom {
            put(f, y, half, "┊", 1, muted)
        }
    }
    put(f, 4, 2, " Categories ", edge - 3, style(Some(226)));
    put(
        f,
        4,
        edge + 2,
        &format!(
            " {} / {} ",
            if CATS[a.cat] == "대피시설" {
                "Facilities"
            } else {
                "Documents"
            },
            // While searching, show the share of the whole category: "5 of 404".
            if a.query.is_empty() || a.cat_total <= rows.len() {
                rows.len().to_string()
            } else {
                format!("{} of {}", rows.len(), a.cat_total)
            }
        ),
        w - edge - 3,
        style(Some(51)),
    );
    put(
        f,
        5,
        edge + 2,
        if CATS[a.cat] == "대피시설" {
            "Facility names / addresses"
        } else {
            "Guides · Facilities: 대피시설"
        },
        w - edge - 4,
        muted,
    );
    if rows.is_empty() {
        put(
            f,
            6,
            edge + 2,
            "No matches. Clear search or change category.",
            w - edge - 4,
            normal,
        );
    }
    for (i, t) in CATS.iter().enumerate() {
        put(
            f,
            6 + i as u16 / 2,
            2 + (i as u16 % 2) * (edge / 2),
            if i == 0 { "ALL" } else { t },
            edge / 2 - 3,
            if a.cat == i {
                style(Some(226))
            } else if a.hover == Some((2, i)) {
                style(Some(244))
            } else {
                normal
            },
        )
    }
    let page = (split - 7).max(1) as usize;
    let start = a.selected / page * page;
    for (i, index) in rows.iter().skip(start).take(page).enumerate() {
        put(
            f,
            6 + i as u16,
            edge + 2,
            &match (shelters::near(&a.query), docs[*index].location.as_ref()) {
                (Some(origin), Some(p)) => format!(
                    "{:.2}km 직선 · {}",
                    shelters::distance(origin, p),
                    docs[*index].title
                ),
                _ => docs[*index].title.clone(),
            },
            w - edge - 5,
            if a.selected == start + i {
                style(Some(51))
            } else if a.hover == Some((3, start + i)) {
                style(Some(244))
            } else {
                normal
            },
        )
    }
    let bw = (if a.view == 1 { half } else { w }) - 5;
    put(f, split, 2, " Source document ", bw, style(Some(244)));
    if !a.collection.is_empty() && a.opened != a.collection_opened {
        // Another document was opened or the view was closed.
        a.collection.clear();
    }
    if !a.collection.is_empty() && !a.info {
        let n = a.collection.len();
        let mut body: Vec<String> = Vec::new();
        a.collection_starts.clear();
        for (k, &i) in a.collection.iter().enumerate() {
            a.collection_starts.push(body.len());
            let d = &docs[i];
            body.extend(wrap(
                &format!("── [{}/{n}] {} ──", k + 1, d.title),
                bw as usize,
            ));
            body.extend(wrap(&d.source, bw as usize));
            body.push(String::new());
            body.extend(wrap(&d.display_body, bw as usize));
            body.push(String::new());
        }
        let visible = bottom.saturating_sub(split + 3).max(1) as usize;
        a.offset = a.offset.min(body.len().saturating_sub(visible));
        let current = a
            .collection_starts
            .iter()
            .rposition(|s| *s <= a.offset)
            .unwrap_or(0);
        a.opened = Some(a.collection[current]);
        a.collection_opened = a.opened;
        a.ai_source_shown = current + 1;
        put(
            f,
            split,
            2,
            &format!(
                " 근거 원문 모음 · {}/{n} · {}–{}/{}줄 ",
                current + 1,
                a.offset + 1,
                (a.offset + visible).min(body.len()),
                body.len()
            ),
            bw,
            style(Some(244)),
        );
        for (row, t) in body.iter().skip(a.offset).take(visible).enumerate() {
            let line = a.offset + row;
            let st = if a.collection_starts.contains(&line) {
                normal.add_modifier(Modifier::BOLD)
            } else {
                normal
            };
            put(f, split + 1 + row as u16, 2, t, bw, st)
        }
        put(f, bottom - 1, 20, "[Source file]", 15, normal);
        put(f, bottom - 1, 36, "[Info]", 6, normal);
        for (k, (x, label)) in source_tabs(n, w, a.view).into_iter().enumerate() {
            let st = if k == current {
                style(Some(51))
            } else {
                normal
            };
            put(f, bottom - 1, x, &label, str_width(&label), st);
        }
    } else if let Some(i) = a.opened {
        let d = &docs[i];
        put(
            f,
            split + 2,
            2,
            &d.title,
            bw,
            normal.add_modifier(Modifier::BOLD),
        );
        put(f, split + 3, 2, &d.source, bw, normal);
        let body = wrap(
            if a.info { &d.record } else { &d.display_body },
            bw as usize,
        );
        let visible = bottom.saturating_sub(split + 6).max(1) as usize;
        a.offset = a.offset.min(body.len().saturating_sub(visible));
        put(
            f,
            split,
            2,
            &format!(
                " Source document · Lines {}–{}/{} ",
                a.offset + 1,
                (a.offset + visible).min(body.len()),
                body.len()
            ),
            bw,
            style(Some(244)),
        );
        for (i, t) in body.iter().skip(a.offset).take(visible).enumerate() {
            put(f, split + 5 + i as u16, 2, t, bw, normal)
        }
        put(f, bottom - 1, 20, "[Source file]", 15, normal);
        if d.location.is_some() {
            put(f, bottom - 1, 44, "[m Map destination]", 20, normal);
        }
        put(
            f,
            bottom - 1,
            36,
            if a.info { "[Back]" } else { "[Info]" },
            6,
            normal,
        )
    } else {
        put(
            f,
            split + 2,
            2,
            "Select a document or press Enter.",
            bw,
            normal,
        )
    }
    put(f, bottom - 1, 2, "[Close document]", 16, style(Some(164)));
    if a.view == 1 {
        let pw = w - half - 4;
        let text = if bench || !ai.phase.is_empty() {
            if ai.text.is_empty() {
                format!("Lifecycle benchmark · {}", ai.phase)
            } else {
                ai.text.clone()
            }
        } else {
            AI_OFF_TEXT.into()
        };
        let lines = wrap(&answer_display(&text), (w - half - 5) as usize);
        // Answer on top, buttons and the question box at the bottom.
        let first = split + 1;
        let max = bottom.saturating_sub(first + 3).max(1) as usize;
        let skip = a.ai_offset.min(lines.len().saturating_sub(max));
        let phase = if ai.phase.is_empty() {
            "OFF"
        } else {
            &ai.phase
        };
        let range = if lines.len() > max {
            format!(
                " · {}–{}/{}줄",
                skip + 1,
                (skip + max).min(lines.len()),
                lines.len()
            )
        } else {
            String::new()
        };
        put(
            f,
            split,
            half + 2,
            &format!(" AI 답변 / {phase}{range} "),
            pw,
            style(Some(21)),
        );
        let mut bold = false;
        for t in &lines[..skip] {
            if t.trim().is_empty() {
                bold = false;
            }
            bold ^= t.matches("**").count() % 2 == 1;
        }
        for (i, t) in lines.iter().skip(skip).take(max).enumerate() {
            put_rich(
                f,
                first + i as u16,
                half + 2,
                t,
                w - half - 5,
                normal,
                &mut bold,
            )
        }
        for (x, label, _) in ai_buttons(phase, a.ai_source_count, a.ai_asked, a.ai_source_shown) {
            put(
                f,
                bottom - 2,
                half + 2 + x,
                &label,
                str_width(&label),
                normal,
            );
        }
        let question = if a.ai_editing {
            format!(" 질문 > {}▏", a.ai_draft)
        } else {
            " 질문 > 여기를 클릭하거나 [a]를 누르고 질문을 입력하세요".into()
        };
        put(
            f,
            bottom - 1,
            half + 2,
            &question,
            pw,
            style(Some(if a.ai_editing { 254 } else { 250 })),
        );
        if bench {
            put(
                f,
                h - 2,
                1,
                "Lifecycle benchmark · Driver-controlled",
                w - 3,
                muted,
            )
        } else if a.ai_editing {
            put(f, h - 2, half + 2, "[Enter] 보내기 · [Esc] 닫기", pw, muted)
        }
    }
    put(f, h - 1, 1, &a.notice, w - 3, normal);
}
fn hit(x: u16, y: u16, w: u16, h: u16, a: &App, count: usize) -> Option<(u8, usize)> {
    if w < 90 || h < 24 {
        return None;
    }
    if y == 0 && x >= w - 24 {
        return Some((8, 0));
    }
    if y == 1 {
        for (i, (left, t)) in tabs(a.view).iter().enumerate() {
            if x >= *left && x < *left + t.len() as u16 {
                return Some((0, i));
            }
        }
    }
    if a.view == 2 {
        return None;
    }
    let (edge, half, split, bottom) = geometry(w, h, a.view);
    if a.view == 1 && x >= half + 2 && x < w - 2 {
        if y == bottom - 1 {
            return Some((9, 0));
        }
        if y == bottom - 2 {
            let rel = x - (half + 2);
            for (at, label, kind) in ai_buttons(
                &a.ai_phase,
                a.ai_source_count,
                a.ai_asked,
                a.ai_source_shown,
            ) {
                if rel >= at && rel < at + str_width(&label) {
                    return Some((kind, 0));
                }
            }
        }
    }
    let sy = if w >= 115 { 1 } else { 2 };
    let sx = if w >= 115 {
        tabs(a.view)
            .last()
            .map(|(x, t)| x + t.len() as u16 + 1)
            .unwrap()
    } else {
        1
    };
    if (a.editing || !a.query.is_empty()) && y == sy && x >= sx {
        return Some((1, 0));
    }
    if (6..6 + CATS.len().div_ceil(2) as u16).contains(&y) && x >= 2 && x < edge - 1 {
        let index = ((y - 6) * 2 + if x < 2 + edge / 2 { 0 } else { 1 }) as usize;
        return (index < CATS.len()).then_some((2, index));
    }
    let page = (split - 7).max(1) as usize;
    let start = a.selected / page * page;
    if (6..split - 1).contains(&y) && x > edge && x < w - 2 && start + ((y - 6) as usize) < count {
        return Some((3, start + (y - 6) as usize));
    }
    if y == bottom - 1 && !a.collection.is_empty() {
        for (k, (at, label)) in source_tabs(a.collection.len(), w, a.view)
            .iter()
            .enumerate()
        {
            if x >= *at && x < at + str_width(label) {
                return Some((14, k));
            }
        }
    }
    if y == bottom - 1 {
        if (44..64).contains(&x) {
            return Some((7, 0));
        }
        if (2..18).contains(&x) {
            return Some((4, 0));
        }
        if (20..35).contains(&x) {
            return Some((5, 0));
        }
        if (36..42).contains(&x) {
            return Some((6, 0));
        }
    }
    None
}
fn windows_source_script(path: &str, page: u64) -> String {
    let path = path.replace('\'', "''");
    if page == 0 {
        return format!("$ErrorActionPreference='Stop'; Start-Process -FilePath '{path}'");
    }
    // Shell-opening a .pdf path drops #page. Edge's PDF viewer accepts a file URI
    // with a page fragment, including WSL's UNC paths and Korean filenames.
    format!(
        "$ErrorActionPreference='Stop'; $sosSource='{path}'; \
         $sosUri=([Uri]::new($sosSource)).AbsoluteUri+'#page={page}'; \
         $sosEdge=@(\"${{env:ProgramFiles(x86)}}\\Microsoft\\Edge\\Application\\msedge.exe\",\"$env:ProgramFiles\\Microsoft\\Edge\\Application\\msedge.exe\") | Where-Object {{ Test-Path $_ }} | Select-Object -First 1; \
         if ($sosEdge) {{ Start-Process -FilePath $sosEdge -ArgumentList ('\"'+$sosUri+'\"') }} \
         else {{ Start-Process -FilePath $sosSource; exit 10 }}"
    )
}
fn powershell(script: &str) -> Command {
    let b = script
        .encode_utf16()
        .flat_map(|v| v.to_le_bytes())
        .collect::<Vec<_>>();
    let mut c = Command::new("powershell.exe");
    c.args([
        "-NoProfile",
        "-NonInteractive",
        "-EncodedCommand",
        &STANDARD.encode(b),
    ]);
    c
}
fn open_pdf(d: &Doc, _pdf: &Path) -> String {
    let result = (|| -> Result<bool, Box<dyn std::error::Error>> {
        let mut windows_fallback = false;
        let mut cmd = if let Ok(handler) = env::var("BROWSER") {
            let mut parts = shell_words::split(&handler)?;
            if parts.is_empty() {
                return Err("Empty opener".into());
            }
            let has = parts.iter().any(|p| p.contains("%s"));
            for p in &mut parts {
                *p = p.replace("%s", &d.original_uri)
            }
            if !has {
                parts.push(d.original_uri.clone())
            }
            let mut c = Command::new(&parts[0]);
            c.args(&parts[1..]);
            c
        } else if cfg!(windows) {
            windows_fallback = true;
            let path = layout::plain(d.original_path.clone());
            let s = windows_source_script(path.to_str().ok_or("Source path")?, d.spread);
            powershell(&s)
        } else if fs::read_to_string("/proc/sys/kernel/osrelease")
            .unwrap_or_default()
            .to_lowercase()
            .contains("microsoft")
            && Command::new("powershell.exe")
                .arg("-Version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok()
        {
            windows_fallback = true;
            let path = Command::new("wslpath")
                .args(["-w", d.original_path.to_str().ok_or("Source path")?])
                .output()?;
            if !path.status.success() {
                return Err("wslpath failed".into());
            }
            let s = windows_source_script(String::from_utf8(path.stdout)?.trim(), d.spread);
            powershell(&s)
        } else {
            let mut c = Command::new(if cfg!(target_os = "macos") {
                "open"
            } else {
                "xdg-open"
            });
            c.arg(&d.original_uri);
            c
        };
        let mut child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let end = Instant::now() + Duration::from_secs(8);
        loop {
            if let Some(s) = child.try_wait()? {
                return if s.success() {
                    Ok(d.spread > 0)
                } else if windows_fallback && s.code() == Some(10) {
                    Ok(false)
                } else {
                    Err("viewer failed".into())
                };
            }
            if Instant::now() > end {
                child.kill()?;
                child.wait()?;
                return Err("viewer timeout".into());
            }
            thread::sleep(Duration::from_millis(20))
        }
    })();
    match result {
        Ok(true) => format!("Source PDF requested · page {}", d.spread),
        Ok(false) if d.spread > 0 => format!(
            "Source opened · navigate to PDF page {} in your viewer",
            d.spread
        ),
        Ok(false) => "Source text opened".into(),
        Err(_) => "Source open failed: viewer unavailable or timed out.".into(),
    }
}
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
    }
}
/// Headless real-model check for a new machine or OS (no terminal UI).
/// Prints one JSON line per step; exits non-zero on the first failure.
fn ai_check_run(
    docs: &[Doc],
    data: &Path,
    question: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use ai_runtime::Controller;
    let emit = |v: serde_json::Value| {
        println!("{v}");
        let _ = io::Write::flush(&mut io::stdout());
    };
    let wait = |c: &mut Controller, phase: &str, secs: u64| -> Result<u128, String> {
        let start = Instant::now();
        loop {
            c.poll();
            // A headless check cannot answer the low-memory question; report it.
            if matches!(
                c.status.phase.as_str(),
                "ERROR" | "UNAVAILABLE" | "CONFIRM_MEMORY"
            ) {
                return Err(c.status.text.clone());
            }
            if c.status.phase == phase {
                return Ok(start.elapsed().as_millis());
            }
            if start.elapsed() > Duration::from_secs(secs) {
                return Err(format!("{phase} 대기 시간 초과 (현재 {})", c.status.phase));
            }
            thread::sleep(Duration::from_millis(50));
        }
    };
    let mut c = Controller::new(data.to_path_buf(), ai_corpus(docs));
    emit(
        serde_json::json!({"step":"platform","platform":layout::platform(),"data":data.display().to_string(),"memory_check_skipped":env::var("SOS_AI_SKIP_MEMORY_CHECK").as_deref() == Ok("1")}),
    );
    if let Some(reason) = &c.unavailable {
        return Err(reason.clone().into());
    }
    c.start();
    let ms = wait(&mut c, "READY", 600)?;
    emit(serde_json::json!({"step":"ready","ms":ms,"note":"첫 실행은 모델·엔진 해시 검사 포함"}));
    c.submit(question.to_string());
    let ms = wait(&mut c, "READY", 900)?;
    let preview: String = c.status.text.chars().take(160).collect();
    emit(
        serde_json::json!({"step":"answer","ms":ms,"sources":c.status.sources.len(),"preview":preview}),
    );
    c.stop();
    let ms = wait(&mut c, "OFF", 15)?;
    emit(serde_json::json!({"step":"off","ms":ms}));
    c.start();
    let ms = wait(&mut c, "READY", 600)?;
    emit(
        serde_json::json!({"step":"ready_again","ms":ms,"note":"같은 실행 안에서는 해시를 다시 계산하지 않음"}),
    );
    c.submit(question.to_string());
    thread::sleep(Duration::from_millis(500));
    c.stop();
    let ms = wait(&mut c, "OFF", 15)?;
    emit(serde_json::json!({"step":"cancel_during_answer","ms":ms}));
    emit(serde_json::json!({"step":"done"}));
    Ok(())
}
/// Classic Windows console with a CJK font: measure which ambiguous-width
/// characters it draws two cells wide and spell them out in displayed text
/// (see `console_width`). Search still uses the original body.
fn narrow_for_console(docs: &mut [Doc]) {
    let candidates: std::collections::HashSet<char> = docs
        .iter()
        .flat_map(|d| {
            d.title
                .chars()
                .chain(d.display_body.chars())
                .chain(d.record.chars())
        })
        .filter(|c| console_width::is_ambiguous(*c))
        .collect();
    if console_width::init(candidates).is_empty() {
        return;
    }
    for d in docs {
        for field in [&mut d.title, &mut d.display_body, &mut d.record] {
            if let std::borrow::Cow::Owned(s) = console_width::text(field) {
                *field = s;
            }
        }
    }
}
fn ai_corpus(docs: &[Doc]) -> Vec<serde_json::Value> {
    docs.iter().map(|d| serde_json::json!({"id":d.id,"title":d.title,"body":d.body,"original_uri":d.original_uri})).collect()
}
/// `s`: the first press gathers every linked source into one scrollable view
/// in the left pane; later presses jump to the next source in it.
fn open_ai_source(a: &mut App, docs: &[Doc], ids: &[String]) {
    if ids.is_empty() {
        a.notice = "연결된 근거가 없습니다. [/] 일반 검색을 이용하세요.".into();
        return;
    }
    let found: Vec<usize> = ids
        .iter()
        .filter_map(|id| docs.iter().position(|d| &d.id == id))
        .collect();
    if found.is_empty() {
        a.notice = "이 자료는 현재 목록에 없습니다. [/] 일반 검색으로 다시 찾아보세요.".into();
        return;
    }
    a.info = false;
    if a.collection != found || a.opened != a.collection_opened {
        a.collection = found;
        a.collection_starts.clear();
        a.opened = Some(a.collection[0]);
        a.collection_opened = a.opened;
        a.offset = 0;
        a.ai_source_shown = 1;
    }
    a.notice = format!(
        "근거 원문 {}건을 왼쪽에 모았습니다. 왼쪽 아래 [1] [2]… 를 누르거나 ←/→로 원문을 옮겨 봅니다.",
        a.collection.len()
    );
}
fn run(docs: &[Doc], pdf: &Path, state: Option<&Path>) -> io::Result<()> {
    enable_raw_mode()?;
    let _g = Guard;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    let mut t = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    t.hide_cursor()?;
    let mut a = App::default();
    let mut ai = Ai::default();
    // The data folder holds content-library, models, runtime and assets.json.
    let root = layout::data_root(pdf);
    let corpus = ai_corpus(docs);
    let mut engine = ai_runtime::Controller::new(root, corpus);
    let mut dirty = true;
    let mut ai_raw = String::new();
    loop {
        if state.is_none() {
            if engine.poll() {
                dirty = true;
                a.ai_offset = 0;
                a.ai_source_shown = 0;
                a.collection.clear();
            }
            if a.ai_pending.is_some() {
                match engine.status.phase.as_str() {
                    "READY" => {
                        engine.submit(a.ai_pending.take().unwrap());
                        a.notice.clear();
                        dirty = true;
                    }
                    "STARTING" | "CONFIRM_MEMORY" => {}
                    _ => {
                        a.ai_pending = None;
                        a.notice = "AI가 켜지지 않아 질문을 보내지 못했습니다. AI 답변 칸의 안내를 확인하세요.".into();
                        dirty = true;
                    }
                }
            }
            a.ai_source_count = engine.status.sources.len();
            a.ai_phase = engine.status.phase.clone();
            if ai.phase != engine.status.phase || ai_raw != engine.status.text {
                ai.phase = engine.status.phase.clone();
                ai_raw = engine.status.text.clone();
                ai.text = console_width::text(&ai_raw).into_owned();
                dirty = true;
            }
        }
        if let Some(p) = state {
            let n = fs::read(p)
                .ok()
                .and_then(|b| serde_json::from_slice::<Ai>(&b).ok())
                .unwrap_or_default();
            if n != ai {
                ai = n;
                dirty = true
            }
        }
        if !dirty && !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let rows = browse(docs, &a.query, CATS[a.cat]);
        a.cat_total = if a.query.is_empty() {
            rows.len()
        } else {
            browse(docs, "", CATS[a.cat]).len()
        };
        a.selected = a.selected.min(rows.len().saturating_sub(1));
        if dirty {
            t.draw(|f| {
                draw(f, &mut a, docs, &rows, &ai, state.is_some());
                console_width::fix_buffer(f.buffer_mut());
            })?;
            dirty = false
        }
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let mut ev = event::read()?;
        if let Event::Mouse(m) = &ev {
            if m.kind == MouseEventKind::Down(MouseButton::Left) {
                let (w, h) = crossterm::terminal::size()?;
                if let Some(code) = hint_hit(m.column, m.row, w, h, a.editing) {
                    a.ai_editing = false;
                    ev = Event::Key(crossterm::event::KeyEvent::new(
                        code,
                        crossterm::event::KeyModifiers::NONE,
                    ));
                }
            }
        }
        match ev {
            Event::Resize(_, _) => dirty = true,
            Event::Key(mut k)
                if k.kind == KeyEventKind::Press || k.kind == KeyEventKind::Repeat =>
            {
                if k.modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                    && matches!(k.code, KeyCode::Char('j') | KeyCode::Char('m'))
                {
                    k.code = KeyCode::Enter;
                }
                if k.modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                    && k.code == KeyCode::Char('c')
                {
                    break;
                }
                // Enter/Esc also work while a Korean input method is composing,
                // which can hold back a single y/n key.
                if engine.status.phase == "CONFIRM_MEMORY"
                    && !a.ai_editing
                    && !a.editing
                    && matches!(
                        k.code,
                        KeyCode::Char('y' | 'Y' | 'n' | 'N' | 'ㅛ' | 'ㅜ')
                            | KeyCode::Enter
                            | KeyCode::Esc
                    )
                {
                    if matches!(k.code, KeyCode::Char('y' | 'Y' | 'ㅛ') | KeyCode::Enter) {
                        engine.confirm_low_memory();
                        a.notice.clear();
                    } else {
                        engine.decline_low_memory();
                        a.ai_pending = None;
                        a.notice = "AI를 켜지 않았습니다.".into();
                    }
                    a.view = 1;
                    a.ai_offset = 0;
                    dirty = true;
                    continue;
                }
                if a.ai_editing {
                    match k.code {
                        KeyCode::Enter => send_question(&mut a, &mut engine),
                        KeyCode::Esc => a.ai_editing = false,
                        KeyCode::Backspace => {
                            a.ai_draft.pop();
                        }
                        KeyCode::Char(c) if a.ai_draft.chars().count() < 1000 => a.ai_draft.push(c),
                        _ => {}
                    }
                } else if a.editing {
                    match k.code {
                        KeyCode::Enter => {
                            a.query = a.draft.clone();
                            a.editing = false;
                            a.selected = 0;
                            a.opened = None
                        }
                        KeyCode::Esc => a.editing = false,
                        KeyCode::Backspace => {
                            a.draft.pop();
                        }
                        KeyCode::Char(c) => a.draft.push(c),
                        _ => {}
                    }
                } else {
                    match k.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('p') if state.is_none() => {
                            engine.toggle();
                            a.view = 1;
                            a.ai_editing = false;
                            a.ai_offset = 0;
                        }
                        KeyCode::Char('a') if state.is_none() => {
                            a.view = 1;
                            a.ai_editing = true;
                        }
                        KeyCode::Char('s') if a.view == 1 => {
                            open_ai_source(&mut a, docs, &engine.status.sources);
                        }
                        KeyCode::Char('1') => a.view = 0,
                        KeyCode::Char('2') => {
                            a.view = if a.view == 1 { 0 } else { 1 };
                            // Opening the AI pane focuses its question box.
                            a.ai_editing = a.view == 1 && state.is_none();
                        }
                        KeyCode::Char('3') => a.view = 2,
                        KeyCode::Char('c') => copy_current(&mut a, docs, &ai_raw),
                        KeyCode::Char('4') => a.notice = maps::launch(pdf),
                        KeyCode::Char('m') => {
                            if let Some(i) = a.opened {
                                a.notice = maps::destination(pdf, docs[i].location.as_ref());
                            }
                        }
                        KeyCode::Char('/') if a.view < 2 => {
                            a.draft = a.query.clone();
                            a.editing = true
                        }
                        KeyCode::Tab if a.view < 2 => {
                            a.cat = (a.cat + 1) % CATS.len();
                            a.selected = 0;
                            a.opened = None
                        }
                        KeyCode::Right if !a.collection.is_empty() => {
                            let next = a.ai_source_shown.min(a.collection.len() - 1);
                            show_source(&mut a, docs, next);
                        }
                        KeyCode::Left if !a.collection.is_empty() => {
                            let prev = a.ai_source_shown.saturating_sub(2);
                            show_source(&mut a, docs, prev);
                        }
                        KeyCode::Down => {
                            a.selected = (a.selected + 1).min(rows.len().saturating_sub(1))
                        }
                        KeyCode::Up => a.selected = a.selected.saturating_sub(1),
                        KeyCode::Enter if !rows.is_empty() => {
                            a.opened = Some(rows[a.selected]);
                            a.offset = 0;
                            a.info = false;
                            a.notice.clear()
                        }
                        KeyCode::Esc => {
                            a.opened = None;
                            a.notice.clear()
                        }
                        KeyCode::Char('i') if a.opened.is_some() => {
                            a.info = !a.info;
                            a.offset = 0
                        }
                        KeyCode::Char('o') => {
                            if let Some(i) = a.opened {
                                a.notice = open_pdf(&docs[i], pdf)
                            }
                        }
                        KeyCode::PageDown => {
                            let (w, h) = crossterm::terminal::size()?;
                            let (_, _, split, bottom) = geometry(w, h, a.view);
                            a.offset += bottom.saturating_sub(split + 6).max(1) as usize
                        }
                        KeyCode::PageUp => {
                            let (w, h) = crossterm::terminal::size()?;
                            let (_, _, split, bottom) = geometry(w, h, a.view);
                            a.offset = a
                                .offset
                                .saturating_sub(bottom.saturating_sub(split + 6).max(1) as usize)
                        }
                        _ => {}
                    }
                }
                dirty = true;
            }
            Event::Mouse(m) => {
                let (w, h) = crossterm::terminal::size()?;
                let target = hit(m.column, m.row, w, h, &a, rows.len());
                if target != a.hover {
                    a.hover = target;
                    dirty = true
                }
                match m.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        if let Some((kind, n)) = target {
                            if !matches!(kind, 8..=14) {
                                a.ai_editing = false;
                            }
                            match kind {
                                11 => copy_current(&mut a, docs, &ai_raw),
                                14 => show_source(&mut a, docs, n),
                                12 => {
                                    engine.confirm_low_memory();
                                    a.notice.clear();
                                }
                                13 => {
                                    engine.decline_low_memory();
                                    a.ai_pending = None;
                                    a.notice = "AI를 켜지 않았습니다.".into();
                                }
                                10 => open_ai_source(&mut a, docs, &engine.status.sources),
                                8 if state.is_none() => {
                                    engine.toggle();
                                    a.view = 1;
                                    a.ai_editing = false;
                                    a.ai_offset = 0;
                                }
                                9 if state.is_none() => {
                                    a.ai_editing = true;
                                    a.editing = false;
                                }
                                0 if n == 3 => a.notice = maps::launch(pdf),
                                0 => {
                                    a.view = if n == 1 {
                                        if a.view == 1 {
                                            0
                                        } else {
                                            1
                                        }
                                    } else {
                                        n as u8
                                    };
                                    a.editing = false;
                                    a.ai_editing = a.view == 1 && state.is_none();
                                }
                                1 => {
                                    a.draft = a.query.clone();
                                    a.editing = true
                                }
                                2 => {
                                    a.cat = n.min(CATS.len() - 1);
                                    a.selected = 0;
                                    a.opened = None;
                                    a.editing = false
                                }
                                3 => {
                                    a.selected = n;
                                    a.opened = Some(rows[n]);
                                    a.offset = 0;
                                    a.info = false;
                                    a.editing = false
                                }
                                4 => a.opened = None,
                                5 => {
                                    if let Some(i) = a.opened {
                                        a.notice = open_pdf(&docs[i], pdf)
                                    }
                                }
                                6 => {
                                    a.info = !a.info;
                                    a.offset = 0
                                }
                                7 => {
                                    if let Some(i) = a.opened {
                                        a.notice =
                                            maps::destination(pdf, docs[i].location.as_ref());
                                    }
                                }
                                _ => {}
                            }
                            dirty = true
                        }
                    }
                    MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                        let down = m.kind == MouseEventKind::ScrollDown;
                        if a.view == 1 && m.column > w / 2 && m.row > geometry(w, h, a.view).2 {
                            a.ai_offset = if down {
                                a.ai_offset.saturating_add(3)
                            } else {
                                a.ai_offset.saturating_sub(3)
                            };
                        } else if m.row > geometry(w, h, a.view).2 && a.opened.is_some() {
                            a.offset = if down {
                                a.offset + 3
                            } else {
                                a.offset.saturating_sub(3)
                            }
                        } else {
                            a.selected = if down {
                                (a.selected + 3).min(rows.len().saturating_sub(1))
                            } else {
                                a.selected.saturating_sub(3)
                            }
                        }
                        dirty = true
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    Ok(())
}
/// Project folder for tests. `SOS_TEST_PROJECT_ROOT` lets a test binary built
/// here run on another OS (e.g. Windows reading `\\wsl.localhost\...`).
#[cfg(test)]
pub(crate) fn project_root() -> PathBuf {
    env::var_os("SOS_TEST_PROJECT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut root: Option<PathBuf> = None;
    let mut state = None;
    let mut dump = false;
    let mut status_only = false;
    let mut ai_check = false;
    let mut local_manuals = false;
    let mut query = String::new();
    let mut cat = "전체".to_string();
    let mut args = env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--root" => root = Some(PathBuf::from(args.next().ok_or("Missing root")?)),
            "--ai-benchmark-state" => {
                state = Some(PathBuf::from(args.next().ok_or("Missing state")?))
            }
            "--dump" => dump = true,
            "--status" => status_only = true,
            "--ai-check" => ai_check = true,
            "--local-manuals" => local_manuals = true,
            "--query" => query = args.next().ok_or("Missing query")?,
            "--category" => cat = args.next().ok_or("Missing category")?,
            _ => return Err(format!("Unknown argument: {a}").into()),
        }
    }
    let root = match root {
        Some(r) => r,
        None => layout::default_content_root()?,
    };
    if status_only {
        print!("{}", status::report(&root)?);
        return Ok(());
    }
    let (mut docs, pdf) = load(&root)?;
    if local_manuals && !library::default_includes_manuals(&root)? {
        docs.extend(library::load_local_manuals(&root)?);
    }
    if ai_check {
        let question = if query.is_empty() {
            "정전 때 냉장고 문은 어떻게 해야 하나요?"
        } else {
            &query
        };
        return ai_check_run(&docs, &layout::data_root(&root), question);
    }
    if dump {
        let selected = search(&docs, &query, &cat);
        println!(
            "{}",
            serde_json::to_string(&selected.iter().map(|i| &docs[*i]).collect::<Vec<_>>())?
        )
    } else {
        if cfg!(windows) {
            narrow_for_console(&mut docs);
        }
        run(&docs, &pdf, state.as_deref())?
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    #[test]
    fn answer_markdown_keeps_emphasis_as_bold_and_plain_list_markers() {
        assert_eq!(
            answer_display("**청소년:**\n*   감정을 적기\n    *   하위 항목\n- 이미 대시"),
            "**청소년:**\n- 감정을 적기\n  - 하위 항목\n- 이미 대시"
        );
        let mut terminal = Terminal::new(TestBackend::new(30, 2)).unwrap();
        terminal
            .draw(|f| {
                let mut bold = false;
                put_rich(f, 0, 0, "a **굵게** b", 28, style(None), &mut bold);
                put_rich(f, 1, 0, "**열림", 28, style(None), &mut bold);
                assert!(bold);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let row: String = (0..12).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert!(!row.contains('*'));
        assert!(buf[(2, 0)].modifier.contains(Modifier::BOLD));
        assert!(!buf[(0, 0)].modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn ai_source_button_opens_original_even_after_off_or_failure() {
        let root = crate::project_root().join("dev/content-library");
        let (docs, _) = load(&root).unwrap();
        let id = docs[0].id.clone();
        let mut a = App {
            view: 1,
            ai_source_count: 1,
            ..App::default()
        };
        let (_, half, _, bottom) = geometry(120, 30, 1);
        let (at, _, kind) = ai_buttons("", 1, false, 0)[1].clone();
        assert_eq!(kind, 10);
        assert_eq!(
            hit(half + 2 + at, bottom - 2, 120, 30, &a, 1),
            Some((10, 0))
        );
        assert_eq!(hit(half + 3, bottom - 1, 120, 30, &a, 1), Some((9, 0)));
        for phase in ["READY", "ERROR", "OFF"] {
            open_ai_source(&mut a, &docs, &["missing".into(), id.clone()]);
            assert_eq!(a.opened, Some(0));
            assert!(!a.info);
            assert_eq!(a.offset, 0);
            let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
            terminal
                .draw(|f| {
                    draw(
                        f,
                        &mut a,
                        &docs,
                        &[0],
                        &Ai {
                            phase: phase.into(),
                            text: "답변 또는 오류 안내".into(),
                            ..Ai::default()
                        },
                        false,
                    )
                })
                .unwrap();
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect::<String>();
            assert!(text.replace(' ', "").contains("[s근거원문보기·1건]"));
            assert!(text.contains("[1]"));
            assert!(text.replace(' ', "").contains("답변또는오류안내"));
        }
        open_ai_source(&mut a, &docs, &[]);
        assert!(a.notice.contains("일반 검색"));
    }

    #[test]
    fn hint_line_and_source_tabs_are_clickable() {
        // Hint line: each entry maps to its key; plain text does not.
        let row = hint_row(120);
        assert_eq!(hint_hit(1, row, 120, 30, false), Some(KeyCode::Char('/')));
        let x = hint_text(false).find("[Tab]").unwrap() as u16 + 1;
        assert_eq!(hint_hit(x, row, 120, 30, false), Some(KeyCode::Tab));
        let x = str_width(&hint_text(false)[..hint_text(false).find("[↑↓]").unwrap()]) + 1;
        assert_eq!(hint_hit(x, row, 120, 30, false), None);
        assert_eq!(hint_hit(1, row, 120, 30, true), None);
        assert_eq!(hint_hit(10, row, 120, 30, true), Some(KeyCode::Enter));
        assert_eq!(hint_hit(1, row + 1, 120, 30, false), None);
        // Source tabs: [1] [2] [3] right-aligned on the left pane, clear of [Info].
        let tabs = source_tabs(3, 120, 1);
        assert_eq!(
            tabs.iter().map(|t| t.1.as_str()).collect::<Vec<_>>(),
            ["[1]", "[2]", "[3]"]
        );
        assert!(tabs[0].0 >= 43 && tabs[2].0 + 3 <= 2 + 60 - 5);
        let a = App {
            view: 1,
            collection: vec![0, 1, 2],
            ..App::default()
        };
        let (_, _, _, bottom) = geometry(120, 30, 1);
        assert_eq!(hit(tabs[1].0, bottom - 1, 120, 30, &a, 3), Some((14, 1)));
    }

    #[test]
    fn tui_facility_routing_fields_and_scrolling() {
        let root = crate::project_root().join("dev/content-library");
        let (docs, _) = load(&root).unwrap();
        let guides = browse(&docs, "", "전체");
        assert!(!guides.is_empty());
        assert!(guides.iter().all(|i| !docs[*i].id.starts_with("shelter:")));
        let facilities = browse(&docs, "", "대피시설");
        assert_eq!(facilities.len(), 23307);
        assert!(facilities
            .iter()
            .all(|i| docs[*i].id.starts_with("shelter:")));
        assert!(browse(&docs, "자하문호텔", "전체").is_empty());
        let rows = browse(&docs, "자하문호텔", "대피시설");
        assert_eq!(rows.len(), 1);
        let d = &docs[rows[0]];
        for text in [
            "도로명주소: 서울특별시 종로구 진흥로 425",
            "지번주소: 서울특별시 종로구 구기동 119",
            "데이터 갱신시점: 2026-10-30 22:56:51",
            "최종수정시점: 2026-10-29 18:59:58",
        ] {
            assert!(d.display_body.contains(text));
        }
        for (w, h) in [(90, 24), (120, 30), (160, 50)] {
            let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
            let mut app = App {
                cat: CATS.len() - 1,
                opened: Some(rows[0]),
                ..Default::default()
            };
            let mut shown = String::new();
            for offset in 0..30 {
                app.offset = offset;
                t.draw(|f| draw(f, &mut app, &docs, &rows, &Ai::default(), false))
                    .unwrap();
                for y in 0..h {
                    let mut x = 0;
                    while x < w {
                        let symbol = t.backend().buffer()[(x, y)].symbol();
                        shown.push_str(symbol);
                        x += symbol
                            .chars()
                            .map(|c| c.width().unwrap_or(0) as u16)
                            .sum::<u16>()
                            .max(1);
                    }
                    shown.push('\n');
                }
            }
            let compact = shown.replace(' ', "");
            assert!(compact.contains("데이터갱신시점:"));
            assert!(
                compact.contains("최종수정시점:"),
                "{w}x{h}: modified timestamp not visible"
            );
            assert!(shown.contains("Facilities"));
            assert!(shown.contains("Source file"));
        }
        // Check every rendered field against the registered, hash-validated source catalogs.
        for manifest in [
            "shelters.json",
            "water-facilities.json",
            "medical-facilities.json",
        ] {
            let m: serde_json::Value =
                serde_json::from_slice(&fs::read(root.join(manifest)).unwrap()).unwrap();
            let records: Vec<serde_json::Value> = serde_json::from_slice(
                &fs::read(
                    root.join(m["directory"].as_str().unwrap())
                        .join("facilities.json"),
                )
                .unwrap(),
            )
            .unwrap();
            let prefix = match manifest {
                "water-facilities.json" => "shelter:water:",
                "medical-facilities.json" => "shelter:hospital:",
                _ => "shelter:",
            };
            let index: std::collections::HashMap<_, _> =
                docs.iter().map(|d| (d.id.as_str(), d)).collect();
            for r in records {
                let d = index[format!("{prefix}{}", r["id"].as_str().unwrap()).as_str()];
                for (key, label) in [
                    ("road_address", "도로명주소"),
                    ("updated", "데이터 갱신시점"),
                    ("modified", "최종수정시점"),
                ] {
                    let value = r[key]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .unwrap_or("원자료 미기재");
                    assert!(d.display_body.contains(&format!("{label}: {value}")));
                }
                assert!(d.original_path.is_file());
                assert!(d.source.starts_with("https://"));
            }
        }
    }

    #[test]
    fn every_category_is_visible_and_clickable() {
        use ratatui::{backend::TestBackend, Terminal};
        for (w, h) in [(90, 24), (120, 30), (160, 50)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            let mut app = App::default();
            terminal
                .draw(|f| draw(f, &mut app, &[], &[], &Ai::default(), false))
                .unwrap();
            let (edge, _, split, _) = geometry(w, h, 0);
            for (i, cat) in CATS.iter().enumerate() {
                let x = 2 + (i as u16 % 2) * (edge / 2);
                let y = 6 + i as u16 / 2;
                assert!(y < split);
                assert_eq!(hit(x, y, w, h, &app, 0), Some((2, i)));
                let shown: String = (x..x + edge / 2 - 3)
                    .map(|col| terminal.backend().buffer()[(col, y)].symbol())
                    .collect();
                assert!(
                    shown
                        .replace(' ', "")
                        .contains(if i == 0 { "ALL" } else { cat }),
                    "{cat}: {shown}"
                );
            }
        }
    }
    #[test]
    fn windows_pdf_opener_keeps_page_and_quotes_paths() {
        let script = windows_source_script(r"\\wsl.localhost\Ubuntu\한글 자료\it's.pdf", 74);
        assert!(script.contains("#page=74"));
        assert!(script.contains("it''s.pdf"));
        assert!(script.contains("AbsoluteUri"));
        assert!(script.contains("exit 10"));
        assert!(!windows_source_script(r"C:\source.txt", 0).contains("msedge"));
    }

    fn corpus() -> (Vec<Doc>, PathBuf) {
        load(&crate::project_root().join("dev/fixtures/emergency-ko")).unwrap()
    }

    #[test]
    fn complete_catalog_matches_preserved_version() {
        let (docs, _) = corpus();
        let mut expected: serde_json::Value =
            serde_json::from_str(include_str!("../dev/tests/catalog.json")).unwrap();
        let actual = serde_json::to_value(&docs).unwrap();
        assert_eq!(docs.len(), 25);
        // Absolute file URIs change when the checkout moves; all document fields remain exact.
        for (old, current) in expected
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(actual.as_array().unwrap())
        {
            old["original_uri"] = current["original_uri"].clone();
        }
        assert_eq!(actual, expected);
    }

    #[test]
    fn every_document_is_searchable_in_its_categories() {
        let (docs, _) = corpus();
        for (index, doc) in docs.iter().enumerate() {
            for category in std::iter::once("전체").chain(doc.categories.iter().map(String::as_str))
            {
                assert!(search(&docs, &doc.title, category).contains(&index));
                let word = doc
                    .body
                    .split_whitespace()
                    .find(|w| w.chars().count() > 2)
                    .unwrap();
                assert!(search(&docs, word, category).contains(&index));
            }
        }
        assert!(search(&docs, "존재하지않는검색어xyz", "전체").is_empty());
    }

    #[test]
    fn render_every_document_at_supported_sizes() {
        let (docs, _) = corpus();
        let rows = search(&docs, "", "전체");
        for (w, h) in [(90, 24), (120, 30), (160, 50)] {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            for index in 0..docs.len() {
                for view in 0..3 {
                    for info in [false, true] {
                        let mut app = App {
                            opened: Some(index),
                            selected: index,
                            view,
                            info,
                            ..Default::default()
                        };
                        terminal
                            .draw(|f| draw(f, &mut app, &docs, &rows, &Ai::default(), false))
                            .unwrap();
                        let buffer = terminal.backend().buffer();
                        let text: String = buffer.content.iter().map(|c| c.symbol()).collect();
                        assert!(text.contains("KOREA SOS ATLAS KIT"));
                    }
                }
            }
        }
    }

    #[test]
    fn archive_manifest_is_intact() {
        let root = crate::project_root().join("bak/2026-11-14");
        if !root.exists() {
            return;
        }
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("BACKUP-MANIFEST.json")).unwrap()).unwrap();
        for item in manifest["files"].as_array().unwrap() {
            let path = item["path"].as_str().unwrap();
            assert_eq!(
                hash(&fs::read(root.join(path)).unwrap()),
                item["sha256"].as_str().unwrap(),
                "{path}"
            );
        }
    }
}
