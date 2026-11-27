//! Save codec owned by the engine, so web and iOS write byte-compatible saves.
//!
//! A save is a JSON envelope: format tag, save version, revision, device, time, checksum and the
//! state itself. While the game is iterating fast there are no migrations: when the state's shape
//! changes, its [`Saved::VERSION`] goes up and older saves are refused (the web starts a new game).
//! The envelope knows nothing about what the state holds; the mechanics layer implements
//! [`Saved`] for its state.

use crate::kernel::hash::fnv64;
use crate::t;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const FORMAT: &str = "idle-proto-save";

/// A state the envelope can carry.
pub trait Saved: Serialize + DeserializeOwned {
    /// The state's shape version; saves of any other version are refused.
    const VERSION: u32;
    fn revision(&self) -> u64;
    fn device(&self) -> &str;
    /// Game time of the save (unix seconds).
    fn saved_at(&self) -> i64;
}

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

pub fn encode<S: Saved>(state: &S) -> String {
    let value = serde_json::to_value(state).expect("state serialises");
    let body = serde_json::to_string(&value).expect("value serialises");
    let env = Envelope {
        format: FORMAT.into(),
        version: S::VERSION,
        revision: state.revision(),
        device: state.device().to_string(),
        saved_at: state.saved_at(),
        checksum: fnv64(body.as_bytes()),
        state: value,
    };
    serde_json::to_string(&env).expect("envelope serialises")
}

pub fn info(json: &str) -> Result<SaveInfo, String> {
    let env: Envelope =
        serde_json::from_str(json).map_err(|e| t!("err.save.unreadable", err = e))?;
    Ok(SaveInfo {
        version: env.version,
        revision: env.revision,
        device: env.device,
        saved_at: env.saved_at,
    })
}

pub fn decode<S: Saved>(json: &str) -> Result<S, String> {
    let env: Envelope =
        serde_json::from_str(json).map_err(|e| t!("err.save.unreadable", err = e))?;
    if env.format != FORMAT {
        return Err(t!("err.save.not_ours"));
    }
    if env.version > S::VERSION {
        return Err(t!(
            "err.save.too_new",
            version = env.version,
            current = S::VERSION
        ));
    }
    if env.version < S::VERSION {
        return Err(t!(
            "err.save.too_old",
            version = env.version,
            current = S::VERSION
        ));
    }
    let body = serde_json::to_string(&env.state).map_err(|e| e.to_string())?;
    if fnv64(body.as_bytes()) != env.checksum {
        return Err(t!("err.save.checksum"));
    }
    serde_json::from_value(env.state).map_err(|e| t!("err.save.bad_shape", err = e))
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
