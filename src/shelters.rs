// SPDX-License-Identifier: AGPL-3.0-only
//! Hash-bound official facility records. Missing coordinates never become guessed locations.
use super::*;
use serde_json::Value;
use std::collections::HashSet;
pub(super) fn near(query: &str) -> Option<(f64, f64)> {
    let token = query.split_whitespace().find(|s| s.starts_with('@'))?;
    let (a, b) = token.trim_start_matches('@').split_once(',')?;
    let lat = a.parse().ok()?;
    let lon = b.parse().ok()?;
    if (33.0..=39.0).contains(&lat) && (124.0..=132.0).contains(&lon) {
        Some((lat, lon))
    } else {
        None
    }
}
/// Where "nearest" is measured from: `@lat,lon`, or `@place` resolved offline from the
/// facility records themselves (a facility name containing the text, otherwise addresses
/// containing it; the median of their coordinates). No geocoder, no network.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Origin {
    pub lat: f64,
    pub lon: f64,
    pub label: String,
}
thread_local! {
    static ORIGIN_CACHE: std::cell::RefCell<Option<(String, usize, Option<Origin>)>> = const { std::cell::RefCell::new(None) };
}
/// `@place words` (the whole query after `@`, unless it is `@lat,lon`).
pub(super) fn place_query(query: &str) -> Option<&str> {
    let q = query.trim();
    let place = q.strip_prefix('@')?.trim();
    (near(q).is_none() && place.chars().filter(|c| !c.is_whitespace()).count() >= 2)
        .then_some(place)
}
pub(super) fn origin(docs: &[Doc], query: &str) -> Option<Origin> {
    if let Some((lat, lon)) = near(query) {
        return Some(Origin {
            lat,
            lon,
            label: format!("좌표 {lat:.4}, {lon:.4}"),
        });
    }
    let place = place_query(query)?;
    if let Some(hit) = ORIGIN_CACHE.with(|c| {
        c.borrow()
            .as_ref()
            .filter(|(k, n, _)| k == place && *n == docs.len())
            .map(|(_, _, o)| o.clone())
    }) {
        return hit;
    }
    let found = resolve_place(docs, place);
    ORIGIN_CACHE.with(|c| *c.borrow_mut() = Some((place.to_owned(), docs.len(), found.clone())));
    found
}
/// Facility titles are "[kind] name · address".
fn name_and_address(title: &str) -> (&str, &str) {
    let rest = title.split_once("] ").map_or(title, |(_, r)| r);
    rest.split_once(" · ").unwrap_or((rest, ""))
}
/// "서울특별시 중구 태평로1가 31" -> "서울특별시 중구"
fn district(address: &str) -> String {
    address
        .split_whitespace()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ")
}
fn resolve_place(docs: &[Doc], place: &str) -> Option<Origin> {
    let words: Vec<String> = place
        .split_whitespace()
        .map(|w| w.case_fold().collect())
        .collect();
    let mut by_name = Vec::new();
    let mut by_address = Vec::new();
    for d in docs.iter().filter(|d| d.id.starts_with("shelter:")) {
        let Some(p) = d.location.as_ref().filter(|p| p.valid()) else {
            continue;
        };
        let (name, address) = name_and_address(&d.title);
        let n = name.case_fold().collect::<String>();
        let full = format!("{n} {}", address.case_fold().collect::<String>());
        if !words.iter().all(|w| full.contains(w.as_str())) {
            continue;
        }
        let hit = (p.lat, p.lon, name.to_owned(), district(address));
        if words.iter().any(|w| n.contains(w.as_str())) {
            by_name.push(hit)
        } else {
            by_address.push(hit)
        }
    }
    let (hits, kind) = if !by_name.is_empty() {
        (by_name, "이름")
    } else {
        (by_address, "주소")
    };
    if hits.is_empty() {
        return None;
    }
    // The same street or place name exists in several cities: use the district with the
    // most matches, never a midpoint between cities.
    let mut counts: Vec<(String, usize)> = Vec::new();
    for h in &hits {
        match counts.iter_mut().find(|(d, _)| *d == h.3) {
            Some(c) => c.1 += 1,
            None => counts.push((h.3.clone(), 1)),
        }
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    let main = counts[0].0.clone();
    let cluster: Vec<_> = hits.iter().filter(|h| h.3 == main).collect();
    let median = |mut v: Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    let lat = median(cluster.iter().map(|h| h.0).collect());
    let lon = median(cluster.iter().map(|h| h.1).collect());
    let mut label = if cluster.len() == 1 && kind == "이름" {
        format!("'{place}' → {} ({main})", cluster[0].2)
    } else {
        format!(
            "'{place}'이(가) {kind}에 있는 {main} 시설 {}곳의 중앙 (대략)",
            cluster.len()
        )
    };
    if counts.len() > 1 {
        let others: Vec<String> = counts[1..]
            .iter()
            .take(2)
            .map(|(d, n)| format!("{d} {n}"))
            .collect();
        label.push_str(&format!(
            " · 다른 지역: {} → 예: @{} {place}",
            others.join(", "),
            counts[1].0.split_whitespace().last().unwrap_or("")
        ));
    }
    Some(Origin { lat, lon, label })
}
pub(super) fn distance(origin: (f64, f64), p: &Location) -> f64 {
    let dlat = (p.lat - origin.0).to_radians();
    let dlon = (p.lon - origin.1).to_radians();
    let a = (dlat / 2.).sin().powi(2)
        + origin.0.to_radians().cos() * p.lat.to_radians().cos() * (dlon / 2.).sin().powi(2);
    6371.0088 * 2. * a.sqrt().min(1.).asin()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Location {
    pub lat: f64,
    pub lon: f64,
    pub name: String,
}
impl Location {
    pub fn valid(&self) -> bool {
        (33.0..=39.0).contains(&self.lat)
            && (124.0..=132.0).contains(&self.lon)
            && !self.name.is_empty()
            && !self.name.chars().any(char::is_control)
    }
}
#[derive(Deserialize)]
struct Facility {
    id: String,
    name: String,
    address: String,
    road_address: String,
    kind: String,
    facility_type: String,
    level: String,
    area_m2: String,
    capacity: String,
    designated: String,
    status: String,
    released: String,
    updated: String,
    modified: String,
    location: Option<Location>,
    source_row: usize,
}
fn checked(root: &Path, path: &str, sha: &Value) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let p = root.join(path).canonicalize()?;
    if !p.starts_with(root.canonicalize()?) || Some(hash(&fs::read(&p)?).as_str()) != sha.as_str() {
        return Err("Shelter file integrity check failed".into());
    }
    Ok(p)
}
pub(super) fn load(root: &Path) -> Result<Vec<Doc>, Box<dyn std::error::Error>> {
    if !root.join("shelters.json").exists() {
        return Ok(Vec::new());
    }
    let m: Value = serde_json::from_slice(&fs::read(root.join("shelters.json"))?)?;
    if m["approved"] != true || m["license"] != "이용허락범위 제한 없음" {
        return Err("Shelter data not approved".into());
    }
    let dir = m["directory"].as_str().ok_or("Missing shelter directory")?;
    let file = checked(
        root,
        &format!(
            "{dir}/{}",
            m["file"].as_str().ok_or("Missing shelter file")?
        ),
        &m["sha256"],
    )?;
    let original = checked(root, &format!("{dir}/original.csv"), &m["original_sha256"])?;
    let policies = m["policy_evidence"]
        .as_array()
        .ok_or("Missing shelter rights evidence")?;
    if policies.is_empty() {
        return Err("Missing shelter rights evidence".into());
    }
    for p in policies {
        checked(
            root,
            p["file"].as_str().ok_or("Missing policy file")?,
            &p["sha256"],
        )?;
    }
    let rows: Vec<Facility> = serde_json::from_slice(&fs::read(file)?)?;
    if m["count"].as_u64() != Some(rows.len() as u64) {
        return Err("Shelter count mismatch".into());
    }
    let collected = m["collected"].as_str().ok_or("Missing collection date")?;
    let mut ids = HashSet::new();
    let mut docs = Vec::new();
    for r in rows {
        if r.status != "사용중"
            || !r.released.is_empty()
            || r.id.is_empty()
            || r.name.is_empty()
            || !ids.insert(r.id.clone())
            || r.location.as_ref().is_some_and(|p| !p.valid())
        {
            return Err("Invalid approved shelter record".into());
        }
        let coordinates = r
            .location
            .as_ref()
            .map(|p| format!("{:.7}, {:.7}\nm: 지도 목적지로 선택", p.lat, p.lon))
            .unwrap_or("원자료에 좌표 없음 — 주소로 확인; 지도 연결 불가".into());
        let card = facility_view::render(facility_view::Fields {
            name: &r.name,
            road: &r.road_address,
            address: &r.address,
            address_is_land_lot: true,
            updated: &r.updated,
            modified: &r.modified,
            status: &r.status,
        });
        let body=format!("{card}\n유형: {} / {}\n시설 위치: {}\n수용인원(원자료): {}명 · 면적: {}㎡\n지정일: {}\n좌표(WGS84): {}\n\n수집일: {collected}\n공식 지정 목록이며 현재 개방·빈자리·손상 여부는 실시간 확인되지 않습니다. 방폭/화생방 성능을 인증하는 자료가 아닙니다. 지진·해일용 대피시설과 구분하세요. 지도 점은 공식 시설 좌표이며 출입구로 검증된 위치가 아닙니다.",r.kind,r.facility_type,r.level,r.capacity,r.area_m2,r.designated,coordinates);
        let title = format!("[민방위] {} · {}", r.name, r.address);
        docs.push(Doc{location:r.location,id:format!("shelter:{}",r.id),title,categories:vec!["대피시설".into()],file:format!("{dir}/facilities.json#{}",r.id),display_body:body.clone(),body,record:format!("행정안전부 / 전국민방위대피시설표준데이터\n관리번호: {}\n원본 CSV 레코드: {} (헤더 포함, 인용부호 내부 줄바꿈은 하나의 레코드)\n이용허락범위 제한 없음\n{}\n공백 정규화 및 사용중/미해제 필터. 좌표 추정 없음.",r.id,r.source_row,m["source_url"].as_str().unwrap_or("")),source:m["source_url"].as_str().unwrap_or("").into(),original_uri:format!("file://{}",utf8_percent_encode(&original.to_string_lossy(),URI_ESC)),original_path:original.clone(),spread:0});
    }
    Ok(docs)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_official_catalog() {
        let root = crate::project_root().join("dev/content-library");
        let docs = load(&root).unwrap();
        assert_eq!(docs.len(), 17241);
        assert_eq!(docs.iter().filter(|d| d.location.is_none()).count(), 8);
        assert!(!search(&docs, "종로 교보", "대피시설").is_empty());
        for d in &docs {
            assert!(d.body.contains("사용중"));
            assert!(d.body.contains("데이터 갱신시점"));
        }
        for idx in [0, docs.len() / 2, docs.len() - 1] {
            for (w, h) in [(90, 24), (120, 32)] {
                let mut t = Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
                let mut a = App {
                    opened: Some(idx),
                    cat: 6,
                    ..Default::default()
                };
                let rows = search(&docs, "", "대피시설");
                t.draw(|f| draw(f, &mut a, &docs, &rows, &Ai::default(), false))
                    .unwrap();
            }
        }
    }
    #[test]
    fn location_bounds() {
        for (lat, lon) in [(0., 0.), (f64::NAN, 127.), (37., f64::INFINITY)] {
            assert!(!Location {
                lat,
                lon,
                name: "test".into()
            }
            .valid());
        }
    }
    #[test]
    fn nearby_and_integrity() {
        let root = crate::project_root().join("dev/content-library");
        let docs = load(&root).unwrap();
        let origin = near("@37.5662,126.9777").unwrap();
        let rows = search(&docs, "@37.5662,126.9777", "대피시설");
        assert_eq!(rows.len(), 17233);
        assert!(rows
            .windows(2)
            .all(|r| distance(origin, docs[r[0]].location.as_ref().unwrap())
                <= distance(origin, docs[r[1]].location.as_ref().unwrap())));
        assert!(distance(origin, docs[rows[0]].location.as_ref().unwrap()) < 0.1);
        assert!(near("@nan,127").is_none());
        assert!(checked(&root, "shelters.json", &Value::String("wrong hash".into())).is_err());
        assert!(checked(&root, "../docs", &Value::Null).is_err());
        assert_eq!(
            maps::destination(&root, None, &[]),
            "No verified coordinates for this record"
        );
    }
}
