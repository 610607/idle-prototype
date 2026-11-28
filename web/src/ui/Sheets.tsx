import { useMemo, useState } from 'react';
import { t, tn } from '../i18n';
import { Icon } from '../icons';
import type { Cond, DistrictView, DryRun, Gambit, OpView, Target } from '../types';
import { Bar, DryRunView, fmtIn, OUTCOME_NAMES, Page, Rarity, RES_KEYS, RES_NAMES, Section, SQUAD_HEX, TagIO, useCtx } from './common';
import { OpRow, TideRows } from './Panels';

// ---------------------------------------------------------------------------
// Gambit wording

function condText(c: Cond, tagName: (t: string) => string): string {
  switch (c.t) {
    case 'always':
      return t('sheets.cond.always');
    case 'self_hp_below':
      return t('sheets.cond.self_hp_below', { pct: c.pct });
    case 'self_hp_above':
      return t('sheets.cond.self_hp_above', { pct: c.pct });
    case 'ally_hp_below':
      return t('sheets.cond.ally_hp_below', { pct: c.pct });
    case 'enemy_has_tag':
      return t('sheets.cond.enemy_has_tag', { tag: tagName(c.tag) });
    case 'self_has_tag':
      return t('sheets.cond.self_has_tag', { tag: tagName(c.tag) });
    case 'enemy_tag_at_least':
      return t('sheets.cond.enemy_tag_at_least', { tag: tagName(c.tag), n: c.n });
    case 'any_tag_at_least':
      return t('sheets.cond.any_tag_at_least', { n: c.n });
    case 'enemy_count_at_least':
      return t('sheets.cond.enemy_count_at_least', { n: c.n });
    case 'wave_at_least':
      return t('sheets.cond.wave_at_least', { n: c.n });
    case 'round_at_least':
      return t('sheets.cond.round_at_least', { n: c.n });
  }
}

const TARGET_NAMES: Record<Target, string> = {
  default: t('sheets.target.default'),
  front_lowest: t('sheets.target.front_lowest'),
  back_first: t('sheets.target.back_first'),
  highest_atk: t('sheets.target.highest_atk'),
  with_tag: t('sheets.target.with_tag'),
  lowest_ally: t('sheets.target.lowest_ally'),
  self: t('sheets.target.self'),
  most_tags: t('sheets.target.most_tags'),
};

// ---------------------------------------------------------------------------

export function OpSheet({ op, onClose }: { op: number; onClose: () => void }) {
  const { view } = useCtx();
  const o = view.operatives.find((x) => x.id === op);
  if (!o)
    return (
      <Page title={t('sheets.op.title')} onClose={onClose}>
        <p className="muted">{t('sheets.op.missing')}</p>
      </Page>
    );
  return (
    <Page
      title={
        <span className="row" style={{ gap: 8 }}>
          {o.name} <Rarity r={o.rarity} name={o.rarity_name} />
          <span className="muted small">{o.class_name}</span>
        </span>
      }
      icon={o.icon}
      onClose={onClose}
      aside={
        <>
          <AssignBlock o={o} />
          <EquipBlock o={o} />
          <GambitEditor o={o} />
          <OpDanger o={o} onClose={onClose} />
        </>
      }
    >
      <OpDetail o={o} />
    </Page>
  );
}

