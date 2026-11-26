import { useState } from 'react';
import { SKILLS } from '../../content/skillsAndTraits.ts';
import { TAG_META } from '../../content/tags.ts';
import { DISTRICTS, OTHERWORLD_DEPTHS } from '../../content/worldNodes.ts';
import { getOperativeEffectiveStats, getSquadLuckDropBonus } from '../../engine/operativeHelpers.ts';
import { gameController, useGameStore } from '../../state/gameStore.ts';
import type { CombatReport } from '../../types/combat.ts';
import { GameTooltip } from './GameTooltip.tsx';
import {
  DistrictTooltipCard,
  EnemyTooltipCard,
  OtherworldTooltipCard,
  SkillTooltipCard,
  TagTooltipCard,
} from './TooltipCards.tsx';

function getContextualCombatTip(
  report: CombatReport,
  hasUnspentSp: boolean,
): string | null {
  if (!report.fullClear) {
    if (hasUnspentSp) {
      return '提示：尚有调查员未分配技能点 (SP)，点亮职业星盘后再出击会更稳。';
    }
    if (report.outcome === 'WIPE') {
      return '提示：遭遇团灭。可去工坊打造装备，或在战术盘调高「警戒掩护 / 战地包扎」与残血紧急撤退的优先级。';
    }
    return '提示：未全波次通关，但撤退指令保留了收益。';
  }
  const zeroPpUnits = report.unitStats.filter((u) => u.ppReactions === 0);
  if (zeroPpUnits.length > 0 && report.totalRounds >= 2) {
    return `战术洞察：${zeroPpUnits.map((u) => u.name).join('、')} 本场未触发 PP 连携。搭配「震击盾撞 → 协频追击」可大幅提速！`;
  }
  return null;
}

