//! Text harness for an LLM player: prints the board as text with a numbered menu, takes menu
//! numbers (or raw JSON commands) and advances time. Same engine and save format as the client.
//!
//! ```text
//! agent_cli new <save> <seed>                     start a cycle (map 0, the bot's three rules on)
//! agent_cli show <save>                           board + menu
//! agent_cli do <save> [n ...] [--json '<cmd>'] [--wait H] [--note '<why>']
//!                                                 run menu items in order, then wait H hours (≤4)
//! agent_cli summary <save>                        one line of results
//! agent_cli autoplay <save> <seed> <hours> <every> scripted bot at the same cadence (baseline)
//! ```
//! The cycle stops at AGENT_HOURS (default 72). Every `do` is appended to `<save>.log` as JSONL.
use idle_engine::api::Engine;
use idle_engine::combat::Outcome;
use idle_engine::commands::Command;
use idle_engine::geom;
use idle_engine::view::{OpView, View};
use std::fs;
use std::io::Write;
#[path = "bot_policy.rs"]
mod bot_policy;

const T0: i64 = 1_700_000_000;
const MAX_WAIT: i64 = 4;

fn limit_h() -> i64 {
    std::env::var("AGENT_HOURS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(72)
}

struct Item {
    label: String,
    cmd: Command,
}

fn outcome(o: Option<Outcome>) -> &'static str {
    match o {
        Some(Outcome::Victory) => "胜",
        Some(Outcome::Planned) => "按计划收队",
        Some(Outcome::Emergency) => "紧急撤退",
        Some(Outcome::Wipe) => "全灭",
        None => "无战斗",
    }
}

fn tags(t: &[idle_engine::content::Tag]) -> String {
    t.iter().map(|x| x.name()).collect::<Vec<_>>().join("/")
}

fn terrain(t: &str) -> &str {
    match t {
        "city" => "城区",
        "rift" => "裂隙",
        "special" => "特殊格",
        "hq" => "总部",
        "barrier" => "屏障",
        _ => "未知",
    }
}

fn parse_label(s: &str) -> Option<u16> {
    let s = s.trim();
    let mut ch = s.chars();
    let col = ch.next()?.to_ascii_uppercase();
    let row: i32 = ch.as_str().parse().ok()?;
    geom::idx(col as i32 - 'A' as i32, row - 1)
}

/// Rows top to bottom; # owned, ! alert, * claimable, o unowned with enemies, . revealed empty,
/// ? unknown, X barrier, H headquarters.
fn map(v: &View) -> String {
    let n = geom::SIZE;
    let mut o = String::from("    ");
    for x in 0..n {
        o.push((b'A' + x as u8) as char);
    }
    o.push('\n');
    for y in 0..n {
        o += &format!("{:>3} ", y + 1);
        for x in 0..n {
            let t = &v.tiles[geom::idx(x, y).unwrap() as usize];
            let c = if t.terrain == "hq" {
                'H'
            } else if t.terrain == "barrier" {
                'X'
            } else if t.alert {
                '!'
            } else if t.owned {
                '#'
            } else if t.claimable {
                '*'
            } else if !t.revealed {
                '?'
            } else if !t.enemies.is_empty() {
                'o'
            } else {
                '.'
            };
            o.push(c);
        }
        o.push('\n');
    }
    o += "图例：H总部 #领地 !告急 *可占领 o有敌人 .已探明 ?未探明 X屏障\n";
    o
}

/// Let `--json` take board labels ("H7") wherever a tile index goes.
fn labels_to_index(j: &str) -> String {
    let Ok(mut v) = serde_json::from_str::<serde_json::Value>(j) else {
        return j.to_string();
    };
    fn fix(x: &mut serde_json::Value) {
        if let Some(t) = x.as_str().and_then(parse_label) {
            *x = serde_json::json!(t);
        }
    }
    if let Some(o) = v.as_object_mut() {
        if let Some(t) = o.get_mut("tile") {
            fix(t);
        }
        if let Some(serde_json::Value::Array(r)) = o.get_mut("route") {
            r.iter_mut().for_each(fix);
        }
    }
    v.to_string()
}

fn hour(e: &Engine, v: &View) -> i64 {
    (e.state.time - v.started_at) / 3600
}

