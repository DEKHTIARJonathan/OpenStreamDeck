//! Explicit manual diagnostics. Automated tests inject a transport, never open HID.
use crate::{artwork, model::Config, protocol};
use serde::Serialize;

struct Step {
    label: String,
    packet: Vec<u8>,
}
pub struct SmokePlan {
    exercise: Vec<Step>,
    restore: Vec<Step>,
}
#[derive(Debug, Serialize)]
pub struct Observation {
    pub step: String,
    pub bytes: usize,
    pub accepted: bool,
    pub error: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct SmokeReport {
    pub at: u64,
    pub platform: String,
    pub boundary: &'static str,
    pub passed: bool,
    pub restoration_succeeded: bool,
    pub observations: Vec<Observation>,
    pub limitation: &'static str,
}
impl SmokeReport {
    pub fn unavailable(error: String) -> Self {
        Self { at:crate::model::now(),platform:std::env::consts::OS.into(),boundary:"HIDAPI",passed:false,restoration_succeeded:false,observations:vec![Observation{step:"connect".into(),bytes:0,accepted:false,error:Some(error)}],limitation:"HIDAPI acceptance verifies writes, not physical pixels, brightness, or switch operation." }
    }
}
fn step(label: &str, packet: Vec<u8>) -> Step {
    Step {
        label: label.into(),
        packet,
    }
}
pub fn plan(w: &Config, sleeping: bool) -> Result<SmokePlan, String> {
    w.validate()?;
    let exercise = vec![
        step("wake", protocol::command(b"DIS")),
        step(
            "brightness +1",
            protocol::brightness(if w.brightness < 100 {
                w.brightness + 1
            } else {
                99
            }),
        ),
        step("heartbeat", protocol::command(b"CONNECT")),
        step("sleep", protocol::command(b"HAN")),
        step("wake after sleep", protocol::command(b"DIS")),
    ];
    let mut restore = vec![step(
        "restore brightness",
        protocol::brightness(w.brightness),
    )];
    let page = w.page().ok_or("No active page")?;
    for key in 1..=6 {
        let a = page
            .bindings
            .get(&format!("lcd{key}"))
            .cloned()
            .unwrap_or_default();
        let jpeg = artwork::render(&a, w.image_size, w.rotation)?;
        for packet in protocol::image_packets(key, &jpeg)? {
            restore.push(step(&format!("restore LCD {key}"), packet));
        }
    }
    restore.push(step("commit six images", protocol::command(b"STP")));
    restore.push(step(
        "restore sleep state",
        protocol::command(if sleeping { b"HAN" } else { b"DIS" }),
    ));
    Ok(SmokePlan { exercise, restore })
}
pub fn execute(
    plan: &SmokePlan,
    mut write: impl FnMut(&[u8]) -> Result<usize, String>,
) -> SmokeReport {
    let mut report=SmokeReport{at:crate::model::now(),platform:std::env::consts::OS.into(),boundary:"HIDAPI",passed:true,restoration_succeeded:true,observations:vec![],limitation:"HIDAPI acceptance verifies writes, not physical pixels, brightness, or switch operation."};
    for (restoring, steps) in [(false, &plan.exercise), (true, &plan.restore)] {
        for step in steps {
            let error = match write(&step.packet) {
                Ok(n) if n == step.packet.len() => None,
                Ok(n) => Some(format!("Short HID write: {n}/{}", step.packet.len())),
                Err(e) => Some(e),
            };
            let accepted = error.is_none();
            report.observations.push(Observation {
                step: step.label.clone(),
                bytes: step.packet.len(),
                accepted,
                error,
            });
            report.passed &= accepted;
            if restoring {
                report.restoration_succeeded &= accepted;
            }
            // Restore is attempted even when exercise failed. Do not keep sending
            // exercise commands into a device that already reported an error.
            if !accepted && !restoring {
                break;
            }
        }
    }
    report
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn smoke_and_restore_are_hardware_independent() {
        for sleeping in [false, true] {
            let w = Config::default();
            let p = plan(&w, sleeping).unwrap();
            let mut packets = vec![];
            let r = execute(&p, |bytes| {
                packets.push(bytes.to_vec());
                Ok(bytes.len())
            });
            assert!(r.passed && r.restoration_succeeded);
            assert!(packets.iter().all(|p| p.len() == 1025));
            assert_eq!(packets[5], protocol::brightness(w.brightness));
            assert_eq!(
                packets.last().unwrap(),
                &protocol::command(if sleeping { b"HAN" } else { b"DIS" })
            );
            assert_eq!(packets.iter().filter(|b| &b[6..9] == b"BAT").count(), 6);
        }
    }
    #[test]
    fn failures_and_short_writes_still_attempt_restoration() {
        let p = plan(&Config::default(), false).unwrap();
        for short in [false, true] {
            let mut count = 0;
            let r = execute(&p, |b| {
                count += 1;
                if count == 2 {
                    if short {
                        Ok(1)
                    } else {
                        Err("disconnected".into())
                    }
                } else {
                    Ok(b.len())
                }
            });
            assert!(!r.passed);
            assert!(r.restoration_succeeded);
            assert_eq!(r.observations[2].step, "restore brightness");
        }
        let r = execute(&p, |_| Err("disconnected".into()));
        assert!(!r.restoration_succeeded);
        assert_eq!(r.observations.last().unwrap().step, "restore sleep state");
    }
}
