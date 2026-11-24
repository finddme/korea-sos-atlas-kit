// SPDX-License-Identifier: AGPL-3.0-only
//! Deterministic help for the small model's retrieval.
//!
//! The model writes broad search terms ("재난 상황 행동 요령") that miss the
//! words the core documents actually use. A fixed, reviewed table adds the
//! core documents' own title terms for common emergency topics, field manuals
//! yield to civilian guidance unless the question is about the field, and
//! questions about finding facilities are answered with the app's own
//! facility search instead of document text (2026-11-24 diagnosis,
//! dev/poc/ai-quality/2026-11-24-baseline/DIAGNOSIS.md).

/// (question words, extra search queries). Queries are words from the
/// titles of the core civilian documents, so each one finds them.
const TOPICS: &[(&[&str], &[&str])] = &[
    (
        &["공습", "미사일", "폭격", "포격", "드론 공격"],
        &["민방위 경보", "대피시설"],
    ),
    (
        &[
            "전쟁",
            "전시",
            "북한",
            "민방위",
            "적의 공격",
            "핵무기",
            "핵 공격",
        ],
        &["민방위 경보", "비상계획", "비상물품"],
    ),
    (&["지진", "흔들"], &["지진", "흔들리는 동안"]),
    (
        &[
            "비상가방",
            "대피가방",
            "생존배낭",
            "생존 배낭",
            "준비물",
            "챙겨",
            "챙길",
            "비상용품",
            "비상물품",
        ],
        &["비상물품", "대피가방"],
    ),
    (&["정전", "전기가 끊", "전기 끊"], &["정전"]),
    (
        &["단수", "수돗물", "마실 물", "식수"],
        &["단수", "비상 식수"],
    ),
    (&["화학", "가스 누출", "유독가스"], &["화학물질 사고"]),
    (&["방사선", "방사능", "원전 사고"], &["방사선 비상"]),
    (&["한파", "난방", "추위"], &["한파"]),
    (&["통신", "전화가 안", "인터넷이 끊", "연락"], &["비상계획"]),
];
const MAX_HINTS: usize = 3;

/// Extra search queries for the question, most specific topic first.
pub fn hints(question: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (words, queries) in TOPICS {
        if words.iter().any(|w| question.contains(w)) {
            for q in *queries {
                if out.len() < MAX_HINTS && !out.iter().any(|o| o == q) {
                    out.push((*q).to_owned());
                }
            }
        }
    }
    out
}

/// Field manuals answer only questions that are about the field.
pub fn wants_field_manual(question: &str) -> bool {
    [
        "군인",
        "군 교범",
        "군교범",
        "군사",
        "의무병",
        "야외",
        "산에서",
        "산행",
        "등산",
        "캠핑",
        "야영",
        "숲",
        "조난",
        "무인도",
        "야생",
    ]
    .iter()
    .any(|w| question.contains(w))
}

pub fn is_field_manual(id: &str) -> bool {
    id.starts_with("army-atp-") || id.starts_with("marines-mcrp-")
}