fn load(path: &str) -> Engine {
    let s = fs::read_to_string(path).unwrap_or_else(|_| panic!("读不到存档 {path}"));
    Engine::load(s.trim()).expect("存档损坏")
}

fn save(e: &Engine, path: &str) {
    fs::write(path, e.save()).unwrap();
}

fn new_game(seed: u64) -> Engine {
    let mut e = Engine::new(seed, T0, "agent");
    e.command(&Command::ChooseMap { index: 0 }).unwrap();
    for r in ["auto_claim", "auto_recruit", "auto_counter"] {
        e.command(&Command::SetRule {
            rule: r.into(),
            on: true,
        })
        .unwrap();
    }
    e
}

/// Finished 见习任务 are free; both players claim them automatically.
fn claim_tasks(e: &mut Engine) -> Vec<String> {
    let done: Vec<(String, String)> = e
        .view()
        .tasks
        .open
        .iter()
        .filter(|t| t.done)
        .map(|t| (t.id.clone(), t.name.clone()))
        .collect();
    let mut out = vec![];
    for (id, name) in done {
        if e.command(&Command::ClaimTask { task: id }).is_ok() {
            out.push(name);
        }
    }
    out
}

fn op_name(v: &View, id: u32) -> String {
    v.operatives
        .iter()
        .find(|o| o.id == id)
        .map(|o| o.name.clone())
        .unwrap_or_else(|| format!("#{id}"))
}

fn op_line(o: &OpView) -> String {
    let d = &o.derived;
    let traits: Vec<String> = o
        .traits
        .iter()
        .map(|t| {
            let rw = if t.reads.is_empty() && t.writes.is_empty() {
                String::new()
            } else {
                format!("[读{}→写{}]", tags(&t.reads), tags(&t.writes))
            };
            format!("{}{}：{}", t.name, rw, t.desc)
        })
        .collect();
    let skills: Vec<String> = o
        .skills
        .iter()
        .map(|s| {
            let rw = if s.reads.is_empty() && s.writes.is_empty() {
                String::new()
            } else {
                format!("[读{}→写{}]", tags(&s.reads), tags(&s.writes))
            };
            format!(
                "{}{}{}：{}",
                if s.passive { "被动·" } else { "" },
                s.name,
                rw,
                s.desc
            )
        })
        .collect();
    format!(
        "  #{} {} {}·{} {} | 生命{} 攻{} 防{} 速{} | {}\n     词条：{}\n     技能：{}",
        o.id,
        o.name,
        o.rarity_name,
        o.class_name,
        if o.front { "前排" } else { "后排" },
        d.hp,
        d.atk,
        d.def,
        d.spd,
        o.status,
        if traits.is_empty() {
            "无".into()
        } else {
            traits.join("；")
        },
        if skills.is_empty() {
            "无".into()
        } else {
            skills.join("；")
        },
    )
}

fn op_short(o: &OpView) -> String {
    let mut r: Vec<idle_engine::content::Tag> = vec![];
    let mut w: Vec<idle_engine::content::Tag> = vec![];
    for (rs, ws) in o
        .skills
        .iter()
        .map(|s| (&s.reads, &s.writes))
        .chain(o.traits.iter().map(|t| (&t.reads, &t.writes)))
    {
        for t in rs {
            if !r.contains(t) {
                r.push(*t);
            }
        }
        for t in ws {
            if !w.contains(t) {
                w.push(*t);
            }
        }
    }
    let tr: Vec<String> = o.traits.iter().map(|t| t.name.clone()).collect();
    format!(
        "  #{} {} {}·{} {} 生命{} 攻{} 防{} 速{} | {} | 写{} 读{} | 词条 {}",
        o.id,
        o.name,
        o.rarity_name,
        o.class_name,
        if o.front { "前排" } else { "后排" },
        o.derived.hp,
        o.derived.atk,
        o.derived.def,
        o.derived.spd,
        o.status,
        if w.is_empty() { "—".into() } else { tags(&w) },
        if r.is_empty() { "—".into() } else { tags(&r) },
        if tr.is_empty() {
            "无".into()
        } else {
            tr.join("、")
        }
    )
}

