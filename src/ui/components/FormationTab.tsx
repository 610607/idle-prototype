import { useMemo, useState } from 'react';
import { CLASSES } from '../../content/classesAndTrees.ts';
import { SKILLS, TACTIC_BLUEPRINTS } from '../../content/skillsAndTraits.ts';
import { TAG_META } from '../../content/tags.ts';
import { DISTRICTS, OTHERWORLD_DEPTHS } from '../../content/worldNodes.ts';
import { simulateExpedition } from '../../engine/combat/simulator.ts';
import {
  buildEnemyUnitInput,
  buildSquadCombatInputs,
  getPoweredFacilitySupportData,
} from '../../engine/economy/baseAndPatrol.ts';
import {
  getClassMasteryStars,
  getOperativeEffectiveStats,
  getOperativeLocation,
  getSquadMemberCap,
} from '../../engine/operativeHelpers.ts';
import { gameController, useGameStore } from '../../state/gameStore.ts';
import type {
  ConditionType,
  GambitActionType,
  GambitRule,
  RowType,
  SlotIndex,
  TagId,
  TargetSelector,
} from '../../types/combat.ts';
import { GameTooltip } from './GameTooltip.tsx';
import {
  ClassTooltipCard,
  SkillTooltipCard,
  TacticRuleTooltipCard,
} from './TooltipCards.tsx';

const CONDITION_OPTIONS: Array<{
  value: ConditionType;
  label: string;
  needsThreshold?: boolean;
  needsTag?: boolean;
  requiredTactic?: string;
}> = [
  { value: 'ALWAYS', label: '无条件执行' },
  { value: 'SELF_HP_BELOW', label: '自身 HP% <', needsThreshold: true },
  { value: 'SELF_HP_ABOVE', label: '自身 HP% >=', needsThreshold: true },
  { value: 'ALLY_LOWEST_HP_BELOW', label: '队友最低 HP% <', needsThreshold: true },
  { value: 'SQUAD_AVG_HP_BELOW', label: '全队平均 HP% <', needsThreshold: true },
  { value: 'FRONT_ROW_ALIVE_BELOW', label: '前排存活 <', needsThreshold: true },
  { value: 'ENEMY_COUNT_AT_LEAST', label: '敌方人数 >=', needsThreshold: true },
  { value: 'WAVE_AT_LEAST', label: '当前波次 >=', needsThreshold: true },
  { value: 'ROUND_AT_LEAST', label: '当前回合 >=', needsThreshold: true },
  {
    value: 'ENEMY_HAS_TAG',
    label: '敌有标签(进阶)',
    needsTag: true,
    requiredTactic: 'tac_cond_tag_check',
  },
  {
    value: 'SELF_HAS_TAG',
    label: '己有标签(进阶)',
    needsTag: true,
    requiredTactic: 'tac_cond_tag_check',
  },
];

const TARGET_OPTIONS: Array<{
  value: TargetSelector;
  label: string;
  requiredTactic?: string;
}> = [
  { value: 'DEFAULT', label: '默认前排' },
  { value: 'FRONT_LOWEST_HP', label: '前排残血' },
  { value: 'LOWEST_HP_ALLY', label: '最低血队友' },
  { value: 'SELF', label: '自身' },
  { value: 'ENEMY_WITH_TAG', label: '带标签敌方' },
  { value: 'BACK_PRIORITY', label: '后排优先(进阶)', requiredTactic: 'tac_target_backline' },
  { value: 'HIGHEST_ATK', label: '最高攻敌(进阶)', requiredTactic: 'tac_target_backline' },
];

const SLOTS: SlotIndex[] = [0, 1, 2];

