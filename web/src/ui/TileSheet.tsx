import { useMemo, useState } from 'react';
// `t` is the tile throughout this file, so the text lookup goes by `tr`.
import { t as tr } from '../i18n';
import { Icon } from '../icons';
import type { Command, Dial, DryRun, GarrisonOption, TileView } from '../types';
import { Cost, DIAL_NAMES, DryRunView, fmtIn, Page, Section, TagIO, useCtx } from './common';

const TERRAIN_NAMES: Record<string, string> = {
  unknown: tr('terrain.unknown'),
  hq: tr('terrain.hq'),
  city: tr('terrain.city'),
  rift: tr('terrain.rift'),
  special: tr('terrain.special'),
  barrier: tr('terrain.barrier'),
};
const SPECIAL_NAMES: Record<string, string> = {
  landmark: tr('tile.special.landmark'),
  cache: tr('tile.special.cache'),
  clue: tr('tile.special.clue'),
  core: tr('tile.special.core'),
};

export function TileSheet({ tile, onClose }: { tile: number; onClose: () => void }) {
  const { view } = useCtx();
  const t = view.tiles[tile]!;
  const district = t.district !== undefined ? view.districts.find((d) => d.id === t.district) : undefined;
  const title = (
    <span>
      {t.label} · {t.special ? SPECIAL_NAMES[t.special] : TERRAIN_NAMES[t.terrain]}
      {district ? <span className="muted small"> · {district.name}</span> : null}
    </span>
  );
  const icon = t.terrain === 'hq' ? 'tile-hq' : t.terrain === 'rift' ? 'tile-rift' : t.terrain === 'barrier' ? 'tile-barrier' : t.revealed ? 'tile-city' : 'fog';
  const hasAside = t.border || (t.owned && t.terrain === 'city') || (t.owned && t.terrain === 'rift' && !!t.rift);
  const aside = hasAside ? (
    <>
      {t.border ? <DefenseBlock t={t} /> : null}
      {t.owned && t.terrain === 'city' ? <BuildBlock t={t} /> : null}
      {t.owned && t.terrain === 'rift' && t.rift ? <DialBlock t={t} /> : null}
    </>
  ) : undefined;
  return (
    <Page title={title} icon={icon} onClose={onClose} aside={aside}>
      <Section title={tr('tile.overview.title')} icon={icon}>
        <Status t={t} />
        {!t.revealed && t.terrain !== 'barrier' ? <p className="muted small">{tr('tile.overview.fog')}</p> : null}
        {t.terrain === 'barrier' ? <p className="muted small">{tr('tile.overview.barrier')}</p> : null}
      </Section>
      {t.claimable ? <ClaimBlock t={t} /> : null}
      {t.alert ? <AlertBlock t={t} /> : null}
      {!t.owned && t.revealed && t.enemies.length ? <EnemyBlock t={t} /> : null}
      {district ? <DistrictLink id={district.id} /> : null}
      {t.owned && t.terrain !== 'hq' ? <AbandonBlock t={t} /> : null}
    </Page>
  );
}

