// SPDX-License-Identifier: AGPL-3.0-only
//! Passage selection preserves whole paragraphs; it does not certify semantic relevance.
use serde_json::{json, Value};
use std::collections::HashSet;
fn terms(s: &str) -> Vec<String> {
    let cleaned: String = s
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let mut out = vec![];
    for word in cleaned.split_whitespace() {
        let mut word = word.to_string();
        for suffix in [
            "에서는",
            "으로",
            "에서",
            "에게",
            "까지",
            "보다",
            "하고",
            "은",
            "는",
            "을",
            "를",
            "의",
            "도",
            "에",
        ] {
            if word.ends_with(suffix) && word.chars().count() > suffix.chars().count() + 1 {
                word.truncate(word.len() - suffix.len());
                break;
            }
        }
        if word.chars().count() < 2
            || [
                "어떻게",
                "알려줘",
                "무엇",
                "뭐야",
                "먼저",
                "경우",
                "상황",
                "대처",
                "대응",
                "좋아",
                "하나요",
                "하는",
                "대한",
                "주의할",
                "있다면",
            ]
            .contains(&word.as_str())
        {
            continue;
        }
        if !out.contains(&word) {
            out.push(word);
        }
    }
    out
}
// Keep a short section's introduction, checklist and exceptions together.
fn sections(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut heading = String::new();
    let mut body = String::new();
    for line in text.lines() {
        if line.starts_with('#') {
            if !body.trim().is_empty() {
                out.push((heading.clone(), body.trim().to_owned()));
            }
            heading = line.to_owned();
            body.clear();
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }
    if !body.trim().is_empty() {
        out.push((heading, body.trim().to_owned()));
    }
    out.into_iter()
        .flat_map(|(h, b)| {
            if h.chars().count() + b.chars().count() <= 1300 {
                vec![(h, b)]
            } else {
                b.split("\n\n")
                    .filter(|b| !b.trim().is_empty())
                    .map(|b| (h.clone(), b.trim().to_owned()))
                    .collect()
            }
        })
        .collect()
}
pub fn passages(docs: &[Value], question: &str, queries: &[String]) -> Vec<Value> {
    let mut query_terms = terms(question);
    for t in terms(&queries.join(" ")) {
        if !query_terms.contains(&t) {
            query_terms.push(t);
        }
    }
    let weights: Vec<usize> = query_terms
        .iter()
        .map(|t| {
            let n = docs
                .iter()
                .filter(|d| d["body"].as_str().unwrap_or("").to_lowercase().contains(t))
                .count();
            (1 + docs.len() / (1 + n)).min(24)
        })
        .collect();
    let mut ranked = vec![];
    for (doc_index, doc) in docs.iter().enumerate() {
        let doc_title = doc["title"].as_str().unwrap_or("").to_lowercase();
        for (index, (heading, block)) in sections(doc["body"].as_str().unwrap_or(""))
            .into_iter()
            .enumerate()
        {
            let body = block.to_lowercase();
            let title = heading.to_lowercase();
            let score = query_terms
                .iter()
                .zip(&weights)
                .map(|(t, w)| {
                    w * (4 * usize::from(body.contains(t))
                        + usize::from(title.contains(t))
                        + (if index == 0 { 8 } else { 4 }) * usize::from(doc_title.contains(t)))
                })
                .sum::<usize>();
            if score == 0 || block.chars().count() + heading.chars().count() > 1300 {
                continue;
            }
            let text = if heading.is_empty() {
                block.to_owned()
            } else {
                format!("{heading}\n\n{block}")
            };
            ranked.push((score,doc_index,index,json!({"source_id":doc["id"],"title":doc["title"],"body":text,"original_uri":doc["original_uri"]})));
        }
    }
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut out = vec![];
    let mut seen = HashSet::new();
    let mut chars = 0;
    let mut per_source = std::collections::HashMap::<String, usize>::new();
    for (_, _, _, mut d) in ranked {
        let body = d["body"].as_str().unwrap();
        if !seen.insert(body.to_owned()) {
            continue;
        }
        let source = d["source_id"].as_str().unwrap_or("").to_owned();
        if per_source.get(&source).copied().unwrap_or(0) >= 2 {
            continue;
        }
        let len = body.chars().count();
        if chars + len > 3300 {
            continue;
        }
        chars += len;
        *per_source.entry(source).or_default() += 1;
        d["id"] = json!(format!("P{}", out.len() + 1));
        out.push(d);
        if out.len() == 8 {
            break;
        }
    }
    out
}
pub fn selected_docs(passages: &[Value], ids: &[Value]) -> Vec<Value> {
    // The small model is a reranker, not the sole authority for discarding evidence.
    // Keep the two highest lexical passages when selection is nonempty. All inputs
    // are still bounded by passages(), and whole paragraphs remain intact.
    let mut ids = ids.to_vec();
    if !ids.is_empty() {
        for p in passages.iter().take(2) {
            if !ids.contains(&p["id"]) {
                ids.push(p["id"].clone());
            }
        }
        // A selected document's bounded companion sections may contain the
        // contraindication/exception. The reranker must not remove them.
        let sources: HashSet<_> = passages
            .iter()
            .filter(|p| ids.contains(&p["id"]))
            .filter_map(|p| p["source_id"].as_str())
            .collect();
        for p in passages {
            if sources.contains(p["source_id"].as_str().unwrap_or("")) && !ids.contains(&p["id"]) {
                ids.push(p["id"].clone());
            }
        }
    }
    let mut out: Vec<Value> = vec![];
    for p in passages.iter().filter(|p| ids.contains(&p["id"])) {
        let id = p["source_id"].clone();
        if let Some(d) = out.iter_mut().find(|d| d["id"] == id) {
            let old = d["body"].as_str().unwrap();
            d["body"] = json!(format!("{old}\n\n{}", p["body"].as_str().unwrap()));
        } else {
            out.push(json!({"id":id,"title":p["title"],"body":p["body"],"original_uri":p["original_uri"]}));
        }
    }
    out
}

/// Append relevant sections from documents already selected, without replacing
/// existing evidence. The same total character budget still applies.
pub fn complete_context(selected: &mut [Value], documents: &[Value], question: &str) {
    let terms = terms(question);
    let audience: &[&str] = if ["아기", "영아", "신생아"]
        .iter()
        .any(|s| question.contains(s))
    {
        &["영아", "신생아", "아기"]
    } else if ["어린이", "소아", "아이"]
        .iter()
        .any(|s| question.contains(s))
    {
        &["어린이", "소아", "아이"]
    } else {
        &[]
    };
    let mut extra = vec![];
    for (i, selected_doc) in selected.iter().enumerate() {
        let Some(doc) = documents.iter().find(|d| d["id"] == selected_doc["id"]) else {
            continue;
        };
        let existing = selected_doc["body"].as_str().unwrap_or("");
        for (heading, body) in sections(doc["body"].as_str().unwrap_or("")) {
            if existing.contains(&body) {
                continue;
            }
            let score = terms
                .iter()
                .filter(|t| t.chars().count() >= 3 && heading.to_lowercase().contains(t.as_str()))
                .count()
                * 12
                + audience.iter().filter(|t| body.contains(**t)).count() * 6;
            if score == 0 {
                continue;
            }
            let text = format!("{heading}\n\n{body}");
            if text.chars().count() <= 1300 {
                extra.push((score, i, text));
            }
        }
    }
    extra.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut used = selected
        .iter()
        .map(|d| d["body"].as_str().unwrap_or("").chars().count())
        .sum::<usize>();
    for (_, i, text) in extra {
        let cost = text.chars().count() + 2;
        if used + cost > 3300 {
            continue;
        }
        let old = selected[i]["body"].as_str().unwrap_or("");
        selected[i]["body"] = json!(format!("{old}\n\n{text}"));
        used += cost;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn context_completion_preserves_existing_and_adds_audience_and_named_checklist() {
        let documents = vec![
            json!({"id":"medicine","body":"## 사용법\n\n기존 행동과 예외를 보존합니다.\n\n## 사용 대상\n\n영아에게는 별도 제품 지침을 확인합니다.\n\n## 인계표\n\n주 지원자와 예비 지원자, 장비 조건을 기록합니다."}),
        ];
        let original =
            json!({"id":"medicine","body":"## 사용법\n\n기존 행동과 예외를 보존합니다."});
        let mut selected = vec![original.clone()];
        complete_context(
            &mut selected,
            &documents,
            "아기에게도 쓸 수 있나요? 인계표도 알려줘",
        );
        let body = selected[0]["body"].as_str().unwrap();
        assert!(body.starts_with(original["body"].as_str().unwrap()));
        assert!(body.contains("영아에게는 별도 제품 지침"));
        assert!(body.contains("예비 지원자"));
        let mut unrelated = vec![original.clone()];
        complete_context(&mut unrelated, &documents, "지도 검색");
        assert_eq!(unrelated, vec![original]);
        let mut full = vec![json!({"id":"medicine","body":"보존".repeat(1649)})];
        let before = full.clone();
        complete_context(&mut full, &documents, "인계표");
        assert_eq!(
            full, before,
            "Budget cannot be met by deleting earlier conditions"
        );
    }
    #[test]
    fn selected_source_companion_is_not_discarded() {
        let passages = vec![
            json!({"id":"P1","source_id":"a","title":"A","body":"첫 문단"}),
            json!({"id":"P2","source_id":"b","title":"B","body":"다른 문서"}),
            json!({"id":"P3","source_id":"a","title":"A","body":"같은 출처의 금지와 예외"}),
        ];
        let selected = selected_docs(&passages, &[json!("P1")]);
        assert!(selected[0]["body"]
            .as_str()
            .unwrap()
            .contains("금지와 예외"));
        assert!(selected_docs(&passages, &[]).is_empty());
    }
    #[test]
    #[ignore = "replay saved 65 query outputs against local corpus; no model"]
    fn replay_saved_retrieval() {
        let root = crate::project_root();
        let baseline = root.join("dev/poc/model-evaluation/runs/2026-11-17-gemma-full-65");
        let output = std::env::var_os("SOS_RETRIEVAL_REPORT").expect("report path");
        let (docs, _) = crate::load(&root.join("dev/content-library")).unwrap();
        let corpus: Vec<_> = docs
            .iter()
            .filter(|d| !d.id.starts_with("shelter:"))
            .map(|d| json!({"id":d.id,"title":d.title,"body":d.body,"original_uri":d.original_uri}))
            .collect();
        let gold = std::fs::read_to_string(baseline.join("gold.jsonl")).unwrap();
        let mut report = vec![];
        for line in gold.lines() {
            let g: Value = serde_json::from_str(line).unwrap();
            let id = g["id"].as_str().unwrap();
            let dir = baseline.join("trace").join(id);
            let query_file = std::fs::read_dir(&dir).ok().and_then(|files| {
                files
                    .filter_map(Result::ok)
                    .map(|e| e.path())
                    .find(|p| p.to_string_lossy().ends_with("-query.json"))
            });
            let Some(file) = query_file else { continue };
            let query: Value = serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
            let q = g["question"].as_str().unwrap();
            let found = crate::search_processing::search(&query["result"].to_string(), q, |s| {
                let terms: Vec<_> = s.split_whitespace().map(str::to_lowercase).collect();
                Ok(corpus
                    .iter()
                    .filter(|d| {
                        let hay = format!(
                            "{} {}",
                            d["title"].as_str().unwrap(),
                            d["body"].as_str().unwrap()
                        )
                        .to_lowercase();
                        terms.iter().all(|t| hay.contains(t))
                    })
                    .cloned()
                    .collect())
            })
            .unwrap();
            let p = passages(&found.candidates, q, &found.queries);
            let joined = p
                .iter()
                .filter_map(|d| d["body"].as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let read_stage = |stage: &str| -> Value {
                let path = std::fs::read_dir(&dir)
                    .unwrap()
                    .filter_map(Result::ok)
                    .map(|e| e.path())
                    .find(|p| p.to_string_lossy().ends_with(&format!("-{stage}.json")))
                    .unwrap();
                serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
            };
            let selection = read_stage("select");
            let mut chosen =
                selected_docs(&p, selection["result"]["selected_ids"].as_array().unwrap());
            complete_context(&mut chosen, &found.candidates, q);
            let chosen_text = chosen
                .iter()
                .filter_map(|d| d["body"].as_str())
                .collect::<Vec<_>>()
                .join("\n\n");
            let baseline_input = read_stage("answer")["user"].as_str().unwrap().to_owned();
            let mut old_hit = 0;
            let mut new_hit = 0;
            let mut hit = 0;
            let mut total = 0;
            for e in g["evidence"].as_array().unwrap() {
                if let Some(quote) = e["quote"].as_str() {
                    for line in quote
                        .lines()
                        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
                    {
                        total += 1;
                        if joined.contains(line) {
                            hit += 1;
                        }
                        if baseline_input.contains(line) {
                            old_hit += 1;
                        }
                        if chosen_text.contains(line) {
                            new_hit += 1;
                        }
                    }
                }
            }
            report.push(json!({"id":id,"gold_lines_present":hit,"gold_lines_total":total,"answer_gold_before":old_hit,"answer_gold_after":new_hit,"chosen":chosen,"passages":p,"document_ids":found.candidates.iter().map(|d|d["id"].clone()).collect::<Vec<_>>()}));
        }
        assert_eq!(report.len(), 63);
        std::fs::write(output, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    }
    #[test]
    fn title_match_keeps_checklist_and_following_exception_in_same_section() {
        let d = json!({"id":"kit","title":"필수품","body":"# 필수품\n\n## 준비\n\n준비 안내입니다.\n\n- 물\n- 음식\n- 개인 약\n\n다만 개인 상태에 맞게 구성합니다.\n\n## 출처\n\n기록"});
        let p = passages(&[d], "필수품", &[]);
        assert!(p[0]["body"].as_str().unwrap().contains("- 물"));
        assert!(p.iter().any(|v| {
            let b = v["body"].as_str().unwrap();
            b.contains("- 물") && b.contains("다만 개인 상태")
        }));
    }
    #[test]
    fn isolates_question_paragraph_without_cutting_conditions() {
        let docs = vec![
            json!({"id":"a","title":"정전 안내","body":"## 냉장고\n\n냉장고 문은 닫습니다. 시간은 보장이 아니므로 온도도 확인합니다.\n\n## 태양광\n\n태양광의 계통 연결은 전문가가 확인합니다."}),
        ];
        let p = passages(&docs, "냉장고 문을 열어도 되나요?", &["냉장고".into()]);
        assert_eq!(p.len(), 1);
        assert!(p[0]["body"].as_str().unwrap().contains("보장이 아니므로"));
        assert!(!p[0]["body"].as_str().unwrap().contains("전문가"));
        let selected = selected_docs(&p, &[json!("P1")]);
        assert_eq!(selected[0]["id"], "a");
    }
    #[test]
    fn unrelated_and_oversized_blocks_are_not_silently_truncated() {
        assert!(passages(
            &[json!({"id":"x","body":"냉장고".repeat(900)})],
            "냉장고",
            &[]
        )
        .is_empty());
        assert!(passages(&[json!({"id":"x","body":"다른 내용"})], "냉장고", &[]).is_empty());
    }
    #[test]
    fn reranker_cannot_drop_both_top_passages_or_override_empty_selection() {
        let p: Vec<_> = (1..=5).map(|i| json!({"id":format!("P{i}"),"source_id":format!("doc{i}"),"title":"t","body":format!("whole paragraph {i}")})).collect();
        assert!(selected_docs(&p, &[]).is_empty());
        let d = selected_docs(&p, &[json!("P3"), json!("P4"), json!("P5")]);
        assert_eq!(d.len(), 5);
        assert_eq!(d[1]["body"], "whole paragraph 2");
    }
}