export function OpDetail({ o }: { o: OpView }) {
  const { game } = useCtx();
  const d = o.derived;
  return (
    <>
      <div className="small muted">
        {o.origin} · {o.status}
        {o.special ? t('sheets.op.special') : ''}
      </div>
      <div className="stat-tiles">
        {(
          [
            ['hp', t('sheets.stat.hp'), d.hp],
            ['atk', t('sheets.stat.atk'), d.atk],
            ['def', t('sheets.stat.def'), d.def],
            ['spd', t('sheets.stat.spd'), d.spd],
            ['ap', t('sheets.stat.ap'), d.ap],
            ['pp', t('sheets.stat.pp'), d.pp],
          ] as const
        ).map(([id, k, v]) => (
          <div key={id} className="stat-tile">
            <span className="k">
              <Icon id={id} /> {k}
            </span>
            <span className="num">{v}</span>
          </div>
        ))}
      </div>
      <details>
        <summary className="small muted" style={{ cursor: 'pointer' }}>
          {t('sheets.op.stats')}
        </summary>
        <div className="statgrid" style={{ marginTop: 4 }}>
          {game.content.stats.map(([id, name], k) => (
            <span key={id} className="small">
              {name} <span className="num">{o.stats[k]}</span>
            </span>
          ))}
        </div>
      </details>
      <Section title={t('sheets.op.traits')} icon="trait" tone="hazard" hint={t('sheets.op.traits_hint')}>
        <div className="stack" style={{ gap: 6 }}>
          {o.traits.map((tr) => (
            <div key={tr.id} className={'trait' + (tr.exp ? ' exp' : '')}>
              <div className="row" style={{ gap: 6 }}>
                <Icon id={tr.icon} size={16} />
                <b>{tr.name}</b>
                {tr.exp ? <span className="chip warn">{t('sheets.op.exp')}</span> : null}
                <span className="grow" />
                <TagIO reads={tr.reads} writes={tr.writes} />
              </div>
              <div className="small muted">{tr.desc}</div>
            </div>
          ))}
          {o.growing.map((g) => (
            <div key={g.id} className="trait" style={{ borderStyle: 'dashed' }}>
              <div className="row small">
                <Icon id="talent" size={14} />
                <span>{t('sheets.op.growing', { name: g.name })}</span>
                <span className="grow" />
                <span className="num faint">
                  {g.have}/{g.need}
                </span>
              </div>
              <div className="tiny faint">{g.hint}</div>
              <Bar value={g.have} max={g.need} tone="warn" />
            </div>
          ))}
        </div>
      </Section>
      <Section title={t('sheets.op.skills')} icon="skill" tone="rust">
        <div className="stack" style={{ gap: 6 }}>
          {o.skills.map((s) => (
            <div key={s.id} className="trait">
              <div className="row" style={{ gap: 6 }}>
                <Icon id={s.icon} size={16} />
                <b>{s.name}</b>
                <span className="chip">{s.passive ? t('sheets.op.skill_passive') : t('sheets.op.skill_active')}</span>
                <span className="grow" />
                <TagIO reads={s.reads} writes={s.writes} />
              </div>
              <div className="small muted">{s.desc}</div>
            </div>
          ))}
        </div>
      </Section>
    </>
  );
}

function AssignBlock({ o }: { o: OpView }) {
  const { view, run, open } = useCtx();
  const a = o.assignment;
  return (
    <Section title={t('sheets.assign.title')} icon="squad" tone="elec" hint={o.status}>
      <div className="row wrap">
        {view.squads.map((s, i) => {
          const inIt = s.members.includes(o.id);
          const full = !inIt && s.members.length >= 3;
          return (
            <button
              key={s.id}
              className={'btn small' + (inIt ? ' selected' : '')}
              disabled={full}
              style={{ borderColor: SQUAD_HEX[i] }}
              onClick={() =>
                run(
                  { cmd: 'set_squad', squad: s.id, members: inIt ? s.members.filter((m) => m !== o.id) : [...s.members, o.id] },
                  inIt ? t('sheets.assign.left', { op: o.name, squad: s.name }) : t('sheets.assign.joined', { op: o.name, squad: s.name }),
                )
              }
            >
              {inIt ? t('sheets.assign.leave', { squad: s.name }) : t('sheets.assign.join', { squad: s.name })}
              {full ? t('sheets.assign.full') : ''}
            </button>
          );
        })}
        {a.t === 'garrison' ? (
          <>
            <button className="btn small" onClick={() => open({ kind: 'tile', tile: a.tile })}>
              {t('sheets.assign.goto', { tile: view.tiles[a.tile]?.label ?? '' })}
            </button>
            <button className="btn small" onClick={() => run({ cmd: 'garrison', op: o.id, tile: null }, t('sheets.assign.unguarded'))}>
              {t('sheets.assign.unguard')}
            </button>
          </>
        ) : (
          <span className="small faint">{t('sheets.assign.garrison_hint')}</span>
        )}
      </div>
    </Section>
  );
}

