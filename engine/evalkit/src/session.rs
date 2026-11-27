//! Play sessions in the evaluation (docs/eval-interface.md §评估会话).
//!
//! - `eval run … --sessions OUTDIR` writes each bot game as a session directory (`write_bot`):
//!   manifest with `schedule` "bot", the journal (setup commands included), no visits, the final
//!   save and final.json.
//! - `eval session DIR…` (`run_dirs`) replays each finished session's journal through
//!   `run::measure` — the same 5-minute steps and hooks as `eval run`, so the same metric ids — and
//!   adds the `llm.*` feedback metrics from visits.jsonl (`llm_metrics`).
//!
//! A session is stale when the engine refuses one of its journal commands or the replayed final
//! save differs from final.json's `save_sha`: the game changed since it was played. Its game
//! metrics would describe some other game, so only `llm.feel.*` and `llm.score.*` are kept.
//! Every replayed session also gets `session.stale` (1 stale, 0 replayed).
use crate::payoff::Ev;
use crate::run::{self, Driver, Game, Metrics, Persona, Replay, RunFile, SeedResult, T0};
use idle_engine::textui::session::{self as proto, Entry, Final, Manifest, VisitLog};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// The manifest `schedule` of a bot's session: the bot acts on its own clock, not on visits.
pub const BOT_SCHEDULE: &str = "bot";
/// The feel label the `llm.delight_given_*` metrics count.
const DELIGHT: &str = "delight";
/// Metric written for every replayed session: 1 when it is stale, 0 when it replayed.
pub const STALE: &str = "session.stale";

fn io(path: &Path, e: impl std::fmt::Display) -> String {
    format!("{}: {e}", path.display())
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    fs::write(path, text).map_err(|e| io(path, e))
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| io(path, e))
}

/// One JSON object per line, as `textui::session` writes its .jsonl files.
fn json_lines<T: Serialize>(rows: &[T]) -> String {
    rows.iter()
        .map(|r| serde_json::to_string(r).unwrap() + "\n")
        .collect()
}

fn pretty<T: Serialize>(x: &T) -> String {
    serde_json::to_string_pretty(x).unwrap() + "\n"
}

/// One bot game as a session directory (`eval run … --sessions`).
pub fn write_bot(
    dir: &Path,
    seed: u64,
    persona: Persona,
    hours: i64,
    g: &mut Game,
) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| io(dir, e))?;
    let m = Manifest {
        protocol: proto::PROTOCOL,
        build: idle_engine::content::fingerprint(),
        seed,
        player: persona.name(),
        schedule: BOT_SCHEDULE.into(),
        hours,
        lang: idle_engine::i18n::locale().into(),
        research: std::env::var("BOT_NORESEARCH").is_err(),
    };
    let save = g.e.save();
    let fin = Final {
        t: hours * 3600,
        summary: proto::summarize(&mut g.e),
        save_sha: proto::save_digest(&save),
    };
    write(&dir.join(proto::MANIFEST), &pretty(&m))?;
    write(&dir.join(proto::JOURNAL), &json_lines(&g.journal))?;
    write(&dir.join(proto::VISITS), "")?;
    write(&dir.join(proto::SAVE), &save)?;
    write(&dir.join(proto::FINAL), &pretty(&fin))
}

/// A finished session directory.
pub struct Session {
    pub dir: PathBuf,
    pub manifest: Manifest,
    pub journal: Vec<Entry>,
    pub visits: Vec<VisitLog>,
    pub fin: Final,
}

