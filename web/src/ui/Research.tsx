import { t, tn } from '../i18n';
import { Icon } from '../icons';
import type { DoctrineView, Tag } from '../types';
import { Section, TagChip, useCtx } from './common';

const RARITY = ['', t('research.rarity.common'), t('research.rarity.uncommon'), t('research.rarity.rare')];

// ---------------------------------------------------------------------------
// 研究 tab: intel buys a draw of three doctrines; held ones join every battle this cycle; a few can
// be filed in the knowledge base for later cycles. The reaction codex lights up as reactions go off.

export function ResearchPanel() {
  const { view } = useCtx();
  if (view.phase !== 'active') {
    return (
      <div className="split">
        <div className="col">
          <Archive />
        </div>
        <div className="col">
          <Codex />
        </div>
      </div>
    );
  }
  return (
    <div className="split">
      <div className="col">
        <Draw />
        <Held />
        <Archive />
      </div>
      <div className="col">
        <Codex />
      </div>
    </div>
  );
}

function Draw() {
  const { view, run } = useCtx();
  const r = view.research;
  return (
    <Section title={t('research.draw.title')} icon="trait-book" tone="elec" hint={t('research.draw.intel', { n: view.resources.intel })}>
      {r.offers.length ? (
        <>
          <div className="small muted" style={{ marginBottom: 6 }}>
            {t('research.draw.offers_note')}
          </div>
          <div className="stack" style={{ gap: 8 }}>
            {r.offers.map((d) => (
              <DoctrineCard key={d.id} d={d}>
                <button className="btn small primary" onClick={() => run({ cmd: 'pick_doctrine', doctrine: d.id }, t('research.draw.picked', { name: d.name }))}>
                  {t('research.draw.pick')}
                </button>
              </DoctrineCard>
            ))}
          </div>
          <div className="row" style={{ marginTop: 8 }}>
            <button className="btn small" disabled={!r.reroll_ok} onClick={() => run({ cmd: 'reroll' })}>
              <Icon id="res-scrap" size={14} /> {t('research.draw.reroll', { n: r.reroll_cost })}
            </button>
            <span className="small faint">{t('research.draw.reroll_note')}</span>
          </div>
        </>
      ) : (
        <div className="stack" style={{ gap: 6 }}>
          <div className="row">
            {r.left > 0 ? (
              <button className="btn primary" disabled={!r.affordable} onClick={() => run({ cmd: 'research' })}>
                <Icon id="res-intel" size={16} /> {t('research.draw.research', { n: r.cost })}
              </button>
            ) : null}
            <span className="small muted grow">
              {r.left === 0 ? t('research.draw.all_done') : t('research.draw.left', { n: r.left, step: r.step })}
            </span>
          </div>
          <div className="small faint">{t('research.draw.intel_source')}</div>
          {!r.reactions_on ? (
            <div className="small">
              <Icon id="trait-resonance" size={14} /> {t('research.draw.reactions_first')}
            </div>
          ) : null}
        </div>
      )}
      {r.unlock_ring !== null && r.unlock_count > 0 ? (
        <div className="callout rift" style={{ marginTop: 10 }}>
          <Icon id="lock" />
          <span>
            {tn('research.draw.unlock', {
              ring: <span className="num">{r.unlock_ring}</span>,
              n: <span className="num">{r.unlock_count}</span>,
            })}
          </span>
        </div>
      ) : null}
    </Section>
  );
}

function Held() {
  const { view, run } = useCtx();
  const r = view.research;
  const full = r.archive.length >= r.archive_slots;
  return (
    <Section title={t('research.held.title')} icon="rules" tone="good" hint={t('research.held.hint', { n: r.held.length, known: r.known, total: r.total })}>
      {r.held.length === 0 ? <div className="empty">{t('research.held.empty')}</div> : null}
      <div className="stack" style={{ gap: 8 }}>
        {r.held.map((d) => (
          <DoctrineCard key={d.id} d={d} compact>
            {d.archived ? (
              <span className="chip good">{t('research.held.archived')}</span>
            ) : (
              <button className="btn small ghost" disabled={full} title={full ? t('research.held.full_title', { n: r.archive_slots }) : t('research.held.archive_title')} onClick={() => run({ cmd: 'archive', doctrine: d.id, on: true }, t('research.held.archived_toast', { name: d.name }))}>
                {t('research.held.archive')}
              </button>
            )}
          </DoctrineCard>
        ))}
      </div>
    </Section>
  );
}