function EquipBlock({ o }: { o: OpView }) {
  const { view, run } = useCtx();
  return (
    <Section title={t('sheets.equip.title')} icon="equip" tone="rust">
      {o.equipment ? (
        <div className="trait">
          <div className="row" style={{ gap: 6 }}>
            <Icon id={o.equipment.icon} size={16} />
            <b>{o.equipment.name}</b>
            <span className="grow" />
            <TagIO reads={o.equipment.reads} writes={o.equipment.writes} />
          </div>
          <div className="small muted">{o.equipment.desc}</div>
          <button className="btn small" style={{ marginTop: 4 }} onClick={() => run({ cmd: 'equip', op: o.id, item: null }, t('sheets.equip.removed'))}>
            {t('sheets.equip.remove')}
          </button>
        </div>
      ) : (
        <div className="empty">{t('sheets.equip.none')}</div>
      )}
      {view.inventory?.length ? (
        <div className="row wrap" style={{ marginTop: 6 }}>
          {view.inventory.map((it, k) => (
            <button key={k} className="btn small" onClick={() => run({ cmd: 'equip', op: o.id, item: it.id }, t('sheets.equip.equipped', { item: it.name }))} title={it.desc}>
              <Icon id={it.icon} size={14} /> {it.name}
            </button>
          ))}
        </div>
      ) : null}
    </Section>
  );
}

function GambitEditor({ o }: { o: OpView }) {
  const { game, run } = useCtx();
  const [list, setList] = useState<Gambit[]>(o.gambits);
  const dirty = JSON.stringify(list) !== JSON.stringify(o.gambits);
  const tagName = (t: string) => game.content.tags.find((x) => x.id === t)?.name ?? t;
  const skillName = (id: string) => game.content.skills.find((s) => s.id === id)?.name ?? id;
  const on = list.filter((g) => g.on).length;
  const move = (k: number, d: number) => {
    const j = k + d;
    if (j < 0 || j >= list.length) return;
    const next = [...list];
    [next[k], next[j]] = [next[j]!, next[k]!];
    setList(next);
  };
  return (
    <Section title={t('sheets.gambit.title')} icon="rules" hint={t('sheets.gambit.hint', { on, slots: o.derived.tactic_slots })}>
      <div className="small faint" style={{ marginBottom: 6 }}>
        {t('sheets.gambit.order')}
      </div>
      <div className="stack" style={{ gap: 4 }}>
        {list.map((g, k) => (
          <div key={k} className={'gambit' + (g.on ? '' : ' off')}>
            <input
              type="checkbox"
              checked={g.on}
              onChange={(e) => setList(list.map((x, j) => (j === k ? { ...x, on: e.target.checked } : x)))}
              aria-label={t('sheets.gambit.enable')}
            />
            <span>
              {tn('sheets.gambit.rule', {
                cond: <b>{condText(g.if, tagName)}</b>,
                and: g.and ? tn('sheets.gambit.and', { cond: <b>{condText(g.and, tagName)}</b> }) : null,
                action:
                  g.do.t === 'skill'
                    ? t('sheets.gambit.use_skill', { skill: skillName(g.do.skill), target: TARGET_NAMES[g.target] })
                    : g.do.t === 'retreat_planned'
                      ? t('sheets.gambit.planned')
                      : t('sheets.gambit.emergency'),
              })}
            </span>
            <span className="row" style={{ gap: 2 }}>
              <button className="btn small ghost" onClick={() => move(k, -1)} aria-label={t('sheets.gambit.up')}>
                ↑
              </button>
              <button className="btn small ghost" onClick={() => move(k, 1)} aria-label={t('sheets.gambit.down')}>
                ↓
              </button>
            </span>
          </div>
        ))}
      </div>
      {dirty ? (
        <div className="row" style={{ marginTop: 6 }}>
          <button className="btn small primary" onClick={() => run({ cmd: 'set_gambits', op: o.id, gambits: list }, t('sheets.gambit.saved'))}>
            {t('sheets.gambit.save')}
          </button>
          <button className="btn small ghost" onClick={() => setList(o.gambits)}>
            {t('sheets.gambit.revert')}
          </button>
        </div>
      ) : null}
    </Section>
  );
}

