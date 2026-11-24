// SPDX-License-Identifier: AGPL-3.0-only
//! Bounded explicit W × hours arithmetic, not a free-form expression evaluator.
//! Only used alongside the reviewed energy document; no medical dosing.
pub fn calculate(question: &str) -> Option<String> {
    let q = question.to_lowercase();
    let daily = ["하루", "매일", "일일"].iter().any(|s| q.contains(s));
    let hours = ["몇 시간", "몇시간", "사용 시간", "사용시간"]
        .iter()
        .any(|s| q.contains(s));
    if (!daily && !hours)
        || [
            "약",
            "복용",
            "용량을",
            "모르",
            "불명",
            "제외",
            "빼고",
            "아니",
            "명목",
            "kwh",
            "kw",
            "mw",
            "~",
            "∼",
        ]
        .iter()
        .any(|s| q.contains(s))
    {
        return None;
    }
    let chars: Vec<char> = q.chars().collect();
    let mut tokens = vec![];
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
            i += 1;
        }
        if start > 0
            && (['-', '−', '–'].contains(&chars[start - 1])
                || chars[start - 1].is_ascii_alphanumeric())
        {
            return None;
        }
        let number = chars[start..i]
            .iter()
            .collect::<String>()
            .parse::<f64>()
            .ok()?;
        if !number.is_finite() || number <= 0.0 || number > 100000.0 {
            return None;
        }
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        let tail = chars[i..].iter().collect::<String>();
        let (unit, len) = if tail.starts_with("wh") {
            ("Wh", 2)
        } else if tail.starts_with('w') {
            ("W", 1)
        } else if tail.starts_with("시간") {
            ("h", 2)
        } else {
            return None;
        };
        tokens.push((number, unit, start));
        i += len;
    }
    if !daily {
        if tokens.len() != 2 || !q.contains("실제") {
            return None;
        }
        let wh = tokens.iter().find(|t| t.1 == "Wh")?.0;
        let w = tokens.iter().find(|t| t.1 == "W")?.0;
        return Some(format!("Rust 산술 결과 · 사용자가 제시한 값이며 기기 실측 확인은 아님\n실제 공급량 {wh}Wh ÷ 일정 부하 {w}W = 계산상 사용 가능 시간 {:.2}시간.\n가정: 부하는 일정하고 추가 부하는 제외한 산술입니다. 실제 공급량에 이미 반영한 손실은 중복 차감하지 않습니다. 미반영 손실·예비잔량·노후·추위·부하 변화 및 실제 시험시간을 확인해야 하며 의료기기 생명유지 시간이나 안전 잔량을 보장하지 않습니다.",wh/w));
    }
    let mut loads = vec![];
    let mut capacity = None;
    let mut n = 0;
    while n < tokens.len() {
        match tokens[n].1 {
            "W" => {
                let next = *tokens.get(n + 1)?;
                if next.1 != "h" || next.0 > 24.0 || next.2 - tokens[n].2 > 40 {
                    return None;
                }
                loads.push((tokens[n].0, next.0));
                n += 2;
            }
            "Wh" => {
                if capacity.is_some() || !q.contains("실제") {
                    return None;
                }
                capacity = Some(tokens[n].0);
                n += 1;
            }
            _ => return None,
        }
    }
    if loads.is_empty() || loads.len() > 10 {
        return None;
    }
    let total = loads.iter().map(|(w, h)| w * h).sum::<f64>();
    let mut result =
        String::from("Rust 산술 결과 · 사용자가 제시한 값이며 기기 실측 확인은 아님\n");
    for (w, h) in &loads {
        result.push_str(&format!("{w}W × {h}시간/일 = {}Wh/일\n", w * h));
    }
    result.push_str(&format!("하루 합계: {total}Wh/일.\n"));
    if let Some(wh) = capacity {
        result.push_str(&format!(
            "사용자가 제시한 실제 공급량 {wh}Wh ÷ {total}Wh/일 = 계산상 사용 가능 기간 {:.2}일.\n",
            wh / total
        ));
    }
    result.push_str("가정: 나열한 사용 시간이 하루 기준이고 다른 부하는 포함하지 않은 산술값입니다. 실제 공급량에 이미 반영한 손실은 중복 차감하지 않으며, 미반영 손실·예비잔량·노후·추위·부하 변화와 실제 시험시간을 확인합니다. 의료기기 생명유지 가능 시간이나 안전 잔량을 보장하지 않습니다.");
    Some(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unit_arithmetic_and_explicit_assumptions() {
        let s=calculate("조명 5W를 4시간, 라디오 2W를 6시간, 노트북 25W를 2시간 쓰면 하루 얼마인가요? 실제 공급량120Wh로 며칠인가요?").unwrap();
        assert!(s.contains("82Wh/일"));
        assert!(s.contains("1.46일"));
        let s = calculate("매일 7W를 3시간 사용하면 하루 얼마인가요? 실제 공급량100Wh").unwrap();
        assert!(s.contains("21Wh/일"));
        assert!(s.contains("4.76일"));
        assert!(
            calculate("실제 공급량400Wh, 일정 부하40W라면 몇 시간인가요?")
                .unwrap()
                .contains("10.00시간")
        );
        for q in [
            "하루 5mg을 4시간마다 복용",
            "하루 5W를 4시간, 명목120Wh",
            "하루 5W를 25시간",
            "하루 -5W를 4시간",
            "하루 5W 제외 4시간",
            "하루 5W 4분",
            "하루 5W 4시간 실제120Wh 모르겠어",
        ] {
            assert!(calculate(q).is_none(), "{q}");
        }
    }
}