/// Reads a session directory; `Ok(None)` when it has no final.json (not finished). A manifest of
/// another protocol version is an error naming the version.
pub fn load(dir: &Path) -> Result<Option<Session>, String> {
    let path = dir.join(proto::MANIFEST);
    let v: serde_json::Value = serde_json::from_str(&read(&path)?).map_err(|e| io(&path, e))?;
    match v.get("protocol").and_then(|p| p.as_u64()) {
        Some(p) if p == u64::from(proto::PROTOCOL) => {}
        Some(p) => {
            return Err(format!(
                "{}: session protocol {p}; this tool reads protocol {}",
                path.display(),
                proto::PROTOCOL
            ))
        }
        None => return Err(format!("{}: no protocol version", path.display())),
    }
    let manifest: Manifest = serde_json::from_value(v).map_err(|e| io(&path, e))?;
    let fpath = dir.join(proto::FINAL);
    if !fpath.exists() {
        return Ok(None);
    }
    let fin: Final = serde_json::from_str(&read(&fpath)?).map_err(|e| io(&fpath, e))?;
    let journal = proto::read_journal(dir)?;
    let vpath = dir.join(proto::VISITS);
    let visits = if vpath.exists() {
        read(&vpath)?
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).map_err(|e| io(&vpath, e)))
            .collect::<Result<Vec<VisitLog>, String>>()?
    } else {
        Vec::new()
    };
    Ok(Some(Session {
        dir: dir.to_path_buf(),
        manifest,
        journal,
        visits,
        fin,
    }))
}

/// One replayed session.
pub struct Replayed {
    pub seed: u64,
    pub metrics: Metrics,
    /// Why the session is stale, if it is.
    pub stale: Option<String>,
    /// Journal commands by name (each was accepted when the session was played).
    pub commands: BTreeMap<String, u32>,
    /// `save_digest` of the replayed final save.
    pub digest: String,
}

/// Replays a session to final.json's `t`: the measured hours, then the entries up to the end
/// (e.g. tasks claimed at the last visit) for the final save.
pub fn replay(s: &Session) -> Replayed {
    let end = s.fin.t;
    let (e, r) = Replay::start(s.manifest.seed, &s.journal);
    let mut driver = Driver::Replay(r);
    let mut g = run::measure(e, &mut driver, end / 3600);
    let Driver::Replay(mut r) = driver else {
        unreachable!()
    };
    r.finish(&mut g.e, T0 + end);
    let digest = proto::save_digest(&g.e.save());
    let stale = if let Some((i, err)) = r.refused.first() {
        Some(format!(
            "{} journal command(s) refused, first line {}: {err}",
            r.refused.len(),
            i + 1
        ))
    } else if digest != s.fin.save_sha {
        Some(format!(
            "replayed save {digest} differs from final.json save_sha {}",
            s.fin.save_sha
        ))
    } else {
        None
    };
    let mut metrics = if stale.is_none() {
        g.metrics
    } else {
        Metrics::new()
    };
    llm_metrics(
        &s.visits,
        stale.is_none().then_some(&g.evs[..]),
        &mut metrics,
    );
    metrics.insert(STALE.into(), Some(if stale.is_some() { 1.0 } else { 0.0 }));
    let mut commands = BTreeMap::new();
    for x in &s.journal {
        *commands
            .entry(idle_engine::textui::cmd_name(&x.cmd))
            .or_insert(0) += 1;
    }
    Replayed {
        seed: s.manifest.seed,
        metrics,
        stale,
        commands,
        digest,
    }
}