function Status({ t }: { t: TileView }) {
  const { view, now } = useCtx();
  const items: [string, React.ReactNode][] = [];
  items.push([tr('tile.status.ring'), tr('tile.status.ring_value', { n: t.ring })]);
  if (t.revealed && t.terrain !== 'barrier') {
    items.push([tr('tile.status.warden'), <span className={t.alert ? 'bad' : ''}>{t.warden} / 100{t.owned ? '' : tr('tile.status.warden_claimable')}</span>]);
  }
  if (t.revealed && !t.owned && t.terrain !== 'barrier' && t.terrain !== 'hq' && t.danger_pct !== 100)
    items.push([tr('tile.status.danger'), <span className={t.danger_pct > 100 ? 'bad' : 'good'}>{tr('tile.status.danger_value', { n: t.danger_pct })}</span>]);
  if (t.revealed && t.natives) items.push([tr('tile.status.natives'), tr('tile.status.natives_value', { n: t.natives })]);
  if (t.owned && t.workers) items.push([tr('tile.status.workers'), `${t.workers[0]} / ${t.workers[1]}`]);
  if (t.frozen) items.push([tr('tile.status.frozen'), t.frozen]);
  if (t.rift) items.push([tr('tile.status.rift'), tr('tile.status.rift_value', { name: t.rift.name, resident: t.rift.resident, n: t.rift.residents })]);
  if (t.next_surge !== undefined)
    items.push([tr('tile.status.next_surge'), tr('tile.status.next_surge_value', { in: fmtIn(t.next_surge - now), from: view.tiles[t.surge_source ?? 0]?.label })]);
  if (t.squads.length) items.push([tr('tile.status.squads'), t.squads.map((id) => view.squads.find((s) => s.id === id)?.name).join(tr('tile.status.squads_sep'))]);
  return (
    <dl className="kv">
      {items.map(([k, v]) => (
        <FragmentKV key={k} k={k} v={v} />
      ))}
    </dl>
  );
}

function FragmentKV({ k, v }: { k: string; v: React.ReactNode }) {
  return (
    <>
      <dt>{k}</dt>
      <dd>{v}</dd>
    </>
  );
}

/** Confirm block with a dry-run preview: the engine says what changes before the player commits. */
function Confirm({ cmd, label, danger, okText, onDone }: { cmd: Command; label: string; danger?: boolean; okText?: string; onDone?: () => void }) {
  const { game, run } = useCtx();
  const [dry, setDry] = useState<DryRun | null>(null);
  return (
    <div className="stack">
      {dry ? <DryRunView dry={dry} /> : null}
      <div className="row wrap">
        {!dry ? (
          <button className="btn small" onClick={() => setDry(game.dryRun(cmd))}>
            <Icon id="tide" size={14} /> {tr('tile.confirm.preview')}
          </button>
        ) : null}
        <button
          className={'btn ' + (danger ? 'danger' : 'primary')}
          disabled={dry !== null && !dry.ok}
          onClick={() => {
            if (run(cmd, okText)) onDone?.();
          }}
        >
          {label}
        </button>
      </div>
    </div>
  );
}

function ClaimBlock({ t }: { t: TileView }) {
  const { game } = useCtx();
  const dry = useMemo(() => game.dryRun({ cmd: 'claim', tile: t.i }), [game, t.i]);
  return (
    <div className="card striped good">
      <Section title={t.terrain === 'rift' ? tr('tile.claim.title_rift') : tr('tile.claim.title')} icon="check" tone="good" hint={tr('tile.claim.hint')}>
        <DryRunView dry={dry} />
        <div className="row" style={{ marginTop: 8 }}>
          <ClaimButton tile={t.i} ok={dry.ok} />
        </div>
      </Section>
    </div>
  );
}

function ClaimButton({ tile, ok }: { tile: number; ok: boolean }) {
  const { run } = useCtx();
  return (
    <button className="btn primary" disabled={!ok} onClick={() => run({ cmd: 'claim', tile }, tr('tile.claim.done'))}>
      {tr('tile.claim.button')}
    </button>
  );
}