/// "Where is the nearest shelter?" is answered by the app's facility list,
/// not by document text. Returns (reply, guide document id).
pub fn app_feature_reply(question: &str) -> Option<(String, &'static str)> {
    let locate = ["어디", "찾", "근처", "가까운", "가까이", "위치"]
        .iter()
        .any(|w| question.contains(w));
    if !locate {
        return None;
    }
    let has = |words: &[&str]| words.iter().any(|w| question.contains(w));
    let coords = "위치를 알면 `/`에 `@위도,경도`(예: @37.5662,126.9777)를 입력하면 가까운 순서로 보여 줍니다. 거리는 직선거리이며 실제 걷는 경로가 아닙니다.";
    let (what, steps, guide) = if has(&["대피소", "대피시설", "대피 장소", "대피장소", "방공호"])
    {
        (
            "민방위 대피시설",
            format!("1. 왼쪽 분류에서 `대피시설`을 고릅니다(Tab 또는 클릭).\n2. `/`를 누르고 동네·도로·건물 이름을 입력합니다(예: 종로 교보).\n3. {coords}\n4. 시설을 열면 주소·수용인원·기준일이 나오고, `m`으로 지도에 목적지를 표시할 수 있습니다(지도가 준비된 경우).\n\n이 목록은 행정안전부가 지정한 민방위 대피시설입니다. 지진·해일 대피장소와는 다르며, 실제 개방 여부는 현장 안내와 방송을 따르세요."),
            "mois-civil-defense-shelters",
        )
    } else if has(&["급수", "물 받", "식수 받", "마실 물"]) {
        (
            "비상급수시설",
            format!("1. 왼쪽 분류에서 `대피시설`을 고릅니다(Tab 또는 클릭).\n2. `/`를 누르고 `급수`와 동네 이름을 함께 입력합니다(예: 급수 종로).\n3. {coords} 결과에는 대피시설과 급수시설이 함께 나오며, 급수시설은 [비상급수]로 표시됩니다.\n\n현재 운영·급수 가능 여부는 이 자료로 확인할 수 없습니다. 지자체 안내를 확인하세요."),
            "mois-civil-defense-water",
        )
    } else if has(&["응급실", "응급의료기관", "응급의료"]) {
        (
            "응급의료기관",
            "1. 왼쪽 분류에서 `대피시설`을 고릅니다(Tab 또는 클릭).\n2. `/`를 누르고 `응급` 또는 병원·동네 이름을 입력합니다.\n\n응급의료기관 목록은 주소만 있어 가까운 순 정렬은 되지 않습니다. 현재 진료 가능 여부는 확인할 수 없으니, 위급하면 119에 먼저 연락하세요.".to_owned(),
            "mois-emergency-hospitals",
        )
    } else {
        return None;
    };
    Some((
        format!("앱 사용 안내 · AI 생성 아님\n가까운 {what} 찾기\n\n{steps}"),
        guide,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broad_topics_add_core_document_terms() {
        assert_eq!(
            hints("전쟁 대비 방법"),
            ["민방위 경보", "비상계획", "비상물품"]
        );
        assert_eq!(
            hints("공습 경보가 울리면 어떻게 해야 해?"),
            ["민방위 경보", "대피시설"]
        );
        assert_eq!(hints("비상가방에 뭘 넣어야 해?"), ["비상물품", "대피가방"]);
        assert!(hints("화상 입었을 때 응급처치 알려줘").is_empty());
        // "핵심" must not trigger the nuclear topic.
        assert!(hints("핵심만 알려줘").is_empty());
        assert!(hints("전쟁 공습 지진 정전").len() <= MAX_HINTS);
    }

    #[test]
    fn field_manuals_only_for_field_questions() {
        assert!(!wants_field_manual("비상가방에 뭘 넣어야 해?"));
        assert!(wants_field_manual("산에서 조난됐을 때 물은?"));
        assert!(is_field_manual("army-atp-3-50-21:page-21"));
        assert!(!is_field_manual("nema-supplies"));
    }

    #[test]
    fn facility_questions_get_app_instructions() {
        let (text, guide) = app_feature_reply("근처 대피소는 어떻게 찾아?").unwrap();
        assert!(text.contains("@위도,경도") && text.contains("대피시설"));
        assert_eq!(guide, "mois-civil-defense-shelters");
        assert_eq!(
            app_feature_reply("가까운 급수시설 어디야").unwrap().1,
            "mois-civil-defense-water"
        );
        assert!(app_feature_reply("응급실 어디 있어?").is_some());
        // Not a location question: leave it to the documents.
        assert!(app_feature_reply("대피소에 갈 때 뭘 챙겨?").is_none());
        assert!(app_feature_reply("화상 입으면 병원 가야 해?").is_none());
    }
}