/// The `llm.*` metrics of one session (docs/eval-interface.md §评估会话); nothing without visits
/// (a bot's session). `evs` is the replayed payoff timeline, `None` for a stale session, which
/// keeps only `llm.feel.*` and `llm.score.*`.
///
/// - `llm.feel.<stage>.<label>`: share of the stage's visits (`stage` of visits.jsonl) with that
///   label; a visit without a feel counts in the denominator.
/// - `llm.score.<dim>.<stage>`: the score given for the stage (`score_stage`; for a line without
///   it, the previous visit's stage, the one that just ended); a mean if scored twice.
/// - `llm.delight_given_felt` / `_none`: visits whose window (previous visit, this visit] — the
///   first visit's starts at the beginning — holds / does not hold a felt payoff (tier ≥ 1), and
///   the share of them labelled delight (a visit without a feel counts, as above);
///   `llm.delight_lift` = felt − none when both exist.
/// - `llm.invalid`: failed action items ÷ all action items.
pub fn llm_metrics(visits: &[VisitLog], evs: Option<&[Ev]>, m: &mut Metrics) {
    if visits.is_empty() {
        return;
    }
    let mut stages: Vec<&str> = Vec::new();
    for v in visits {
        if !stages.contains(&v.stage.as_str()) {
            stages.push(&v.stage);
        }
    }
    for st in stages {
        let of: Vec<&VisitLog> = visits.iter().filter(|v| v.stage == st).collect();
        for label in proto::FEELS {
            let n = of
                .iter()
                .filter(|v| v.feel.as_ref().is_some_and(|f| f.label == label))
                .count();
            m.insert(
                format!("llm.feel.{st}.{label}"),
                Some(n as f64 / of.len() as f64),
            );
        }
    }
    let mut scores: BTreeMap<(String, String), Vec<f64>> = BTreeMap::new();
    for (i, v) in visits.iter().enumerate() {
        let Some(sc) = &v.scores else { continue };
        let st = v
            .score_stage
            .clone()
            .or_else(|| i.checked_sub(1).map(|j| visits[j].stage.clone()))
            .unwrap_or_else(|| v.stage.clone());
        for (dim, s) in sc {
            scores
                .entry((dim.clone(), st.clone()))
                .or_default()
                .push(s.score as f64);
        }
    }
    for ((dim, st), xs) in scores {
        let mean = xs.iter().sum::<f64>() / xs.len() as f64;
        m.insert(format!("llm.score.{dim}.{st}"), Some(mean));
    }
    let Some(evs) = evs else { return };
    let felt: Vec<f64> = evs.iter().filter(|e| e.tier >= 1).map(|e| e.t_h).collect();
    // (visits, delighted) with and without a felt payoff in the window.
    let (mut with, mut without) = ((0u32, 0u32), (0u32, 0u32));
    let mut prev: Option<f64> = None;
    for v in visits {
        let now = v.t as f64 / 3600.0;
        let hit = felt.iter().any(|&t| t <= now && prev.is_none_or(|p| t > p));
        let delight = v.feel.as_ref().is_some_and(|f| f.label == DELIGHT);
        let side = if hit { &mut with } else { &mut without };
        side.0 += 1;
        side.1 += u32::from(delight);
        prev = Some(now);
    }
    let share = |(n, d): (u32, u32)| (n > 0).then(|| d as f64 / n as f64);
    let (felt_d, none_d) = (share(with), share(without));
    m.insert("llm.delight_given_felt".into(), felt_d);
    m.insert("llm.delight_given_none".into(), none_d);
    if let (Some(a), Some(b)) = (felt_d, none_d) {
        m.insert("llm.delight_lift".into(), Some(a - b));
    }
    let items: Vec<bool> = visits
        .iter()
        .flat_map(|v| v.actions.iter().map(|a| a.ok))
        .collect();
    let failed = items.iter().filter(|ok| !**ok).count();
    m.insert(
        "llm.invalid".into(),
        (!items.is_empty()).then(|| failed as f64 / items.len() as f64),
    );
}

/// The run's persona for a session's player: a bot persona's own name, else `llm:<player>`.
pub fn persona_of(player: &str) -> String {
    Persona::parse(player).map_or_else(|| format!("llm:{player}"), |p| p.name())
}

/// How one directory went.
pub struct Outcome {
    pub dir: PathBuf,
    pub seed: u64,
    pub stale: Option<String>,
    pub digest: String,
}