function OpDanger({ o, onClose }: { o: OpView; onClose: () => void }) {
  const { run } = useCtx();
  const [sure, setSure] = useState(false);
  return (
    <div className="row wrap">
      <button className="btn small" onClick={() => run({ cmd: 'lock', op: o.id, locked: !o.locked }, o.locked ? t('sheets.lock.unlocked') : t('sheets.lock.locked'))}>
        <Icon id={o.locked ? 'unlock' : 'lock'} size={14} /> {o.locked ? t('sheets.lock.unlock') : t('sheets.lock.lock')}
      </button>
      {!o.special && !o.locked ? (
        sure ? (
          <button className="btn small danger" onClick={() => run({ cmd: 'retire', op: o.id }, t('sheets.retire.done', { name: o.name })) && onClose()}>
            {t('sheets.retire.confirm')}
          </button>
        ) : (
          <button className="btn small ghost" onClick={() => setSure(true)}>
            <Icon id="retire" size={14} /> {t('sheets.retire.ask')}
          </button>
        )
      ) : null}
    </div>
  );
}

// ---------------------------------------------------------------------------

export function CandidateSheet({ candidate, onClose }: { candidate: number; onClose: () => void }) {
  const { view, run, game } = useCtx();
  const c = view.candidates?.find((x) => x.id === candidate);
  const dry = useMemo(() => (c ? game.dryRun({ cmd: 'hire', candidate }) : null), [c, game, candidate]);
  if (!c)
    return (
      <Page title={t('sheets.candidate.title')} onClose={onClose}>
        <p className="muted">{t('sheets.candidate.missing')}</p>
      </Page>
    );
  return (
    <Page
      title={
        <span className="row" style={{ gap: 8 }}>
          {c.op.name} <Rarity r={c.op.rarity} name={c.op.rarity_name} />
          <span className="muted small">{c.op.class_name}</span>
        </span>
      }
      icon={c.op.icon}
      onClose={onClose}
      aside={
        <div className="card striped">
          <Section title={t('sheets.candidate.hire_title')} icon="hire" tone="good" hint={c.source}>
            <DryRunView dry={dry} />
            <div className="row" style={{ marginTop: 10 }}>
            <button className="btn primary" disabled={!dry?.ok} onClick={() => run({ cmd: 'hire', candidate }, t('sheets.candidate.hired', { name: c.op.name })) && onClose()}>
              {c.cost ? t('sheets.candidate.hire_cost', { cost: c.cost }) : t('sheets.candidate.hire_free')}
            </button>
            <button className="btn ghost" onClick={() => run({ cmd: 'dismiss', candidate }, t('sheets.candidate.dismissed')) && onClose()}>
              {t('sheets.candidate.dismiss')}
            </button>
            </div>
          </Section>
        </div>
      }
    >
      <OpDetail o={c.op} />
    </Page>
  );
}

// ---------------------------------------------------------------------------

