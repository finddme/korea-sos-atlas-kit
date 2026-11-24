// SPDX-License-Identifier: AGPL-3.0-only
//! Reviewed metadata refers to whole, hash-bound sections of the loaded corpus.
//! It is not a second corpus and cannot authorize unapproved content.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
fn digest(s: &str) -> String {
    format!("{:x}", Sha256::digest(s.as_bytes()))
}

/// `root` is the data folder (parent of content-library).
pub fn attach(root: &Path, corpus: &mut [Value]) {
    let Ok(bytes) = fs::read(root.join("content-library/evidence-bundles.json")) else {
        return;
    };
    let Ok(data) = serde_json::from_slice::<Value>(&bytes) else {
        return;
    };
    if data["version"] != 1 {
        return;
    }
    let Some(rows) = data["bundles"].as_array() else {
        return;
    };
    for row in rows {
        let Some(doc) = corpus.iter_mut().find(|d| d["id"] == row["source_id"]) else {
            continue;
        };
        if valid(row, doc) {
            doc["reviewed_bundle"] = row.clone();
        }
    }
}
fn valid(row: &Value, doc: &Value) -> bool {
    let Some(body) = doc["body"].as_str() else {
        return false;
    };
    if row["review"] != "source-checked" || row["source_body_sha256"] != digest(body) {
        return false;
    }
    let Some(sections) = row["sections"].as_array().filter(|s| !s.is_empty()) else {
        return false;
    };
    if !row["match_all_groups"].as_array().is_some_and(|g| {
        !g.is_empty()
            && g.iter().all(|g| {
                g.as_array().is_some_and(|a| {
                    !a.is_empty() && a.iter().all(|s| s.as_str().is_some_and(|s| !s.is_empty()))
                })
            })
    }) {
        return false;
    }
    sections.iter().all(|s| {
        let Some(quote) = s["quote"].as_str() else {
            return false;
        };
        let Some(start) = body.find(quote) else {
            return false;
        };
        let tail = &body[start..];
        let expected = if quote.starts_with("## ") {
            let next = tail[3..].find("\n## ").map(|p| p + 3).unwrap_or(tail.len());
            tail[..next].trim()
        } else if start == 0 {
            body.split("## ").next().unwrap_or("").trim()
        } else {
            return false;
        };
        !quote.is_empty()
            && quote == expected
            && s["sha256"] == digest(quote)
            && s["body_line"].as_u64() == Some(body[..start].lines().count() as u64 + 1)
    })
}
pub fn for_question(corpus: &[Value], question: &str) -> Vec<Value> {
    let q = question.to_lowercase();
    corpus.iter().filter_map(|doc|{
        let row=&doc["reviewed_bundle"];
        if row.is_null() || !valid(row,doc){return None;}
        if !row["match_all_groups"].as_array()?.iter().all(|g|g.as_array().unwrap().iter().any(|s|q.contains(s.as_str().unwrap()))){return None;}
        // Never transfer adult CPR parameters to a child in a mixed question.
        if doc["id"]=="nfa-adult-cpr" && ["영아","아기","소아","어린이"].iter().any(|s|q.contains(s)){return None;}
        let body=row["sections"].as_array()?.iter().map(|s|s["quote"].as_str().unwrap()).collect::<Vec<_>>().join("\n\n");
        Some(json!({"id":doc["id"],"title":doc["title"],"original_uri":doc["original_uri"],"body":body,"reviewed_bundle":row,"mandatory":true}))
    }).collect()
}
pub fn merge(mut selected: Vec<Value>, bundles: Vec<Value>, question: &str) -> Vec<Value> {
    if !bundles.is_empty()
        && !["군인", "군 교범", "군교범", "의무병", "군사"]
            .iter()
            .any(|s| question.contains(s))
    {
        selected.retain(|d| {
            !d["id"]
                .as_str()
                .is_some_and(|id| id.starts_with("army-atp-") || id.starts_with("marines-mcrp-"))
        });
    }
    selected.retain(|d| !bundles.iter().any(|b| b["id"] == d["id"]));
    let mut out = bundles;
    out.extend(selected);
    out
}
pub fn required_display(docs: &[&Value]) -> String {
    let mut blocks = vec![];
    for (i, d) in docs.iter().enumerate() {
        if d["mandatory"] == true {
            blocks.push(format!(
                "[{}] {}\n{}",
                i + 1,
                d["title"].as_str().unwrap_or(""),
                d["body"].as_str().unwrap_or("")
            ));
        }
    }
    if blocks.is_empty() {
        String::new()
    } else {
        format!(
            "필수 행동·조건 · 검토된 본문에서 그대로 제공\n{}\n\n",
            blocks.join("\n\n")
        )
    }
}