fn info(e: &mut Engine) -> String {
    let v = e.view();
    let mut o = String::from("[队员详情]\n");
    for op in &v.operatives {
        o += &op_line(op);
        o += "\n";
    }
    if !v.candidates.is_empty() {
        o += "[候选详情]\n";
        for c in &v.candidates {
            o += &op_line(&c.op);
            o += "\n";
        }
    }
    o += "[设施]\n";
    for f in v.facilities.iter().filter(|f| f.visible) {
        o += &format!(
            "  「{}」岗位{} 耗电{} 废材{} 零件{}：{}\n",
            f.name, f.jobs, f.power, f.cost.scrap, f.cost.parts, f.desc
        );
    }
    o += "[政策]\n";
    for d in v.districts.iter().filter(|d| d.owned > 0) {
        for p in &d.policy_options {
            o += &format!(
                "  {}·「{}」：{}{}\n",
                d.name,
                p.name,
                p.desc,
                p.reason
                    .as_ref()
                    .map(|r| format!("（{r}）"))
                    .unwrap_or_default()
            );
        }
    }
    o
}

fn route_options(e: &mut Engine, squad: u32) -> Vec<(Vec<u16>, String)> {
    let v = e.view();
    let mut targets: Vec<_> = v
        .tiles
        .iter()
        .filter(|t| {
            t.revealed
                && !t.owned
                && !t.claimable
                && t.terrain != "barrier"
                && !t.enemies.is_empty()
        })
        .map(|t| (t.ring, t.i))
        .collect();
    targets.sort();
    let mut out = vec![];
    for (ring, t) in targets.iter().take(10) {
        let tv = &v.tiles[*t as usize];
        let foes: Vec<String> = tv
            .enemies
            .iter()
            .flatten()
            .map(|en| {
                if en.writes.is_empty() {
                    en.name.clone()
                } else {
                    format!("{}(会挂{})", en.name, tags(&en.writes))
                }
            })
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut best: Option<(Vec<u16>, idle_engine::api::RoutePreview)> = None;
        let mut tries = vec![vec![*t]];
        tries.extend(geom::neighbors(*t).map(|n| vec![n, *t]));
        for r in tries {
            let p = e.preview_route(squad, &r);
            if !p.ok {
                continue;
            }
            let win = p.outcome == Some(Outcome::Victory);
            let better = match &best {
                None => true,
                Some((_, b)) => win && b.outcome != Some(Outcome::Victory),
            };
            if better {
                best = Some((r, p));
            }
            if win {
                break;
            }
        }
        if let Some((r, p)) = best {
            let labels: Vec<String> = r.iter().map(|x| geom::label(*x)).collect();
            out.push((
                r,
                format!(
                    "路线 {}（目标第{}圈，镇守度{}，{}；敌人：{}；预演：{}，{}分钟，关注度{}→{}）",
                    labels.join("→"),
                    ring,
                    tv.warden,
                    terrain(tv.terrain),
                    if foes.is_empty() {
                        "未知".into()
                    } else {
                        foes.join("、")
                    },
                    outcome(p.outcome),
                    p.minutes,
                    p.attention_before,
                    p.attention_after
                ),
            ));
        }
        if out.len() >= 4 {
            break;
        }
    }
    out
}

