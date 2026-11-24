// SPDX-License-Identifier: AGPL-3.0-only
//! Official water facilities; preserve missing coordinates and unknown drinking-water status.
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
        return Err("Water facility integrity check failed".into());
    }
    Ok(p)
}
pub(super) fn load(root: &Path) -> Result<Vec<Doc>, Box<dyn std::error::Error>> {
    if !root.join("water-facilities.json").exists() {
        return Ok(vec![]);
    }
    let m: Value = serde_json::from_slice(&fs::read(root.join("water-facilities.json"))?)?;
    if m["approved"] != true || m["license"] != "이용허락범위 제한 없음" {
        return Err("Water facilities not approved".into());
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
        return Err("Water facility count mismatch".into());
    }
    let source = m["source_url"].as_str().ok_or("source URL")?;
    let date = m["collected"].as_str().ok_or("collection date")?;
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
        let (id, name, address) = (
            get("id")?,
            get("name")?,
            if !r["road_address"].as_str().unwrap_or("").is_empty() {
                get("road_address")?
            } else {
                get("address")?
            },
        );
        if !ids.insert(id.to_string())
            || r["status"] != "사용중"
            || r["status_code"] != "01"
            || ["released", "cancelled", "closed"]
                .iter()
                .any(|k| r[k] != "")
            || r["source_row"].as_u64().unwrap_or(0) < 2
        {
            return Err("Invalid water facility record".into());
        }
        let location: Option<shelters::Location> = serde_json::from_value(r["location"].clone())?;
        if let Some(l) = &location {
            if !(33.0..=39.0).contains(&l.lat) || !(124.0..=132.0).contains(&l.lon) {
                return Err("Invalid water coordinate".into());
            }
        }
        let field = |key: &str| {
            r[key]
                .as_str()
                .filter(|x| !x.is_empty())
                .unwrap_or("미기재")
        };
        let coordinate = if location.is_some() {
            "원본 EPSG:5174에서 변환한 지도 좌표 있음. 출입구·급수구 위치 보증 아님."
        } else {
            "원자료 좌표 누락/이상: 주소로 확인. 지도 좌표 추정 안 함."
        };
        let card = facility_view::render(facility_view::Fields {
            name,
            road: r["road_address"].as_str().unwrap_or(""),
            address: r["address"].as_str().unwrap_or(""),
            address_is_land_lot: true,
            updated: r["updated"].as_str().unwrap_or(""),
            modified: r["modified"].as_str().unwrap_or(""),
            status: "사용중",
        });
        let body=format!("{card}\n비상시설위치: {}\n시설 구분: {}\n시설명·건물명: {}\n수집일: {date}\n\n{coordinate}\n\n음용/생활용 구분·수질검사·현재 급수·개방시간: 원자료 미제공. 시설명으로 음용 가능 여부를 추정하지 마세요. 현장 수질·용도 표시와 관리자 안내를 확인하세요. ‘사용중’은 행정 등록 상태이며 지금 물이 나오거나 출입할 수 있다는 보장이 아닙니다.\n\nOSM 일반 POI와 구분되는 행정안전부 공식 민방위급수시설 목록입니다. 도로 통행과 시설 손상은 실시간 반영되지 않습니다.",field("facility_position"),field("kind"),field("building"));
        docs.push(Doc{location,id:format!("shelter:water:{id}"),title:format!("[비상급수] {name} · {address}"),categories:vec!["대피시설".into(),"식수".into()],file:format!("{dir}/facilities.json#{id}"),body:body.clone(),display_body:body,record:format!("행정안전부 민방위급수시설 / 관리번호{id}\nCSV 원본 행:{}\n이용허락범위 제한 없음\n{source}\nCP949 원본 보존, 사용중·미해제 필터, 공백 정규화 및 EPSG:5174→4326 좌표 변환.",r["source_row"]),source:source.into(),original_uri:format!("file://{}",utf8_percent_encode(&original.to_string_lossy(),URI_ESC)),original_path:original.clone(),spread:0});
    }
    Ok(docs)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_water_rows_render_and_search() {
        let root = crate::project_root().join("dev/content-library");
        let docs = load(&root).unwrap();
        assert_eq!(docs.len(), 5787);
        assert_eq!(docs.iter().filter(|d| d.location.is_some()).count(), 5535);
        let rows = search(&docs, "", "전체");
        assert_eq!(rows.len(), docs.len());
        let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(100, 30)).unwrap();
        for (idx, d) in docs.iter().enumerate() {
            assert!(d.body.contains("원자료 미제공"));
            assert!(d.title.starts_with("[비상급수]"));
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
        for q in ["자하문호텔", "부산광역시", "제주특별자치도"] {
            assert!(!search(&docs, q, "식수").is_empty());
        }
        assert!(checked(&root, "water-facilities.json", &Value::String("bad".into())).is_err());
    }
}