/// `eval session DIR…`: loads every directory (an unfinished one is skipped with a warning),
/// checks they are one player's sessions of one length with distinct seeds, and replays them on
/// `threads` threads. Returns the run file (persona from `persona_of`) and how each one went.
pub fn replay_dirs(dirs: &[PathBuf], threads: usize) -> Result<(RunFile, Vec<Outcome>), String> {
    let mut sessions = Vec::new();
    for d in dirs {
        match load(d)? {
            Some(s) => sessions.push(s),
            None => eprintln!(
                "warning: {}: unfinished (no {}), skipped",
                d.display(),
                proto::FINAL
            ),
        }
    }
    if sessions.is_empty() {
        return Err("no finished session to replay".into());
    }
    let players: BTreeSet<&str> = sessions
        .iter()
        .map(|s| s.manifest.player.as_str())
        .collect();
    if players.len() > 1 {
        let list: Vec<&str> = players.into_iter().collect();
        return Err(format!(
            "sessions of several players ({}): run eval session once per player",
            list.join(", ")
        ));
    }
    let player = sessions[0].manifest.player.clone();
    let mut seen: BTreeMap<u64, &Path> = BTreeMap::new();
    for s in &sessions {
        if let Some(other) = seen.insert(s.manifest.seed, &s.dir) {
            return Err(format!(
                "seed {} twice for player {player} ({} and {}): one session per seed",
                s.manifest.seed,
                other.display(),
                s.dir.display()
            ));
        }
    }
    let ends: BTreeSet<i64> = sessions.iter().map(|s| s.fin.t).collect();
    if ends.len() > 1 {
        let list: Vec<String> = ends
            .iter()
            .map(|t| format!("{} h", *t as f64 / 3600.0))
            .collect();
        return Err(format!(
            "sessions of different lengths ({}): checkpoints would not line up; run eval session \
             once per length",
            list.join(", ")
        ));
    }
    let end = sessions[0].fin.t;

    let threads = threads.clamp(1, sessions.len());
    let sessions = &sessions;
    let mut done: Vec<(usize, Replayed)> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..threads)
            .map(|k| {
                sc.spawn(move || {
                    (k..sessions.len())
                        .step_by(threads)
                        .map(|i| (i, replay(&sessions[i])))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    done.sort_by_key(|(_, r)| r.seed);
    let mut outcomes = Vec::new();
    let mut results: Vec<SeedResult> = Vec::new();
    for (i, r) in done {
        let dir = sessions[i].dir.clone();
        if let Some(why) = &r.stale {
            eprintln!(
                "warning: {}: stale ({why}); the game changed since this session was played, so \
                 only llm.feel.* and llm.score.* are kept",
                dir.display()
            );
        }
        outcomes.push(Outcome {
            dir,
            seed: r.seed,
            stale: r.stale,
            digest: r.digest,
        });
        results.push((r.seed, r.metrics, r.commands));
    }
    let mut f = RunFile::collect(persona_of(&player), end / 3600, results);
    // The build the replays ran on (a stale session was played on another one).
    f.build = Some(idle_engine::content::fingerprint());
    Ok((f, outcomes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use idle_engine::textui::session::{Action, Feel, Observation, Schedule, Score, Setup};

    fn tmp(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("idle-eval-session-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    /// Ids whose values differ between two runs of the same seeds (exactly, NaN equal to NaN), and
    /// ids only one side has.
    fn differ(a: &RunFile, b: &RunFile) -> Vec<String> {
        let ids: BTreeSet<&String> = a.metrics.keys().chain(b.metrics.keys()).collect();
        let bits = |v: Option<&Vec<Option<f64>>>| {
            v.map(|v| v.iter().map(|x| x.map(f64::to_bits)).collect::<Vec<_>>())
        };
        ids.into_iter()
            .filter(|id| bits(a.metrics.get(*id)) != bits(b.metrics.get(*id)))
            .cloned()
            .collect()
    }

    /// A bot game written as a session replays to the same final save and exactly the same
    /// metrics and commands as the bot run measured them.
    #[test]
    fn bot_session_replays_to_the_same_metrics() {
        let (seed, hours) = (0, 12);
        let dir = tmp("bot");
        let persona = Persona::REFERENCE;
        let mut g = run::play_bot(seed, persona, hours);
        let used = crate::bot::USED.with(|u| u.borrow().clone());
        write_bot(&dir, seed, persona, hours, &mut g).unwrap();
        let bot = RunFile::collect(persona.name(), hours, vec![(seed, g.metrics, used)]);

        let (mut f, out) = replay_dirs(std::slice::from_ref(&dir), 1).unwrap();
        assert!(out[0].stale.is_none(), "{:?}", out[0].stale);
        assert_eq!(out[0].digest, proto::save_digest(&g.e.save()));
        assert_eq!(f.persona, "greedy");
        assert_eq!(f.hours, hours);
        assert_eq!(f.build, Some(idle_engine::content::fingerprint()));
        assert!(f.metrics.contains_key("owned.h12"));
        assert!(!f.metrics.keys().any(|k| k.starts_with("llm.")));
        // The one id a replay adds.
        assert_eq!(f.metrics.remove(STALE), Some(vec![Some(0.0)]));
        assert_eq!(differ(&bot, &f), Vec::<String>::new());
        assert_eq!(f.commands, bot.commands);
        let _ = fs::remove_dir_all(&dir);
    }

    fn scores(n: i64) -> BTreeMap<String, Score> {
        proto::DIMS
            .iter()
            .map(|d| {
                let s = Score {
                    score: n,
                    why: "scripted".into(),
                };
                (d.to_string(), s)
            })
            .collect()
    }

    /// Plays a session through the library API: the first menu item at every visit; feel delight,
    /// flat, none in turn; every dimension 3 when asked.
    fn play_scripted(dir: &Path, setup: &Setup) {
        let mut obs: Observation = proto::create(dir, setup).unwrap();
        let mut k = 0;
        while obs.need != "done" {
            let feel = ["delight", "flat", ""][k % 3];
            let a = Action {
                pick: obs
                    .menu
                    .first()
                    .map(|m| vec![m.id.clone()])
                    .unwrap_or_default(),
                feel: (!feel.is_empty()).then(|| Feel {
                    label: feel.into(),
                    text: "scripted".into(),
                }),
                scores: (obs.need == "scores").then(|| scores(3)),
                ..Action::default()
            };
            obs = proto::act(dir, &a).unwrap();
            k += 1;
        }
    }

    #[test]
    fn llm_session_replays_with_feedback_metrics() {
        let dir = tmp("llm");
        let setup = Setup {
            seed: 1,
            schedule: Schedule::Every(3),
            hours: 24,
            player: "scripted".into(),
            ..Setup::default()
        };
        play_scripted(&dir, &setup);
        let (f, out) = replay_dirs(std::slice::from_ref(&dir), 1).unwrap();
        assert!(out[0].stale.is_none(), "{:?}", out[0].stale);
        assert_eq!(
            (f.persona.as_str(), f.hours, f.seeds.as_slice()),
            ("llm:scripted", 24, &[1u64][..])
        );
        let one = |id: &str| f.metrics.get(id).and_then(|v| v[0]);
        // Visits at hours 0, 3, …, 24 are all day 1; every third has no feel.
        let feel: Vec<f64> = proto::FEELS
            .iter()
            .map(|l| one(&format!("llm.feel.d1.{l}")).unwrap())
            .collect();
        assert!(feel.iter().sum::<f64>() <= 1.0, "{feel:?}");
        assert_eq!(feel, vec![3.0 / 9.0, 3.0 / 9.0, 0.0]);
        for d in proto::DIMS {
            assert_eq!(one(&format!("llm.score.{d}.d1")), Some(3.0), "{d}");
        }
        assert_eq!(one("llm.invalid"), Some(0.0));
        assert_eq!(one(STALE), Some(0.0));
        let (felt, none) = (one("llm.delight_given_felt"), one("llm.delight_given_none"));
        assert!(felt.is_some() || none.is_some());
        assert_eq!(one("llm.delight_lift"), felt.zip(none).map(|(a, b)| a - b));
        assert!(one("owned.h12").is_some());
        assert!(one("payoff.hook_h").is_some());
        let reg = crate::registry::load();
        let bad: Vec<&String> = f
            .metrics
            .keys()
            .filter(|id| crate::registry::find(&reg, id).is_none())
            .collect();
        assert!(bad.is_empty(), "unregistered metric ids: {bad:?}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_changed_ending_is_stale() {
        let dir = tmp("stale");
        let setup = Setup {
            seed: 2,
            schedule: Schedule::Every(3),
            hours: 6,
            player: "scripted".into(),
            ..Setup::default()
        };
        play_scripted(&dir, &setup);
        let path = dir.join(proto::FINAL);
        let mut fin: Final = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        fin.save_sha = "0000000000000000".into();
        fs::write(&path, pretty(&fin)).unwrap();
        let (f, out) = replay_dirs(std::slice::from_ref(&dir), 1).unwrap();
        assert!(out[0].stale.is_some());
        assert!(f.metrics.keys().any(|k| k.starts_with("llm.score.")));
        assert_eq!(f.metrics.get(STALE), Some(&vec![Some(1.0)]));
        let other: Vec<&String> = f
            .metrics
            .keys()
            .filter(|k| {
                !k.starts_with("llm.feel.") && !k.starts_with("llm.score.") && k.as_str() != STALE
            })
            .collect();
        assert!(
            other.is_empty(),
            "a stale session keeps only feel and scores: {other:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    /// A finished-looking directory without playing: manifest, empty journal, final.json.
    fn fake(dir: &Path, protocol: u32, player: &str, seed: u64, finished: bool) {
        fs::create_dir_all(dir).unwrap();
        let mut m = serde_json::to_value(Manifest {
            protocol: proto::PROTOCOL,
            build: String::new(),
            seed,
            player: player.into(),
            schedule: "every:3".into(),
            hours: 6,
            lang: idle_engine::i18n::DEFAULT.into(),
            research: true,
        })
        .unwrap();
        m["protocol"] = protocol.into();
        fs::write(dir.join(proto::MANIFEST), m.to_string()).unwrap();
        fs::write(dir.join(proto::JOURNAL), "").unwrap();
        if finished {
            let mut e = idle_engine::textui::new_game(seed);
            let fin = Final {
                t: 6 * 3600,
                summary: proto::summarize(&mut e),
                save_sha: String::new(),
            };
            fs::write(dir.join(proto::FINAL), pretty(&fin)).unwrap();
        }
    }

    #[test]
    fn unreadable_or_mixed_sessions_are_refused() {
        let root = tmp("refuse");
        let d = |name: &str| root.join(name);
        fake(&d("p2"), 2, "a", 0, true);
        let err = load(&d("p2")).err().unwrap();
        assert!(err.contains("protocol 2"), "{err}");
        assert!(replay_dirs(&[d("p2")], 1).is_err());

        fake(&d("open"), proto::PROTOCOL, "a", 0, false);
        assert!(
            load(&d("open")).unwrap().is_none(),
            "no final.json: unfinished"
        );
        let err = replay_dirs(&[d("open")], 1).err().unwrap();
        assert!(err.contains("no finished session"), "{err}");

        fake(&d("a0"), proto::PROTOCOL, "a", 0, true);
        fake(&d("a0bis"), proto::PROTOCOL, "a", 0, true);
        fake(&d("b1"), proto::PROTOCOL, "b", 1, true);
        let err = replay_dirs(&[d("a0"), d("b1")], 1).err().unwrap();
        assert!(err.contains("several players"), "{err}");
        let err = replay_dirs(&[d("a0"), d("open"), d("a0bis")], 1)
            .err()
            .unwrap();
        assert!(err.contains("seed 0 twice"), "{err}");

        assert_eq!(persona_of("greedy@4"), "greedy@4");
        assert_eq!(persona_of("haiku"), "llm:haiku");
        let _ = fs::remove_dir_all(&root);
    }
}