fn menu(e: &mut Engine) -> Vec<Item> {
    let v = e.view();
    let mut m: Vec<Item> = vec![];
    // Research.
    let r = &v.research;
    let no_research = std::env::var("AGENT_NORESEARCH").is_ok();
    if no_research {
    } else if r.offers.is_empty() {
        if r.left > 0 && r.affordable {
            m.push(Item {
                label: format!("研究：花 {} 情报抽三条条令（之后会出现三选一）", r.cost),
                cmd: Command::Research,
            });
        }
    } else {
        for o in &r.offers {
            let links = if o.links_held.is_empty() {
                String::new()
            } else {
                format!("（接得上：{}）", o.links_held.join("、"))
            };
            m.push(Item {
                label: format!("要条令「{}」{}：{}", o.name, links, o.desc),
                cmd: Command::PickDoctrine {
                    doctrine: o.id.clone(),
                },
            });
        }
        if r.reroll_ok {
            m.push(Item {
                label: format!("换一批条令（{} 废材）", r.reroll_cost),
                cmd: Command::Reroll,
            });
        }
    }
    // Claims the auto rule leaves to the player.
    for p in v.pending.iter().filter(|p| p.kind == "claim") {
        if let Some(t) = p.tile {
            m.push(Item {
                label: format!("占领：{}", p.text),
                cmd: Command::Claim { tile: t },
            });
        }
    }
    // Counter alerts the auto rule did not take (it only takes predicted wins).
    let mut hopeless = vec![];
    for t in v.tiles.iter().filter(|t| t.alert) {
        let mut copy = Engine::from_state(e.state.clone());
        let ok = copy.command(&Command::Counter { tile: t.i }).is_ok();
        let fixed = ok && !copy.view().tiles[t.i as usize].alert;
        if !fixed {
            hopeless.push(t.label.clone());
            continue;
        }
        m.push(Item {
            label: format!(
                "反击告急格 {}（第{}圈；预演能赢，恢复镇守度）",
                t.label, t.ring
            ),
            cmd: Command::Counter { tile: t.i },
        });
    }
    if !hopeless.is_empty() {
        m.push(Item {
            label: format!(
                "（告急格 {} 现在反击打不赢；可以放弃其中一格：选这项放弃 {}）",
                hopeless.join(" "),
                hopeless[0]
            ),
            cmd: Command::Abandon {
                tile: parse_label(&hopeless[0]).unwrap(),
            },
        });
    }
    // Garrisons for tiles the forecast says will fall.
    let mut lost: Vec<u16> = v
        .tide
        .iter()
        .filter(|t| !t.held && !t.skipped)
        .map(|t| t.tile)
        .collect();
    lost.sort();
    lost.dedup();
    for tile in lost.iter().take(6) {
        let cur = v.tiles[*tile as usize].garrison;
        let opts = e.garrison_options(*tile);
        let mut n = 0;
        for o in opts.iter().filter(|o| o.held && !o.current) {
            let Some(id) = o.op else { continue };
            let busy = o
                .busy
                .clone()
                .map(|b| format!("，现在{b}"))
                .unwrap_or_default();
            m.push(Item {
                label: format!(
                    "驻守 {} ← {}（预演守得住，剩{}%生命{}{}）",
                    geom::label(*tile),
                    o.name,
                    o.hp_left_pct,
                    busy,
                    if cur.is_some() {
                        "，替换现驻守"
                    } else {
                        ""
                    }
                ),
                cmd: Command::Garrison {
                    op: id,
                    tile: Some(*tile),
                },
            });
            n += 1;
            if n >= 2 {
                break;
            }
        }
    }
    // Hires.
    for c in &v.candidates {
        let o = &c.op;
        let tr: Vec<String> = o.traits.iter().map(|t| t.name.clone()).collect();
        m.push(Item {
            label: format!(
                "招募候选 {}（{}·{}，词条：{}；{} 废材）",
                o.name,
                o.rarity_name,
                o.class_name,
                tr.join("、"),
                c.cost
            ),
            cmd: Command::Hire { candidate: c.id },
        });
    }
    // Builds: one suggested tile per affordable facility.
    for f in v.facilities.iter().filter(|f| f.visible && f.affordable) {
        let spot = v
            .tiles
            .iter()
            .filter(|t| t.owned && t.terrain == "city" && t.usage == "housing")
            .filter_map(|t| {
                let d = v.districts.iter().find(|d| Some(d.id) == t.district)?;
                Some((t.i, d.population - d.employed, d.name.clone()))
            })
            .filter(|(_, idle, _)| *idle > 0)
            .max_by_key(|(i, idle, _)| (*idle, std::cmp::Reverse(*i)));
        let Some((tile, idle, dname)) = spot else {
            continue;
        };
        let cmd = Command::Build {
            tile,
            facility: Some(f.id.clone()),
        };
        let dry = e.dry_run(&cmd);
        if !dry.ok {
            continue;
        }
        let deltas: Vec<String> = dry
            .deltas
            .iter()
            .map(|d| format!("{} {}→{}", d.label, d.before, d.after))
            .collect();
        let cost = &f.cost;
        let built = v.tiles.iter().any(|t| t.usage == f.id);
        m.push(Item {
            label: format!(
                "建「{}」于 {}（{}，闲置居民{}；岗位{}，耗电{}；花费 废材{} 零件{}；会变化：{}）{}",
                f.name,
                geom::label(tile),
                dname,
                idle,
                f.jobs,
                f.power,
                cost.scrap,
                cost.parts,
                if deltas.is_empty() {
                    "无".into()
                } else {
                    deltas.join("，")
                },
                if built {
                    String::new()
                } else {
                    format!("：{}", f.desc)
                }
            ),
            cmd,
        });
    }
    // Policies.
    for d in v.districts.iter().filter(|d| d.policy_unlocked) {
        for p in d.policy_options.iter().filter(|p| p.available) {
            if d.policy.as_deref() == Some(p.id.as_str()) {
                continue;
            }
            m.push(Item {
                label: format!("街区「{}」政策定为「{}」：{}", d.name, p.name, p.desc),
                cmd: Command::SetPolicy {
                    district: d.id,
                    policy: Some(p.id.clone()),
                },
            });
        }
    }
    // Routes.
    for s in &v.squads {
        if s.members.is_empty() {
            continue;
        }
        for (route, label) in route_options(e, s.id) {
            if route == s.route {
                continue;
            }
            m.push(Item {
                label: format!("{} 改走{}", s.name, label),
                cmd: Command::SetRoute { squad: s.id, route },
            });
        }
    }
    m
}

