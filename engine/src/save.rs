//! Save codec owned by the engine, so web and iOS write byte-compatible saves.
//!
//! A save is a JSON envelope: format tag, save version, revision, device, time, checksum and the
//! state itself. Older versions are upgraded step by step through `MIGRATIONS` on the raw JSON
//! before deserialising, so old saves keep loading after the structs change.

use crate::state::{GameState, SAVE_VERSION};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const FORMAT: &str = "idle-proto-save";

#[derive(Serialize, Deserialize)]
pub struct Envelope {
    pub format: String,
    pub version: u32,
    pub revision: u64,
    pub device: String,
    pub saved_at: i64,
    pub checksum: String,
    pub state: Value,
}

/// Header without the state, for "which save is newer" decisions in a future sync.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SaveInfo {
    pub version: u32,
    pub revision: u64,
    pub device: String,
    pub saved_at: i64,
}

type Migration = fn(&mut Value) -> Result<(), String>;

/// `MIGRATIONS[n]` upgrades a version `n + 1` state to version `n + 2`.
const MIGRATIONS: &[Migration] = &[];

fn fnv64(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{:016x}", h)
}

pub fn encode(state: &GameState) -> String {
    let value = serde_json::to_value(state).expect("state serialises");
    let body = serde_json::to_string(&value).expect("value serialises");
    let env = Envelope {
        format: FORMAT.into(),
        version: SAVE_VERSION,
        revision: state.revision,
        device: state.device.clone(),
        saved_at: state.time,
        checksum: fnv64(body.as_bytes()),
        state: value,
    };
    serde_json::to_string(&env).expect("envelope serialises")
}

pub fn info(json: &str) -> Result<SaveInfo, String> {
    let env: Envelope = serde_json::from_str(json).map_err(|e| format!("存档读不出来：{e}"))?;
    Ok(SaveInfo {
        version: env.version,
        revision: env.revision,
        device: env.device,
        saved_at: env.saved_at,
    })
}

pub fn decode(json: &str) -> Result<GameState, String> {
    let env: Envelope = serde_json::from_str(json).map_err(|e| format!("存档读不出来：{e}"))?;
    if env.format != FORMAT {
        return Err("不是这个游戏的存档".into());
    }
    if env.version > SAVE_VERSION {
        return Err(format!(
            "存档来自更新的版本（v{}），当前只认到 v{}",
            env.version, SAVE_VERSION
        ));
    }
    if env.version == 0 {
        return Err("存档版本号无效".into());
    }
    let body = serde_json::to_string(&env.state).map_err(|e| e.to_string())?;
    if fnv64(body.as_bytes()) != env.checksum {
        return Err("存档校验不通过，可能被截断或改动过".into());
    }
    let mut value = env.state;
    for v in env.version..SAVE_VERSION {
        let step =
            MIGRATIONS
                .get((v - 1) as usize)
                .ok_or(format!("缺少 v{} → v{} 的迁移", v, v + 1))?;
        step(&mut value)?;
    }
    let mut state: GameState =
        serde_json::from_value(value).map_err(|e| format!("存档结构不对：{e}"))?;
    state.version = SAVE_VERSION;
    Ok(state)
}

/// Later of two saves of the same game: higher revision wins, then later time, then device name
/// (only so the choice is deterministic).
pub fn newer<'a>(a: &'a SaveInfo, b: &'a SaveInfo) -> &'a SaveInfo {
    let ka = (a.revision, a.saved_at, &a.device);
    let kb = (b.revision, b.saved_at, &b.device);
    if kb > ka {
        b
    } else {
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{commands, sim};

    #[test]
    fn round_trip_keeps_state_and_future() {
        let mut s = sim::new_game(7, 1_000_000, "test");
        commands::apply(&mut s, &commands::Command::ChooseMap { index: 0 }).unwrap();
        sim::advance(&mut s, 1_000_000 + 5 * 3600);
        let json = encode(&s);
        let mut loaded = decode(&json).unwrap();
        assert_eq!(encode(&loaded), json);
        // The loaded copy and the original continue identically.
        sim::advance(&mut s, 1_000_000 + 30 * 3600);
        sim::advance(&mut loaded, 1_000_000 + 30 * 3600);
        assert_eq!(encode(&s), encode(&loaded));
    }

    #[test]
    fn rejects_tampering_and_future_versions() {
        let s = sim::new_game(7, 1_000_000, "test");
        let json = encode(&s);
        let tampered = json.replacen("\"time\":1000000", "\"time\":1000001", 1);
        assert!(decode(&tampered).is_err());
        let mut env: Envelope = serde_json::from_str(&json).unwrap();
        env.version = SAVE_VERSION + 1;
        assert!(decode(&serde_json::to_string(&env).unwrap()).is_err());
    }

    #[test]
    fn newer_prefers_revision() {
        let a = SaveInfo {
            version: 1,
            revision: 5,
            device: "web".into(),
            saved_at: 100,
        };
        let b = SaveInfo {
            version: 1,
            revision: 6,
            device: "ios".into(),
            saved_at: 50,
        };
        assert_eq!(newer(&a, &b), &b);
    }
}