function AlertBlock({ t }: { t: TileView }) {
  const { game, open } = useCtx();
  const report = useMemo(() => game.previewSurge(t.i, undefined), [game, t.i]);
  return (
    <div className="card striped bad">
      <Section title={tr('tile.alert.title')} icon="alert" tone="bad" hint={tr('tile.alert.hint')}>
        <p className="small muted" style={{ margin: 0 }}>
          {tr('tile.alert.body', { n: game.balance.warden_counter_restore, clinic: game.balance.warden_counter_restore_clinic })}
        </p>
        {report ? (
          <div className="row small" style={{ margin: '8px 0' }}>
            <span className={report.outcome === 'victory' ? 'good' : 'bad'}>
              {report.outcome === 'victory' ? tr('tile.alert.preview_win') : tr('tile.alert.preview_lose', { wave: report.lost_wave ?? 1 })}
            </span>
            <button
              className="btn small ghost"
              onClick={() => open({ kind: 'report', title: tr('tile.alert.report_title', { tile: t.label }), load: () => game.previewSurge(t.i, undefined) })}
            >
              {tr('tile.alert.replay')}
            </button>
          </div>
        ) : null}
        <Confirm cmd={{ cmd: 'counter', tile: t.i }} label={tr('tile.alert.counter')} okText={tr('tile.alert.counter_done')} />
      </Section>
    </div>
  );
}

function DefenseBlock({ t }: { t: TileView }) {
  const { game, view, run, open } = useCtx();
  const [opts, setOpts] = useState<GarrisonOption[] | null>(null);
  const current = t.garrison !== undefined ? view.operatives.find((o) => o.id === t.garrison) : undefined;
  const tide = view.tide.filter((e) => e.tile === t.i);
  const load = () => setOpts(game.garrisonOptions(t.i));
  return (
    <Section title={tr('tile.defense.title')} icon="garrison" tone="hazard" hint={current ? tr('tile.defense.held_by', { name: current.name }) : tr('tile.defense.none')}>
      {tide.length ? (
        <div className="row wrap small" style={{ gap: 6, marginBottom: 8 }}>
          <span className="muted">{tr('tile.defense.tide', { h: game.balance.tide_horizon / 3600 })}</span>
          {tide.map((e, k) => (
            // A surge on a tile already on alert is skipped (no wave is fought), so it has no lost wave.
            <span key={k} className={'chip ' + (e.skipped ? '' : e.held ? 'good' : 'bad')} style={e.skipped ? { color: 'var(--ink-3)' } : undefined}>
              <Icon id="wave" size={12} />
              {e.skipped ? tr('panels.tide.skipped') : e.held ? tr('tile.defense.tide_held') : tr('tile.defense.tide_lost', { wave: e.lost_wave ?? '?' })}
            </span>
          ))}
        </div>
      ) : null}
      {!opts ? (
        <button className="btn" onClick={load}>
          <Icon id="garrison" size={16} /> {tr('tile.defense.pick')}
        </button>
      ) : (
        <div className="list">
          {opts.map((o) => (
            <div className="list-item" key={String(o.op)}>
              <span className="op-avatar"><Icon id={o.icon} size={32} /></span>
              <div className="grow">
                <div className="row" style={{ gap: 6 }}>
                  <b>{o.name}</b>
                  {o.current ? <span className="chip">{tr('tile.defense.current')}</span> : null}
                  {o.busy ? <span className="faint tiny">{o.busy}</span> : null}
                </div>
                <div className={'small ' + (o.held ? 'good' : 'bad')}>
                  {o.held
                    ? tr('tile.defense.option_held', { hp: o.hp_left_pct, rounds: o.rounds })
                    : tr('tile.defense.option_lost', { wave: o.lost_wave!, total: o.total_waves })}
                  {o.warden_gain ? <span className="muted">{tr('tile.defense.grounding', { n: o.warden_gain })}</span> : null}
                </div>
              </div>
              <button
                className="btn small ghost"
                onClick={() =>
                  open({ kind: 'report', title: tr('tile.defense.report_title', { tile: t.label, name: o.name }), load: () => game.previewSurge(t.i, o.op) })
                }
              >
                {tr('tile.defense.replay')}
              </button>
              {!o.current ? (
                <button
                  className="btn small"
                  onClick={() => {
                    const cmd: Command = o.op === null ? { cmd: 'garrison', op: t.garrison!, tile: null } : { cmd: 'garrison', op: o.op, tile: t.i };
                    if (run(cmd, o.op === null ? tr('tile.defense.withdrawn') : tr('tile.defense.posted', { name: o.name, tile: t.label }))) load();
                  }}
                >
                  {o.op === null ? tr('tile.defense.withdraw') : tr('tile.defense.post')}
                </button>
              ) : null}
            </div>
          ))}
        </div>
      )}
    </Section>
  );
}

