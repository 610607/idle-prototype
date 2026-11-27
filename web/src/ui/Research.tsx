import { Icon } from '../icons';
import type { DoctrineView, Tag } from '../types';
import { Section, TagChip, useCtx } from './common';

const RARITY = ['', '常见', '少见', '罕见'];

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
    <Section title="研究" icon="trait-book" hint={`情报 ${view.resources.intel}`}>
      {r.offers.length ? (
        <>
          <div className="small muted" style={{ marginBottom: 6 }}>
            三条里挑一条。挑中的条令这一轮里对所有路线战和守城战生效。
          </div>
          <div className="stack" style={{ gap: 8 }}>
            {r.offers.map((d) => (
              <DoctrineCard key={d.id} d={d}>
                <button className="btn small primary" onClick={() => run({ cmd: 'pick_doctrine', doctrine: d.id }, `拿到条令「${d.name}」`)}>
                  要这条
                </button>
              </DoctrineCard>
            ))}
          </div>
          <div className="row" style={{ marginTop: 8 }}>
            <button className="btn small" disabled={!r.reroll_ok} onClick={() => run({ cmd: 'reroll' })}>
              <Icon id="res-scrap" size={14} /> 换一批（{r.reroll_cost} 废材）
            </button>
            <span className="small faint">换掉这三条；本轮每换一次贵一些。</span>
          </div>
        </>
      ) : (
        <div className="stack" style={{ gap: 6 }}>
          <div className="row">
            <button className="btn primary" disabled={!r.affordable || r.left === 0} onClick={() => run({ cmd: 'research' })}>
              <Icon id="res-intel" size={16} /> 研究（{r.cost} 情报）
            </button>
            <span className="small muted grow">
              {r.left === 0 ? '这一轮能研究的条令都拿到了' : `还有 ${r.left} 条可研究；每次研究价格 +${r.step}`}
            </span>
          </div>
          <div className="small faint">情报来自档案室（文员）、线索格和守城后的残留。</div>
          {!r.reactions_on ? (
            <div className="small">
              <Icon id="trait-resonance" size={14} /> 先拿「反应协议」：它打开标签反应表，别的条令大多接在它上面才会相乘。
            </div>
          ) : null}
        </div>
      )}
    </Section>
  );
}

function Held() {
  const { view, run } = useCtx();
  const r = view.research;
  const full = r.archive.length >= r.archive_slots;
  return (
    <Section title="本轮条令" icon="rules" hint={`${r.held.length} 条 · 图鉴 ${r.known}/${r.total}`}>
      {r.held.length === 0 ? <div className="empty">还没有条令。研究一次就有了。</div> : null}
      <div className="stack" style={{ gap: 8 }}>
        {r.held.map((d) => (
          <DoctrineCard key={d.id} d={d} compact>
            {d.archived ? (
              <span className="chip good">在知识库</span>
            ) : (
              <button className="btn small ghost" disabled={full} title={full ? '知识库满了' : '下一轮开局就带着'} onClick={() => run({ cmd: 'archive', doctrine: d.id, on: true }, `「${d.name}」收进知识库`)}>
                收进知识库
              </button>
            )}
          </DoctrineCard>
        ))}
      </div>
    </Section>
  );
}

function Archive() {
  const { view, run } = useCtx();
  const r = view.research;
  return (
    <Section title="知识库" icon="seal" hint={`${r.archive.length}/${r.archive_slots}`}>
      {r.archive.length === 0 ? <div className="empty">收进知识库的条令，之后每一轮开局就带着。组织 Lv.3 和 Lv.5 各多一格。</div> : null}
      <div className="stack" style={{ gap: 6 }}>
        {r.archive.map((d) => (
          <div key={d.id} className="row small">
            <Icon id={d.icon} size={16} />
            <b>{d.name}</b>
            <span className="grow muted">{d.desc}</span>
            <button className="btn small ghost" onClick={() => run({ cmd: 'archive', doctrine: d.id, on: false })}>
              取出
            </button>
          </div>
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
        {d.new && !d.held ? <span className="chip warn">新</span> : null}
        <span className="grow" />
        {children}
      </div>
      <div className={compact ? 'small muted' : 'small'}>{d.desc}</div>
      {d.links_held.length ? (
        <div className="small good" style={{ marginTop: 2 }}>
          接得上：{d.links_held.map((n) => `「${n}」`).join('')}
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
    <Section title="反应图鉴" icon="trait-resonance" hint={`${seen}/${r.codex.length}`}>
      {!r.reactions_on && view.phase === 'active' ? (
        <div className="small muted" style={{ marginBottom: 8 }}>
          反应表还没打开。研究到「反应协议」以后，两种标签在同一个敌人身上相遇就会起反应，第一次触发会点亮这里。
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
                  本轮 {e.this_cycle} · 累计 {e.total}
                </span>
              ) : null}
            </div>
            <div className={'small ' + (e.seen ? '' : 'muted')}>{e.seen ? e.text : `传闻：${e.text}`}</div>
          </div>
        ))}
      </div>
    </Section>
  );
}
