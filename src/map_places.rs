// SPDX-License-Identifier: AGPL-3.0-only
//! Official facilities as Organic Maps "saved places": one KML list per province, so the
//! map shows civil-defence shelters and emergency water points without the user typing
//! coordinates. Written into the viewer's bookmarks folder before it starts; rewritten
//! only when the facility data changes (the viewer may resave the files, e.g. when a list
//! is hidden, and that choice is kept).
use super::*;

const STAMP: &str = "korea-sos-places.stamp";
const PREFIX: &str = "korea-sos-";
/// Hidden below this zoom so the country view stays readable and fast.
const MIN_ZOOM: u8 = 13;
const PROVINCES: &[&str] = &[
    "서울특별시", "부산광역시", "대구광역시", "인천광역시", "대전광역시", "울산광역시", "세종특별자치시",
    "경기도", "강원특별자치도", "충청북도", "충청남도", "전북특별자치도", "전남광주통합특별시",
    "경상북도", "경상남도", "제주특별자치도",
];

struct Place<'a> {
    lat: f64,
    lon: f64,
    name: &'a str,
    address: &'a str,
    water: bool,
}

fn xml(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .map(|c| match c {
            '&' => "&amp;".into(),
            '<' => "&lt;".into(),
            '>' => "&gt;".into(),
            '"' => "&quot;".into(),
            _ => c.to_string(),
        })
        .collect()
}

fn places(docs: &[Doc]) -> Vec<(String, Place<'_>)> {
    docs.iter()
        .filter(|d| d.id.starts_with("shelter:") && !d.id.starts_with("shelter:hospital:"))
        .filter_map(|d| {
            let p = d.location.as_ref().filter(|p| p.valid())?;
            let rest = d.title.split_once("] ").map_or(d.title.as_str(), |(_, r)| r);
            let (name, address) = rest.split_once(" · ").unwrap_or((rest, ""));
            let first = address.split_whitespace().next().unwrap_or("");
            let province = if PROVINCES.contains(&first) { first.to_owned() } else { "기타 지역".to_owned() };
            Some((province, Place { lat: p.lat, lon: p.lon, name, address, water: d.id.starts_with("shelter:water:") }))
        })
        .collect()
}

fn kml(province: &str, list: &[&Place]) -> String {
    let mut out = String::with_capacity(list.len() * 520 + 1024);
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<kml xmlns=\"http://www.opengis.net/kml/2.2\">\n<Document>\n");
    out.push_str(&format!(
        "  <name>대피시설 · {}</name>\n  <description>행정안전부 공공데이터(민방위 대피시설·비상급수시설). 현재 개방·급수 가능 여부는 확인되지 않습니다.</description>\n  <visibility>1</visibility>\n",
        xml(province)
    ));
    for p in list {
        let (kind, style, icon) = if p.water { ("비상급수", "placemark-blue", "Water") } else { ("민방위", "placemark-red", "Building") };
        out.push_str(&format!(
            "  <Placemark>\n    <name>[{kind}] {}</name>\n    <description>{}</description>\n    <styleUrl>#{style}</styleUrl>\n    <Point><coordinates>{:.7},{:.7}</coordinates></Point>\n    <ExtendedData xmlns:mwm=\"https://omaps.app\">\n      <mwm:icon>{icon}</mwm:icon>\n      <mwm:minZoom>{MIN_ZOOM}</mwm:minZoom>\n    </ExtendedData>\n  </Placemark>\n",
            xml(p.name),
            xml(p.address),
            p.lon,
            p.lat
        ));
    }
    out.push_str("</Document>\n</kml>\n");
    out
}

/// Writes the province lists into `dir` (the viewer's bookmarks folder) unless the same
/// data was written before. Returns how many places the lists hold.
pub(super) fn write(dir: &Path, docs: &[Doc]) -> io::Result<usize> {
    let all = places(docs);
    let mut groups: Vec<(&str, Vec<&Place>)> = Vec::new();
    for name in PROVINCES.iter().copied().chain(std::iter::once("기타 지역")) {
        let list: Vec<&Place> = all.iter().filter(|(p, _)| p == name).map(|(_, x)| x).collect();
        if !list.is_empty() {
            groups.push((name, list));
        }
    }
    let files: Vec<(String, String)> = groups
        .iter()
        .enumerate()
        .map(|(i, (name, list))| (format!("{PREFIX}{:02}.kml", i + 1), kml(name, list)))
        .collect();
    let stamp = {
        let mut h = Sha256::new();
        for (f, body) in &files {
            h.update(f.as_bytes());
            h.update(body.as_bytes());
        }
        format!("{:x}\n", h.finalize())
    };
    fs::create_dir_all(dir)?;
    if fs::read_to_string(dir.join(STAMP)).ok().as_deref() == Some(stamp.as_str()) {
        return Ok(all.len());
    }
    // Replace only our own lists; the user's saved places stay untouched.
    for e in fs::read_dir(dir)?.flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if n.starts_with(PREFIX) && n.ends_with(".kml") {
            fs::remove_file(e.path())?;
        }
    }
    for (f, body) in &files {
        let tmp = dir.join(format!("{f}.tmp"));
        fs::write(&tmp, body)?;
        fs::rename(&tmp, dir.join(f))?;
    }
    fs::write(dir.join(STAMP), stamp)?;
    Ok(all.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn doc(id: &str, title: &str, lat: f64, lon: f64) -> Doc {
        Doc {
            location: Some(shelters::Location { lat, lon, name: "x".into() }),
            id: id.into(),
            title: title.into(),
            categories: vec!["대피시설".into()],
            file: String::new(),
            body: String::new(),
            display_body: String::new(),
            record: String::new(),
            source: String::new(),
            original_uri: String::new(),
            spread: 0,
            original_path: PathBuf::new(),
        }
    }
    #[test]
    fn province_lists_are_written_once_and_escape_names() {
        let dir = env::temp_dir().join(format!("sos-places-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("my-own.kml"), "user").unwrap();
        let docs = vec![
            doc("shelter:1", "[민방위] 시청 <지하> & 역 · 서울특별시 중구 태평로1가 31", 37.5662, 126.9777),
            doc("shelter:water:2", "[비상급수] 학교 · 부산광역시 해운대구 우동 1", 35.16, 129.16),
            doc("shelter:hospital:3", "[응급의료 주소] 병원 · 서울특별시 중구", 37.5, 127.0),
        ];
        assert_eq!(write(&dir, &docs).unwrap(), 2);
        let seoul = fs::read_to_string(dir.join("korea-sos-01.kml")).unwrap();
        assert!(seoul.contains("<name>대피시설 · 서울특별시</name>"));
        assert!(seoul.contains("시청 &lt;지하&gt; &amp; 역"));
        assert!(seoul.contains("<coordinates>126.9777000,37.5662000</coordinates>"));
        assert!(seoul.contains("<mwm:icon>Building</mwm:icon>"));
        let busan = fs::read_to_string(dir.join("korea-sos-02.kml")).unwrap();
        assert!(busan.contains("<mwm:icon>Water</mwm:icon>") && busan.contains("#placemark-blue"));
        // the viewer may resave a list (e.g. hidden); the same data is not rewritten
        fs::write(dir.join("korea-sos-01.kml"), "resaved by viewer").unwrap();
        write(&dir, &docs).unwrap();
        assert_eq!(fs::read_to_string(dir.join("korea-sos-01.kml")).unwrap(), "resaved by viewer");
        assert_eq!(fs::read_to_string(dir.join("my-own.kml")).unwrap(), "user");
        let _ = fs::remove_dir_all(&dir);
    }
}