function BuildBlock({ t }: { t: TileView }) {
  const { view, game, run } = useCtx();
  const [pick, setPick] = useState<string | null>(null);
  const [dry, setDry] = useState<DryRun | null>(null);
  const current = t.usage;
  const options = view.facilities.filter((f) => f.visible);
  const hiddenCount = view.facilities.length - options.length;
  const choose = (id: string | null) => {
    setPick(id ?? 'housing');
    setDry(game.dryRun({ cmd: 'build', tile: t.i, facility: id }));
  };
  const usageName =
    current === 'housing' ? tr('tile.build.housing') : current === 'none' ? tr('tile.build.empty') : view.facilities.find((f) => f.id === current)?.name ?? current;
  return (
    <Section title={tr('tile.build.title')} icon="housing" tone="good" hint={tr('tile.build.now', { name: usageName })}>
      <div className="list">
        {current !== 'housing' ? (
          <button className={'list-item' + (pick === 'housing' ? ' selected' : '')} onClick={() => choose(null)}>
            <Icon id="housing" size={20} />
            <div className="grow">
              <b>{tr('tile.build.revert')}</b>
              <div className="small muted">{tr('tile.build.revert_desc', { n: game.balance.housing_capacity })}</div>
            </div>
          </button>
        ) : null}
        {options
          .filter((f) => f.id !== current)
          .map((f) => (
            <button key={f.id} className={'list-item' + (pick === f.id ? ' selected' : '')} onClick={() => choose(f.id)}>
              <Icon id={f.icon} size={20} />
              <div className="grow">
                <div className="row" style={{ gap: 6 }}>
                  <b>{f.name}</b>
                  {f.jobs ? <span className="faint tiny">{tr('tile.build.jobs', { n: f.jobs })}</span> : null}
                  {f.power ? <span className="faint tiny">{tr('tile.build.power', { n: f.power })}</span> : null}
                </div>
                <div className="small muted">{f.desc}</div>
              </div>
              <Cost cost={f.cost} have={view.resources} />
            </button>
          ))}
      </div>
      {hiddenCount > 0 ? <div className="faint tiny">{tr('tile.build.hidden', { n: hiddenCount })}</div> : null}
      {pick && dry ? (
        <div className="card flat" style={{ marginTop: 8 }}>
          <DryRunView dry={dry} />
          <div className="row" style={{ marginTop: 8 }}>
            <button
              className="btn primary"
              disabled={!dry.ok}
              onClick={() => {
                if (run({ cmd: 'build', tile: t.i, facility: pick === 'housing' ? null : pick }, tr('tile.build.done'))) {
                  setPick(null);
                  setDry(null);
                }
              }}
            >
              {tr('tile.build.confirm')}
            </button>
            <button className="btn ghost" onClick={() => setPick(null)}>
              {tr('tile.build.cancel')}
            </button>
          </div>
        </div>
      ) : null}
    </Section>
  );
}