/// A limited prohibition in a paragraph must not be presented under a broader
/// generated prohibition label. Keep the entire explanatory paragraph intact.
/// This is a presentation rule, not a factual/semantic validation of the body.
pub fn neutralize_prohibition_labels(answer: &str) -> String {
    answer
        .lines()
        .map(|line| {
            let Some(start) = line.find("**") else {
                return line.to_owned();
            };
            let rest = &line[start + 2..];
            let Some(end) = rest.find("**") else {
                return line.to_owned();
            };
            let label = rest[..end].trim().trim_end_matches([':', '：']).trim();
            let body = rest[end + 2..]
                .trim_start()
                .trim_start_matches([':', '：'])
                .trim_start();
            let subject = label.split_whitespace().next().unwrap_or("");
            let broad = [
                "사용 금지",
                "운용 금지",
                "운전 금지",
                "작동 금지",
                "복용 금지",
                "섭취 금지",
            ]
            .iter()
            .any(|s| label.ends_with(s));
            let limited = ["대신", "대체", "보장", "임의로", "무단으로"]
                .iter()
                .any(|s| body.contains(s));
            if broad && subject.chars().count() >= 2 && body.contains(subject) && limited {
                format!("{}**주의:** {}", &line[..start], body)
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn air_raid_bundle_is_valid_and_matches_only_air_raid_questions() {
        let root = crate::project_root().join("dev");
        if !root.join("content-library/approved.json").exists() {
            return;
        }
        let (docs, _) = crate::load(&root.join("content-library")).unwrap();
        let mut corpus = crate::ai_corpus(&docs);
        attach(&root, &mut corpus);
        let ids = |q: &str| -> Vec<String> {
            for_question(&corpus, q)
                .iter()
                .map(|b| b["id"].as_str().unwrap().to_owned())
                .collect()
        };
        let hit = ids("공습 경보가 울리면 어떻게 해야 해?");
        assert!(hit.contains(&"kmcc-civil-alert-2026".to_owned()), "{hit:?}");
        let body = for_question(&corpus, "공습 경보가 울리면 어떻게 해야 해?")
            .into_iter()
            .find(|b| b["id"] == "kmcc-civil-alert-2026")
            .unwrap()["body"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(body.contains("가까운 지정 대피소로 갑니다"));
        assert!(
            !ids("화상 입었을 때 응급처치 알려줘").contains(&"kmcc-civil-alert-2026".to_owned())
        );
    }
    #[test]
    fn broad_labels_are_neutralized_without_rewriting_body_or_conditions() {
        let bodies = [
            (
                "항히스타민제 사용 금지",
                "항히스타민제는 응급 처치를 대신할 수 없습니다.",
            ),
            (
                "의료기기 운용 금지",
                "의료기기를 임의로 간헐 운전해서는 안 됩니다.",
            ),
            (
                "의료기기 운전 금지",
                "의료기기를 임의로 간헐 운전해서는 안 됩니다.",
            ),
        ];
        for (label, body) in bodies {
            let input = format!("* **{label}:** {body}");
            assert_eq!(
                neutralize_prohibition_labels(&input),
                format!("* **주의:** {body}")
            );
        }
        for input in [
            "* **본인에게 처방된 자동주사기가 있을 때:** 제품 지시를 따릅니다.",
            "* **영아 사용 금지:** 신생아와 6개월 미만에게 사용하지 않습니다.",
            "* **의료기기 운용 금지:**",
            "인공호흡을 할 수 없다면 압박만 계속합니다.",
            "* **약물 복용 금지:** 복용하지 않습니다.",
        ] {
            assert_eq!(neutralize_prohibition_labels(input), input);
        }
    }
    #[test]
    fn civilian_bundle_does_not_import_generic_military_procedures() {
        let docs = vec![
            json!({"id":"army-atp-3-50-21:quick"}),
            json!({"id":"nhs-cuts"}),
        ];
        let bundles = vec![json!({"id":"nfa-adult-cpr","mandatory":true})];
        assert_eq!(
            merge(docs.clone(), bundles.clone(), "성인 심폐소생술").len(),
            2
        );
        assert_eq!(merge(docs.clone(), bundles, "군인 교범과 비교").len(), 3);
        assert_eq!(merge(docs, vec![], "야생 생존 매듭").len(), 2);
    }
    #[test]
    fn edited_source_or_quote_cannot_reuse_review() {
        let body = "## 조건\n성인에게만 적용합니다.";
        let row = json!({"review":"source-checked","source_body_sha256":digest(body),"match_all_groups":[["성인"]],"sections":[{"body_line":1,"quote":body,"sha256":digest(body)}]});
        let doc = json!({"body":body});
        assert!(valid(&row, &doc));
        assert!(!valid(
            &row,
            &json!({"body":"## 조건\n소아에게 적용합니다."})
        ));
        let mut damaged = row.clone();
        damaged["sections"][0]["body_line"] = json!(9);
        assert!(!valid(&damaged, &doc));
        let mut damaged = row;
        damaged["sections"][0]["quote"] = json!("성인");
        assert!(!valid(&damaged, &doc));
    }
}
