// SPDX-License-Identifier: AGPL-3.0-only
//! Runtime contracts. Passing these checks is not a semantic safety certification.
use serde_json::{json, Value};
use std::{
    collections::HashSet,
    time::{Duration, Instant},
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    Cancelled,
    Deadline,
    Budget,
    Context,
    Length,
    Repetition,
    Protocol,
    Schema,
    NoEvidence,
    Freshness,
    UnsupportedClaim,
    Memory,
    Asset,
    Engine,
    Internal,
}
#[derive(Debug, Clone)]
pub struct Failure {
    pub code: Code,
    pub detail: String,
}
impl Failure {
    pub fn new(code: Code, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
    pub fn stops_engine(&self) -> bool {
        matches!(
            self.code,
            Code::Deadline | Code::Repetition | Code::Protocol | Code::Engine | Code::Memory
        )
    }
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?}] {}", self.code, self.detail)
    }
}
impl From<String> for Failure {
    fn from(s: String) -> Self {
        Self::new(Code::Internal, s)
    }
}
impl From<&str> for Failure {
    fn from(s: &str) -> Self {
        Self::new(
            if s == "Cancelled" {
                Code::Cancelled
            } else {
                Code::Internal
            },
            s,
        )
    }
}
pub type Result<T> = std::result::Result<T, Failure>;
pub fn fail(code: Code, detail: impl Into<String>) -> Failure {
    Failure::new(code, detail)
}
pub const CONTEXT: usize = 4096;
// Output completeness is determined by engine EOS/limit, not a display-length guess.
// Keep a generous resource ceiling; a complete checklist may legitimately exceed 1200 chars.
pub const ANSWER_CHARS: usize = 16_000;
pub const ANSWER_THINKING_TOKENS: usize = 512;
pub struct Budget {
    pub start: Instant,
    pub calls: usize,
    pub reserved: usize,
    pub stage: &'static str,
}
impl Budget {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            calls: 0,
            reserved: 0,
            stage: "query",
        }
    }
    pub fn remaining(&self) -> Result<Duration> {
        Duration::from_secs(240)
            .checked_sub(self.start.elapsed())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| {
                fail(
                    Code::Deadline,
                    "질문 전체 처리 시간 240초를 초과했습니다. 모델을 종료합니다.",
                )
            })
    }
    pub fn reserve(&mut self, n: usize) -> Result<()> {
        self.remaining()?;
        if self.calls >= 6 || self.reserved + n > 4000 {
            return Err(fail(
                Code::Budget,
                "질문당 추론 6회/출력 예약 4000토큰 한도를 초과했습니다.",
            ));
        }
        self.calls += 1;
        self.reserved += n;
        Ok(())
    }
}
pub fn validate(v: &Value, schema: &Value) -> Result<()> {
    let bad = || {
        fail(
            Code::Schema,
            "모델 출력의 타입·필드·범위가 요청 형식과 다릅니다.",
        )
    };
    if let Some(items) = schema["enum"].as_array() {
        if !items.contains(v) {
            return Err(bad());
        }
    }
    match schema["type"].as_str() {
        Some("object") => {
            let obj = v.as_object().ok_or_else(bad)?;
            if let Some(keys) = schema["required"].as_array() {
                for k in keys {
                    if !obj.contains_key(k.as_str().ok_or_else(bad)?) {
                        return Err(bad());
                    }
                }
            }
            for (k, value) in obj {
                if let Some(s) = schema["properties"].get(k) {
                    validate(value, s)?;
                } else if schema["additionalProperties"] == false {
                    return Err(bad());
                }
            }
        }
        Some("array") => {
            let a = v.as_array().ok_or_else(bad)?;
            if a.len() < schema["minItems"].as_u64().unwrap_or(0) as usize
                || a.len() > schema["maxItems"].as_u64().unwrap_or(u64::MAX) as usize
            {
                return Err(bad());
            }
            let mut seen = HashSet::new();
            for item in a {
                validate(item, &schema["items"])?;
                if !seen.insert(item.to_string()) {
                    return Err(fail(Code::Schema, "중복 항목을 반환했습니다."));
                }
            }
        }
        Some("string") => {
            let s = v.as_str().ok_or_else(bad)?;
            if s.trim().is_empty()
                || s.chars().count() < schema["minLength"].as_u64().unwrap_or(0) as usize
                || s.chars().count() > schema["maxLength"].as_u64().unwrap_or(u64::MAX) as usize
            {
                return Err(bad());
            }
        }
        Some("integer") => {
            if !v.is_i64() && !v.is_u64() {
                return Err(bad());
            }
        }
        _ => {}
    }
    Ok(())
}
pub fn repetitive(s: &str) -> bool {
    let c: Vec<_> = s.chars().collect();
    // A long periodic tail is stronger evidence than a repeated short safety word.
    for width in 8..=80 {
        if c.len() >= width * 4 {
            let end = c.len();
            let pattern = &c[end - width..];
            if pattern.iter().any(|c| c.is_alphanumeric())
                && (2..=4).all(|i| &c[end - width * i..end - width * (i - 1)] == pattern)
            {
                return true;
            }
        }
    }
    s.ends_with(&" ".repeat(160)) || s.ends_with(&"\n".repeat(80))
}
pub fn completion(v: &Value) -> Result<bool> {
    if v["truncated"] == true {
        return Err(fail(Code::Context, "엔진이 입력 문맥 잘림을 보고했습니다."));
    }
    if v["stopped_limit"] == true || v["stop_type"] == "limit" {
        return Ok(false);
    }
    if v["stop_type"] != "eos" || v["stop"] == false {
        return Err(fail(Code::Protocol, "정상 EOS 종료가 확인되지 않았습니다."));
    }
    let text = v["content"]
        .as_str()
        .ok_or_else(|| fail(Code::Protocol, "응답 본문이 없습니다."))?;
    if repetitive(text) {
        return Err(fail(Code::Repetition, "반복 생성을 감지했습니다."));
    }
    Ok(true)
}
pub fn numbers(s: &str) -> HashSet<String> {
    let content = s
        .lines()
        .map(|line| {
            let line = line.trim_start();
            let marker_line = line.trim_start_matches('#').trim_start();
            let marker_line = marker_line.strip_prefix("**").unwrap_or(marker_line);
            if let Some((prefix, rest)) = marker_line.split_once(' ') {
                let prefix = prefix.trim_end_matches('*');
                let digits = prefix.trim_end_matches(['.', ')']);
                if digits.len() < prefix.len()
                    && !digits.is_empty()
                    && digits.chars().all(|c| c.is_ascii_digit())
                {
                    return rest;
                }
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n");
    content
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .filter(|s| s.chars().any(|c| c.is_ascii_digit()))
        .map(|s| s.trim_matches('.').to_string())
        .collect()
}
pub fn thinking_answer(raw: &str) -> Result<String> {
    let body = raw
        .trim_start()
        .strip_prefix("<|channel>thought")
        .ok_or_else(|| fail(Code::Protocol, "사고/최종 답변 채널을 확인할 수 없습니다."))?;
    let (_, answer) = body
        .split_once("<channel|>")
        .ok_or_else(|| fail(Code::Protocol, "최종 답변 없이 사고 채널이 끝났습니다."))?;
    let answer = answer.trim();
    if answer.is_empty() || answer.contains("<channel|>") || answer.contains("<|channel>") {
        return Err(fail(Code::Protocol, "잘못된 최종 답변 채널입니다."));
    }
    Ok(answer.to_owned())
}
pub fn answer_check(text: &str, evidence: &str) -> Result<()> {
    if text
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(fail(
            Code::Protocol,
            "답변에 터미널 제어 문자가 포함되어 있습니다.",
        ));
    }
    if text.chars().count() >= ANSWER_CHARS {
        return Err(fail(
            Code::Length,
            "답변이 표시 자원 상한을 초과했습니다. 원문을 확인하세요.",
        ));
    }
    if repetitive(text) {
        return Err(fail(Code::Repetition, "답변이 같은 내용을 반복합니다."));
    }
    if text.contains("<|") || text.contains("[INST]") || text.contains("source_ids") {
        return Err(fail(
            Code::Protocol,
            "답변에 내부 역할 토큰이 노출됐습니다.",
        ));
    }
    // Conservative tripwire, not validation of units, negation, or semantic support.
    if !numbers(text).is_subset(&numbers(evidence)) {
        return Err(fail(
            Code::UnsupportedClaim,
            "답변에 선택한 원문에 없는 숫자가 있습니다. 원문을 확인하세요.",
        ));
    }
    // Conservative lexical tripwire for newly invented exclusivity. This does
    // not establish semantic entailment (including the converse of a condition).
    for phrase in ["경우에만", "때에만", "상황에서만", "무조건", "예외 없이"] {
        if text.contains(phrase) && !evidence.contains(phrase) {
            return Err(fail(
                Code::UnsupportedClaim,
                "답변이 원문에 없는 한정·단정 표현을 추가했습니다. 원문의 적용 조건을 확인하세요.",
            ));
        }
    }
    if copied_prohibition_lost(text, evidence) {
        return Err(fail(
            Code::UnsupportedClaim,
            "원문 금지 문장과 거의 같은 답변에서 부정 표현이 빠졌습니다. 원문을 확인하세요.",
        ));
    }
    if energy_is_called_duration(text) {
        return Err(fail(
            Code::UnsupportedClaim,
            "에너지 값을 시간 단위로 설명했습니다. 에너지와 사용 시간을 구분해야 합니다.",
        ));
    }
    Ok(())
}

// Catch the explicit dimensional error "에너지는 1.46일입니다". This is
// deliberately narrow, not a calculator or a general dimensional proof.
fn energy_is_called_duration(text: &str) -> bool {
    let compact: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '*')
        .collect();
    for marker in [
        "에너지는",
        "에너지값은",
        "하루부하는",
        "하루사용량은",
        "하루소비량은",
    ] {
        for (_, tail) in compact
            .match_indices(marker)
            .map(|(i, _)| (i, &compact[i + marker.len()..]))
        {
            let mut value = tail;
            for _ in 0..3 {
                if let Some(s) = ["계산상", "대략", "약"]
                    .into_iter()
                    .find_map(|p| value.strip_prefix(p))
                {
                    value = s;
                } else {
                    break;
                }
            }
            let end = value
                .find(|c: char| !c.is_ascii_digit() && c != '.')
                .unwrap_or(value.len());
            if end == 0 {
                continue;
            }
            let unit = &value[end..];
            for name in ["시간", "분", "일"] {
                if let Some(rest) = unit.strip_prefix(name) {
                    if ["입니다", "이다", "이며", "이고", "이므로"]
                        .iter()
                        .any(|p| rest.starts_with(p))
                    {
                        return true;
                    }
                }
            }
        }
    }
    false
}

// Detect near-verbatim copying with a dropped prohibition, not arbitrary NLI.
// Two-sided overlap avoids treating a short positive fragment as a reversal of
// an unrelated long sentence. Ambiguous cases remain subject to source review.
fn copied_prohibition_lost(answer: &str, evidence: &str) -> bool {
    fn sentences(s: &str) -> Vec<String> {
        s.split(['.', '\n', '!', '?', ':', '：'])
            .map(|s| {
                s.chars()
                    .filter(|c| c.is_alphanumeric())
                    .flat_map(char::to_lowercase)
                    .collect::<String>()
                    .replace("의료장치", "의료기기")
            })
            .filter(|s| s.chars().count() >= 16)
            .collect()
    }
    fn negative(s: &str) -> bool {
        [
            "지않",
            "지못",
            "지말",
            "지마",
            "하면안",
            "해서는안",
            "안됩니다",
            "금지",
            "아닙니다",
            "아니다",
        ]
        .iter()
        .any(|w| s.contains(w))
    }
    fn grams(s: &str) -> HashSet<String> {
        s.chars()
            .collect::<Vec<_>>()
            .windows(4)
            .map(|w| w.iter().collect())
            .collect()
    }
    let prohibitions: Vec<_> = sentences(evidence)
        .iter()
        .filter(|s| negative(s))
        .map(|s| grams(s))
        .collect();
    for sentence in sentences(answer).iter().filter(|s| !negative(s)) {
        let words = grams(sentence);
        for source in &prohibitions {
            let overlap = words.intersection(source).count();
            if overlap * 100 >= words.len() * 70 && overlap * 100 >= source.len() * 55 {
                return true;
            }
        }
    }
    false
}
pub fn fallback_queries(q: &str) -> Value {
    let clean: String = q
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect();
    let terms: Vec<_> = clean
        .split_whitespace()
        .filter(|s| s.chars().count() >= 2)
        .take(3)
        .map(|s| s.chars().take(80).collect::<String>())
        .collect();
    json!({"queries":terms})
}
pub fn needs_live_status(q: &str) -> bool {
    let time = ["지금", "현재", "오늘", "내일", "실시간"]
        .iter()
        .any(|w| q.contains(w));
    let status = [
        "운영", "개방", "열려", "통행", "경보", "재고", "수질", "영업",
    ]
    .iter()
    .any(|w| q.contains(w));
    time && status
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "audit lexical condition guards on saved local responses; no model"]
    fn audit_saved_condition_guards() {
        use std::fs;
        let root = crate::project_root();
        let base = root.join("dev/poc/model-evaluation/runs");
        let mut rows = vec![];
        for run in [
            "2026-11-17-gemma-full-65",
            "2026-11-18-omission-fix",
            "2026-11-18-omission-fix/final",
        ] {
            let dir = base.join(run).join("trace");
            for case in fs::read_dir(dir)
                .unwrap()
                .filter_map(std::result::Result::ok)
            {
                let Some(path) = fs::read_dir(case.path())
                    .unwrap()
                    .filter_map(std::result::Result::ok)
                    .map(|e| e.path())
                    .find(|p| p.to_string_lossy().ends_with("-answer.json"))
                else {
                    continue;
                };
                let record: serde_json::Value =
                    serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
                let Some(answer) = record["result"].as_str() else {
                    continue;
                };
                let evidence = record["user"]
                    .as_str()
                    .unwrap()
                    .split("\n\n사용자 질문:")
                    .next()
                    .unwrap();
                let exclusive: Vec<_> = ["경우에만", "때에만", "상황에서만", "무조건", "예외 없이"]
                    .into_iter()
                    .filter(|s| answer.contains(s) && !evidence.contains(s))
                    .collect();
                rows.push(serde_json::json!({"run":run,"id":case.file_name().to_string_lossy(),
                    "negation_loss":copied_prohibition_lost(answer,evidence),"new_exclusivity":exclusive,
                    "energy_unit_error":energy_is_called_duration(answer)}));
            }
        }
        rows.sort_by_key(|r| format!("{}{}", r["run"], r["id"]));
        assert_eq!(rows.len(), 80);
        assert!(rows
            .iter()
            .any(|r| r["id"] == "KO-042" && r["negation_loss"] == true));
        fs::write(
            base.join("2026-11-18-omission-fix/condition-guard-audit.json"),
            serde_json::to_string_pretty(&rows).unwrap(),
        )
        .unwrap();
    }
    #[test]
    fn stop_reasons_and_truncation() {
        for reason in ["none", "word", "unknown", ""] {
            assert_eq!(
                completion(&json!({"stop_type":reason,"content":"{}"}))
                    .unwrap_err()
                    .code,
                Code::Protocol
            );
        }
        assert!(!completion(&json!({"stop_type":"limit"})).unwrap());
        assert_eq!(
            completion(&json!({"stop_type":"eos","content":"{}","truncated":true}))
                .unwrap_err()
                .code,
            Code::Context
        );
        assert!(completion(&json!({"stop_type":"eos","content":"{}"})).unwrap());
    }
    #[test]
    fn strict_schema_checks() {
        let s = json!({"type":"object","required":["ids"],"additionalProperties":false,"properties":{"ids":{"type":"array","minItems":1,"maxItems":2,"items":{"type":"string","enum":["a","b"]}}}});
        for v in [
            json!({}),
            json!({"ids":["x"]}),
            json!({"ids":[1]}),
            json!({"ids":["a","a"]}),
            json!({"ids":["a"],"evil":1}),
            json!({"ids":[]}),
        ] {
            assert!(validate(&v, &s).is_err());
        }
        assert!(validate(&json!({"ids":["a","b"]}), &s).is_ok());
    }
    #[test]
    fn bounded_budget_and_deadline() {
        let mut b = Budget::new();
        for _ in 0..6 {
            b.reserve(100).unwrap();
        }
        assert_eq!(b.reserve(1).unwrap_err().code, Code::Budget);
        let mut b = Budget::new();
        assert_eq!(b.reserve(4001).unwrap_err().code, Code::Budget);
        b.start = Instant::now() - Duration::from_secs(241);
        assert_eq!(b.remaining().unwrap_err().code, Code::Deadline);
    }
    #[test]
    fn repetition_and_normal_lists() {
        assert!(repetitive(&"같은 문장 반복입니다. ".repeat(8)));
        assert!(!repetitive("물 준비\n약 준비\n연락처 준비"));
        assert!(answer_check("- 물\n- 개인 약\n- 종이 연락처", "").is_ok());
        assert!(answer_check("확인된 물품 목록\n여러 조건을 포함한 설명", "").is_ok());
    }
    #[test]
    fn numeric_tripwire_is_not_semantic_verification() {
        assert!(answer_check("3일", "3일 이상").is_ok());
        assert!(answer_check("1. 물 준비\n2) 약 준비", "").is_ok());
        assert!(answer_check("**1. 판단**\n## 2. 해야 할 일\n**3.** 주의", "").is_ok());
        assert_eq!(
            numbers("**20 mg**\n## 3.5 mg\n**2.** 물"),
            ["20".to_owned(), "3.5".to_owned()].into_iter().collect()
        );
        assert_eq!(
            answer_check("30일", "3일").unwrap_err().code,
            Code::UnsupportedClaim
        );
        assert!(
            answer_check("하지 않습니다.", "합니다.").is_ok(),
            "semantic negation is deliberately not claimed to be checked"
        );
    }
    #[test]
    fn medical_device_synonym_cannot_hide_copied_negation_loss() {
        let source = "의료장치 배터리가 모두 소진될 때까지 기다린 뒤 이동을 시작하지 않습니다.";
        assert_eq!(
            answer_check(
                "의료기기 배터리가 모두 소진될 때까지 기다린 뒤 이동을 시작해야 합니다.",
                source
            )
            .unwrap_err()
            .code,
            Code::UnsupportedClaim
        );
        assert_eq!(answer_check("* **이동 시작 기준:** 의료장치 배터리가 모두 소진될 때까지 기다린 뒤 이동을 시작해야 합니다.",source).unwrap_err().code,Code::UnsupportedClaim);
        assert!(answer_check(
            "의료기기 배터리가 모두 소진될 때까지 기다린 뒤 이동을 시작하지 않습니다.",
            source
        )
        .is_ok());
        assert!(answer_check("의료기기 사용법은 담당자에게 확인합니다.", source).is_ok());
    }
    #[test]
    fn invented_exclusivity_is_not_accepted_as_a_condition() {
        assert_eq!(
            answer_check(
                "망이 모두 끊긴 경우에만 라디오를 사용합니다.",
                "망이 모두 끊기면 라디오를 사용합니다."
            )
            .unwrap_err()
            .code,
            Code::UnsupportedClaim
        );
        assert!(answer_check(
            "망이 모두 끊기면 라디오를 사용합니다.",
            "망이 모두 끊기면 라디오를 사용합니다."
        )
        .is_ok());
        assert!(answer_check(
            "조건을 충족한 경우에만 사용합니다.",
            "조건을 충족한 경우에만 사용합니다."
        )
        .is_ok());
    }
    #[test]
    fn near_copied_prohibition_cannot_drop_negation() {
        let source = "의료장치 배터리가 모두 소진될 때까지 기다린 뒤 이동을 시작하지 않습니다.";
        assert_eq!(
            answer_check(
                "의료장치 배터리가 모두 소진될 때까지 기다린 뒤 이동을 시작해야 합니다.",
                source
            )
            .unwrap_err()
            .code,
            Code::UnsupportedClaim
        );
        assert!(answer_check(source, source).is_ok());
        assert!(answer_check(
            "의료장치 배터리가 모두 소진될 때까지 기다린 뒤 이동을 시작하지 마십시오.",
            source
        )
        .is_ok());
        assert!(answer_check(
            "의료장치 배터리가 모두 소진될 때까지 기다린 뒤 이동을 시작하지 말아야 합니다.",
            source
        )
        .is_ok());
        assert!(answer_check("이동 방법을 미리 기록하고 지원자와 확인합니다.", source).is_ok());
    }
    #[test]
    fn energy_value_cannot_be_labelled_as_duration() {
        assert_eq!(
            answer_check(
                "하루 부하 에너지는 약1.46일입니다.",
                "82Wh/일 부하는 계산상 약1.46일입니다."
            )
            .unwrap_err()
            .code,
            Code::UnsupportedClaim
        );
        assert_eq!(
            answer_check(
                "계산상 하루 부하는 약1.46일입니다.",
                "82Wh/일 부하는 계산상 약1.46일입니다."
            )
            .unwrap_err()
            .code,
            Code::UnsupportedClaim
        );
        assert!(answer_check(
            "하루 부하 에너지는82Wh입니다. 사용 시간은 계산상1.46일입니다.",
            "82Wh 1.46일"
        )
        .is_ok());
        assert!(answer_check("에너지는1일 기준82Wh입니다.", "1일 82Wh").is_ok());
    }
    #[test]
    fn cap_and_role_tokens() {
        assert_eq!(
            answer_check("\u{1b}[2J", "").unwrap_err().code,
            Code::Protocol
        );
        assert_eq!(
            answer_check(&"가".repeat(ANSWER_CHARS), "")
                .unwrap_err()
                .code,
            Code::Length
        );
        assert_eq!(
            answer_check("<|assistant|>", "").unwrap_err().code,
            Code::Protocol
        );
        assert!(answer_check(
            &(0..1400)
                .map(|i| char::from_u32(0xac00 + i).unwrap())
                .collect::<String>(),
            ""
        )
        .is_ok());
    }
    #[test]
    fn offline_status_boundary() {
        for q in [
            "내일 서울 급수시설 운영 중인지",
            "오늘 도로 통행 가능한지",
            "현재 병원 영업 중?",
        ] {
            assert!(needs_live_status(q));
        }
        for q in [
            "지금 냉장고 문을 어떻게 해야 해?",
            "급수시설 운영 상태는 어디서 확인하나?",
        ] {
            assert!(!needs_live_status(q));
        }
    }
}
#[test]
fn thought_is_never_displayed_or_accepted_without_final_channel() {
    assert_eq!(
        thinking_answer("<|channel>thought\ninternal\n<channel|>\n답변").unwrap(),
        "답변"
    );
    for s in [
        "internal only",
        "<|channel>thought\ninternal",
        "<|channel>thought\n<channel|>",
        "<|channel>thought\n<channel|>answer<channel|>late",
    ] {
        assert!(thinking_answer(s).is_err());
    }
}
