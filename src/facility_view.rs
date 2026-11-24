// SPDX-License-Identifier: AGPL-3.0-only
//! Deterministic display of hash-validated source fields. No inference.
pub(super) struct Fields<'a> {
    pub name: &'a str,
    pub road: &'a str,
    pub address: &'a str,
    pub address_is_land_lot: bool,
    pub updated: &'a str,
    pub modified: &'a str,
    pub status: &'a str,
}
fn value(s: &str) -> &str {
    if s.is_empty() {
        "원자료 미기재"
    } else {
        s
    }
}
pub(super) fn render(f: Fields<'_>) -> String {
    format!("시설명: {}\n도로명주소: {}\n{}: {}\n데이터 갱신시점: {}\n최종수정시점: {}\n등록 상태: {}\n현재 운영·급수·수용 가능 여부: 실시간 확인 불가",
        value(f.name), value(f.road),
        if f.address_is_land_lot { "지번주소" } else { "원자료 주소(종류 미분류)" },
        value(f.address), value(f.updated), value(f.modified), value(f.status))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_field_identity_and_missing_values() {
        let s = render(Fields {
            name: "시설",
            road: "도로 42",
            address: "지번 7",
            address_is_land_lot: true,
            updated: "갱신",
            modified: "수정",
            status: "사용중",
        });
        assert!(s.contains("도로명주소: 도로 42\n지번주소: 지번 7"));
        assert!(s.contains("데이터 갱신시점: 갱신\n최종수정시점: 수정"));
        let s = render(Fields {
            name: "병원",
            road: "",
            address: "주소 7",
            address_is_land_lot: false,
            updated: "",
            modified: "",
            status: "",
        });
        assert!(s.contains("도로명주소: 원자료 미기재"));
        assert!(s.contains("원자료 주소(종류 미분류): 주소 7"));
        assert!(s.contains("실시간 확인 불가"));
    }
}
