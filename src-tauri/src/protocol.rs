//! N3 v3 report codec. Packet layout and limitations are in docs/PROTOCOL.md.
use crate::model::{now, DeviceEvent};
pub const REPORT_SIZE: usize = 1025;
pub fn command(cmd: &[u8]) -> Vec<u8> {
    let mut b = vec![0; REPORT_SIZE];
    b[1..6].copy_from_slice(b"CRT\0\0");
    b[6..6 + cmd.len()].copy_from_slice(cmd);
    b
}
pub fn brightness(value: u8) -> Vec<u8> {
    command(&[b'L', b'I', b'G', 0, 0, value.min(100)])
}
pub fn image_packets(key: u8, jpeg: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    if !(1..=6).contains(&key) {
        return Err("Only LCD keys 1–6 accept images".into());
    }
    if jpeg.is_empty() || jpeg.len() > 65535 {
        return Err("Image must contain 1–65535 bytes".into());
    }
    let len = jpeg.len() as u16;
    let mut out = vec![command(&[
        b'B',
        b'A',
        b'T',
        0,
        0,
        (len >> 8) as u8,
        len as u8,
        key,
    ])];
    for chunk in jpeg.chunks(1024) {
        let mut b = vec![0; REPORT_SIZE];
        b[1..1 + chunk.len()].copy_from_slice(chunk);
        out.push(b)
    }
    Ok(out)
}
pub fn decode(b: &[u8]) -> Option<DeviceEvent> {
    let b = if b.starts_with(&[0, b'A', b'C', b'K']) {
        &b[1..]
    } else {
        b
    };
    if b.len() < 11 || !b.starts_with(b"ACK\0\0OK") {
        return None;
    }
    let code = b[9];
    let state = b[10];
    let (control, event) = match code {
        1..=6 => (format!("lcd{code}"), press(state)?),
        0x25 => ("button1".into(), press(state)?),
        0x30 => ("button2".into(), press(state)?),
        0x31 => ("button3".into(), press(state)?),
        0x35 => ("knob1".into(), press(state)?),
        0x33 => ("knob2".into(), press(state)?),
        0x34 => ("knob3".into(), press(state)?),
        0x50 => ("knob1.ccw".into(), "rotate"),
        0x51 => ("knob1.cw".into(), "rotate"),
        0x90 => ("knob2.ccw".into(), "rotate"),
        0x91 => ("knob2.cw".into(), "rotate"),
        0x60 => ("knob3.ccw".into(), "rotate"),
        0x61 => ("knob3.cw".into(), "rotate"),
        _ => return None,
    };
    Some(DeviceEvent {
        control,
        event: event.into(),
        code,
        state,
        at: now(),
    })
}
fn press(state: u8) -> Option<&'static str> {
    match state {
        1 => Some("down"),
        0 | 2 => Some("up"),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn brightness_encodes_clamped_report() {
        let b = brightness(255);
        assert_eq!(&b[..12], &[0, 67, 82, 84, 0, 0, 76, 73, 71, 0, 0, 100]);
        assert_eq!(b.len(), 1025);
        assert!(b[12..].iter().all(|v| *v == 0));
    }
    #[test]
    fn image_boundaries() {
        let data = vec![0xab; 1025];
        let p = image_packets(6, &data).unwrap();
        assert_eq!(&p[0][11..14], &[4, 1, 6]);
        assert_eq!(p.len(), 3);
        assert_eq!(p[2][1], 0xab);
        assert_eq!(p[2][2], 0);
        assert!(image_packets(7, &data).is_err());
        assert!(image_packets(1, &vec![0; 65536]).is_err());
    }
    #[test]
    fn decode_controls() {
        for (c, key) in [
            (1, "lcd1"),
            (6, "lcd6"),
            (0x25, "button1"),
            (0x35, "knob1"),
            (0x33, "knob2"),
            (0x34, "knob3"),
        ] {
            for s in [0, 1, 2] {
                let mut b = b"ACK\0\0OK\0\0".to_vec();
                b.extend([c, s]);
                let e = decode(&b).unwrap();
                assert_eq!(e.control, key);
                assert_eq!(e.event, if s == 1 { "down" } else { "up" });
            }
        }
        assert!(decode(b"ACK").is_none());
        assert!(decode(&[0; 512]).is_none());
    }
}

