use serde_json::{json, Value};

pub fn query_schema() -> Value {
    json!({"type":"json_schema","schema":{"type":"object","properties":{"queries":{"type":"array","items":{"type":"string","maxLength":256},"minItems":1,"maxItems":3}},"required":["queries"],"additionalProperties":false}})
}
// Treat model output as data only. No shell evaluation, no question-specific dictionary.
pub fn queries(raw: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<Value>(raw) else {
        return vec![];
    };
    let mut result = vec![];
    if let Some(a) = v["queries"].as_array() {
        for s in a.iter().take(3).filter_map(Value::as_str) {
            if s.chars().count() > 256 {
                continue;
            }
            let clean: String = s
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c.is_whitespace() {
                        c
                    } else {
                        ' '
                    }
                })
                .collect();
            let words: Vec<_> = clean.split_whitespace().collect();
            if words.is_empty() {
                continue;
            }
            let q = words.join(" ");
            if !result.contains(&q) {
                result.push(q);
            }
        }
    }
    result
}
pub fn relaxed(q: &str) -> Vec<String> {
    if q.split_whitespace().count() < 2 {
        return vec![];
    }
    q.split_whitespace()
        .filter(|w| w.chars().count() >= 2)
        .take(6)
        .map(str::to_owned)
        .collect()
}
pub fn relevance(doc: &Value, queries: &[String]) -> usize {
    let title = doc["title"].as_str().unwrap_or("").to_lowercase();
    let body = doc["body"].as_str().unwrap_or("").to_lowercase();
    let mut terms = vec![];
    for q in queries {
        for w in q.split_whitespace() {
            let w = w.to_lowercase();
            if !terms.contains(&w) {
                terms.push(w);
            }
        }
    }
    terms
        .iter()
        .map(|t| {
            if title.contains(t) {
                4
            } else if body.contains(t) {
                1
            } else {
                0
            }
        })
        .sum()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_json_and_size_not_word_count() {
        assert!(queries("ORS 물").is_empty());
        assert_eq!(
            queries(r#"{"queries":["ORS 봉지 물 한 컵"]}"#),
            vec!["ORS 봉지 물 한 컵"]
        );
        assert!(queries(&json!({"queries":["가".repeat(257)]}).to_string()).is_empty());
    }
    #[test]
    fn normalizes_and_deduplicates() {
        assert_eq!(
            queries(r#"{"queries":["**ORS** + 물","ORS 물"]}"#),
            vec!["ORS 물"]
        );
    }
    #[test]
    fn bounded_relaxation() {
        assert_eq!(relaxed("ORS 희석"), vec!["ORS", "희석"]);
        assert!(relaxed("ORS").is_empty());
    }
    #[test]
    fn ranks_title_above_body() {
        let qs = vec!["화학 경보".into()];
        assert!(
            relevance(&json!({"title":"화학 경보","body":""}), &qs)
                > relevance(&json!({"title":"기타","body":"화학 경보"}), &qs)
        );
    }
}

#[cfg(test)]
mod integration {
    #[test]
    #[ignore = "requires the fixed local corpus and compiled app"]
    fn empty_and_query_recovers_with_single_term() {
        use std::process::Command;
        let search = |query: &str| {
            let root = env!("CARGO_MANIFEST_DIR");
            let o = Command::new(format!("{root}/target/release/korea-sos-atlas-kit"))
                .current_dir(root)
                .args(["--dump", "--query", query])
                .output()
                .unwrap();
            assert!(o.status.success());
            serde_json::from_slice::<Vec<serde_json::Value>>(&o.stdout)
                .unwrap()
                .len()
        };
        let q = "ORS zzznomatch";
        assert_eq!(search(q), 0);
        let terms = super::relaxed(q);
        assert_eq!(terms.len(), 2);
        let hits: Vec<_> = terms.iter().map(|q| search(q)).collect();
        assert!(hits[0] > 0);
        assert_eq!(hits[1], 0);
        println!(
            "empty AND query -> bounded term retries: {:?}, hits {:?}",
            terms, hits
        );
    }
}
