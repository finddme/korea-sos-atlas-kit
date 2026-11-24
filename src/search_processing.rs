use crate::agentic_policy;
use serde_json::{json, Value};
use std::time::Instant;

// Character count is a preview target, not a model token budget.
// Never silently cut a condition or negation out of a paragraph.
pub fn excerpt(doc: &Value, queries: &[String], question: &str, target: usize) -> String {
    let body = doc["body"].as_str().unwrap_or("");
    let terms = terms(&format!("{} {}", queries.join(" "), question));
    let compact_blocks: Vec<String> = body
        .split("\n\n")
        .map(|b| b.to_lowercase().split_whitespace().collect())
        .collect();
    let weights: Vec<usize> = terms
        .iter()
        .map(|t| {
            1 + 8 / compact_blocks
                .iter()
                .filter(|b| b.contains(t))
                .count()
                .max(1)
        })
        .collect();
    let mut heading = "";
    let mut blocks = vec![];
    for (index, block) in body.split("\n\n").enumerate() {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }
        if block
            .lines()
            .all(|l| l.trim().starts_with('#') || l.trim().starts_with("<!--"))
        {
            if block.starts_with('#') {
                heading = block;
            }
            continue;
        }
        let h = if block.starts_with('#') { "" } else { heading };
        let lower: String = block.to_lowercase().split_whitespace().collect();
        let hlower: String = h.to_lowercase().split_whitespace().collect();
        let score = terms
            .iter()
            .zip(&weights)
            .map(|(t, w)| {
                w * (4 * usize::from(lower.contains(t)) + usize::from(hlower.contains(t)))
            })
            .sum::<usize>();
        blocks.push((index, block, h, score));
        if block.starts_with('#') {
            heading = block.lines().next().unwrap_or(heading);
        }
    }
    blocks.sort_by(|a, b| b.3.cmp(&a.3).then(a.0.cmp(&b.0)));
    let mut chosen = vec![];
    let mut used = 0;
    for (i, block, h, _) in blocks {
        let len = block.chars().count() + h.chars().count() + 2;
        if chosen.is_empty() || used + len <= target {
            // Explicitly signal an unreasonably large block instead of returning only headings.
            if chosen.is_empty() && len > target.max(512) {
                return String::new();
            }
            chosen.push((i, block, h));
            used += len;
        }
        if used >= target {
            break;
        }
    }
    chosen.sort_by_key(|x| x.0);
    let mut previous = "";
    let mut out = vec![];
    for (_, block, h) in chosen {
        if !h.is_empty() && h != previous {
            out.push(h);
            previous = h;
        }
        out.push(block);
    }
    out.join("\n\n")
}
fn terms(s: &str) -> Vec<String> {
    let s: String = s
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    let mut out = vec![];
    for t in s.split_whitespace().filter(|t| t.chars().count() >= 2) {
        if !out.iter().any(|x| x == t) {
            out.push(t.to_string());
        }
    }
    out
}
pub struct SearchResult {
    pub queries: Vec<String>,
    pub tools: Vec<Value>,
    pub candidates: Vec<Value>,
}
pub fn search<F>(raw: &str, _question: &str, mut fetch: F) -> Result<SearchResult, String>
where
    F: FnMut(&str) -> Result<Vec<Value>, String>,
{
    let initial = agentic_policy::queries(raw);
    let mut queue: Vec<(String, bool)> = initial.iter().cloned().map(|q| (q, false)).collect();
    let mut seen = vec![];
    let mut cursor = 0;
    let mut lists = vec![];
    let mut tools = vec![];
    while cursor < queue.len() && tools.len() < 9 {
        let (q, retry) = queue[cursor].clone();
        cursor += 1;
        if seen.contains(&q) {
            continue;
        }
        seen.push(q.clone());
        let now = Instant::now();
        let mut docs = fetch(&q)?;
        if docs.is_empty() && !retry {
            for term in agentic_policy::relaxed(&q) {
                queue.push((term, true));
            }
        }
        docs.sort_by_key(|d| std::cmp::Reverse(agentic_policy::relevance(d, &initial)));
        tools.push(json!({"query":q,"retry":retry,"elapsed_ms":now.elapsed().as_millis(),"hits":docs.len(),"hit_ids":docs.iter().map(|d|d["id"].clone()).collect::<Vec<_>>()}));
        lists.push(docs);
    }
    let mut candidates: Vec<Value> = vec![];
    for rank in 0..24 {
        for list in &lists {
            if let Some(d) = list.get(rank) {
                if candidates.len() < 24 && !candidates.iter().any(|c| c["id"] == d["id"]) {
                    candidates.push(d.clone());
                }
            }
        }
    }

    Ok(SearchResult {
        queries: initial,
        tools,
        candidates,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_safety_paragraph_not_replaced_with_heading() {
        let paragraph = format!(
            "{} 화학 경보에서 실내 대피 지시를 받으면 외부로 나가지 않습니다.",
            "상황 확인. ".repeat(40)
        );
        let body = format!("## 경보\n\n{paragraph}\n\n## 기타\n\n짧은 문장.");
        let v = excerpt(&json!({"body":body}), &["경보".into()], "대피 지시", 260);
        assert!(v.contains(&paragraph));
        assert!(v.contains("나가지 않습니다."));
    }
    #[test]
    fn question_keeps_specific_condition() {
        let body="## 식수\n\n정전에는 물을 얼려 둡니다.\n\n## 건물 급수\n\n생활급수 펌프는 전원 공급을 확인해야 합니다. 발전기가 있다고 자동으로 운전되는 것은 아닙니다.";
        let v = excerpt(
            &json!({"body":body}),
            &["정전".into()],
            "생활급수 펌프 발전기",
            30,
        );
        assert!(v.contains("자동으로 운전되는 것은 아닙니다."));
        assert!(!v.contains("얼려"));
    }
    #[test]
    fn headings_alone_are_not_evidence() {
        assert!(excerpt(&json!({"body":"## 경보\n\n### 대피"}), &[], "", 260).is_empty());
    }
    #[test]
    fn oversized_block_is_explicit() {
        let doc = json!({"id":"long","title":"긴 자료","body":"가".repeat(5000)});
        assert!(excerpt(&doc, &[], "", 260).is_empty());
        let result = search(r#"{"queries":["긴 자료"]}"#, "", |_| {
            Ok(vec![doc.clone()])
        })
        .unwrap();
        assert_eq!(result.candidates.len(), 1);
    }
    #[test]
    fn bounded_retries_preserve_long_initial_queries() {
        let raw = r#"{"queries":["화학물질 경보 지하","ORS 봉지 물 양","경보 해제 기준"]}"#;
        let mut seen = vec![];
        let r = search(raw, "", |q| {
            seen.push(q.to_string());
            Ok(vec![])
        })
        .unwrap();
        assert_eq!(
            &seen[..3],
            &["화학물질 경보 지하", "ORS 봉지 물 양", "경보 해제 기준"]
        );
        assert!(seen.len() <= 9);
        assert_eq!(r.queries.len(), 3);
    }
}