export function DistrictSheet({ district, onClose }: { district: number; onClose: () => void }) {
  const { game, view, open } = useCtx();
  const b = game.balance;
  const d = view.districts.find((x) => x.id === district)!;
  return (
    <Page
      title={d.name}
      icon="trust"
      onClose={onClose}
      aside={
        <>
          <DistrictPriority d={d} />
          {d.policy_options ? <DistrictPolicy d={d} /> : null}
          {d.customize ? <DistrictCustomize d={d} /> : null}
          <Section title={t('sheets.district.tiles')} icon="map" hint={`${d.owned}/${d.total}`}>
            <div className="row wrap" style={{ gap: 4 }}>
              {d.tiles.map((i) => (
                <button key={i} className={'btn small' + (view.tiles[i]!.owned ? '' : ' ghost')} onClick={() => open({ kind: 'tile', tile: i })}>
                  {view.tiles[i]!.label}
                </button>
              ))}
            </div>
          </Section>
        </>
      }
    >
      <div className="small muted">
        {t('sheets.district.summary', {
          owned: d.owned,
          total: d.total,
          contiguous: d.contiguous ? t('sheets.district.contiguous', { output: b.contiguity_output_bp / 100, trust: b.contiguity_trust }) : '',
          population: d.population,
          housing: d.housing,
          employed: d.employed,
        })}
        {d.unhoused ? <b className="bad">{t('sheets.district.unhoused', { n: d.unhoused })}</b> : null}
      </div>
      <div className="trait">
        <div className="row small">
          <Icon id={d.flavor.icon} size={14} /> {tn('sheets.district.flavor', { name: <b>{d.flavor.name}</b> })}
          <span className="faint">{t('sheets.district.flavor_hint')}</span>
        </div>
        <div className="tiny muted">{d.flavor.desc}</div>
      </div>

      <Section title={t('sheets.district.trust', { n: d.trust })} icon="trust" tone="good" hint={t('sheets.district.trust_hint', { n: d.trust_target })}>
        <Bar value={d.trust} max={100} tone={d.trust >= 60 ? 'good' : d.trust < 35 ? 'bad' : undefined} />
        <div className="stack" style={{ gap: 2, marginTop: 6 }}>
          {d.trust_factors.map(([k, v]) => (
            <div key={k} className="row small">
              <span className="grow">{k}</span>
              <span className={'num ' + (v > 0 ? 'good' : v < 0 ? 'bad' : '')}>{v > 0 ? `+${v}` : v}</span>
            </div>
          ))}
        </div>
        <div className="small muted" style={{ marginTop: 4 }}>
          {t('sheets.district.militia', { militia: d.militia, volunteers: d.volunteers })}
        </div>
      </Section>

      <Section title={t('sheets.district.talent')} icon="talent" hint={d.training_bp ? t('sheets.district.training', { pct: d.training_bp / 100 }) : undefined}>
        <div className="stack" style={{ gap: 6 }}>
          {d.talent.map((tl) => (
            <div key={tl.prof}>
              <div className="row small">
                <Icon id={tl.icon} size={14} />
                <span>{tl.name}</span>
                <span className="faint tiny">
                  {t('sheets.district.jobs', { workers: tl.workers, jobs: tl.jobs })}
                  {tl.instructors ? t('sheets.district.instructors', { n: tl.instructors }) : ''}
                </span>
                <span className="grow" />
                <span className="num tiny">
                  {tl.pct}%{tl.eta_hours !== null ? t('sheets.district.eta', { h: tl.eta_hours }) : ''}
                </span>
              </div>
              <Bar value={tl.pct} max={100} tone="warn" />
            </div>
          ))}
        </div>
      </Section>

    </Page>
  );
}

function DistrictPriority({ d }: { d: DistrictView }) {
  const { run } = useCtx();
  const [prio, setPrio] = useState<string[]>(d.priority);
  const profName = (id: string) => d.talent.find((t) => t.prof === id)?.name ?? id;
  const prioDirty = JSON.stringify(prio) !== JSON.stringify(d.priority);
  return (
    <Section title={t('sheets.priority.title')} icon="priority" hint={t('sheets.priority.hint')}>
      <div className="stack" style={{ gap: 3 }}>
        {prio.map((p, k) => (
          <div key={p} className="row small">
            <span className="num faint">{k + 1}</span>
            <span className="grow">{profName(p)}</span>
            <button
              className="btn small ghost"
              disabled={k === 0}
              onClick={() => {
                const n = [...prio];
                [n[k - 1], n[k]] = [n[k]!, n[k - 1]!];
                setPrio(n);
              }}
            >
              ↑
            </button>
          </div>
        ))}
      </div>
      {prioDirty ? (
        <button className="btn small primary" style={{ marginTop: 6 }} onClick={() => run({ cmd: 'set_priority', district: d.id, priority: prio }, t('sheets.priority.saved'))}>
          {t('sheets.priority.save')}
        </button>
      ) : null}
    </Section>
  );
}