#[cfg(test)]
mod live_fixture_tests {
    #[test]
    fn recorded_n3_inputs_decode() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/live-input.json")).unwrap();
        for report in fixture["unique_reports"].as_array().unwrap() {
            let hex = report["hex"].as_str().unwrap();
            let bytes: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect();
            let event = super::decode(&bytes).expect("recorded input must decode");
            assert!(!event.control.is_empty());
            assert_eq!(bytes.len(), 512);
        }
    }
}

#[cfg(test)]
mod exhaustive_trigger_tests {
    use super::*;
    fn packet(code: u8, state: u8) -> Vec<u8> {
        let mut p = vec![0; 512];
        p[..9].copy_from_slice(b"ACK\0\0OK\0\0");
        p[9] = code;
        p[10] = state;
        p
    }
    #[test]
    fn every_press_and_release_with_both_api_prefix_forms() {
        for (code, name) in [
            (1, "lcd1"),
            (2, "lcd2"),
            (3, "lcd3"),
            (4, "lcd4"),
            (5, "lcd5"),
            (6, "lcd6"),
            (0x25, "button1"),
            (0x30, "button2"),
            (0x31, "button3"),
            (0x35, "knob1"),
            (0x33, "knob2"),
            (0x34, "knob3"),
        ] {
            for state in [0, 1, 2] {
                for prefix in [false, true] {
                    let mut p = packet(code, state);
                    if prefix {
                        p.insert(0, 0)
                    }
                    let e = decode(&p).unwrap();
                    assert_eq!(e.control, name);
                    assert_eq!(e.event, if state == 1 { "down" } else { "up" });
                    assert_eq!(e.code, code);
                    assert_eq!(e.state, state);
                }
            }
            for state in 3..=255 {
                assert!(decode(&packet(code, state)).is_none());
            }
        }
    }
    #[test]
    fn every_rotation_and_rapid_interleaving() {
        let mappings = [
            (0x50, "knob1.ccw"),
            (0x51, "knob1.cw"),
            (0x90, "knob2.ccw"),
            (0x91, "knob2.cw"),
            (0x60, "knob3.ccw"),
            (0x61, "knob3.cw"),
        ];
        for _ in 0..100 {
            for (code, name) in mappings {
                let e = decode(&packet(code, 0)).unwrap();
                assert_eq!(e.control, name);
                assert_eq!(e.event, "rotate");
            }
        }
        let events = [packet(1, 1), packet(2, 1), packet(1, 0), packet(2, 0)]
            .iter()
            .map(|p| decode(p).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            events
                .iter()
                .map(|e| e.control.as_str())
                .collect::<Vec<_>>(),
            ["lcd1", "lcd2", "lcd1", "lcd2"]
        );
    }
    #[test]
    fn malformed_reports_and_unknown_codes_are_ignored() {
        let valid = packet(1, 1);
        for length in 0..11 {
            assert!(decode(&valid[..length]).is_none())
        }
        for code in 0..=255 {
            let known = [
                1, 2, 3, 4, 5, 6, 0x25, 0x30, 0x31, 0x35, 0x33, 0x34, 0x50, 0x51, 0x90, 0x91, 0x60,
                0x61,
            ]
            .contains(&code);
            assert_eq!(decode(&packet(code, 1)).is_some(), known, "code {code}");
        }
        for index in 0..7 {
            let mut bad = valid.clone();
            bad[index] ^= 0xff;
            assert!(decode(&bad).is_none());
        }
    }
    #[test]
    fn every_image_chunk_boundary() {
        for len in [1, 1023, 1024, 1025, 2048, 65535] {
            let bytes = vec![0x5a; len];
            for key in 1..=6 {
                let packets = image_packets(key, &bytes).unwrap();
                assert_eq!(packets.len(), 1 + len.div_ceil(1024));
                let mut decoded: Vec<_> = packets[1..]
                    .iter()
                    .flat_map(|p| p[1..].iter().copied())
                    .collect();
                assert!(decoded[len..].iter().all(|b| *b == 0));
                decoded.truncate(len);
                assert_eq!(decoded, bytes);
            }
        }
        assert!(image_packets(0, &[1]).is_err());
        assert!(image_packets(1, &[]).is_err());
    }
}
