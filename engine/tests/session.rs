//! Play-session protocol (docs/eval-interface.md, `textui::session`): a mock player plays through
//! the library API and through `serve`, the journal replays to the same final save, and refused
//! actions leave the directory as it was.
use idle_engine::client::textui::session::{
    self, Action, Feel, Final, Manifest, Observation, Schedule, Score, Setup, DIMS, FINAL, JOURNAL,
    MANIFEST, PROTOCOL, SAVE, VISITS,
};
use idle_engine::client::textui::{self};
use idle_engine::mech::commands::Command;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("idle-session-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    d
}

fn scores(n: i64) -> BTreeMap<String, Score> {
    DIMS.iter()
        .map(|d| {
            (
                d.to_string(),
                Score {
                    score: n,
                    why: "mock".into(),
                },
            )
        })
        .collect()
}

/// First menu id, feel "flat", every score 3.
fn mock(obs: &Observation) -> Action {
    Action {
        pick: obs
            .menu
            .first()
            .map(|m| vec![m.id.clone()])
            .unwrap_or_default(),
        feel: Some(Feel {
            label: "flat".into(),
            text: "mock".into(),
        }),
        scores: (obs.need == "scores").then(|| scores(3)),
        ..Action::default()
    }
}

fn json_lines(path: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn keys(v: &serde_json::Value) -> Vec<String> {
    let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
    k.sort();
    k
}

fn has_keys(v: &serde_json::Value, want: &[&str]) {
    let k = keys(v);
    for w in want {
        assert!(k.iter().any(|x| x == w), "missing {w} in {k:?}");
    }
}

fn snapshot(dir: &Path) -> Vec<String> {
    [MANIFEST, SAVE, JOURNAL, VISITS]
        .iter()
        .map(|f| fs::read_to_string(dir.join(f)).unwrap_or_default())
        .collect()
}

#[test]
fn mock_player_plays_a_week_and_the_journal_replays() {
    let dir = tmp("week");
    let setup = Setup {
        seed: 0,
        player: "mock".into(),
        ..Setup::default()
    };
    let mut obs = session::create(&dir, &setup).unwrap();
    let plan = Schedule::WeekDecay.visits(session::WEEK_HOURS);
    let (mut hours, mut scored) = (vec![], vec![]);
    while obs.need != "done" {
        assert_eq!(obs.protocol, PROTOCOL);
        assert_eq!(obs.visit, hours.len() + 1);
        assert_eq!(obs.t, obs.hour * 3600);
        assert!(!obs.text.contains("cli."), "unresolved key:\n{}", obs.text);
        hours.push(obs.hour);
        if obs.need == "scores" {
            assert_eq!(obs.score_dims.len(), DIMS.len());
            scored.push((obs.hour, obs.score_stage.clone().unwrap()));
        } else {
            assert_eq!(obs.need, "act");
            assert!(obs.score_dims.is_empty());
        }
        let next = session::act(&dir, &mock(&obs)).unwrap();
        if next.need != "done" {
            assert_eq!(Some(next.hour), obs.next_hour);
        }
        obs = next;
    }
    assert_eq!(hours, plan, "visits follow the schedule");
    let want: Vec<(i64, String)> =
        vec![(24, "d1".into()), (72, "d2_3".into()), (168, "d4_7".into())];
    assert_eq!(
        scored, want,
        "scores at the first visit after each stage and at the end"
    );
    assert!(
        session::act(&dir, &mock(&obs)).is_err(),
        "done takes no action"
    );

    // The directory, as docs/eval-interface.md lays it out.
    let m: Manifest =
        serde_json::from_str(&fs::read_to_string(dir.join(MANIFEST)).unwrap()).unwrap();
    assert_eq!(m.build, idle_engine::mech::fingerprint());
    assert_eq!(
        (m.seed, m.hours, m.schedule.as_str()),
        (0, 168, "week-decay")
    );
    let visits = json_lines(&dir.join(VISITS));
    assert_eq!(visits.len(), plan.len());
    for (i, v) in visits.iter().enumerate() {
        has_keys(
            v,
            &[
                "visit", "t", "stage", "actions", "feel", "scores", "summary",
            ],
        );
        assert_eq!(v["visit"], i + 1);
        assert_eq!(v["t"], plan[i] * 3600);
        assert_eq!(v["feel"]["label"], "flat");
        let rated = want.iter().any(|(h, _)| *h == plan[i]);
        assert_eq!(v["scores"].is_object(), rated, "visit at hour {}", plan[i]);
        if rated {
            assert_eq!(v["scores"].as_object().unwrap().len(), DIMS.len());
        }
        for a in v["actions"].as_array().unwrap() {
            has_keys(a, &["id", "cmd", "ok", "err"]);
        }
    }
    let fin_v: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.join(FINAL)).unwrap()).unwrap();
    assert_eq!(keys(&fin_v), ["save_sha", "summary", "t"]);
    let fin: Final = serde_json::from_value(fin_v).unwrap();
    assert_eq!(fin.t, 168 * 3600);

    // Replaying the journal from the seed gives the same final save.
    let journal = session::read_journal(&dir).unwrap();
    assert!(journal.windows(2).all(|w| w[0].t <= w[1].t));
    let picks = visits
        .iter()
        .flat_map(|v| v["actions"].as_array().unwrap().iter())
        .filter(|a| a["ok"] == true)
        .count();
    assert!(
        journal.len() > picks,
        "setup, picks and claimed tasks are all journaled"
    );
    let mut e = session::replay(m.seed, &journal, fin.t).unwrap();
    let save = e.save();
    assert_eq!(session::save_digest(&save), fin.save_sha);
    assert_eq!(save, fs::read_to_string(dir.join(SAVE)).unwrap());
    assert_eq!(session::summarize(&mut e), fin.summary);
    let _ = fs::remove_dir_all(&dir);
}

