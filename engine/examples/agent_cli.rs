//! Command line for the text client (`idle_engine::textui`): prints the board and a numbered
//! menu, runs menu numbers or JSON commands, and advances time. Same engine and save format as
//! the web client. `agent_cli` with no arguments prints the usage. Every `do` is appended to
//! `<save>.log` as JSONL. Settings: AGENT_HOURS (default 72), AGENT_NORESEARCH=1 (control group),
//! AGENT_LANG (default zh-CN).
use idle_engine::api::Engine;
use idle_engine::t;
use idle_engine::textui::{self, Options, MAX_WAIT};
use std::fs;
use std::io::Write;
#[path = "bot_policy.rs"]
mod bot_policy;

fn options() -> Options {
    Options {
        hours: std::env::var("AGENT_HOURS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(72),
        research: std::env::var("AGENT_NORESEARCH").is_err(),
    }
}

fn fail(msg: String) -> ! {
    eprintln!("{msg}");
    std::process::exit(2);
}

fn load(path: &str) -> Engine {
    let s = fs::read_to_string(path).unwrap_or_else(|_| fail(t!("cli.run.no_save", path = path)));
    Engine::load(s.trim()).unwrap_or_else(|e| fail(t!("cli.run.bad_save", err = e)))
}

fn save(e: &Engine, path: &str) {
    fs::write(path, e.save()).unwrap();
}

fn log(path: &str, entry: serde_json::Value) {
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(format!("{path}.log"))
        .unwrap();
    writeln!(f, "{entry}").unwrap();
}

fn show(e: &mut Engine, since: i64, opts: &Options) {
    print!("{}", textui::board(e, since, opts));
    let m = textui::menu(e, opts);
    print!("{}", textui::menu_text(e, &m));
}

fn main() {
    if let Ok(lang) = std::env::var("AGENT_LANG") {
        idle_engine::i18n::set_locale(&lang).unwrap_or_else(|e| fail(e));
    }
    let opts = options();
    let a: Vec<String> = std::env::args().collect();
    let cmd = a.get(1).map(String::as_str).unwrap_or("");
    let path = a.get(2).cloned().unwrap_or_default();
    let num = |i: usize, d: i64| a.get(i).and_then(|s| s.parse().ok()).unwrap_or(d);
    match cmd {
        "new" => {
            let mut e = textui::new_game(num(3, 0) as u64);
            textui::claim_tasks(&mut e);
            save(&e, &path);
            let since = e.state.time - 1;
            show(&mut e, since, &opts);
        }
        "show" => {
            let mut e = load(&path);
            let since = e.state.time - MAX_WAIT * 3600;
            show(&mut e, since, &opts);
        }
        "info" => print!("{}", textui::info(&mut load(&path))),
        "summary" => println!("{}", textui::summary(&mut load(&path))),
        "do" => {
            let mut e = load(&path);
            let start = e.state.time;
            let m = textui::menu(&mut e, &opts);
            let (mut picks, mut json) = (Vec::<usize>::new(), Vec::<String>::new());
            let (mut wait, mut note) = (0i64, String::new());
            let mut i = 3;
            while i < a.len() {
                let next = a.get(i + 1).cloned().unwrap_or_default();
                match a[i].as_str() {
                    "--json" => {
                        json.push(next);
                        i += 1;
                    }
                    "--wait" => {
                        wait = next.parse().unwrap_or(0);
                        i += 1;
                    }
                    "--note" => {
                        note = next;
                        i += 1;
                    }
                    "--route" => {
                        let route: Vec<serde_json::Value> = a
                            .get(i + 2)
                            .map(|r| r.split(',').map(|x| x.trim().into()).collect())
                            .unwrap_or_default();
                        let squad: serde_json::Value =
                            next.parse::<u32>().map(Into::into).unwrap_or(next.into());
                        json.push(
                            serde_json::json!({"cmd": "set_route", "squad": squad, "route": route})
                                .to_string(),
                        );
                        i += 2;
                    }
                    s => picks.extend(s.split(',').filter_map(|p| p.trim().parse::<usize>().ok())),
                }
                i += 1;
            }
            let mut done = vec![];
            for n in &picks {
                let line = match m.get(n.wrapping_sub(1)) {
                    Some(it) => match textui::run(&mut e, &it.cmd, &opts) {
                        Ok(()) => t!("cli.run.ok", n = n, label = it.label),
                        Err(err) => t!("cli.run.fail", n = n, label = it.label, err = err),
                    },
                    None => t!("cli.run.no_item", n = n),
                };
                println!("{line}");
                done.push(line);
            }
            for j in &json {
                let r = textui::parse_json(j).and_then(|c| textui::run(&mut e, &c, &opts));
                let line = match r {
                    Ok(()) => t!("cli.run.json_ok", json = j),
                    Err(err) => t!("cli.run.json_fail", json = j, err = err),
                };
                println!("{line}");
                done.push(line);
            }
            let v = e.view();
            let h = textui::hour(&e, &v);
            let wait = wait.clamp(0, MAX_WAIT).min(opts.hours - h).max(0);
            if wait > 0 {
                let now = e.state.time;
                e.advance(now + wait * 3600);
                println!("{}", t!("cli.run.waited", h = wait));
            }
            let claimed = textui::claim_tasks(&mut e);
            if !claimed.is_empty() {
                let tasks = claimed.join(&t!("cli.sep.list"));
                println!("{}", t!("cli.run.tasks_claimed", tasks = tasks));
            }
            save(&e, &path);
            let sum = textui::summary(&mut e);
            log(
                &path,
                serde_json::json!({"picks": picks, "json": json, "wait": wait, "note": note,
                    "results": done, "summary": sum}),
            );
            println!();
            let v = e.view();
            if textui::hour(&e, &v) >= opts.hours {
                print!("{}", textui::board(&mut e, start, &opts));
                println!("{}", t!("cli.run.time_up", summary = sum));
            } else {
                show(&mut e, start, &opts);
            }
        }
        "autoplay" => {
            let seed = num(3, 0) as u64;
            let (hours, every) = (num(4, 72), num(5, 4));
            let mut e = textui::new_game(seed);
            let mut h = 0;
            while h < hours {
                bot_policy::step(&mut e);
                let now = e.state.time;
                e.advance(now + every * 3600);
                h += every;
                textui::claim_tasks(&mut e);
                if h % 24 == 0 || h >= hours {
                    println!("seed {seed} {}", textui::summary(&mut e));
                }
            }
            if !path.is_empty() && path != "-" {
                save(&e, &path);
            }
        }
        _ => fail(t!("cli.run.usage")),
    }
}
