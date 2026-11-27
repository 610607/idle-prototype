import { useEffect, useMemo, useRef, useState } from 'react';
import { t } from '../i18n';
import { Icon } from '../icons';
import type { BattleReport as Report, LogEntry, SnapUnit, Tag } from '../types';
import { TAGS } from '../types';
import { OUTCOME_NAMES, Page, TagChip, useCtx } from './common';

export function ReportSheet({ title, load, onClose }: { title: string; load: () => Report | null; onClose: () => void }) {
  const report = useMemo(load, [load]);
  return (
    <Page title={title} icon="report" onClose={onClose}>
      {report ? <BattleReport report={report} /> : <p className="muted">{t('report.missing')}</p>}
    </Page>
  );
}

export function BattleReport({ report }: { report: Report }) {
  const log = report.log;
  const [cursor, setCursor] = useState(0);
  const [playing, setPlaying] = useState(true);
  const [speed, setSpeed] = useState(1);
  const [formulas, setFormulas] = useState(false);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!playing) return;
    if (cursor >= log.length - 1) {
      setPlaying(false);
      return;
    }
    const e = log[cursor + 1];
    const base = e?.kind === 'wave' ? 900 : e?.kind === 'round' ? 350 : e && e.depth > 0 ? 450 : 600;
    const id = window.setTimeout(() => setCursor((c) => Math.min(log.length - 1, c + 1)), base / speed);
    return () => window.clearTimeout(id);
  }, [playing, cursor, speed, log]);

  useEffect(() => {
    const el = listRef.current?.querySelector<HTMLElement>(`[data-k="${cursor}"]`);
    el?.scrollIntoView({ block: 'nearest' });
  }, [cursor]);

  const cur = log[cursor];
  // Field state: the latest snapshot at or before the cursor.
  const snap = useMemo(() => {
    for (let k = cursor; k >= 0; k--) {
      const s = log[k]?.snap;
      if (s && s.length) return s;
    }
    return null;
  }, [cursor, log]);

  // Cause chain of the current entry: ancestors and direct consequences.
  const chain = useMemo(() => {
    const set = new Set<number>();
    if (!cur) return set;
    const byId = new Map(log.map((e) => [e.id, e]));
    let c: LogEntry | undefined = cur;
    while (c?.cause !== undefined) {
      set.add(c.cause);
      c = byId.get(c.cause);
    }
    for (const e of log) if (e.cause === cur.id) set.add(e.id);
    return set;
  }, [cur, log]);

  const outcomeTone = report.outcome === 'victory' || report.outcome === 'planned' ? 'good' : 'bad';

  return (
    <div className="split">
      <div className="col" style={{ gap: 12 }}>
        <div className="row wrap" style={{ gap: 12 }}>
          <h3 className={outcomeTone} style={{ fontSize: 20 }}>{OUTCOME_NAMES[report.outcome]}</h3>
          <span className="small muted">
            {t('report.summary', { cleared: report.waves_cleared, total: report.total_waves, rounds: report.rounds, yield: Math.round(report.yield_bp / 100) })}
          </span>
          {report.warden_gain ? <span className="small good">{t('report.warden_gain', { n: report.warden_gain })}</span> : null}
        </div>
        {report.reactions?.length ? <ReactionLine reactions={report.reactions} /> : null}

        {snap ? <Field snap={snap} actor={cur?.actor} target={cur?.target} /> : null}

        <div className="player">
          <button className="btn small" onClick={() => setCursor(0)} aria-label={t('report.player.start')}>
            ⏮
          </button>
          <button className="btn small" onClick={() => setCursor((c) => Math.max(0, c - 1))} aria-label={t('report.player.prev')}>
            ◀
          </button>
          <button className="btn small primary" onClick={() => setPlaying((p) => !p)}>
            <Icon id={playing ? 'pause' : 'play'} />
            {playing ? t('report.player.pause') : t('report.player.play')}
          </button>
          <button className="btn small" onClick={() => setCursor((c) => Math.min(log.length - 1, c + 1))} aria-label={t('report.player.next')}>
            ▶
          </button>
          <button
            className="btn small"
            onClick={() => {
              setPlaying(false);
              setCursor(log.length - 1);
            }}
          >
            {t('report.player.end')}
          </button>
          <div className="seg">
            {[1, 2, 4].map((s) => (
              <button key={s} aria-pressed={speed === s} onClick={() => setSpeed(s)}>
                {s}×
              </button>
            ))}
          </div>
          <label className="switch small">
            <input type="checkbox" checked={formulas} onChange={(e) => setFormulas(e.target.checked)} />
            {t('report.player.formulas')}
          </label>
        </div>
        <input
          type="range"
          min={0}
          max={Math.max(0, log.length - 1)}
          value={cursor}
          onChange={(e) => {
            setPlaying(false);
            setCursor(Number(e.target.value));
          }}
          aria-label={t('report.player.seek')}
        />
        <Summary report={report} />
      </div>

      <div className="col">
        <div className="log" ref={listRef}>
          {log.map((e, k) => (
            <div
              key={e.id}
              data-k={k}
              className={
                'log-entry ' +
                e.kind +
                (k > cursor ? ' future' : '') +
                (k === cursor ? ' current' : '') +
                (chain.has(e.id) ? ' chain' : '')
              }
              style={{ paddingLeft: 6 + e.depth * 18 }}
              onClick={() => {
                setPlaying(false);
                setCursor(k);
              }}
            >
              {e.kind === 'round' ? t('report.log.round', { n: e.round }) : e.text}
              {e.tags && e.tags.length && e.kind !== 'wave' ? (
                <span style={{ marginLeft: 6 }}>
                  {e.tags.map((t) => (
                    <TagChip key={t} tag={t} />
                  ))}
                </span>
              ) : null}
              {e.formula && (formulas || k === cursor) ? <div className="formula">{e.formula}</div> : null}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function Field({ snap, actor, target }: { snap: SnapUnit[]; actor?: string; target?: string }) {
  const allies = snap.filter((u) => u.side === 'ally');
  const enemies = snap.filter((u) => u.side === 'enemy');
  return (
    <div className="field">
      <div className="side">
        {allies.map((u, k) => (
          <UnitBox key={k} u={u} actor={actor} target={target} />
        ))}
      </div>
      <div className="vs">{t('report.field.vs')}</div>
      <div className="side">
        {enemies.map((u, k) => (
          <UnitBox key={k} u={u} actor={actor} target={target} />
        ))}
      </div>
    </div>
  );
}

function UnitBox({ u, actor, target }: { u: SnapUnit; actor?: string; target?: string }) {
  if (u.fixture) {
    return (
      <div className={'unit ally fixture' + (actor === u.name ? ' actor' : '')} title={t('report.unit.fixture_title')}>
        <div className="row" style={{ gap: 4 }}>
          <Icon id={u.icon} />
          <span className="grow ellipsis">{u.name}</span>
          <span className="faint tiny">{t('report.unit.fixture')}</span>
        </div>
        {u.pp > 0 ? <div className="faint tiny">PP {u.pp}</div> : null}
      </div>
    );
  }
  const cls = 'unit ' + u.side + (u.alive ? '' : ' dead') + (actor === u.name ? ' actor' : '') + (target === u.name ? ' target' : '');
  const pct = Math.max(0, Math.min(100, (u.hp / Math.max(1, u.max_hp)) * 100));
  const tone = pct < 30 ? 'bad' : pct < 60 ? 'warn' : 'good';
  return (
    <div className={cls}>
      <div className="row" style={{ gap: 4 }}>
        <Icon id={u.icon} />
        <span className="grow ellipsis">
          {u.name}
          <span className="faint tiny"> {u.front ? t('report.unit.front') : t('report.unit.back')}</span>
        </span>
        <span className="num tiny">
          {u.hp}/{u.max_hp}
        </span>
      </div>
      <div className={'bar ' + tone} style={{ marginTop: 2 }}>
        <i style={{ width: pct + '%' }} />
      </div>
      <div className="tags">
        {u.shield > 0 ? <span className="chip tiny">{t('report.unit.shield', { n: u.shield })}</span> : null}
        {TAGS.map((t, k) => (u.tags[k] ?? 0) > 0 && t !== 'shield' ? <TagChip key={t} tag={t as Tag} n={u.tags[k]} /> : null)}
        {u.pp > 0 ? <span className="faint tiny">PP {u.pp}</span> : null}
      </div>
    </div>
  );
}

function Summary({ report }: { report: Report }) {
  const { game } = useCtx();
  const allies = report.units.filter((u) => u.side === 'ally');
  return (
    <details open>
      <summary>{t('report.stats.title')}</summary>
      <table className="tide" style={{ marginTop: 6 }}>
        <thead>
          <tr>
            <th>{t('report.stats.ally')}</th>
            <th>{t('report.stats.dealt')}</th>
            <th>{t('report.stats.taken')}</th>
            <th>{t('report.stats.healed')}</th>
            <th>{t('report.stats.reactions')}</th>
          </tr>
        </thead>
        <tbody>
          {allies.map((u) => (
            <tr key={u.key}>
              <td>
                {u.name} {u.alive ? '' : t('report.stats.down')}
              </td>
              <td className="num">{u.dealt}</td>
              <td className="num">{u.taken}</td>
              <td className="num">{u.healed}</td>
              <td className="num">{u.reactions}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="row wrap small" style={{ gap: 6, marginTop: 6 }}>
        <span className="muted">{t('report.stats.tags')}</span>
        {game.content.tags.map((t, k) =>
          report.applied[k] || report.consumed[k] ? (
            <span key={t.id} className="row" style={{ gap: 2 }}>
              <TagChip tag={t.id} />
              <span className="num">
                {report.applied[k]}/{report.consumed[k]}
              </span>
            </span>
          ) : null,
        )}
      </div>
    </details>
  );
}

function ReactionLine({ reactions }: { reactions: [string, number][] }) {
  const { view } = useCtx();
  const name = (id: string) => view.research.codex.find((e) => e.id === id)?.name ?? id;
  return (
    <div className="row wrap small" style={{ gap: 6 }}>
      <Icon id="trait-resonance" size={14} />
      <span className="muted">{t('report.reactions')}</span>
      {reactions.map(([id, n]) => (
        <span key={id} className="chip">
          {name(id)} <span className="num">×{n}</span>
        </span>
      ))}
    </div>
  );
}