export function FormationTab() {
  const { state, selectedSquadId, selectedOperativeId } = useGameStore();
  const [slotPicker, setSlotPicker] = useState<{ row: RowType; slot: SlotIndex } | null>(null);
  const [previewTargetId, setPreviewTargetId] = useState<string>('district_1');

  const squad =
    state.run.squads.find((s) => s.id === selectedSquadId) ?? state.run.squads[0]!;
  const memberCap = getSquadMemberCap(state);
  const u = state.hq.story.unfolding;

  const squadOps = useMemo(
    () =>
      squad.members
        .map((m) => {
          const op = state.run.roster.find((o) => o.id === m.operativeId);
          return op ? { member: m, op } : null;
        })
        .filter((x): x is NonNullable<typeof x> => x !== null),
    [squad.members, state.run.roster],
  );

  const selectedOp =
    state.run.roster.find((o) => o.id === selectedOperativeId) ??
    squadOps[0]?.op ??
    state.run.roster[0] ??
    null;

  const selectedOpInSquad = selectedOp
    ? squad.members.some((m) => m.operativeId === selectedOp.id)
    : false;

  const unlockedTacticSet = useMemo(
    () => new Set(state.run.selectedLoadout.tactics),
    [state.run.selectedLoadout.tactics],
  );

  const previewReport = useMemo(() => {
    const allies = buildSquadCombatInputs(state, squad.id);
    if (allies.length === 0) return null;
    const dist = DISTRICTS.find((d) => d.id === previewTargetId);
    const depth = OTHERWORLD_DEPTHS.find((d) => d.id === previewTargetId);
    const waves = dist?.waves ?? depth?.waves;
    if (!waves) return null;

    const support = getPoweredFacilitySupportData(state);
    return simulateExpedition({
      allies,
      enemiesByWave: waves.map((w, wi) =>
        w.map((e, ei) => buildEnemyUnitInput(e, `prev_${wi}_${ei}`)),
      ),
      baseTravelSec: depth?.baseTravelSec ?? 10,
      poweredFacilities: support.levels,
      supportBonusPct: support.bonusPct,
      modifierId: state.run.activeModifierId,
    });
  }, [state, squad.id, previewTargetId]);

  const updateGambitRule = (idx: number, patch: Partial<GambitRule>) => {
    if (!selectedOp) return;
    const next = selectedOp.gambits.map((g, i) => (i === idx ? { ...g, ...patch } : g));
    gameController.doUpdateGambits(selectedOp.id, next);
  };

  const moveGambitRule = (idx: number, dir: -1 | 1) => {
    if (!selectedOp) return;
    const targetIdx = idx + dir;
    if (targetIdx < 0 || targetIdx >= selectedOp.gambits.length) return;
    const next = [...selectedOp.gambits];
    const tmp = next[idx]!;
    next[idx] = next[targetIdx]!;
    next[targetIdx] = tmp;
    gameController.doUpdateGambits(selectedOp.id, next);
  };

  const deleteGambitRule = (idx: number) => {
    if (!selectedOp) return;
    const next = selectedOp.gambits.filter((_, i) => i !== idx);
    gameController.doUpdateGambits(selectedOp.id, next);
  };

  const addGambitRule = (kind: 'active' | 'passive' | 'retreat') => {
    if (!selectedOp) return;
    const eff = getOperativeEffectiveStats(selectedOp, state.hq);
    const candidates = eff.skillIds.filter((sid) => {
      const sk = SKILLS[sid];
      return kind === 'passive' ? sk?.kind === 'passive' : sk?.kind === 'active';
    });
    const newRule: GambitRule =
      kind === 'retreat'
        ? {
            id: `g_${Date.now()}_${selectedOp.gambits.length}`,
            enabled: true,
            actionType: 'RETREAT_EMERGENCY',
            condition1: { type: 'SQUAD_AVG_HP_BELOW', threshold: 35 },
            target: 'SELF',
          }
        : {
            id: `g_${Date.now()}_${selectedOp.gambits.length}`,
            enabled: true,
            actionType: 'SKILL',
            skillId: candidates[0] ?? eff.skillIds[0] ?? 'baton_strike',
            condition1: { type: 'ALWAYS' },
            target: kind === 'passive' ? 'LOWEST_HP_ALLY' : 'DEFAULT',
          };
    gameController.doUpdateGambits(selectedOp.id, [...selectedOp.gambits, newRule]);
  };

  const renderRowSlots = (row: RowType, label: string) => (
    <div>
      <div className="muted" style={{ fontSize: '11px', marginBottom: '2px' }}>
        {label}
      </div>
      <div className="row-grid">
        {SLOTS.map((slot) => {
          const member = squad.members.find((m) => m.row === row && m.slot === slot);
          const op = member
            ? state.run.roster.find((o) => o.id === member.operativeId)
            : undefined;
          const eff = op ? getOperativeEffectiveStats(op, state.hq) : null;
          const isSelected = op && selectedOp?.id === op.id;

          return (
            <div
              key={`${row}_${slot}`}
              className={`slot-box ${op ? 'occupied' : ''} ${isSelected ? 'selected' : ''}`}
            >
              {op && eff ? (
                <>
                  <div
                    style={{ cursor: 'pointer' }}
                    onClick={() => gameController.setSelectedOperative(op.id)}
                  >
                    <div style={{ fontWeight: 600, fontSize: '11.5px' }}>
                      <span className={`rarity-${op.rarity}`}>[{op.rarity}]</span> {op.name}
                    </div>
                    <div className="mono muted" style={{ fontSize: '10px' }}>
                      AP{eff.maxAp}/PP{eff.maxPp}
                    </div>
                  </div>
                  <div style={{ display: 'flex', gap: '3px' }}>
                    <button
                      className="small"
                      style={{ flex: 1, fontSize: '10.5px' }}
                      title="打开调查员档案与配装"
                      onClick={() => gameController.openOperativeDossier(op.id)}
                    >
                      档案 ↗
                    </button>
                    <button
                      className="small"
                      style={{ fontSize: '10.5px' }}
                      title="移出本槽位"
                      onClick={() => gameController.doRemoveSquadMember(squad.id, op.id)}
                    >
                      ×
                    </button>
                  </div>
                </>
              ) : (
                <button
                  className="small"
                  style={{ height: '100%', background: 'transparent', fontSize: '11px' }}
                  disabled={squad.members.length >= memberCap}
                  onClick={() => setSlotPicker({ row, slot })}
                >
                  {squad.members.length >= memberCap ? '满' : '+'}
                </button>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );

  const selectedEff = selectedOp
    ? getOperativeEffectiveStats(selectedOp, state.hq)
    : null;
  const selectedClass = selectedOp ? CLASSES[selectedOp.classId] : null;
  const selectedLoc = selectedOp ? getOperativeLocation(state, selectedOp.id) : null;

  const renderGambitRow = (rule: GambitRule, idx: number, effSkillIds: string[], cap: number) => {
    const sk = rule.skillId ? SKILLS[rule.skillId] : null;
    const condMeta1 = CONDITION_OPTIONS.find((c) => c.value === rule.condition1.type);
    const condMeta2 = rule.condition2
      ? CONDITION_OPTIONS.find((c) => c.value === rule.condition2?.type)
      : null;
    const plannedUnlocked = unlockedTacticSet.has('tac_dir_planned_retreat');
    const overCap = idx >= cap;

    return (
      <div
        key={rule.id}
        className="gambit-line"
        style={overCap ? { opacity: 0.45, borderStyle: 'dashed' } : undefined}
      >
        <GameTooltip content={<TacticRuleTooltipCard rule={rule} />}>
          <span
            className="pill mono"
            style={{ cursor: 'help', borderColor: 'rgba(56,189,248,0.45)' }}
          >
            #{idx + 1} 🔍
          </span>
        </GameTooltip>

        <input
          type="checkbox"
          checked={rule.enabled}
          title="启用此战术"
          onChange={(e) => updateGambitRule(idx, { enabled: e.target.checked })}
        />

        <select
          value={rule.actionType}
          onChange={(e) => {
            const act = e.target.value as GambitActionType;
            updateGambitRule(idx, {
              actionType: act,
              skillId: act === 'SKILL' ? rule.skillId ?? effSkillIds[0] : undefined,
            });
          }}
        >
          <option value="SKILL">技能</option>
          <option value="RETREAT_EMERGENCY">紧急撤退(60%)</option>
          <option
            value="RETREAT_PLANNED"
            disabled={!plannedUnlocked && rule.actionType !== 'RETREAT_PLANNED'}
          >
            计划收队(100%){!plannedUnlocked ? '[锁]' : ''}
          </option>
        </select>

        {rule.actionType === 'SKILL' && (
          <GameTooltip
            content={sk ? <SkillTooltipCard skillId={sk.id} /> : null}
            disabled={!sk}
          >
            <select
              value={rule.skillId ?? ''}
              onChange={(e) => updateGambitRule(idx, { skillId: e.target.value })}
            >
              {effSkillIds.map((sid) => {
                const sDef = SKILLS[sid];
                return (
                  <option key={sid} value={sid}>
                    {sDef ? `${sDef.name} (${sDef.kind === 'passive' ? 'PP' : 'AP'})` : sid}
                  </option>
                );
              })}
            </select>
          </GameTooltip>
        )}

        <span className="muted">若</span>
        <select
          value={rule.condition1.type}
          onChange={(e) => {
            const val = e.target.value as ConditionType;
            const meta = CONDITION_OPTIONS.find((c) => c.value === val);
            updateGambitRule(idx, {
              condition1: {
                type: val,
                threshold: meta?.needsThreshold ? 50 : undefined,
                tag: meta?.needsTag ? 'shocked' : undefined,
              },
            });
          }}
        >
          {CONDITION_OPTIONS.map((opt) => {
            const locked =
              opt.requiredTactic &&
              !unlockedTacticSet.has(opt.requiredTactic) &&
              rule.condition1.type !== opt.value;
            return (
              <option key={opt.value} value={opt.value} disabled={Boolean(locked)}>
                {opt.label}
                {locked ? `[需:${TACTIC_BLUEPRINTS[opt.requiredTactic!]?.name}]` : ''}
              </option>
            );
          })}
        </select>

        {condMeta1?.needsThreshold && (
          <input
            type="number"
            style={{ width: '50px' }}
            value={rule.condition1.threshold ?? 50}
            onChange={(e) =>
              updateGambitRule(idx, {
                condition1: { ...rule.condition1, threshold: Number(e.target.value) },
              })
            }
          />
        )}

        {condMeta1?.needsTag && (
          <select
            value={rule.condition1.tag ?? 'shocked'}
            onChange={(e) =>
              updateGambitRule(idx, {
                condition1: { ...rule.condition1, tag: e.target.value as TagId },
              })
            }
          >
            {Object.values(TAG_META).map((tm) => (
              <option key={tm.id} value={tm.id}>
                {tm.label}
              </option>
            ))}
          </select>
        )}

        <span className="muted">▸</span>
        <select
          value={rule.target}
          onChange={(e) =>
            updateGambitRule(idx, { target: e.target.value as TargetSelector })
          }
        >
          {TARGET_OPTIONS.map((opt) => {
            const locked =
              opt.requiredTactic &&
              !unlockedTacticSet.has(opt.requiredTactic) &&
              rule.target !== opt.value;
            return (
              <option key={opt.value} value={opt.value} disabled={Boolean(locked)}>
                {opt.label}
              </option>
            );
          })}
        </select>

        {rule.condition2 ? (
          <>
            <span className="muted">且</span>
            <select
              value={rule.condition2.type}
              onChange={(e) => {
                const val = e.target.value as ConditionType;
                const meta = CONDITION_OPTIONS.find((c) => c.value === val);
                updateGambitRule(idx, {
                  condition2: {
                    type: val,
                    threshold: meta?.needsThreshold ? 2 : undefined,
                    tag: meta?.needsTag ? 'shocked' : undefined,
                  },
                });
              }}
            >
              {CONDITION_OPTIONS.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
            {condMeta2?.needsThreshold && (
              <input
                type="number"
                style={{ width: '48px' }}
                value={rule.condition2.threshold ?? 2}
                onChange={(e) =>
                  updateGambitRule(idx, {
                    condition2: { ...rule.condition2!, threshold: Number(e.target.value) },
                  })
                }
              />
            )}
            <button
              className="small"
              title="移除条件2"
              onClick={() => updateGambitRule(idx, { condition2: undefined })}
            >
              -且
            </button>
          </>
        ) : (
          <button
            className="small"
            title="附加第二条件"
            onClick={() =>
              updateGambitRule(idx, {
                condition2: { type: 'WAVE_AT_LEAST', threshold: 2 },
              })
            }
          >
            +且
          </button>
        )}

        <div style={{ marginLeft: 'auto', display: 'flex', gap: '2px' }}>
          <button
            className="small"
            disabled={idx === 0}
            onClick={() => moveGambitRule(idx, -1)}
          >
            ↑
          </button>
          <button
            className="small"
            disabled={idx === (selectedOp?.gambits.length ?? 1) - 1}
            onClick={() => moveGambitRule(idx, 1)}
          >
            ↓
          </button>
          <button className="small danger" onClick={() => deleteGambitRule(idx)}>
            ×
          </button>
        </div>
      </div>
    );
  };

  return (
    <div className="squad-layout">
      {/* 左栏：站位棋盘 + 快速战前演练 */}
      <div className="scroll-col">
        <section className="panel">
          <div className="panel-header">
            <h3>站位棋盘 ({squad.members.length}/{memberCap})</h3>
            {state.run.squads.length > 1 ? (
              <select
                value={squad.id}
                onChange={(e) => {
                  const nextId = e.target.value;
                  gameController.setSelectedSquad(nextId);
                  const nextSq = state.run.squads.find((s) => s.id === nextId);
                  if (nextSq?.members[0]) {
                    gameController.setSelectedOperative(nextSq.members[0].operativeId);
                  }
                }}
              >
                {state.run.squads.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.name} ({s.members.length}/{memberCap})
                  </option>
                ))}
              </select>
            ) : (
              <span className="pill muted">{squad.name}</span>
            )}
          </div>

          {renderRowSlots('back', '后排')}
          {renderRowSlots('front', '前排')}

          {slotPicker && (
            <div className="card" style={{ borderColor: 'var(--cyan)' }}>
              <div className="card-title-row">
                <strong>
                  指派至{slotPicker.row === 'front' ? '前' : '后'}#{slotPicker.slot + 1}
                </strong>
                <button className="small" onClick={() => setSlotPicker(null)}>
                  取消
                </button>
              </div>
              {state.run.roster.map((op) => {
                const loc = getOperativeLocation(state, op.id);
                return (
                  <button
                    key={op.id}
                    className="small"
                    style={{ textAlign: 'left', display: 'flex', justifyContent: 'space-between' }}
                    onClick={() => {
                      gameController.doAssignSquadMember(
                        squad.id,
                        op.id,
                        slotPicker.row,
                        slotPicker.slot,
                      );
                      gameController.setSelectedOperative(op.id);
                      setSlotPicker(null);
                    }}
                  >
                    <span>
                      [{op.rarity}] {op.name}
                    </span>
                    <span className="muted">{loc.label}</span>
                  </button>
                );
              })}
            </div>
          )}

          <button
            className="small"
            onClick={() =>
              selectedOp
                ? gameController.openOperativeDossier(selectedOp.id)
                : gameController.setTab('operatives')
            }
          >
            👤 前往「调查员」筛选花名册 / 调配驻扎 ↗
          </button>
        </section>

        <section className="panel">
          <div className="panel-header">
            <h3>快速战前演练</h3>
          </div>
          <select value={previewTargetId} onChange={(e) => setPreviewTargetId(e.target.value)}>
            {DISTRICTS.map((d) => (
              <option key={d.id} value={d.id}>
                {d.name}
              </option>
            ))}
            {u.otherworldUnlocked &&
              OTHERWORLD_DEPTHS.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.name}
                </option>
              ))}
          </select>
          {previewReport ? (
            <div className="card-title-row">
              <span
                className={`pill ${
                  previewReport.outcome === 'PLANNED'
                    ? 'emerald'
                    : previewReport.outcome === 'EMERGENCY'
                      ? 'amber'
                      : 'red'
                }`}
              >
                {previewReport.fullClear
                  ? '全清 100%'
                  : previewReport.outcome === 'PLANNED'
                    ? '波间收队'
                    : previewReport.outcome === 'EMERGENCY'
                      ? '紧急撤退 60%'
                      : '败退 30%'}
              </span>
              <button
                className="small primary"
                onClick={() => gameController.setActiveReport(previewReport, true)}
              >
                看战报 →
              </button>
            </div>
          ) : (
            <div className="muted" style={{ fontSize: '11.5px' }}>
              本小队暂无上阵成员，点击空槽 `+` 即可编入。
            </div>
          )}
        </section>
      </div>

      {/* 右主舞台：本小队成员快捷切换 + AP / PP 战术盘 */}
      <div className="scroll-col">
        <section className="panel" style={{ flex: 1 }}>
          <div className="panel-header" style={{ flexWrap: 'wrap', gap: '6px' }}>
            <div style={{ display: 'flex', gap: '5px', flexWrap: 'wrap', alignItems: 'center' }}>
              <strong style={{ fontSize: '12.5px' }}>编队成员：</strong>
              {squadOps.length === 0 ? (
                <span className="muted" style={{ fontSize: '12px' }}>
                  空编队
                </span>
              ) : (
                squadOps.map(({ member, op }) => {
                  const isSel = selectedOp?.id === op.id;
                  return (
                    <button
                      key={op.id}
                      className={`small ${isSel ? 'primary' : ''}`}
                      onClick={() => gameController.setSelectedOperative(op.id)}
                    >
                      [{member.row === 'front' ? '前' : '后'}#{member.slot + 1}] {op.name}
                    </button>
                  );
                })
              )}
            </div>

            {selectedOp && (
              <button
                className="small primary"
                onClick={() => gameController.openOperativeDossier(selectedOp.id)}
              >
                👤 打开「{selectedOp.name}」档案与星盘 ↗
              </button>
            )}
          </div>

          {!selectedOp || !selectedEff ? (
            <div className="muted">选择一名小队成员编辑 AP / PP 条件战术。</div>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '9px' }}>
              <div className="card" style={{ padding: '7px 10px' }}>
                <div className="card-title-row" style={{ flexWrap: 'wrap', gap: '6px' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '6px', flexWrap: 'wrap' }}>
                    <span className={`pill rarity-${selectedOp.rarity}`}>{selectedOp.rarity}</span>
                    <strong>{selectedOp.name}</strong>
                    {selectedClass && (
                      <GameTooltip
                        content={
                          <ClassTooltipCard
                            classId={selectedClass.id}
                            stars={getClassMasteryStars(selectedOp.classMastery[selectedClass.id] ?? 0)}
                          />
                        }
                      >
                        <span className="pill cyan" style={{ cursor: 'help' }}>
                          {selectedClass.icon} {selectedClass.name}
                        </span>
                      </GameTooltip>
                    )}
                    <span className="mono pill">
                      AP{selectedEff.maxAp} / PP{selectedEff.maxPp}
                    </span>
                    {!selectedOpInSquad && selectedLoc && (
                      <span className="pill amber">
                        当前处于「{selectedLoc.label}」，尚未编入{squad.name}
                      </span>
                    )}
                  </div>

                  <span className="mono" style={{ fontSize: '11.5px' }}>
                    已配规则 <strong>{selectedOp.gambits.length}</strong> / 上限{' '}
                    <strong>{selectedEff.gambitSlotCap}</strong>
                  </span>
                </div>

                {/* 可用技能快捷悬停栏 */}
                <div style={{ display: 'flex', gap: '5px', flexWrap: 'wrap', alignItems: 'center' }}>
                  <span className="muted" style={{ fontSize: '11px' }}>
                    已掌握技能：
                  </span>
                  {selectedEff.skillIds.map((sid) => {
                    const sk = SKILLS[sid];
                    if (!sk) return null;
                    return (
                      <GameTooltip key={sid} content={<SkillTooltipCard skillId={sid} />}>
                        <span
                          className={`pill ${sk.kind === 'passive' ? 'purple' : 'cyan'}`}
                          style={{ cursor: 'help' }}
                        >
                          {sk.kind === 'passive' ? 'PP' : 'AP'} · {sk.name}
                        </span>
                      </GameTooltip>
                    );
                  })}
                </div>
              </div>

              <div className="card-title-row">
                <span className="muted" style={{ fontSize: '11.5px' }}>
                  提示：战斗按 AP/PP 分轨从 #1 自上而下判定，满足「若」与可选「且」条件即执行。
                </span>
                <div style={{ display: 'flex', gap: '5px' }}>
                  <button
                    className="small primary"
                    disabled={selectedOp.gambits.length >= selectedEff.gambitSlotCap}
                    onClick={() => addGambitRule('active')}
                  >
                    + AP主动
                  </button>
                  <button
                    className="small"
                    disabled={selectedOp.gambits.length >= selectedEff.gambitSlotCap}
                    onClick={() => addGambitRule('passive')}
                  >
                    + PP被动
                  </button>
                  <button
                    className="small"
                    disabled={selectedOp.gambits.length >= selectedEff.gambitSlotCap}
                    onClick={() => addGambitRule('retreat')}
                  >
                    + 撤退
                  </button>
                </div>
              </div>

              {/* AP 主动行动区 */}
              <div className="tactics-group ap">
                <strong style={{ color: 'var(--cyan)', fontSize: '12px' }}>
                  🔵 主动行动优先级 (AP) — 轮到自身回合时自上而下扫描
                </strong>
                {selectedOp.gambits.map((rule, idx) => {
                  const sk = rule.skillId ? SKILLS[rule.skillId] : null;
                  if (rule.actionType !== 'SKILL' || sk?.kind === 'passive') return null;
                  return renderGambitRow(
                    rule,
                    idx,
                    selectedEff.skillIds,
                    selectedEff.gambitSlotCap,
                  );
                })}
              </div>

              {/* PP 被动与撤退区 */}
              <div className="tactics-group pp">
                <strong style={{ color: 'var(--purple)', fontSize: '12px' }}>
                  🟣 被动反应 (PP) 与撤退指令 — 响应掩护/追击或危机撤退
                </strong>
                {selectedOp.gambits.map((rule, idx) => {
                  const sk = rule.skillId ? SKILLS[rule.skillId] : null;
                  if (rule.actionType === 'SKILL' && sk?.kind !== 'passive') return null;
                  return renderGambitRow(
                    rule,
                    idx,
                    selectedEff.skillIds,
                    selectedEff.gambitSlotCap,
                  );
                })}
              </div>
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