fn board(e: &mut Engine, since: i64) -> String {
    let v = e.view();
    let mut o = String::new();
    let h = hour(e, &v);
    let res = &v.resources;
    o += &format!(
        "== 第 {h} 小时 / 共 {} 小时（第 {} 轮）==\n",
        limit_h(),
        v.cycle
    );
    o += &format!(
        "资源：废材{} 零件{} 情报{} 晶体{}；每小时 +{:.1}/+{:.1}/+{:.1}/+{:.1}\n",
        res.scrap,
        res.parts,
        res.intel,
        res.crystal,
        v.income[0] as f64 / 100.0,
        v.income[1] as f64 / 100.0,
        v.income[2] as f64 / 100.0,
        v.income[3] as f64 / 100.0
    );
    o += &format!(
        "电力：供{} 需{}（效率{}%）；关注度{}（最深路线第{}圈，来潮强度×{}%）\n",
        v.power.0,
        v.power.1,
        v.power.2 / 100,
        v.attention.0,
        v.attention.1,
        v.attention.2 / 100
    );
    let owned = v.tiles.iter().filter(|t| t.owned).count();
    let alerts: Vec<String> = v
        .tiles
        .iter()
        .filter(|t| t.alert)
        .map(|t| t.label.clone())
        .collect();
    let st = &e.state.cycle.stats;
    o += &format!(
        "领地：{} 格，最远第{}圈，边界{}格；来潮累计 守住{} 失守{}；告急{}格{}\n",
        owned,
        st.max_ring,
        v.border_count,
        st.surges_held,
        st.surges_lost,
        alerts.len(),
        if alerts.is_empty() {
            String::new()
        } else {
            format!("（{}）", alerts.join(" "))
        }
    );
    let lost: Vec<String> = v
        .tide
        .iter()
        .filter(|t| !t.held && !t.skipped)
        .map(|t| {
            format!(
                "{}（{}小时后，第{}/{}潮守不住，镇守度{}→{}）",
                t.label,
                (t.t - v.time + 3599) / 3600,
                t.lost_wave.unwrap_or(0),
                t.total_waves,
                t.warden_before,
                t.warden_after
            )
        })
        .collect();
    o += &format!(
        "未来12小时来潮：{} 次，预报会失守 {} 次{}\n",
        v.tide.len(),
        lost.len(),
        if lost.is_empty() {
            String::new()
        } else {
            format!(
                "：{}",
                lost.into_iter().take(8).collect::<Vec<_>>().join("；")
            )
        }
    );
    o += &format!(
        "本轮成果（收束时换组织经验，越高越好）：{}（管理{} 防御{} 探索{}）\n",
        v.results.total, v.results.management, v.results.defense, v.results.exploration
    );
    if !v.results.lines.is_empty() {
        let l: Vec<String> = v
            .results
            .lines
            .iter()
            .map(|(k, n)| format!("{k}{n}"))
            .collect();
        o += &format!("  成果明细：{}\n", l.join("，"));
    }
    o += "\n[地图]\n";
    o += &map(&v);
    // Squads.
    o += "\n[小队]\n";
    for s in &v.squads {
        let members: Vec<String> = s.members.iter().map(|id| op_name(&v, *id)).collect();
        let route: Vec<String> = s.route.iter().map(|t| geom::label(*t)).collect();
        let last = s
            .last
            .as_ref()
            .map(|t| {
                let r: Vec<String> = t
                    .results
                    .iter()
                    .map(|x| format!("{}{}", x.label, outcome(Some(x.outcome))))
                    .collect();
                if r.is_empty() {
                    format!("上一趟：{}（这趟没有打仗）", outcome(Some(t.outcome)))
                } else {
                    format!("上一趟：{}（{}）", outcome(Some(t.outcome)), r.join(" "))
                }
            })
            .unwrap_or_else(|| "还没出过门".into());
        let state = if s.running.is_some() {
            "路上".to_string()
        } else if s.rest_until > v.time {
            format!("休整到 {} 分钟后", (s.rest_until - v.time) / 60)
        } else {
            "待命".into()
        };
        let stale = !s.route.is_empty()
            && s.route
                .iter()
                .all(|t| v.tiles[*t as usize].owned || v.tiles[*t as usize].claimable);
        o += &format!(
            "  {}（id {}）：{}；路线 {}{}；{}；{}\n",
            s.name,
            s.id,
            if members.is_empty() {
                "无人".into()
            } else {
                members.join("、")
            },
            if route.is_empty() {
                "未定".into()
            } else {
                route.join("→")
            },
            if stale {
                "（路线上的格子都已镇守度满或已占领，再走没有收益）"
            } else {
                ""
            },
            state,
            last
        );
    }
    let front: Vec<String> = v
        .tiles
        .iter()
        .filter(|t| {
            t.revealed
                && !t.owned
                && t.terrain != "barrier"
                && geom::neighbors(t.i).any(|n| v.tiles[n as usize].owned)
        })
        .map(|t| {
            format!(
                "{}({}圈{}，镇守{}{})",
                t.label,
                t.ring,
                terrain(t.terrain),
                t.warden,
                if t.claimable { "，可占领" } else { "" }
            )
        })
        .collect();
    o += &format!("  领地旁的格子：{}\n", front.join(" "));
    // Roster: one line each; `info` prints skills and traits in full.
    o += &format!(
        "\n[队员 {}/{}]（技能和词条全文：agent_cli info <存档>）\n",
        v.operatives.len(),
        v.roster_cap
    );
    for op in &v.operatives {
        o += &op_short(op);
        o += "\n";
    }
    // Doctrines and codex.
    let r = &v.research;
    o += &format!(
        "\n[条令] 已有 {} 条，研究池还剩 {} 条；下次研究 {} 情报（每研究一次贵 {}）\n",
        r.held.len(),
        r.left,
        r.cost,
        r.step
    );
    for d in &r.held {
        o += &format!("  「{}」：{}\n", d.name, d.desc);
    }
    if !r.offers.is_empty() {
        o += "  三选一等你挑（见菜单）\n";
    }
    if r.reactions_on {
        o += "[反应图鉴]（两种标签在同一个敌人身上相遇时触发）\n";
        for c in &r.codex {
            if c.seen {
                o += &format!(
                    "  {}+{} =「{}」：{}（本轮{}次）\n",
                    c.a.name, c.b.name, c.name, c.text, c.this_cycle
                );
            } else {
                o += &format!("  ？+？ 未点亮，传闻：{}\n", c.text);
            }
        }
    } else {
        o += "[反应图鉴] 未开启（需要条令「反应协议」）\n";
    }
    // Districts.
    o += "\n[街区]\n";
    let empty = v
        .districts
        .iter()
        .filter(|d| d.owned > 0 && d.population == 0 && d.housing == 0)
        .count();
    for d in v
        .districts
        .iter()
        .filter(|d| d.owned > 0 && (d.population > 0 || d.housing > 0))
    {
        o += &format!(
            "  {}：占{}/{}格，居民{}/住房{}{}，在岗{}，民兵{}，信任{}→{}{}\n",
            d.name,
            d.owned,
            d.total,
            d.population,
            d.housing,
            if d.unhoused > 0 {
                format!("（{} 人无处安置，会陆续离开）", d.unhoused)
            } else {
                String::new()
            },
            d.employed,
            d.militia,
            d.trust,
            d.trust_target,
            d.policy
                .as_ref()
                .map(|p| format!("，政策{p}"))
                .unwrap_or_default()
        );
    }
    if empty > 0 {
        o += &format!("  另有 {empty} 个街区只占了没有民居的格子，没有居民\n");
    }
    let mut built: std::collections::BTreeMap<String, usize> = Default::default();
    for t in v
        .tiles
        .iter()
        .filter(|t| t.owned && t.usage != "housing" && t.usage != "hq")
    {
        if t.terrain == "city" || t.usage != "none" {
            *built.entry(t.usage.clone()).or_default() += 1;
        }
    }
    let names: Vec<String> = built
        .iter()
        .map(|(k, n)| {
            let name = v
                .facilities
                .iter()
                .find(|f| &f.id == k)
                .map(|f| f.name.clone())
                .unwrap_or(k.clone());
            format!("{name}×{n}")
        })
        .collect();
    o += &format!(
        "  设施：{}\n",
        if names.is_empty() {
            "无".into()
        } else {
            names.join(" ")
        }
    );
    // Events since the last look.
    let ev: Vec<String> = v
        .events
        .iter()
        .filter(|x| x.t > since)
        .map(|x| x.text.clone())
        .collect();
    let n = ev.len();
    o += &format!("\n[这段时间发生的事] {} 条", n);
    if n > 12 {
        o += "，只列最近 12 条";
    }
    o += "\n";
    for t in ev.into_iter().skip(n.saturating_sub(12)) {
        o += &format!("  - {t}\n");
    }
    let pend: Vec<String> = v.pending.iter().map(|p| p.text.clone()).collect();
    if !pend.is_empty() {
        o += "[待你决定]\n";
        for p in pend.iter().take(8) {
            o += &format!("  - {p}\n");
        }
    }
    o
}

