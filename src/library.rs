// SPDX-License-Identifier: AGPL-3.0-only
// Only explicitly reviewed, hash-bound text is exposed to search and the TUI.
use super::*;
use serde_json::Value;
use std::collections::HashSet;
type Error = Box<dyn std::error::Error>;
fn manual_display(body: &str) -> String {
    body.lines()
        .filter(|line| !line.trim_start().starts_with("<!--"))
        .filter(|line| {
            !(line.contains('|')
                && line
                    .chars()
                    .all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t')))
        })
        .map(|line| {
            let line = line.trim_start_matches('#').trim_start();
            let line = line.strip_prefix("> ").unwrap_or(line);
            let line = line.replace("**", "").replace('`', "");
            if let Some(item) = line.strip_prefix("- ") {
                format!("• {item}")
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn within(root: &Path, name: &str) -> Result<PathBuf, Error> {
    let p = root.join(name).canonicalize()?;
    if !p.starts_with(root.canonicalize()?) {
        return Err("Content path escapes library".into());
    }
    Ok(p)
}
fn checked(root: &Path, name: &str, expected: &Value) -> Result<Vec<u8>, Error> {
    let b = fs::read(within(root, name)?)?;
    if expected.as_str() != Some(&hash(&b)) {
        return Err(format!("Content hash mismatch: {name}").into());
    }
    Ok(b)
}
pub(super) fn load_approved(root: &Path) -> Result<(Vec<Doc>, PathBuf), Error> {
    let entries: Vec<String> = serde_json::from_slice(&fs::read(root.join("approved.json"))?)?;
    let mut ids = HashSet::new();
    let mut docs = Vec::new();
    for entry in entries {
        let dir = within(root, &entry)?;
        let r: Value = serde_json::from_slice(&fs::read(dir.join("record.json"))?)?;
        for flag in ["redistribute", "translate", "edit"] {
            if r["rights"][flag] != true {
                return Err(format!("Rights not approved: {entry}/{flag}").into());
            }
        }
        if r["content_fit"] != "passed"
            || r["app_approved"] != true
            || r["rights_reviewed_before_acquisition"] != true
            || r["semantic_review"] != "passed"
            || r["korean_review"] != "passed"
        {
            return Err(format!("Review not approved: {entry}").into());
        }
        let id = r["id"].as_str().ok_or("Missing ID")?.to_owned();
        if !ids.insert(id.clone()) {
            return Err("Duplicate approved ID".into());
        }
        let source_name = r["source_file"].as_str().unwrap_or("original.html");
        // Raw site HTML is internal evidence, excluded from distributable text bundles.
        // When present, verify it too; approved source text and Korean text are always required.
        if dir.join(source_name).exists() {
            checked(&dir, source_name, &r["source_sha256"])?;
        }
        checked(&dir, "text.txt", &r["text_sha256"])?;
        let text = String::from_utf8(checked(&dir, "ko.md", &r["ko_sha256"])?)?;
        let policies = r["policy_evidence"]
            .as_array()
            .ok_or("Missing rights evidence")?;
        if policies.is_empty() {
            return Err("Empty rights evidence".into());
        }
        for p in policies {
            checked(
                root,
                p["file"].as_str().ok_or("Missing policy path")?,
                &p["sha256"],
            )?;
        }
        let (content, record) = text
            .split_once("\n## 출처·편집 기록\n")
            .ok_or("Missing attribution")?;
        let body = content
            .split_once('\n')
            .ok_or("Missing title")?
            .1
            .trim()
            .to_owned();
        if body.chars().count() < 300 || body.chars().any(|c| c < ' ' && c != '\n' && c != '\t') {
            return Err("Insufficient or corrupt reviewed content".into());
        }
        // Offline source is the licensed text, never downloaded site design or remote resources.
        let original_path = dir.join("text.txt").canonicalize()?;
        docs.push(Doc {
            location: None,
            id,
            title: content
                .lines()
                .next()
                .unwrap()
                .trim_start_matches("# ")
                .into(),
            categories: serde_json::from_value(r["app_categories"].clone())?,
            file: format!("{entry}/ko.md"),
            display_body: body
                .lines()
                .map(|l| l.trim_start_matches("## "))
                .collect::<Vec<_>>()
                .join("\n"),
            body,
            record: record.trim().into(),
            source: r["url"].as_str().unwrap_or("").into(),
            original_uri: format!(
                "file://{}",
                utf8_percent_encode(&original_path.to_string_lossy(), URI_ESC)
            ),
            original_path,
            spread: 0,
        });
    }
    Ok((docs, root.to_path_buf()))
}

// Explicit local validation opt-in. This never adds a source to approved.json or
// grants redistribution rights. Every exposed section is bound to reviewed text.
pub(super) fn load_local_manuals(root: &Path) -> Result<Vec<Doc>, Error> {
    let manifest: Value = serde_json::from_slice(&fs::read(root.join("local-manuals.json"))?)?;
    if manifest["release_status"] != "local_validation_only" {
        return Err("Local manuals require an explicit local-only release status".into());
    }
    let mut docs = Vec::new();
    let mut ids = HashSet::new();
    for m in manifest["manuals"].as_array().ok_or("Missing manuals")? {
        if m["content_review"] != "passed" || m["coverage_review"] != "passed" {
            return Err("Local manual review incomplete".into());
        }
        let dir_name = m["directory"].as_str().ok_or("Missing manual directory")?;
        let dir = within(root, dir_name)?;
        checked(&dir, "original.pdf", &m["original_sha256"])?;
        let text = String::from_utf8(checked(&dir, "ko.md", &m["ko_sha256"])?)?;
        let original_path = within(&dir, "original.pdf")?;
        let (content, record) = text
            .split_once("\n## 출처·편집 기록\n")
            .ok_or("Missing attribution")?;
        let page_count = m["pages"].as_u64().ok_or("Missing page count")?;
        let manual_id = m["id"].as_str().ok_or("Missing manual ID")?;
        let label = m["title"].as_str().ok_or("Missing manual title")?;
        for s in m["sections"]
            .as_array()
            .ok_or("Missing reviewed sections")?
        {
            let start = s["start_byte"].as_u64().ok_or("Missing section start")? as usize;
            let end = s["end_byte"].as_u64().ok_or("Missing section end")? as usize;
            let body = content
                .get(start..end)
                .ok_or("Invalid section bounds")?
                .trim();
            if body.is_empty() || body.chars().any(|c| c < ' ' && c != '\n' && c != '\t') {
                return Err("Empty or corrupt manual section".into());
            }
            let page = s["page"].as_u64().ok_or("Missing source page")?;
            if page == 0 || page > page_count {
                return Err("Invalid source page".into());
            }
            let id = format!(
                "{manual_id}:{}",
                s["id"].as_str().ok_or("Missing section ID")?
            );
            if !ids.insert(id.clone()) {
                return Err("Duplicate manual section".into());
            }
            let categories: Vec<String> = serde_json::from_value(s["categories"].clone())?;
            if categories.is_empty() || categories.iter().any(|c| !CATS.contains(&c.as_str())) {
                return Err("Invalid manual category".into());
            }
            let title = s["title"].as_str().ok_or("Missing section title")?;
            docs.push(Doc {
                location: None,
                id,
                title: format!("{title} · {label}"),
                categories,
                file: format!("{dir_name}/ko.md"),
                body: body.into(),
                display_body: manual_display(body),
                record: format!("개인 검증용 · 공개 배포 승인과 별개\n{}", record.trim()),
                source: format!("{label} / PDF {page}쪽 · 개인 검증용"),
                original_uri: format!(
                    "file://{}#page={page}",
                    utf8_percent_encode(&original_path.to_string_lossy(), URI_ESC)
                ),
                spread: page,
                original_path: original_path.clone(),
            });
        }
    }
    if docs.is_empty() {
        return Err("Empty local manual catalog".into());
    }
    Ok(docs)
}
// A local launch profile is not the redistributable approved.json catalog.
pub(super) fn default_includes_manuals(root: &Path) -> Result<bool, Error> {
    let path = root.join("default-local.json");
    if !path.exists() {
        return Ok(false);
    }
    let profile: Value = serde_json::from_slice(&fs::read(path)?)?;
    if profile["scope"] != "user_authorized_local_default" {
        return Err("Invalid local default scope".into());
    }
    Ok(profile["include_manuals"] == true)
}
pub(super) fn load_default_local(root: &Path) -> Result<Vec<Doc>, Error> {
    let path = root.join("default-local.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let include_manuals = default_includes_manuals(root)?;
    let profile: Value = serde_json::from_slice(&fs::read(path)?)?;
    let mut docs = if include_manuals {
        load_local_manuals(root)?
    } else {
        Vec::new()
    };
    let mut ids: HashSet<String> = docs.iter().map(|d| d.id.clone()).collect();
    for entry in profile["documents"]
        .as_array()
        .ok_or("Missing local documents")?
    {
        let name = entry.as_str().ok_or("Invalid local document path")?;
        let dir = within(root, name)?;
        let r: Value = serde_json::from_slice(&fs::read(dir.join("record.json"))?)?;
        let review: Value = serde_json::from_slice(&fs::read(dir.join("local-review.json"))?)?;
        if r["local_default_authorized"] != true
            || r["korean_review"] != "passed_local_editorial"
            || review["content_review"] != "passed_local_editorial"
            || review["coverage_review"] != "passed_source_sections"
            || review["ko_sha256"] != r["ko_sha256"]
            || review["source_sha256"] != r["source_sha256"]
            || review["text_sha256"] != r["text_sha256"]
        {
            return Err(format!("Local document review incomplete: {name}").into());
        }
        checked(
            &dir,
            r["source_file"].as_str().ok_or("Missing original")?,
            &r["source_sha256"],
        )?;
        checked(&dir, "text.txt", &r["text_sha256"])?;
        let text = String::from_utf8(checked(&dir, "ko.md", &r["ko_sha256"])?)?;
        let (content, record) = text
            .split_once("\n## 출처·편집 기록\n")
            .ok_or("Missing attribution")?;
        let (title, body) = content.split_once('\n').ok_or("Missing title")?;
        if body.chars().count() < 300 || body.chars().any(|c| c < ' ' && c != '\n' && c != '\t') {
            return Err("Invalid Korean body".into());
        }
        let categories: Vec<String> = serde_json::from_value(r["app_categories"].clone())?;
        if categories.is_empty() || categories.iter().any(|c| !CATS.contains(&c.as_str())) {
            return Err("Invalid local category".into());
        }
        let id = r["id"].as_str().ok_or("Missing ID")?.to_string();
        if !ids.insert(id.clone()) {
            return Err("Duplicate local document".into());
        }
        // HTML pages may contain remote scripts and assets: open the self-contained source text.
        let original_path = within(
            &dir,
            if r["source_file"] == "original.pdf" {
                "original.pdf"
            } else {
                "text.txt"
            },
        )?;
        docs.push(Doc {
            location: None,
            id,
            title: title.trim_start_matches("# ").into(),
            categories,
            file: format!("{name}/ko.md"),
            body: body.trim().into(),
            display_body: manual_display(body.trim()),
            record: record.trim().into(),
            source: r["publisher"].as_str().unwrap_or("Official source").into(),
            original_uri: format!(
                "file://{}",
                utf8_percent_encode(&original_path.to_string_lossy(), URI_ESC)
            ),
            original_path,
            spread: 0,
        });
    }
    Ok(docs)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_manuals_require_review_and_preserve_search_and_pdf_page() {
        let root = env::temp_dir().join(format!("sos-manual-test-{}", std::process::id()));
        fs::create_dir_all(root.join("manual")).unwrap();
        let text = "# 교범\n\n### 물 구하기\n빗물을 모아 안전하게 처리한다.\n\n## 출처·편집 기록\n원전과 수정 근거\n";
        let start = text.find("###").unwrap();
        let end = text.find("\n## 출처·편집 기록\n").unwrap();
        fs::write(root.join("manual/ko.md"), text).unwrap();
        fs::write(root.join("manual/original.pdf"), b"PDF fixture").unwrap();
        let mut manifest = serde_json::json!({
            "release_status":"local_validation_only", "manuals":[{
                "directory":"manual", "id":"test", "title":"시험 교범", "pages":5,
                "content_review":"passed", "coverage_review":"passed",
                "ko_sha256":hash(text.as_bytes()), "original_sha256":hash(b"PDF fixture"),
                "sections":[{"id":"water", "title":"물 구하기", "page":3,
                    "start_byte":start, "end_byte":end, "categories":["식수"]}]
            }]
        });
        let write = |m: &Value| {
            fs::write(
                root.join("local-manuals.json"),
                serde_json::to_vec(m).unwrap(),
            )
            .unwrap()
        };
        write(&manifest);
        let docs = load_local_manuals(&root).unwrap();
        assert_eq!(search(&docs, "빗물", "식수"), vec![0]);
        assert!(search(&docs, "원전과", "전체").is_empty()); // edit history is not search evidence
        assert!(docs[0].original_uri.ends_with("#page=3"));
        assert!(!root.join("approved.json").exists());
        manifest["manuals"][0]["content_review"] = "pending".into();
        write(&manifest);
        assert!(load_local_manuals(&root).is_err());
        manifest["manuals"][0]["content_review"] = "passed".into();
        manifest["manuals"][0]["sections"][0]["start_byte"] = (start + 5).into();
        write(&manifest);
        assert!(load_local_manuals(&root).is_err()); // UTF-8 interior boundary
        manifest["manuals"][0]["sections"][0]["start_byte"] = start.into();
        write(&manifest);
        fs::write(root.join("manual/ko.md"), "changed").unwrap();
        assert!(load_local_manuals(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires the private reviewed manual corpus; run explicitly after preparation"]
    fn full_local_manual_corpus_search_render_and_source_links() {
        use ratatui::backend::TestBackend;
        let root = crate::project_root().join("dev/content-library");
        let docs = load_local_manuals(&root).unwrap();
        let manifest: Value =
            serde_json::from_slice(&fs::read(root.join("local-manuals.json")).unwrap()).unwrap();
        let expected: usize = manifest["manuals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["sections"].as_array().unwrap().len())
            .sum();
        assert_eq!(docs.len(), expected);
        assert!(docs.len() > 250);
        for (id, category) in [
            ("army-atp-3-50-21:page-106", "식량·조리"),
            ("army-atp-3-50-21:page-55", "식수"),
            ("army-atp-3-50-21:page-127", "야외기술"),
            ("army-atp-3-50-21:page-143", "피난처·체온"),
            ("army-atp-3-50-21:page-202", "야외기술"),
            ("marines-mcrp-3-02h:page-77", "식수"),
            ("marines-mcrp-3-02h:page-90", "대피·안전"),
        ] {
            let index = docs.iter().position(|d| d.id == id).unwrap();
            assert!(search(&docs, &docs[index].title, category).contains(&index));
        }
        let fish = docs
            .iter()
            .position(|d| d.id == "army-atp-3-50-21:page-106")
            .unwrap();
        assert!(!search(&docs, "", "식수").contains(&fish));
        let rows = search(&docs, "", "전체");
        for (index, doc) in docs.iter().enumerate() {
            assert!(search(&docs, &doc.title, "전체").contains(&index));
            for category in &doc.categories {
                assert!(search(&docs, &doc.title, category).contains(&index));
            }
            let word = doc
                .body
                .split_whitespace()
                .find(|w| w.chars().any(|c| ('가'..='힣').contains(&c)) && !doc.title.contains(*w))
                .unwrap();
            assert!(search(&docs, word, "전체").contains(&index));
            assert!(doc.original_uri.ends_with(&format!("#page={}", doc.spread)));
            assert!(doc.original_path.is_file());
            for (w, h) in [(90, 24), (120, 30), (160, 50)] {
                let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                let mut app = App {
                    opened: Some(index),
                    selected: index,
                    ..Default::default()
                };
                terminal
                    .draw(|f| draw(f, &mut app, &docs, &rows, &Ai::default(), false))
                    .unwrap();
            }
        }
        for q in ["지혈", "피난처", "나침반", "식수", "매듭"] {
            assert!(
                !search(&docs, q, "전체").is_empty(),
                "Missing search topic: {q}"
            );
        }
    }
    #[test]
    fn default_local_documents_are_complete_searchable_and_scrollable() {
        use ratatui::backend::TestBackend;
        let root = crate::project_root().join("dev/content-library");
        let docs = load_default_local(&root).unwrap();
        assert!(default_includes_manuals(&root).unwrap());
        let profile: Value =
            serde_json::from_slice(&fs::read(root.join("default-local.json")).unwrap()).unwrap();
        assert_eq!(
            docs.len(),
            299 + profile["documents"].as_array().unwrap().len()
        );
        for entry in profile["documents"].as_array().unwrap() {
            let dir = root.join(entry.as_str().unwrap());
            let r: Value =
                serde_json::from_slice(&fs::read(dir.join("record.json")).unwrap()).unwrap();
            let i = docs
                .iter()
                .position(|d| d.id == r["id"].as_str().unwrap())
                .unwrap();
            let doc = &docs[i];
            let text = fs::read_to_string(dir.join("ko.md")).unwrap();
            let full = text
                .split_once("\n## 출처·편집 기록\n")
                .unwrap()
                .0
                .split_once('\n')
                .unwrap()
                .1
                .trim();
            assert_eq!(doc.body, full);
            assert!(doc.original_path.is_file());
            let tail = doc
                .body
                .split_whitespace()
                .rev()
                .find(|w| w.chars().any(|c| ('가'..='힣').contains(&c)) && !doc.title.contains(*w))
                .unwrap();
            assert!(search(&docs, tail, "전체").contains(&i));
            for cat in &doc.categories {
                assert!(search(&docs, &doc.title, cat).contains(&i));
            }
            for (w, h) in [(90, 24), (120, 30), (160, 50)] {
                let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                let rows = search(&docs, "", "전체");
                let mut app = App {
                    opened: Some(i),
                    selected: i,
                    offset: usize::MAX,
                    ..Default::default()
                };
                terminal
                    .draw(|f| draw(f, &mut app, &docs, &rows, &Ai::default(), false))
                    .unwrap();
                // A localized article may fit entirely in a tall viewport.
                let (_, _, split, bottom) = geometry(w, h, 0);
                let visible = bottom.saturating_sub(split + 6).max(1) as usize;
                let expected = wrap(&doc.display_body, (w - 5) as usize)
                    .len()
                    .saturating_sub(visible);
                assert_eq!(app.offset, expected, "{} at {w}x{h}", doc.id);
                assert!(app.offset < usize::MAX);
            }
        }
        // Rights-reviewed articles can move from local-only into the public collection.
        let (mut searchable, _) = load_approved(&root).unwrap();
        searchable.extend(docs);
        for query in ["포스핀", "24시간", "검문소", "인신매매", "의사소통", "매몰"]
        {
            assert!(!search(&searchable, query, "전체").is_empty(), "{query}");
        }
    }

    #[test]
    fn korean_instructions_exclude_foreign_contacts_and_preserve_reference() {
        let root = crate::project_root().join("dev/content-library");
        let mut docs = load_default_local(&root).unwrap();
        docs.extend(load_approved(&root).unwrap().0);
        for (id, foreign, domestic) in [
            ("unhcr-syria-protection", "0952535262", "경찰112"),
            (
                "unicef-fleeing-ukraine",
                "아동학대 신고 119",
                "실종아동등182",
            ),
            ("icrc-gaza-help", "08 283 2400", "긴급복지 상담은129"),
            ("ukhsa-chemical-emergency", "NHS111", "구조·구급은119"),
            ("ukhsa-radiation-emergency", "2~3일분", "수도사업자"),
            ("mfs-clothing-fire", "50센트", "동전 크기 하나로"),
            ("uk-telecom", "Ofcom", "국내 긴급"),
            ("uk-power", "Priority Services Register", "한전123"),
        ] {
            let index = docs.iter().position(|d| d.id == id).unwrap();
            let doc = &docs[index];
            assert!(!doc.body.contains(foreign), "foreign instruction in {id}");
            assert!(doc.record.contains(foreign), "lost source detail in {id}");
            assert!(doc.body.contains(domestic), "missing Korean action in {id}");
            assert!(!search(&docs, foreign, "전체").contains(&index));
            assert!(search(&docs, domestic, "전체").contains(&index));
        }
    }

    #[test]
    fn default_local_rejects_changed_or_unreviewed_korean() {
        let source = crate::project_root().join("dev/content-library");
        let profile: Value =
            serde_json::from_slice(&fs::read(source.join("default-local.json")).unwrap()).unwrap();
        let entry = profile["documents"][0].as_str().unwrap();
        let root = env::temp_dir().join(format!("sos-local-article-test-{}", std::process::id()));
        let dir = root.join("item");
        fs::create_dir_all(&dir).unwrap();
        for file in [
            "original.html",
            "text.txt",
            "ko.md",
            "record.json",
            "local-review.json",
        ] {
            fs::copy(source.join(entry).join(file), dir.join(file)).unwrap();
        }
        fs::write(root.join("default-local.json"),r#"{"scope":"user_authorized_local_default","include_manuals":false,"documents":["item"]}"#).unwrap();
        assert_eq!(load_default_local(&root).unwrap().len(), 1);
        let review_path = dir.join("local-review.json");
        let original_review = fs::read(&review_path).unwrap();
        let mut review: Value = serde_json::from_slice(&original_review).unwrap();
        review["coverage_review"] = "pending".into();
        fs::write(&review_path, serde_json::to_vec(&review).unwrap()).unwrap();
        assert!(load_default_local(&root).is_err());
        fs::write(&review_path, original_review).unwrap();
        fs::write(dir.join("ko.md"), "changed").unwrap();
        assert!(load_default_local(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn approved_library_search_render_and_source_links() {
        use ratatui::backend::TestBackend;
        let root = crate::project_root().join("dev/content-library");
        let (docs, _) = load_approved(&root).unwrap();
        let entries: Vec<String> =
            serde_json::from_slice(&fs::read(root.join("approved.json")).unwrap()).unwrap();
        assert_eq!(docs.len(), entries.len());
        assert!(!docs.is_empty());
        for (i, d) in docs.iter().enumerate() {
            assert!(search(&docs, &d.title, "전체").contains(&i));
            assert!(d.original_path.is_file());
            let korean = fs::read_to_string(root.join(&d.file)).unwrap();
            let full_body = korean
                .split_once("\n## 출처·편집 기록\n")
                .unwrap()
                .0
                .split_once('\n')
                .unwrap()
                .1
                .trim();
            assert_eq!(d.body, full_body, "{} truncated Korean body", d.id);
            let tail = d
                .body
                .split_whitespace()
                .rev()
                .find(|s| s.chars().any(|c| ('가'..='힣').contains(&c)))
                .unwrap();
            for cat in &d.categories {
                assert!(CATS.contains(&cat.as_str()));
                assert!(
                    search(&docs, tail, cat).contains(&i),
                    "{} tail not indexed",
                    d.id
                );
            }
            for (w, h) in [(90, 24), (120, 30), (160, 50)] {
                let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                let mut app = App {
                    opened: Some(i),
                    selected: i,
                    ..Default::default()
                };
                let rows = search(&docs, "", "전체");
                terminal
                    .draw(|f| draw(f, &mut app, &docs, &rows, &Ai::default(), false))
                    .unwrap();
                for view in [0, 1] {
                    app.view = view;
                    app.offset = usize::MAX;
                    terminal
                        .draw(|f| draw(f, &mut app, &docs, &rows, &Ai::default(), false))
                        .unwrap();
                    assert!(app.offset < usize::MAX);
                    let (_, half, split, bottom) = geometry(w, h, view);
                    let width = (if view == 1 { half } else { w }) - 5;
                    let mut screen = String::new();
                    for y in split + 5..bottom - 1 {
                        let mut x = 2;
                        while x < 2 + width {
                            let symbol = terminal.backend().buffer()[(x, y)].symbol();
                            screen.push_str(symbol);
                            x += symbol
                                .chars()
                                .map(|c| c.width().unwrap_or(0) as u16)
                                .sum::<u16>()
                                .max(1);
                        }
                    }
                    let compact: String = screen.chars().filter(|c| !c.is_whitespace()).collect();
                    let expected: String = tail.chars().filter(|c| !c.is_whitespace()).collect();
                    assert!(
                        compact.contains(&expected),
                        "{} tail not visible at {w}x{h}, view {view}: expected {expected:?}, got {compact:?}",
                        d.id
                    );
                }
            }
        }
        assert!(search(&docs, "아세트아미노펜", "전체").len() > 0);
        assert!(search(&docs, "30분", "식수").len() > 0);
    }
    #[test]
    fn current_first_aid_keeps_korean_protocols_and_original_provenance_separate() {
        let root = crate::project_root().join("dev/content-library");
        let (docs, _) = load_approved(&root).unwrap();
        let body = |id: &str| docs.iter().find(|d| d.id == id).unwrap().body.as_str();
        let cpr = body("nhs-child-cpr");
        assert!(cpr.contains("두 엄지") && cpr.contains("30회") && cpr.contains("2분간"));
        assert!(!cpr.contains("두 손가락") && !cpr.contains("999"));
        let choking = body("nhs-child-choking");
        assert!(
            choking.contains("한 손의 손꿈치") && choking.contains("배를 밀어내는 처치는 하지")
        );
        assert!(body("nhs-frostbite").contains("재동결"));
        assert!(
            body("nhs-eye-injury").contains("관통상")
                && body("nhs-eye-injury").contains("최소 20분")
        );
        assert!(body("nhs-anaphylaxis").contains("서거나 걷지"));
        let original =
            fs::read_to_string(root.join("sources/응급처치/nhs-child-cpr/text.txt")).unwrap();
        assert!(original.contains("2 fingers") && original.contains("5 initial rescue breaths"));
        let full = fs::read_to_string(root.join("sources/응급처치/nhs-child-cpr/ko.md")).unwrap();
        assert!(full
            .split_once("\n## 출처·편집 기록\n")
            .unwrap()
            .1
            .contains("교정"));
    }

    #[test]
    fn refuses_unreviewed_and_changed_content() {
        let root = std::env::temp_dir().join(format!("sos-library-gate-{}", std::process::id()));
        fs::create_dir_all(root.join("item")).unwrap();
        fs::write(root.join("approved.json"), "[\"item\"]").unwrap();
        fs::write(
            root.join("item/record.json"),
            r#"{"rights":{"redistribute":true,"translate":true,"edit":true},"app_approved":false}"#,
        )
        .unwrap();
        assert!(load_approved(&root)
            .err()
            .unwrap()
            .to_string()
            .contains("Review not approved"));
        fs::write(root.join("item/ko.md"), "changed").unwrap();
        assert!(checked(
            &root.join("item"),
            "ko.md",
            &Value::String(hash(b"original"))
        )
        .is_err());
        assert!(within(&root, "../").is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