function DistrictPolicy({ d }: { d: DistrictView }) {
  const { game, run } = useCtx();
  const [dry, setDry] = useState<{ id: string | null; r: DryRun } | null>(null);
  return (
    <Section title={t('sheets.policy.title')} icon="policy" tone="hazard" hint={d.policy_unlocked ? (d.policy ? t('sheets.policy.set') : t('sheets.policy.open')) : t('sheets.policy.locked', { n: game.balance.trust_policy })}>
      <div className="stack" style={{ gap: 6 }}>
        {(d.policy_options ?? []).map((p) => (
          <button
            key={p.id}
            disabled={!p.available && d.policy !== p.id}
            onClick={() => setDry({ id: d.policy === p.id ? null : p.id, r: game.dryRun({ cmd: 'set_policy', district: d.id, policy: d.policy === p.id ? null : p.id }) })}
            className={'list-item' + (d.policy === p.id ? ' selected' : '')}
          >
            <div className="grow">
              <b>{p.name}</b> {d.policy === p.id ? <span className="chip good">{t('sheets.policy.active')}</span> : null}
              <div className="small muted">{p.desc}</div>
              {!p.available && p.reason ? <div className="tiny faint">{p.reason}</div> : null}
            </div>
          </button>
        ))}
      </div>
      {dry ? (
        <div className="card flat" style={{ marginTop: 6 }}>
          <DryRunView dry={dry.r} />
          <button className="btn small primary" style={{ marginTop: 6 }} disabled={!dry.r.ok} onClick={() => run({ cmd: 'set_policy', district: d.id, policy: dry.id }, t('sheets.policy.saved')) && setDry(null)}>
            {dry.id ? t('sheets.policy.adopt') : t('sheets.policy.cancel')}
          </button>
        </div>
      ) : null}
    </Section>
  );
}

const RARITY_FLOORS: [number | null, string][] = [
  [null, t('sheets.customize.no_floor')],
  [1, 'R'],
  [2, 'SR'],
  [3, 'SSR'],
];

