// SPDX-License-Identifier: AGPL-3.0-only
use crate::ai_policy::{fail, repetitive, Code, Result};
use serde_json::Value;
#[derive(Default)]
pub struct Stream {
    pending: Vec<u8>,
    text: String,
    final_packet: Option<Value>,
}
impl Stream {
    pub fn feed(&mut self, bytes: &[u8]) -> Result<()> {
        self.pending.extend_from_slice(bytes);
        while let Some(end) = self.pending.iter().position(|b| *b == b'\n') {
            let line = self.pending.drain(..=end).collect::<Vec<_>>();
            let line = std::str::from_utf8(&line)
                .map_err(|_| fail(Code::Protocol, "엔진 스트림 UTF-8 오류"))?
                .trim();
            let Some(data) = line.strip_prefix("data:").map(str::trim) else {
                continue;
            };
            if data == "[DONE]" {
                continue;
            }
            let packet: Value = serde_json::from_str(data)
                .map_err(|_| fail(Code::Protocol, "엔진 스트림 JSON 오류"))?;
            if self.final_packet.is_some() {
                return Err(fail(Code::Protocol, "종료 뒤 추가 스트림 패킷"));
            }
            if packet.get("error").is_some() {
                return Err(fail(Code::Engine, "엔진 스트림 오류"));
            }
            if let Some(s) = packet["content"].as_str() {
                self.text.push_str(s);
                if repetitive(&self.text) {
                    return Err(fail(
                        Code::Repetition,
                        "반복 생성을 감지하여 추론을 중단했습니다.",
                    ));
                }
            }
            if packet["stop"] == true {
                if self.final_packet.is_some() {
                    return Err(fail(Code::Protocol, "중복 종료 패킷"));
                }
                self.final_packet = Some(packet);
            }
        }
        if self.pending.len() > 256 * 1024 || self.text.len() > 128 * 1024 {
            return Err(fail(Code::Protocol, "스트림 크기 제한 초과"));
        }
        Ok(())
    }
    pub fn finish(self) -> Result<Value> {
        if !self.pending.is_empty() {
            return Err(fail(Code::Protocol, "불완전한 스트림 패킷"));
        }
        let mut packet = self
            .final_packet
            .ok_or_else(|| fail(Code::Protocol, "종료 패킷 없이 연결이 끝났습니다."))?;
        packet["content"] = Value::String(self.text);
        Ok(packet)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utf8_packet_boundaries_and_final_status() {
        let data="data: {\"content\":\"한국어\",\"stop\":false}\n\ndata: {\"content\":\"\",\"stop\":true,\"stop_type\":\"eos\"}\n\n";
        let mut stream = Stream::default();
        for b in data.as_bytes() {
            stream.feed(&[*b]).unwrap();
        }
        let v = stream.finish().unwrap();
        assert_eq!(v["content"], "한국어");
        assert_eq!(v["stop_type"], "eos");
    }
    #[test]
    fn repeated_text_stops_before_final_packet() {
        let mut s = Stream::default();
        let v = serde_json::json!({"content":"반복 문장을 출력합니다. ".repeat(8),"stop":false});
        assert_eq!(
            s.feed(format!("data: {v}\n\n").as_bytes())
                .unwrap_err()
                .code,
            Code::Repetition
        );
    }
    #[test]
    fn missing_final_and_bad_json_fail() {
        assert!(Stream::default().finish().is_err());
        assert!(Stream::default().feed(b"data: {bad}\n").is_err());
        let mut s = Stream::default();
        s.feed(b"data: {\"content\":\"abc\"}\n").unwrap();
        assert!(s.finish().is_err());
        let mut s = Stream::default();
        s.feed(b"data: {\"content\":\"\",\"stop\":true}\n").unwrap();
        assert!(s
            .feed(b"data: {\"content\":\"late\",\"stop\":false}\n")
            .is_err());
    }
}