fn print_menu(e: &mut Engine, m: &[Item]) -> String {
    let v = e.view();
    let sq = v.squads.first().map(|s| s.id).unwrap_or(1);
    let mut o = String::from("\n[菜单] 用编号执行，可以一次写多个（按顺序执行）\n");
    for (i, it) in m.iter().enumerate() {
        o += &format!("  {}. {}\n", i + 1, it.label);
    }
    o += &format!(
        "  其他：--route {sq} H7,G7 让小队按格子走自定路线（相邻格，从领地边上出发）；\
         --json '{{\"cmd\":\"set_squad\",\"squad\":{sq},\"members\":[1,2,3]}}' 换小队成员（每队最多3人）；\
         --json '{{\"cmd\":\"garrison\",\"op\":2,\"tile\":null}}' 撤回驻守\n"
    );
    o
}

fn log(path: &str, entry: serde_json::Value) {
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(format!("{path}.log"))
        .unwrap();
    writeln!(f, "{entry}").unwrap();
}

fn summary(e: &mut Engine) -> String {
    let v = e.view();
    let st = e.state.cycle.stats.clone();
    let lit = v.research.codex.iter().filter(|c| c.seen).count();
    let held: Vec<String> = v.research.held.iter().map(|d| d.name.clone()).collect();
    format!(
        "h={} owned={} ring={} held={} lost={} alerts={} exp={} ops={} doctrines={} reactions={} lit={} scrap={} intel={} [{}]",
        hour(e, &v),
        v.tiles.iter().filter(|t| t.owned).count(),
        st.max_ring,
        st.surges_held,
        st.surges_lost,
        v.tiles.iter().filter(|t| t.alert).count(),
        v.results.total,
        v.operatives.len(),
        held.len(),
        e.state.cycle.reactions.values().sum::<u32>(),
        lit,
        v.resources.scrap,
        v.resources.intel,
        held.join(",")
    )
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let cmd = a.get(1).map(String::as_str).unwrap_or("");
    let path = a.get(2).cloned().unwrap_or_default();
    match cmd {
        "new" => {
            let seed: u64 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
            let mut e = new_game(seed);
            claim_tasks(&mut e);
            save(&e, &path);
            let t = e.state.time - 1;
            print!("{}", board(&mut e, t));
            let m = menu(&mut e);
            print!("{}", print_menu(&mut e, &m));
        }
        "show" => {
            let mut e = load(&path);
            let since = e.state.time - 4 * 3600;
            print!("{}", board(&mut e, since));
            let m = menu(&mut e);
            print!("{}", print_menu(&mut e, &m));
        }
        "info" => {
            let mut e = load(&path);
            print!("{}", info(&mut e));
        }
        "summary" => {
            let mut e = load(&path);
            println!("{}", summary(&mut e));
        }
        "do" => {
            let mut e = load(&path);
            let start = e.state.time;
            let m = menu(&mut e);
            let mut picks: Vec<usize> = vec![];
            let mut json: Vec<String> = vec![];
            let mut wait: i64 = 0;
            let mut note = String::new();
            let mut i = 3;
            while i < a.len() {
                match a[i].as_str() {
                    "--json" => {
                        json.push(a.get(i + 1).cloned().unwrap_or_default());
                        i += 1;
                    }
                    "--wait" => {
                        wait = a.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(0);
                        i += 1;
                    }
                    "--route" => {
                        let sq = a.get(i + 1).cloned().unwrap_or_default();
                        let tiles: Vec<String> = a
                            .get(i + 2)
                            .map(|r| {
                                r.split(',')
                                    .map(|x| match parse_label(x) {
                                        Some(t) => t.to_string(),
                                        None => "null".into(),
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        json.push(format!(
                            "{{\"cmd\":\"set_route\",\"squad\":{sq},\"route\":[{}]}}",
                            tiles.join(",")
                        ));
                        i += 2;
                    }
                    "--note" => {
                        note = a.get(i + 1).cloned().unwrap_or_default();
                        i += 1;
                    }
                    s => {
                        for part in s.split(',') {
                            if let Ok(n) = part.trim().parse::<usize>() {
                                picks.push(n);
                            }
                        }
                    }
                }
                i += 1;
            }
            let mut done = vec![];
            for n in &picks {
                match m.get(n.wrapping_sub(1)) {
                    Some(it) => {
                        let r = e.command(&it.cmd);
                        let line = match r {
                            Ok(()) => format!("✓ {}. {}", n, it.label),
                            Err(err) => format!("✗ {}. {}：{}", n, it.label, err),
                        };
                        println!("{line}");
                        done.push(line);
                    }
                    None => {
                        let line = format!("✗ 没有第 {n} 项");
                        println!("{line}");
                        done.push(line);
                    }
                }
            }
            for j in &json {
                let fixed = labels_to_index(j);
                let r = match serde_json::from_str::<Command>(&fixed) {
                    Ok(c) => e.command(&c),
                    Err(err) => Err(format!("命令格式不对：{err}")),
                };
                let line = match r {
                    Ok(()) => format!("✓ {j}"),
                    Err(err) => format!("✗ {j}：{err}"),
                };
                println!("{line}");
                done.push(line);
            }
            let v = e.view();
            let h = hour(&e, &v);
            let wait = wait.clamp(0, MAX_WAIT).min(limit_h() - h);
            if wait > 0 {
                let t = e.state.time;
                e.advance(t + wait * 3600);
                println!("…过了 {wait} 小时");
            }
            let claimed = claim_tasks(&mut e);
            if !claimed.is_empty() {
                println!("（自动领取见习任务：{}）", claimed.join("、"));
            }
            save(&e, &path);
            let sum = summary(&mut e);
            log(
                &path,
                serde_json::json!({"picks": picks, "json": json, "wait": wait, "note": note,
                    "results": done, "summary": sum}),
            );
            println!();
            print!("{}", board(&mut e, start));
            let v = e.view();
            if hour(&e, &v) >= limit_h() {
                println!("\n== 时间到。最终：{} ==", summary(&mut e));
            } else {
                let m = menu(&mut e);
                print!("{}", print_menu(&mut e, &m));
            }
        }
        "autoplay" => {
            let seed: u64 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(0);
            let hours: i64 = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(72);
            let every: i64 = a.get(5).and_then(|s| s.parse().ok()).unwrap_or(4);
            let mut e = new_game(seed);
            let mut h = 0;
            while h < hours {
                bot_policy::step(&mut e);
                let t = e.state.time;
                e.advance(t + every * 3600);
                h += every;
                claim_tasks(&mut e);
                if h % 24 == 0 || h >= hours {
                    println!("seed {seed} {}", summary(&mut e));
                }
            }
            if !path.is_empty() && path != "-" {
                save(&e, &path);
            }
        }
        _ => {
            eprintln!("用法见文件头注释");
            std::process::exit(2);
        }
    }
}