/** 定制招募: spend this district's residents on a candidate of your choosing. */
function DistrictCustomize({ d }: { d: DistrictView }) {
  const { game, run, view } = useCtx();
  // Only rendered while custom recruiting is unlocked (the block is there).
  const cz = d.customize!;
  const [prof, setProf] = useState<string | null>(null);
  const [cls, setCls] = useState<string | null>(null);
  const [lock, setLock] = useState<string | null>(null);
  const [rarity, setRarity] = useState<number | null>(null);
  const opt = cz.options.find((o) => o.prof === prof) ?? cz.options.find((o) => o.ok) ?? null;
  const klass = opt?.classes.find((c) => c.id === cls && c.ok) ?? opt?.classes[0] ?? null;
  const cost = cz.base + (klass?.mixed ? cz.class_cost : 0) + (lock ? cz.lock_cost : 0) + (rarity !== null ? cz.rarity_cost[rarity]! : 0);
  const cmd =
    opt && klass
      ? ({ cmd: 'customize', district: d.id, profession: opt.prof, class: klass.mixed ? klass.id : null, lock, rarity } as const)
      : null;
  const key = JSON.stringify(cmd);
  const dry = useMemo(
    () => (cmd && cz.unlocked && opt?.ok ? game.dryRun(cmd) : null),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [key, cz.unlocked, opt?.ok, game, view.revision, view.time],
  );
  const hint = cz.unlocked ? t('sheets.customize.hint', { n: d.population }) : t('sheets.customize.closed');
  return (
    <Section title={t('sheets.customize.title')} icon="candidate" hint={hint}>
      {!cz.unlocked ? (
        <div className="small muted">{t('sheets.customize.locked_hint', { hint: cz.hint })}</div>
      ) : cz.options.length === 0 ? (
        <div className="small muted">{t('sheets.customize.no_workers')}</div>
      ) : (
        <div className="stack" style={{ gap: 8 }}>
          <div className="stack" style={{ gap: 4 }}>
            {cz.options.map((o) => (
              <button
                key={o.prof}
                disabled={!o.ok}
                className={'list-item' + (opt?.prof === o.prof ? ' selected' : '')}
                onClick={() => {
                  setProf(o.prof);
                  setCls(null);
                  setLock(null);
                }}
              >
                <Icon id={o.prof_icon} size={18} />
                <div className="grow">
                  <b>{o.prof_name}</b> <span className="small muted">{t('sheets.customize.workers', { n: o.workers })}</span>
                  <div className="tiny faint">{o.reason ?? t('sheets.customize.yields', { classes: o.classes.map((c) => c.name).join(' / ') })}</div>
                </div>
              </button>
            ))}
          </div>
          {opt && opt.ok && opt.classes.length > 1 ? (
            <div className="stack" style={{ gap: 4 }}>
              <div className="seg">
                {opt.classes.map((c) => (
                  <button
                    key={c.id}
                    disabled={!c.ok}
                    aria-pressed={klass?.id === c.id}
                    onClick={() => {
                      setCls(c.id);
                      setLock(null);
                    }}
                  >
                    {c.name}
                    {c.mixed ? ` +${cz.class_cost}` : ''}
                  </button>
                ))}
              </div>
              {opt.classes
                .filter((c) => !c.ok && c.reason)
                .map((c) => (
                  <div key={c.id} className="tiny faint">
                    {t('sheets.customize.class_reason', { name: c.name, reason: c.reason ?? '' })}
                  </div>
                ))}
            </div>
          ) : null}
          {opt && opt.ok && klass && klass.traits.length ? (
            <div className="stack" style={{ gap: 4 }}>
              <div className="small">
                {tn('sheets.customize.lock', { cost: <span className="faint">{t('sheets.customize.lock_cost', { n: cz.lock_cost })}</span> })}
              </div>
              <button className={'list-item' + (lock === null ? ' selected' : '')} onClick={() => setLock(null)}>
                <div className="grow small">{t('sheets.customize.any_trait')}</div>
              </button>
              {klass.traits.map((t) => (
                <button key={t.id} className={'list-item' + (lock === t.id ? ' selected' : '')} onClick={() => setLock(t.id)}>
                  <Icon id={t.icon} size={16} />
                  <div className="grow">
                    <b className="small">{t.name}</b>
                    <div className="tiny muted">{t.desc}</div>
                  </div>
                </button>
              ))}
            </div>
          ) : null}
          {opt && opt.ok ? (
            <div className="stack" style={{ gap: 4 }}>
              <div className="small">{t('sheets.customize.floor')}</div>
              <div className="seg">
                {RARITY_FLOORS.map(([r, name]) => (
                  <button key={name} aria-pressed={rarity === r} onClick={() => setRarity(r)}>
                    {name}
                    {r !== null && cz.rarity_cost[r] ? ` +${cz.rarity_cost[r]}` : ''}
                  </button>
                ))}
              </div>
            </div>
          ) : null}
          {opt && opt.ok ? (
            <div className="card flat">
              <div className="small">{tn('sheets.customize.total', { cost: <b>{cost}</b>, have: d.population })}</div>
              <div style={{ marginTop: 6 }}>
                <DryRunView dry={dry} />
              </div>
              <button
                className="btn small primary"
                style={{ marginTop: 6 }}
                disabled={!dry?.ok}
                onClick={() => cmd && run(cmd, t('sheets.customize.ordered'))}
              >
                {t('sheets.customize.order')}
              </button>
            </div>
          ) : null}
        </div>
      )}
    </Section>
  );
}

// ---------------------------------------------------------------------------

