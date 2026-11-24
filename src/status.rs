// SPDX-License-Identifier: AGPL-3.0-only
//! Single generated source for project numbers. dev/STATUS.md (development only) is produced from the
//! same loaders the app uses, and a test fails when the file drifts.
use super::*;
use serde_json::Value;
use std::collections::BTreeMap;
type Error = Box<dyn std::error::Error>;

fn license_of(record: &Value) -> String {
    record["rights"]["license"]
        .as_str()
        .or_else(|| record["license"].as_str())
        .unwrap_or("(라이선스 표기 없음)")
        .to_string()
}

pub(super) fn report(root: &Path) -> Result<String, Error> {
    let entries: Vec<String> = serde_json::from_slice(&fs::read(root.join("approved.json"))?)?;
    let (approved, _) = library::load_approved(root)?;
    let mut licenses: BTreeMap<String, usize> = BTreeMap::new();
    for entry in &entries {
        let r: Value = serde_json::from_slice(&fs::read(root.join(entry).join("record.json"))?)?;
        *licenses.entry(license_of(&r)).or_default() += 1;
    }
    let public_manuals = library::load_public_manuals(root)?.len();
    let manuals = if library::default_includes_manuals(root)? {
        library::load_local_manuals(root)?.len()
    } else {
        0
    };
    let local_total = library::load_default_local(root)?.len();
    let shelters = shelters::load(root)?.len();
    let medical = medical_facilities::load(root)?.len();
    let water = water_facilities::load(root)?.len();
    let (all, _) = load(root)?;

    let tsv = fs::read_to_string(root.join("coverage.tsv"))?;
    let mut lines = tsv.lines();
    let header: Vec<&str> = lines
        .next()
        .ok_or("Empty coverage.tsv")?
        .split('\t')
        .collect();
    let col = |name: &str| header.iter().position(|h| *h == name);
    let state_col = col("요구사항 상태").ok_or("coverage.tsv: missing 요구사항 상태")?;
    let public_col = col("공개승인 근거수").ok_or("coverage.tsv: missing 공개승인 근거수")?;
    let mut states: BTreeMap<String, usize> = BTreeMap::new();
    let (mut reqs, mut public_zero, mut public_one) = (0, 0, 0);
    for line in lines.filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        reqs += 1;
        *states
            .entry(f.get(state_col).unwrap_or(&"").to_string())
            .or_default() += 1;
        match f.get(public_col).and_then(|v| v.trim().parse::<u32>().ok()) {
            Some(0) => public_zero += 1,
            Some(1) => public_one += 1,
            _ => {}
        }
    }

    let mut s = String::new();
    s.push_str("# 현재 상태\n\n");
    s.push_str("> 이 파일은 자동 생성된다. 직접 고치지 말고 `dev/status.sh`로 다시 만든다. ");
    s.push_str("앱이 실제로 불러오는 자료를 같은 로더로 센 값이며, `dev/check.sh`가 이 파일과 현재 자료의 일치를 검사한다.\n\n");
    s.push_str("## 기본 실행에 표시되는 자료\n\n");
    s.push_str("| 구분 | 수 | 공개 재배포 |\n|---|---:|---|\n");
    s.push_str(&format!(
        "| 공개 승인 자료 (`approved.json`) | {}개 자료 · {}개 문서 | 가능 — 재배포·번역·편집 권리를 모두 확인한 자료만 등록 |\n",
        entries.len(),
        approved.len()
    ));
    s.push_str(&format!(
        "| 공개 승인 교범 항목 (`local-manuals.json`의 public_approved) | {public_manuals} | 가능 — 미국 정부 저작물(퍼블릭 도메인)·배포 무제한 표기 확인 |\n"
    ));
    s.push_str(&format!(
        "| 로컬 전용 교범 항목 | {manuals} | 불가 — 개인 검증용 |\n"
    ));
    s.push_str(&format!(
        "| 로컬 전용 문서(국제기구·지자체 등) | {} | 불가 — 수정 금지·사전 서면 허가 필요 또는 이용 조건 확인 불가 |\n",
        local_total - manuals
    ));
    s.push_str(&format!(
        "| 공식 시설 (대피 {shelters} · 급수 {water} · 의료 {medical}) | {} | 가능 — 공공데이터포털 \"이용허락범위 제한 없음\" |\n",
        shelters + water + medical
    ));
    s.push_str(&format!("| **합계** | **{}** | |\n\n", all.len()));
    s.push_str(&format!(
        "공개판에 넣을 수 있는 본문은 공개 승인 {}개 문서, 공개 승인 교범 {public_manuals}항목과 공식 시설 데이터다. 로컬 전용 {}개는 공개판에서 빠진다.\n\n",
        approved.len(),
        local_total
    ));
    s.push_str("## 공개 승인 자료의 라이선스\n\n| 라이선스 | 자료 수 |\n|---|---:|\n");
    for (license, n) in &licenses {
        s.push_str(&format!("| {} | {n} |\n", license.replace('|', "/")));
    }
    s.push_str("\n## 요구사항 커버리지 (`coverage.tsv`)\n\n| 상태 | 요구사항 수 |\n|---|---:|\n");
    for (state, n) in &states {
        s.push_str(&format!("| {state} | {n} |\n"));
    }
    s.push_str(&format!(
        "| **전체** | **{reqs}** |\n\n공개 승인 근거가 0건인 요구사항 {public_zero}개, 1건뿐인 요구사항 {public_one}개. ",
    ));
    s.push_str("근거가 없거나 하나뿐이면 공개판에서 그 주제가 비거나 얇아진다.\n\n");
    s.push_str("## AI\n\n");
    s.push_str("- 모델: Gemma 4 E2B IT Q4_0 + llama.cpp (2026-11-18 확정)\n");
    s.push_str("- 모델 평가: 2026-11-23 동결. 새 모델 비교·전수 재평가는 하지 않는다.\n");
    s.push_str("- 안전 답변 품질: 승인 보류. AI 답변은 실험 기능이며 원문이 우선한다.\n");
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_file_is_current() {
        let base = crate::project_root();
        let root = base.join("dev/content-library");
        if !root.join("approved.json").exists() {
            return;
        }
        let expected = report(&root).unwrap();
        let actual = fs::read_to_string(base.join("dev/STATUS.md")).unwrap_or_default();
        assert!(
            actual == expected,
            "dev/STATUS.md is stale: run dev/status.sh"
        );
    }
}