/** The 知识库: every doctrine ever researched; a few slots say which ones the next cycle starts with. */
export function Archive() {
  const { game, view, run } = useCtx();
  const r = view.research;
  const full = r.archive.length >= r.archive_slots;
  return (
    <Section title={t('research.archive.title')} icon="seal" tone="rift" hint={t('research.archive.hint', { n: r.archive.length, slots: r.archive_slots, have: r.library.length, total: r.total })}>
      {r.library.length === 0 ? (
        <div className="empty">
          {t('research.archive.empty', {
            levels: game.balance.org_perks.archive.map((n) => t('research.archive.level', { n })).join(t('research.archive.level_sep')),
          })}
        </div>
      ) : (
        <div className="small muted" style={{ marginBottom: 6 }}>
          {t('research.archive.note')}
        </div>
      )}
      <div className="stack" style={{ gap: 4 }}>
        {r.library.map((d) => (
          <label key={d.id} className="switch small" style={{ alignItems: 'flex-start' }}>
            <input
              type="checkbox"
              checked={d.archived}
              disabled={!d.archived && full}
              onChange={(e) => run({ cmd: 'archive', doctrine: d.id, on: e.target.checked })}
            />
            <Icon id={d.icon} size={16} />
            <span>
              <b>{d.name}</b> <span className="muted">{d.desc}</span>
            </span>
          </label>
        ))}
      </div>
    </Section>
  );
}

function DoctrineCard({ d, compact, children }: { d: DoctrineView; compact?: boolean; children?: React.ReactNode }) {
  return (
    <div className="card flat">
      <div className="row">
        <Icon id={d.icon} size={20} />
        <b>{d.name}</b>
        <span className={`rarity r${d.rarity}`}>{RARITY[d.rarity] ?? ''}</span>
        {d.new && !d.held ? <span className="chip warn">{t('research.card.new')}</span> : null}
        <span className="grow" />
        {children}
      </div>
      <div className={compact ? 'small muted' : 'small'}>{d.desc}</div>
      {d.links_held.length ? (
        <div className="small good" style={{ marginTop: 2 }}>
          {t('research.card.links', { names: d.links_held.map((n) => t('research.card.link', { name: n })).join('') })}
        </div>
      ) : null}
    </div>
  );
}

function Codex() {
  const { view } = useCtx();
  const r = view.research;
  const seen = r.codex.filter((e) => e.seen).length;
  return (
    <Section title={t('research.codex.title')} icon="trait-resonance" tone="rift" hint={`${seen}/${r.codex.length}`}>
      {!r.reactions_on && view.phase === 'active' ? (
        <div className="small muted" style={{ marginBottom: 8 }}>
          {t('research.codex.locked')}
        </div>
      ) : null}
      <div className="stack" style={{ gap: 8 }}>
        {r.codex.map((e) => (
          <div key={e.id} className={'card flat' + (e.seen ? '' : ' unseen')}>
            <div className="row wrap" style={{ gap: 4 }}>
              <TagChip tag={e.a.id as Tag} />
              <span className="faint">+</span>
              <TagChip tag={e.b.id as Tag} />
              <span className="faint">→</span>
              <b className={e.seen ? '' : 'faint'}>{e.name}</b>
              <span className="grow" />
              {e.seen ? (
                <span className="tiny muted num">
                  {t('research.codex.counts', { cycle: e.this_cycle, total: e.total })}
                </span>
              ) : null}
            </div>
            <div className={'small ' + (e.seen ? '' : 'muted')}>{e.seen ? e.text : t('research.codex.rumor', { text: e.text })}</div>
          </div>
        ))}
      </div>
    </Section>
  );
}