export function SquadSheet({ squad, onClose }: { squad: number; onClose: () => void }) {
  const { view, open, run, setRouteEdit } = useCtx();
  const s = view.squads.find((x) => x.id === squad)!;
  const free = view.operatives.filter((o) => !s.members.includes(o.id));
  const trip = s.last ?? s.running;
  return (
    <Page
      title={s.name}
      icon="squad"
      onClose={onClose}
      aside={trip ? <TripBlock squad={squad} /> : undefined}
    >
      <Section title={t('sheets.squad.members')} icon="roster" hint={`${s.members.length}/3`}>
        <div className="list">
          {s.members.map((id) => {
            const o = view.operatives.find((x) => x.id === id);
            return o ? (
              <OpRow
                key={id}
                o={o}
                onClick={() => open({ kind: 'op', op: id })}
                right={
                  <span
                    className="btn small ghost"
                    onClick={(e) => {
                      e.stopPropagation();
                      run({ cmd: 'set_squad', squad, members: s.members.filter((m) => m !== id) });
                    }}
                  >
                    {t('sheets.squad.remove')}
                  </span>
                }
              />
            ) : null;
          })}
        </div>
        {s.members.length < 3 && free.length ? (
          <div className="row wrap" style={{ marginTop: 6 }}>
            {free.map((o) => (
              <button key={o.id} className="btn small" onClick={() => run({ cmd: 'set_squad', squad, members: [...s.members, o.id] }, t('sheets.assign.joined', { op: o.name, squad: s.name }))}>
                <Icon id={o.icon} size={14} /> {o.name}
                <span className="faint tiny">{o.status}</span>
              </button>
            ))}
          </div>
        ) : null}
      </Section>
      <Section title={t('sheets.squad.route')} icon="route" tone="elec" hint={t('sheets.squad.trips', { n: s.trips })}>
        <div className="small">{s.route.length ? s.route.map((i) => view.tiles[i]!.label).join(' → ') : t('sheets.squad.no_route')}</div>
        <button className="btn small" style={{ marginTop: 6 }} onClick={() => setRouteEdit({ squad, route: s.route })} disabled={!s.members.length}>
          {t('sheets.squad.edit_route')}
        </button>
      </Section>
    </Page>
  );
}

function TripBlock({ squad }: { squad: number }) {
  const { view, now, open, game } = useCtx();
  const s = view.squads.find((x) => x.id === squad)!;
  const trip = (s.last ?? s.running)!;
  return (
    <Section title={s.running ? t('sheets.trip.current') : t('sheets.trip.last')} icon="report" hint={s.running ? t('sheets.trip.back_in', { time: fmtIn(s.running.end - now) }) : OUTCOME_NAMES[trip.outcome]}>
      <div className="list">
        {trip.results.map((r) => (
          <div key={r.tile} className="list-item small">
            <span className={r.outcome === 'victory' ? 'good' : 'bad'}>{r.label}</span>
            <span className="grow">{OUTCOME_NAMES[r.outcome]}</span>
            <button
              className="btn small ghost"
              onClick={() => open({ kind: 'report', title: `${s.name} · ${r.label}`, load: () => game.tripBattle(squad, r.tile) ?? (r.report !== null ? game.report(r.report) : null) })}
            >
              {t('sheets.trip.replay')}
            </button>
          </div>
        ))}
        {trip.results.length === 0 ? <div className="empty">{t('sheets.trip.patrol_only')}</div> : null}
      </div>
      <div className="row wrap small" style={{ gap: 10, marginTop: 6 }}>
        <span className="muted">{t('sheets.trip.loot')}</span>
        {RES_KEYS.filter((k) => trip.loot[k] > 0).map((k) => (
          <span key={k} className="res-item">
            <Icon id={`res-${k}`} size={14} />
            {RES_NAMES[k]} <span className="num">{trip.loot[k]}</span>
          </span>
        ))}
        {trip.drops.map((d, k) => (
          <span key={k} className="chip">
            {d}
          </span>
        ))}
      </div>
    </Section>
  );
}

export function TideSheet({ onClose }: { onClose: () => void }) {
  const { game, view } = useCtx();
  const b = game.balance;
  return (
    <Page title={t('sheets.tide.title', { h: b.tide_horizon / 3600 })} icon="tide" onClose={onClose}>
      <p className="small muted" style={{ margin: 0 }}>
        {t('sheets.tide.intro', { h: b.surge_interval / 3600 })}
      </p>
      <TideRows rows={view.tide} />
      {view.modifier ? (
        <div className="small muted">
          {t('sheets.tide.modifier', { name: view.modifier[1], desc: view.modifier[2] })}
        </div>
      ) : null}
    </Page>
  );
}
