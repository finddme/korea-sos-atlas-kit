// SPDX-License-Identifier: AGPL-3.0-only
//! Official static emergency-center addresses, with no inferred live capacity or coordinates.
use super::*;
use serde_json::Value;
use std::collections::HashSet;
fn checked(
    root: &Path,
    file: &str,
    expected: &Value,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let p = root.join(file).canonicalize()?;
    if !p.starts_with(root.canonicalize()?)
        || Some(hash(&fs::read(&p)?).as_str()) != expected.as_str()
    {
        return Err("Medical facility integrity check failed".into());
    }
    Ok(p)
}
pub(super) fn load(root: &Path) -> Result<Vec<Doc>, Box<dyn std::error::Error>> {
    if !root.join("medical-facilities.json").exists() {
        return Ok(vec![]);
    }
    let m: Value = serde_json::from_slice(&fs::read(root.join("medical-facilities.json"))?)?;
    if m["approved"] != true || m["license"] != "이용허락범위 제한 없음" {
        return Err("Medical facilities not approved".into());
    }
    let dir = m["directory"].as_str().ok_or("missing directory")?;
    let original = checked(root, &format!("{dir}/original.csv"), &m["original_sha256"])?;
    let records = checked(root, &format!("{dir}/facilities.json"), &m["sha256"])?;
    let p = m["policy_evidence"].as_array().ok_or("missing policy")?;
    if p.is_empty() {
        return Err("missing policy".into());
    }
    for e in p {
        checked(root, e["file"].as_str().ok_or("policy path")?, &e["sha256"])?;
    }
    let rows: Vec<Value> = serde_json::from_slice(&fs::read(records)?)?;
    if Some(rows.len() as u64) != m["count"].as_u64() {
        return Err("Medical facility count mismatch".into());
    }
    let source = m["source_url"].as_str().ok_or("source URL")?;
    let date = m["source_date"].as_str().ok_or("source date")?;
    let mut ids = HashSet::new();
    let mut docs = vec![];
    for r in rows {
        let get = |key: &str| -> Result<&str, Box<dyn std::error::Error>> {
            let s = r[key].as_str().ok_or("missing field")?;
            if s.is_empty() || s.chars().any(char::is_control) {
                return Err("invalid facility text".into());
            }
            Ok(s)
        };
        let (id, name, address) = (get("id")?, get("name")?, get("address")?);
        if !ids.insert(id.to_string())
            || r["emergency_center"] != "Y"
            || r["deleted"] != "N"
            || r["source_row"].as_u64().unwrap_or(0) < 2
        {
            return Err("Invalid medical facility record".into());
        }
        let card = facility_view::render(facility_view::Fields {
            name,
            road: "",
            address,
            address_is_land_lot: false,
            updated: "",
            modified: "",
            status: "",
        });
        let body=format!("{card}\n자료 기준일: {date}\n수집일: 2026-11-15\n원자료 표시: 응급진료센터 운영 Y / 삭제 N\n\n행정안전부 NDMS 병원정보의 정적 주소입니다. 현재 진료·응급실 개방·병상·의료진·수용 가능 여부는 확인되지 않습니다. 전국 응급의료기관 전체 목록과 동일하지 않습니다.\n\n원자료에 위도·경도와 전화번호가 없어 추정하지 않았습니다. 지도에서 병원명과 주소를 함께 대조하고 현장 안내 또는119를 통해 진료 가능 여부를 확인하세요. 중증 환자 이송 판단은 의료·구급 담당자의 지시를 따릅니다.\n\nOSM 일반 의료시설 표시와 별도의 공식 자료입니다. 도로 통행·시설 손상은 지도에 실시간 반영되지 않습니다.");
        docs.push(Doc{location:None,id:format!("shelter:hospital:{id}"),title:format!("[응급의료 주소] {name} · {address}"),categories:vec!["대피시설".into(),"응급·건강".into()],file:format!("{dir}/facilities.json#{id}"),body:body.clone(),display_body:body,record:format!("행정안전부 병원정보 / 관리번호{id}\nCSV 원본 행:{}\n이용허락범위 제한 없음\n{source}\n비삭제·응급센터Y·주소확인 조건 필터, 주소 공백 정규화. 현재 운영 보증 아님.",r["source_row"]),source:source.into(),original_uri:format!("file://{}",utf8_percent_encode(&original.to_string_lossy(),URI_ESC)),original_path:original.clone(),spread:0});
    }
    Ok(docs)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_and_search() {
        let root = crate::project_root().join("dev/content-library");
        let docs = load(&root).unwrap();
        assert_eq!(docs.len(), 279);
        for d in &docs {
            assert!(d.location.is_none());
            assert!(d.body.contains("현재 진료"));
            assert!(!search(&docs, &d.title, "응급·건강").is_empty());
        }
        let rows = search(&docs, "", "전체");
        for idx in 0..docs.len() {
            for (w, h) in [(90, 24), (120, 32), (160, 50)] {
                let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
                let mut app = App {
                    opened: Some(idx),
                    selected: idx,
                    offset: usize::MAX,
                    ..Default::default()
                };
                terminal
                    .draw(|f| draw(f, &mut app, &docs, &rows, &Ai::default(), false))
                    .unwrap();
                assert!(app.offset < usize::MAX);
            }
        }
        assert!(checked(
            &root,
            "medical-facilities.json",
            &Value::String("wrong".into())
        )
        .is_err());
    }
}