export function ExpeditionTab() {
  const { state, selectedSquadId, activeReport } = useGameStore();
  const [logMode, setLogMode] = useState<'summary' | 'all'>('summary');
  const [focusedNodeId, setFocusedNodeId] = useState<string>('district_1');

  const squad =
    state.run.squads.find((s) => s.id === selectedSquadId) ?? state.run.squads[0]!;
  const report = activeReport ?? state.run.lastBattleReport;
  const u = state.hq.story.unfolding;
  const luckBonusPct = Math.round(getSquadLuckDropBonus(state, squad.id) * 100);
  const hasUnspentSp = state.run.roster.some((o) => o.skillPoints > 0);

  const firstUncapturedIdx = DISTRICTS.findIndex(
    (d) => !state.run.districts.find((rt) => rt.id === d.id)?.captured,
  );
  const visibleDistricts =
    firstUncapturedIdx === -1 ? DISTRICTS : DISTRICTS.slice(0, firstUncapturedIdx + 1);
  const hasHiddenDistricts =
    firstUncapturedIdx !== -1 && firstUncapturedIdx + 1 < DISTRICTS.length;

  const firstUnclearedDepthIdx = OTHERWORLD_DEPTHS.findIndex(
    (d) => !state.run.clearedDepths[d.id],
  );
  const visibleDepths =
    firstUnclearedDepthIdx === -1
      ? OTHERWORLD_DEPTHS
      : OTHERWORLD_DEPTHS.slice(0, firstUnclearedDepthIdx + 1);
  const hasHiddenDepths =
    firstUnclearedDepthIdx !== -1 && firstUnclearedDepthIdx + 1 < OTHERWORLD_DEPTHS.length;

  const focusedDistrict = DISTRICTS.find((d) => d.id === focusedNodeId);
  const focusedDepth = OTHERWORLD_DEPTHS.find((d) => d.id === focusedNodeId);
  const previewEnemies =
    focusedDistrict?.waves[focusedDistrict.waves.length - 1] ??
    focusedDepth?.waves[focusedDepth.waves.length - 1] ??
    DISTRICTS[0]!.waves[0]!;

  const tip = report ? getContextualCombatTip(report, hasUnspentSp) : null;
  const unitStatById = new Map((report?.unitStats ?? []).map((s) => [s.id, s]));

  const filteredEntries = (report?.entries ?? []).filter((e) => {
    if (logMode === 'all') return true;
    return (
      e.kind === 'PASSIVE_PP' ||
      e.kind === 'FACILITY_SUPPORT' ||
      e.kind === 'RETREAT' ||
      e.kind === 'VICTORY' ||
      e.kind === 'DEFEAT'
    );
  });

  return (
    <div className="expedition-layout">
      {/* 左栏：紧凑战争迷雾节点列表（详情全靠悬停） */}
      <div className="scroll-col">
        <div className="panel" style={{ padding: '7px 10px' }}>
          <div className="card-title-row">
            <strong>出征编队</strong>
            <div style={{ display: 'flex', gap: '4px', alignItems: 'center' }}>
              {luckBonusPct > 0 && (
                <span className="pill amber mono">LUK爆率 +{luckBonusPct}%</span>
              )}
              <button
                className="small"
                title="调整小队站位与 AP/PP 战术"
                onClick={() => gameController.openSquadFormation(squad.id)}
              >
                编队 ↗
              </button>
            </div>
          </div>
          {state.run.squads.length > 1 ? (
            <div style={{ display: 'flex', gap: '5px', flexWrap: 'wrap' }}>
              {state.run.squads.map((s) => (
                <button
                  key={s.id}
                  className={`small ${s.id === squad.id ? 'primary' : ''}`}
                  onClick={() => gameController.setSelectedSquad(s.id)}
                >
                  {s.name} ({s.members.length})
                </button>
              ))}
            </div>
          ) : (
            <div className="muted" style={{ fontSize: '11.5px' }}>
              {squad.name} · 上阵 {squad.members.length} 人（悬停任意节点查看详单与掉落）
            </div>
          )}
        </div>

        <section className="panel">
          <div className="panel-header">
            <h3>城市街区</h3>
            <span className="pill muted mono">
              {state.run.districts.filter((d) => d.captured).length}/{DISTRICTS.length}
            </span>
          </div>

          {visibleDistricts.map((dist) => {
            const captured =
              state.run.districts.find((d) => d.id === dist.id)?.captured ?? false;
            return (
              <GameTooltip
                key={dist.id}
                as="div"
                content={<DistrictTooltipCard dist={dist} />}
              >
                <div
                  className={`card ${focusedNodeId === dist.id ? 'selected' : ''}`}
                  style={{ cursor: 'pointer' }}
                  onClick={() => setFocusedNodeId(dist.id)}
                >
                  <div className="card-title-row">
                    <strong>{dist.name}</strong>
                    <span className={`pill ${captured ? 'emerald' : 'amber'}`}>
                      {captured ? '已收复' : '前沿'}
                    </span>
                  </div>
                  <div className="card-title-row">
                    <span className="muted mono" style={{ fontSize: '11px' }}>
                      {dist.waves.length}波 · 奖+2地块
                    </span>
                    <button
                      className={`small ${captured ? '' : 'primary'}`}
                      disabled={squad.members.length === 0}
                      onClick={(e) => {
                        e.stopPropagation();
                        setFocusedNodeId(dist.id);
                        gameController.doAssaultDistrict(squad.id, dist.id);
                      }}
                    >
                      {captured ? '演练' : '攻坚'}
                    </button>
                  </div>
                </div>
              </GameTooltip>
            );
          })}

          {hasHiddenDistricts && (
            <div className="card fogged">
              <div className="card-title-row">
                <strong className="muted">??? · 灰雾封锁街区</strong>
                <span className="pill muted">未探明</span>
              </div>
            </div>
          )}
        </section>

        {!u.otherworldUnlocked ? (
          <section className="panel" style={{ opacity: 0.65, borderStyle: 'dashed' }}>
            <div className="panel-header">
              <h3>🔒 异界褶皱</h3>
            </div>
            <div className="muted" style={{ fontSize: '11.5px' }}>
              积累 12 精工零件或推进二环后开启挂机刷宝裂隙。
            </div>
          </section>
        ) : (
          <section className="panel">
            <div className="panel-header">
              <h3>异界探索</h3>
              <span className="pill purple mono">
                {Object.values(state.run.clearedDepths).filter(Boolean).length}/
                {OTHERWORLD_DEPTHS.length}
              </span>
            </div>

            {visibleDepths.map((depth) => {
              const cleared = Boolean(state.run.clearedDepths[depth.id]);
              const patrolling = state.run.squads.find((s) => s.assignedDepthId === depth.id);
              const bench = patrolling ? state.run.patrolBenchmarks[patrolling.id] : null;
              const pct =
                patrolling && bench
                  ? Math.min(100, Math.round((patrolling.patrolProgressSec / bench.durationSec) * 100))
                  : 0;

              return (
                <GameTooltip
                  key={depth.id}
                  as="div"
                  content={<OtherworldTooltipCard depth={depth} cleared={cleared} />}
                >
                  <div
                    className={`card ${focusedNodeId === depth.id ? 'selected' : ''}`}
                    style={{ cursor: 'pointer' }}
                    onClick={() => setFocusedNodeId(depth.id)}
                  >
                    <div className="card-title-row">
                      <strong>{depth.name}</strong>
                      <span className={`pill ${cleared ? 'cyan' : 'purple'}`}>
                        {cleared ? '可巡逻' : '未首通'}
                      </span>
                    </div>
                    <div className="card-title-row">
                      <button
                        className={`small ${cleared ? '' : 'primary'}`}
                        disabled={squad.members.length === 0}
                        onClick={(e) => {
                          e.stopPropagation();
                          setFocusedNodeId(depth.id);
                          gameController.doAssaultOtherworld(squad.id, depth.id);
                        }}
                      >
                        {cleared ? '出击' : '首通攻坚'}
                      </button>

                      {cleared && (
                        <button
                          className={`small ${
                            squad.assignedDepthId === depth.id ? 'danger' : 'primary'
                          }`}
                          disabled={squad.members.length === 0 && squad.assignedDepthId !== depth.id}
                          onClick={(e) => {
                            e.stopPropagation();
                            gameController.doAssignPatrol(
                              squad.id,
                              squad.assignedDepthId === depth.id ? null : depth.id,
                            );
                          }}
                        >
                          {squad.assignedDepthId === depth.id ? '停巡' : '派驻巡逻'}
                        </button>
                      )}
                    </div>

                    {patrolling && bench && (
                      <div style={{ display: 'flex', flexDirection: 'column', gap: '3px' }}>
                        <div className="mono muted" style={{ fontSize: '10.5px', display: 'flex', justifyContent: 'space-between' }}>
                          <span>{patrolling.name} 巡逻中</span>
                          <span>
                            {patrolling.patrolProgressSec.toFixed(0)}/{bench.durationSec}s
                          </span>
                        </div>
                        <div className="progress-track">
                          <div className="progress-fill" style={{ width: `${pct}%` }} />
                        </div>
                      </div>
                    )}
                  </div>
                </GameTooltip>
              );
            })}

            {hasHiddenDepths && (
              <div className="card fogged">
                <div className="card-title-row">
                  <strong className="muted">??? · 深层褶皱</strong>
                  <span className="pill muted">未打通</span>
                </div>
              </div>
            )}
          </section>
        )}
      </div>

      {/* 右栏：战术对抗舞台 + 可扫读战报流水 */}
      <div className="scroll-col">
        <section className="arena-stage">
          <div className="card-title-row">
            <strong>
              战术焦点：
              {focusedDistrict?.name ?? focusedDepth?.name ?? '一环 · 老商业街口'}
            </strong>
            {report && (
              <div style={{ display: 'flex', gap: '6px', flexWrap: 'wrap' }}>
                <span
                  className={`pill ${
                    report.fullClear
                      ? 'emerald'
                      : report.outcome === 'PLANNED' || report.outcome === 'EMERGENCY'
                        ? 'amber'
                        : 'red'
                  }`}
                >
                  {report.fullClear
                    ? '★ 全波次凯旋 100%'
                    : report.outcome === 'PLANNED'
                      ? `⚑ 波间收队 (${Math.round(report.yieldFactor * 100)}%)`
                      : report.outcome === 'EMERGENCY'
                        ? '⚠ 紧急撤退 60% / 0.5x掉率'
                        : '✖ 战斗败退 30%'}
                </span>
                <span className="pill mono">
                  波次 {report.wavesCleared}/{report.totalWaves} · {report.totalRounds}回合 ·{' '}
                  {report.durationSec}s
                </span>
              </div>
            )}
          </div>

          <div className="arena-versus">
            {/* 己方阵型棋子 */}
            <div className="arena-side">
              <div className="muted" style={{ fontSize: '11px' }}>
                己方出征阵型（悬停技能可查详情）
              </div>
              {squad.members.map((m) => {
                const op = state.run.roster.find((o) => o.id === m.operativeId);
                if (!op) return null;
                const eff = getOperativeEffectiveStats(op, state.hq);
                const uStat = unitStatById.get(op.id);
                const hpPct = uStat ? uStat.hpEndPct : 100;
                return (
                  <div
                    key={op.id}
                    className={`unit-token ${uStat && !uStat.aliveAtEnd ? 'ko' : ''}`}
                  >
                    <div className="card-title-row">
                      <span>
                        <span className="muted" style={{ fontSize: '10.5px' }}>
                          [{m.row === 'front' ? '前' : '后'}]
                        </span>{' '}
                        <strong>{op.name}</strong>
                      </span>
                      <div style={{ display: 'flex', gap: '4px', alignItems: 'center' }}>
                        <span className="mono" style={{ fontSize: '10.5px', color: 'var(--cyan)' }}>
                          AP{eff.maxAp}/PP{eff.maxPp}
                        </span>
                        <button
                          className="small"
                          style={{ padding: '1px 5px', fontSize: '10px' }}
                          title="查看调查员档案与星盘"
                          onClick={() => gameController.openOperativeDossier(op.id)}
                        >
                          档案
                        </button>
                      </div>
                    </div>
                    <div className="hp-track">
                      <div
                        className={`hp-fill ${
                          hpPct === 0 ? 'dead' : hpPct < 45 ? 'low' : ''
                        }`}
                        style={{ width: `${hpPct}%` }}
                      />
                    </div>
                    <div className="card-title-row" style={{ fontSize: '10.5px' }}>
                      {uStat ? (
                        <span className="mono muted">
                          伤{uStat.damageDealt} 盾疗{uStat.healingShieldDone} PP{uStat.ppReactions}
                        </span>
                      ) : (
                        <span className="mono muted">HP {eff.maxHp} · ATK {eff.atk}</span>
                      )}
                      <div style={{ display: 'flex', gap: '3px', flexWrap: 'wrap' }}>
                        {eff.skillIds.slice(0, 2).map((sid) => (
                          <GameTooltip key={sid} content={<SkillTooltipCard skillId={sid} />}>
                            <span className="pill cyan" style={{ cursor: 'help', fontSize: '10px' }}>
                              {SKILLS[sid]?.name ?? sid}
                            </span>
                          </GameTooltip>
                        ))}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>

            <div style={{ textAlign: 'center', fontWeight: 700, color: 'var(--dim)' }}>VS</div>

            {/* 敌方阵型棋子（悬停查看完整数值与技能） */}
            <div className="arena-side">
              <div className="muted" style={{ fontSize: '11px' }}>
                敌方主力样本（悬停查技能）
              </div>
              {previewEnemies.map((enemy, idx) => (
                <GameTooltip key={idx} as="div" content={<EnemyTooltipCard enemy={enemy} />}>
                  <div className="unit-token enemy" style={{ cursor: 'help' }}>
                    <div className="card-title-row">
                      <span>
                        <span className="muted" style={{ fontSize: '10.5px' }}>
                          [{enemy.row === 'front' ? '前' : '后'}]
                        </span>{' '}
                        <strong style={{ color: '#fca5a5' }}>{enemy.name}</strong>
                      </span>
                      <span className="mono muted" style={{ fontSize: '10.5px' }}>
                        HP {enemy.hp} · ATK {enemy.atk}
                      </span>
                    </div>
                    <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap' }}>
                      {enemy.skillIds.map((sid) => (
                        <span key={sid} className="pill red" style={{ fontSize: '10px' }}>
                          {SKILLS[sid]?.name ?? sid}
                        </span>
                      ))}
                    </div>
                  </div>
                </GameTooltip>
              ))}
            </div>
          </div>

          {tip && (
            <div
              style={{
                background: 'rgba(251, 191, 36, 0.1)',
                border: '1px solid rgba(251, 191, 36, 0.45)',
                borderRadius: '5px',
                padding: '5px 8px',
                color: '#fde68a',
                fontSize: '11.5px',
              }}
            >
              {tip}
            </div>
          )}
        </section>

        {/* 战报时间线 */}
        <section className="panel" style={{ flex: 1 }}>
          <div className="panel-header">
            <h3>战报触发流水</h3>
            {report && (
              <div style={{ display: 'flex', gap: '5px' }}>
                <button
                  className={`small ${logMode === 'summary' ? 'primary' : ''}`}
                  onClick={() => setLogMode('summary')}
                >
                  关键连携摘要
                </button>
                <button
                  className={`small ${logMode === 'all' ? 'primary' : ''}`}
                  onClick={() => setLogMode('all')}
                >
                  完整明细 ({report.entries.length})
                </button>
              </div>
            )}
          </div>

          {!report ? (
            <div className="muted" style={{ fontSize: '12px' }}>
              点击左侧「攻坚」发起战斗后，此处会高亮展示 PP 反应链与基地设施联动触发；悬停任意标签可实时查看词条规则。
            </div>
          ) : (
            <div className="battle-log">
              {filteredEntries.map((entry, idx) => (
                <div key={idx} className={`log-entry log-${entry.kind}`}>
                  <span>{entry.text}</span>
                  {entry.tags && entry.tags.length > 0 && (
                    <span style={{ marginLeft: '6px', display: 'inline-flex', gap: '4px' }}>
                      {entry.tags.map((tg, ti) => {
                        const meta = TAG_META[tg];
                        return (
                          <GameTooltip key={`${tg}_${ti}`} content={<TagTooltipCard tagId={tg} />}>
                            <span
                              style={{
                                color: meta.color,
                                background: meta.bgColor,
                                padding: '0 5px',
                                borderRadius: '4px',
                                fontSize: '10.5px',
                                cursor: 'help',
                              }}
                            >
                              {meta.label}
                            </span>
                          </GameTooltip>
                        );
                      })}
                    </span>
                  )}
                </div>
              ))}
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