/// A session starts exactly where `textui::new_game` (plus claimed tasks) does, and its journal
/// starts with the setup commands, so a replay needs nothing but the seed.
#[test]
fn session_start_matches_new_game() {
    let dir = tmp("start");
    let setup = Setup {
        seed: 3,
        ..Setup::default()
    };
    session::create(&dir, &setup).unwrap();
    let journal = session::read_journal(&dir).unwrap();
    assert!(matches!(journal[0].cmd, Command::ChooseMap { index: 0 }));
    assert!(journal.iter().all(|x| x.t == 0));
    let mut plain = textui::new_game(3);
    textui::claim_tasks(&mut plain);
    assert_eq!(
        session::replay(3, &journal, 0).unwrap().save(),
        plain.save()
    );
    assert_eq!(fs::read_to_string(dir.join(SAVE)).unwrap(), plain.save());
    assert!(
        session::create(&dir, &setup).is_err(),
        "no overwriting a session"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn refused_actions_change_nothing() {
    let dir = tmp("refuse");
    let setup = Setup {
        hours: 30,
        ..Setup::default()
    };
    let first = session::create(&dir, &setup).unwrap();
    assert!(!first.menu.is_empty());
    let before = snapshot(&dir);
    let bad_id = Action {
        pick: vec![first.menu[0].id.clone(), "nothere".into()],
        ..Action::default()
    };
    let err = session::act(&dir, &bad_id).unwrap_err();
    assert!(err.contains("nothere"), "{err}");
    let bad_feel = Action {
        feel: Some(Feel {
            label: "meh".into(),
            text: String::new(),
        }),
        ..Action::default()
    };
    assert!(session::act(&dir, &bad_feel).is_err());
    assert_eq!(snapshot(&dir), before, "a refused action writes nothing");
    let again = session::observe(&dir).unwrap();
    assert_eq!(again.visit, 1);
    let ids = |o: &Observation| o.menu.iter().map(|m| m.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&again), ids(&first), "menu ids are stable");

    // Up to hour 24, where day 1 must be scored first.
    let mut obs = again;
    while obs.need != "scores" {
        obs = session::act(&dir, &Action::default()).unwrap();
    }
    assert_eq!((obs.hour, obs.score_stage.as_deref()), (24, Some("d1")));
    let before = snapshot(&dir);
    let mut partial = scores(3);
    partial.remove("clarity");
    let mut high = scores(3);
    high.get_mut("flow").unwrap().score = 6;
    let mut extra = scores(3);
    extra.insert(
        "fun".into(),
        Score {
            score: 3,
            why: String::new(),
        },
    );
    for s in [None, Some(partial), Some(high), Some(extra)] {
        let a = Action {
            scores: s,
            ..Action::default()
        };
        assert!(session::act(&dir, &a).is_err());
    }
    assert_eq!(snapshot(&dir), before);

    // A command the engine refuses is a failed item of the visit, not a refused action.
    let a = Action {
        commands: vec![
            serde_json::json!({"cmd": "claim", "tile": 0}),
            "{oops".into(),
        ],
        scores: Some(scores(4)),
        ..Action::default()
    };
    let next = session::act(&dir, &a).unwrap();
    assert_eq!(next.hour, 28);
    assert_eq!(next.results.len(), 2);
    let last = json_lines(&dir.join(VISITS)).pop().unwrap();
    assert_eq!(last["actions"][0]["ok"], false);
    assert_eq!(last["actions"][1]["cmd"], "{oops");
    assert_eq!(last["scores"]["growth"]["score"], 4);
    assert_eq!(last["score_stage"], "d1");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn serve_round_trip() {
    let dir = tmp("serve");
    let setup = Setup {
        schedule: Schedule::Every(6),
        hours: 30,
        player: "mock".into(),
        ..Setup::default()
    };
    session::create(&dir, &setup).unwrap();
    // Scores on every line: ignored where none are asked for.
    let good = serde_json::to_string(&Action {
        feel: Some(Feel {
            label: "delight".into(),
            text: "mock".into(),
        }),
        scores: Some(scores(2)),
        ..Action::default()
    })
    .unwrap();
    let mut input = String::from("not json\n{\"picks\": [\"abc\"]}\n");
    for _ in 0..10 {
        input += &good;
        input.push('\n');
    }
    let mut out = vec![];
    session::serve(&dir, input.as_bytes(), &mut out).unwrap();
    let obs: Vec<Observation> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).unwrap();
            has_keys(
                &v,
                &[
                    "protocol",
                    "visit",
                    "t",
                    "hour",
                    "hours",
                    "stage",
                    "next_hour",
                    "need",
                    "text",
                    "menu",
                    "results",
                    "feel_labels",
                    "score_dims",
                ],
            );
            serde_json::from_value(v).unwrap()
        })
        .collect();
    let seen: Vec<(usize, i64, &str)> = obs
        .iter()
        .map(|o| (o.visit, o.hour, o.need.as_str()))
        .collect();
    assert_eq!(
        seen,
        vec![
            (1, 0, "act"),
            (1, 0, "act"),
            (1, 0, "act"),
            (2, 6, "act"),
            (3, 12, "act"),
            (4, 18, "act"),
            (5, 24, "scores"),
            (6, 30, "scores"),
            (6, 30, "done"),
        ]
    );
    assert_eq!(
        obs[1].results.len(),
        1,
        "a malformed line comes back as a result"
    );
    assert_eq!(obs[2].results.len(), 1, "so does an unknown field");
    assert_eq!(obs[6].score_stage.as_deref(), Some("d1"));
    assert_eq!(obs[7].score_stage.as_deref(), Some("d2_3"));
    assert!(
        obs[7].menu.is_empty(),
        "the last visit only takes feel and scores"
    );
    let visits = json_lines(&dir.join(VISITS));
    let rated: Vec<bool> = visits.iter().map(|v| v["scores"].is_object()).collect();
    assert_eq!(rated, [false, false, false, false, true, true]);
    assert!(dir.join(FINAL).exists());
    let _ = fs::remove_dir_all(&dir);
}