function DialBlock({ t }: { t: TileView }) {
  const { game, run } = useCtx();
  const r = t.rift!;
  const dials: Dial[] = ['mine', 'balance', 'seal'];
  const [dry, setDry] = useState<{ d: Dial; r: DryRun } | null>(null);
  return (
    <Section title={tr('tile.dial.title')} icon={`dial-${r.dial}`} tone="elec" hint={tr('tile.dial.talent', { pct: r.talent_pct })}>
      <p className="small muted" style={{ margin: '0 0 8px' }}>
        {tr('tile.dial.help')}
      </p>
      <div className="seg">
        {dials.map((d) => (
          <button key={d} aria-pressed={r.dial === d} onClick={() => (r.dial === d ? null : setDry({ d, r: game.dryRun({ cmd: 'set_dial', tile: t.i, dial: d }) }))}>
            {DIAL_NAMES[d]}
          </button>
        ))}
      </div>
      {dry ? (
        <div className="card flat" style={{ marginTop: 8 }}>
          <DryRunView dry={dry.r} />
          <div className="row" style={{ marginTop: 8 }}>
            <button
              className="btn primary"
              onClick={() => run({ cmd: 'set_dial', tile: t.i, dial: dry.d }, tr('tile.dial.done', { dial: DIAL_NAMES[dry.d] })) && setDry(null)}
            >
              {tr('tile.dial.set', { dial: DIAL_NAMES[dry.d] })}
            </button>
          </div>
        </div>
      ) : null}
    </Section>
  );
}

function EnemyBlock({ t }: { t: TileView }) {
  const { view, setRouteEdit, open, toast, game } = useCtx();
  return (
    <Section title={tr('tile.enemy.title')} icon="wave" tone="rift" hint={tr('tile.enemy.waves', { n: t.enemies.length })}>
      <div className="stack" style={{ gap: 4 }}>
        {t.enemies.map((w, k) => (
          <div key={k} className="row wrap small" style={{ gap: 10 }}>
            <span className="faint num">{tr('tile.enemy.wave', { n: k + 1 })}</span>
            {w.map((e, j) => (
              <span key={j} className="row" style={{ gap: 4 }}>
                <Icon id={e.icon} size={16} />
                {e.name}
                <TagIO reads={[]} writes={e.writes} />
              </span>
            ))}
          </div>
        ))}
      </div>
      <p className="small muted" style={{ margin: '8px 0' }}>
        {tr('tile.enemy.help', { n: game.balance.warden_route_win, max: game.balance.warden_max })}
      </p>
      <div className="row wrap">
        {view.squads
          .filter((s) => s.members.length)
          .map((s) => (
            <button
              key={s.id}
              className="btn small"
              onClick={() => {
                const next = s.route.includes(t.i) ? s.route : [...s.route, t.i];
                const p = game.previewRoute(s.id, next);
                if (!p.ok) {
                  toast(p.error ?? tr('tile.enemy.route_blocked'), true);
                  setRouteEdit({ squad: s.id, route: s.route });
                } else {
                  setRouteEdit({ squad: s.id, route: next });
                }
                open(null);
              }}
            >
              <Icon id="route" size={14} /> {tr('tile.enemy.add_route', { name: s.name })}
            </button>
          ))}
      </div>
    </Section>
  );
}

function DistrictLink({ id }: { id: number }) {
  const { view, open } = useCtx();
  const d = view.districts.find((x) => x.id === id)!;
  return (
    <button className="list-item card flat" onClick={() => open({ kind: 'district', district: id })}>
      <Icon id="trust" size={20} />
      <div className="grow">
        <b>{d.name}</b>
        <div className="small muted">
          {tr('tile.district.stats', { owned: d.owned, total: d.total, trust: d.trust, target: d.trust_target, pop: d.population })}
          {d.contiguous ? tr('tile.district.contiguous') : ''}
        </div>
      </div>
      <Icon id="chevron-right" size={16} />
    </button>
  );
}

function AbandonBlock({ t }: { t: TileView }) {
  const [show, setShow] = useState(false);
  if (!show)
    return (
      <div>
        <button className="btn small ghost faint" onClick={() => setShow(true)}>
          {tr('tile.abandon.open')}
        </button>
      </div>
    );
  return (
    <div className="card flat bad">
      <Section title={tr('tile.abandon.title')} icon="warn" tone="bad">
        <Confirm cmd={{ cmd: 'abandon', tile: t.i }} label={tr('tile.abandon.confirm')} danger okText={tr('tile.abandon.done')} />
      </Section>
    </div>
  );
}
