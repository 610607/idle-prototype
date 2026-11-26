import { useEffect, useMemo, useState } from 'react';
import { CLASSES, CLASS_IDS } from '../../content/classesAndTrees.ts';
import { EQUIPMENT_TEMPLATES } from '../../content/equipment.ts';
import { FACILITIES } from '../../content/facilities.ts';
import { TRAITS } from '../../content/skillsAndTraits.ts';
import { RECLASS_MANUAL_COST, getBoostOperativeManualCost } from '../../engine/actions.ts';
import {
  getClassMasteryProgressPct,
  getClassMasteryStars,
  getOperativeEffectiveStats,
  getOperativeLocation,
  getOperativeTreeNodes,
  getSquadMemberCap,
  getStationingBonusPct,
  getVaultCapacity,
  getXpToNextLevel,
} from '../../engine/operativeHelpers.ts';
import { gameController, useGameStore } from '../../state/gameStore.ts';
import type { CoCStatKey, RowType, SlotIndex } from '../../types/combat.ts';
import type { EquipmentSlot } from '../../types/game.ts';
import { GameTooltip } from './GameTooltip.tsx';
import { SkillTreeGraph } from './SkillTreeGraph.tsx';
import {
  ClassTooltipCard,
  CoCStatTooltipCard,
  EquipmentTooltipCard,
  TraitTooltipCard,
} from './TooltipCards.tsx';

type OperativesSubTab = 'profile' | 'tree';
type RosterFilter = 'all' | 'squad' | 'station' | 'idle';

const COC_LABELS: Array<{ key: CoCStatKey; name: string; tip: string }> = [
  { key: 'str', name: 'STR 力量', tip: '每高出 40 点 +0.5%/点近战技能倍率；加成柴油发电与废材站驻扎。' },
  { key: 'con', name: 'CON 体质', tip: '推导生命上限与基础防御力；加成发电机房值守。' },
  { key: 'siz', name: 'SIZ 体型', tip: '撑高前排生命厚实度；加成废材分类站搬运效率。' },
  { key: 'dex', name: 'DEX 敏捷', tip: '决定战斗先攻 SPD 与远程伤害倍率；加成改装工坊。' },
  { key: 'app', name: 'APP 外貌', tip: '加成街区联络处高稀有招募率与鼓舞哨令强度。' },
  { key: 'int', name: 'INT 智力', tip: '每 30 点扩展 1 个战术槽上限；加成工坊、研习与支援设施。' },
  { key: 'pow', name: 'POW 意志', tip: '加成侵蚀/异常系技能伤害与联合演武传授速度。' },
  { key: 'edu', name: 'EDU 教育', tip: '≥65 时 Lv.4/8 升级额外 +1 SP；加速档案研习与战地治疗。' },
  { key: 'luk', name: 'LUK 幸运', tip: '提升所在小队远征稀有掉率（不影响战斗确定性）与招募品质。' },
];

const EQ_SLOTS: Array<{ slot: EquipmentSlot; label: string }> = [
  { slot: 'weapon', label: '武器' },
  { slot: 'armor', label: '护甲' },
  { slot: 'module', label: '插件' },
];

const POSITIONS: Array<{ row: RowType; slot: SlotIndex; label: string }> = [
  { row: 'front', slot: 0, label: '前排 #1' },
  { row: 'front', slot: 1, label: '前排 #2' },
  { row: 'front', slot: 2, label: '前排 #3' },
  { row: 'back', slot: 0, label: '后排 #1' },
  { row: 'back', slot: 1, label: '后排 #2' },
  { row: 'back', slot: 2, label: '后排 #3' },
];

function renderStars(count: number): string {
  return '★'.repeat(count) + '☆'.repeat(Math.max(0, 5 - count));
}

function getLocBadgeClass(kind: ReturnType<typeof getOperativeLocation>['kind']): string {
  if (kind === 'squad') return 'cyan';
  if (kind === 'station') return 'emerald';
  if (kind === 'dojo_mentor' || kind === 'dojo_student') return 'amber';
  return 'muted';
}

export function OperativesTab() {
  const { state, selectedSquadId, selectedOperativeId, vaultMarkedUids } = useGameStore();
  const [filter, setFilter] = useState<RosterFilter>('all');
  const [subTab, setSubTab] = useState<OperativesSubTab>('profile');
  const [deploySquadId, setDeploySquadId] = useState<string>(selectedSquadId);
  const [deployPosKey, setDeployPosKey] = useState<string>('front:0');
  const [deployPlotId, setDeployPlotId] = useState<string>('');

  const selectedOp =
    state.run.roster.find((o) => o.id === selectedOperativeId) ?? state.run.roster[0] ?? null;
  const u = state.hq.story.unfolding;
  const memberCap = getSquadMemberCap(state);
  const vaultCap = getVaultCapacity(state.hq);

  const builtPlots = useMemo(
    () => state.run.plots.filter((p) => p.facilityId !== null),
    [state.run.plots],
  );

  const rosterWithLoc = useMemo(
    () =>
      state.run.roster.map((op) => ({
        op,
        loc: getOperativeLocation(state, op.id),
      })),
    [state],
  );

  const counts = useMemo(() => {
    let squad = 0;
    let station = 0;
    let idle = 0;
    for (const item of rosterWithLoc) {
      if (item.loc.kind === 'squad') squad++;
      else if (item.loc.kind === 'station') station++;
      else if (item.loc.kind === 'idle') idle++;
    }
    return { all: rosterWithLoc.length, squad, station, idle };
  }, [rosterWithLoc]);

  const filteredRoster = useMemo(() => {
    if (filter === 'all') return rosterWithLoc;
    return rosterWithLoc.filter(({ loc }) => loc.kind === filter);
  }, [rosterWithLoc, filter]);

  const selectedLoc = selectedOp ? getOperativeLocation(state, selectedOp.id) : null;

  useEffect(() => {
    if (!selectedOp) return;
    const loc = getOperativeLocation(state, selectedOp.id);
    if (loc.kind === 'squad') {
      setDeploySquadId(loc.squadId);
      const sq = state.run.squads.find((s) => s.id === loc.squadId);
      const member = sq?.members.find((m) => m.operativeId === selectedOp.id);
      if (member) {
        setDeployPosKey(`${member.row}:${member.slot}`);
      }
    } else {
      setDeploySquadId(selectedSquadId);
      const sq =
        state.run.squads.find((s) => s.id === selectedSquadId) ?? state.run.squads[0];
      const firstOpen = POSITIONS.find(
        (pos) => !sq?.members.some((m) => m.row === pos.row && m.slot === pos.slot),
      );
      if (firstOpen) {
        setDeployPosKey(`${firstOpen.row}:${firstOpen.slot}`);
      }
    }
    if (loc.kind === 'station') {
      setDeployPlotId(loc.plotId);
    } else if (!deployPlotId || !builtPlots.some((p) => p.plotId === deployPlotId)) {
      setDeployPlotId(builtPlots[0]?.plotId ?? '');
    }
  }, [selectedOp?.id, selectedSquadId]);

  const selectedEff = selectedOp
    ? getOperativeEffectiveStats(selectedOp, state.hq)
    : null;
  const selectedClass = selectedOp ? CLASSES[selectedOp.classId] : null;
  const selectedTrees = selectedOp ? getOperativeTreeNodes(selectedOp) : null;

  const targetSquad =
    state.run.squads.find((s) => s.id === deploySquadId) ?? state.run.squads[0]!;
  const [posRowRaw, posSlotRaw] = deployPosKey.split(':');
  const targetRow: RowType = posRowRaw === 'back' ? 'back' : 'front';
  const targetSlot: SlotIndex = posSlotRaw === '1' ? 1 : posSlotRaw === '2' ? 2 : 0;

  const currentSlotOccupant = selectedOp
    ? targetSquad.members.find((m) => m.row === targetRow && m.slot === targetSlot)
    : undefined;
  const isAlreadyAtTargetSlot =
    Boolean(selectedOp) && currentSlotOccupant?.operativeId === selectedOp?.id;
  const remainingAfterAssign = selectedOp
    ? targetSquad.members.filter(
        (m) =>
          m.operativeId !== selectedOp.id &&
          !(m.row === targetRow && m.slot === targetSlot),
      ).length
    : 0;
  const canAssignTargetSlot =
    Boolean(selectedOp) && !isAlreadyAtTargetSlot && remainingAfterAssign < memberCap;

  const activePlot =
    builtPlots.find((p) => p.plotId === deployPlotId) ?? builtPlots[0] ?? null;
  const activePlotDef = activePlot?.facilityId ? FACILITIES[activePlot.facilityId] : null;
  const isAlreadyOnActivePlot =
    Boolean(selectedOp && activePlot) && activePlot?.stationedOperativeId === selectedOp?.id;
  const stationPreviewPct =
    selectedOp && activePlotDef
      ? getStationingBonusPct(selectedOp, activePlotDef, state.hq)
      : 0;

  return (
    <div className="squad-layout">
      {/* 左栏：调查员花名册 + 状态筛选 */}
      <div className="scroll-col">
        <section className="panel">
          <div className="panel-header">
            <h3>调查员名录 ({filteredRoster.length}/{counts.all})</h3>
          </div>

          <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap' }}>
            {(
              [
                { id: 'all', label: `全部 ${counts.all}` },
                { id: 'squad', label: `编队 ${counts.squad}` },
                { id: 'station', label: `驻扎 ${counts.station}` },
                { id: 'idle', label: `待命 ${counts.idle}` },
              ] as Array<{ id: RosterFilter; label: string }>
            ).map((f) => (
              <button
                key={f.id}
                className={`small ${filter === f.id ? 'primary' : ''}`}
                style={{ flex: 1, fontSize: '11px' }}
                onClick={() => setFilter(f.id)}
              >
                {f.label}
              </button>
            ))}
          </div>

          {filteredRoster.length === 0 ? (
            <div className="muted" style={{ fontSize: '12px', padding: '10px 0' }}>
              当前筛选条件下暂无调查员。
            </div>
          ) : (
            filteredRoster.map(({ op, loc }) => {
              const cls = CLASSES[op.classId];
              const isSel = selectedOp?.id === op.id;
              return (
                <div
                  key={op.id}
                  className={`card ${isSel ? 'selected' : ''}`}
                  style={{ padding: '6px 8px', cursor: 'pointer' }}
                  onClick={() => gameController.setSelectedOperative(op.id)}
                >
                  <div className="card-title-row">
                    <span>
                      <span className={`pill rarity-${op.rarity}`} style={{ marginRight: '4px' }}>
                        {op.rarity}
                      </span>
                      <strong>{op.name}</strong>
                    </span>
                    <div style={{ display: 'flex', gap: '4px', alignItems: 'center' }}>
                      {op.skillPoints > 0 && (
                        <span className="pill emerald mono">+{op.skillPoints}SP</span>
                      )}
                      <span className={`pill ${getLocBadgeClass(loc.kind)}`}>{loc.label}</span>
                    </div>
                  </div>
                  <div className="muted mono" style={{ fontSize: '10.5px' }}>
                    {cls?.icon} {cls?.name} · Lv.{op.level} ·{' '}
                    {renderStars(getClassMasteryStars(op.classMastery[op.classId] ?? 0))}
                  </div>
                </div>
              );
            })
          )}
        </section>
      </div>

      {/* 右主舞台：档案头 + 岗位调配栏 + 配装/星盘 */}
      <div className="scroll-col">
        <section className="panel" style={{ flex: 1 }}>
          {!selectedOp || !selectedEff || !selectedLoc ? (
            <div className="muted">选择一名调查员展开个人档案与调配。</div>
          ) : (
            <>
              <div className="panel-header" style={{ flexWrap: 'wrap', gap: '6px' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
                  <span className={`pill rarity-${selectedOp.rarity}`}>{selectedOp.rarity}</span>
                  <h2 style={{ fontSize: '15px' }}>{selectedOp.name}</h2>
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
                        {selectedClass.icon} {selectedClass.name}{' '}
                        {renderStars(getClassMasteryStars(selectedOp.classMastery[selectedClass.id] ?? 0))}
                      </span>
                    </GameTooltip>
                  )}
                  <span className="pill mono">
                    Lv.{selectedOp.level} ({selectedOp.xp}/{getXpToNextLevel(selectedOp.level)})
                  </span>
                  {selectedOp.skillPoints > 0 && (
                    <span className="pill emerald mono">SP {selectedOp.skillPoints}</span>
                  )}
                  <span className={`pill ${getLocBadgeClass(selectedLoc.kind)}`}>
                    当前：{selectedLoc.label}
                  </span>
                </div>

                <div style={{ display: 'flex', gap: '6px', flexWrap: 'wrap' }}>
                  <button
                    className="small primary"
                    onClick={() =>
                      gameController.openSquadFormation(
                        selectedLoc.kind === 'squad' ? selectedLoc.squadId : undefined,
                        selectedOp.id,
                      )
                    }
                  >
                    {selectedLoc.kind === 'squad'
                      ? `♟ 前往「${selectedLoc.label}」调整站位/战术 ↗`
                      : '♟ 打开战术编队 ↗'}
                  </button>
                  <button
                    className="small affordable"
                    disabled={
                      selectedOp.level >= 10 ||
                      state.run.resources.manuals < getBoostOperativeManualCost(selectedOp.level)
                    }
                    onClick={() => gameController.doBoostOperative(selectedOp.id)}
                  >
                    {selectedOp.level >= 10
                      ? '满级'
                      : `📖 手札特训 (-${getBoostOperativeManualCost(selectedOp.level)})`}
                  </button>
                </div>
              </div>

              {/* 岗位调配栏：直接在队员界面分配到编队或街区建筑 */}
              <div className="card" style={{ borderColor: 'rgba(56,189,248,0.28)' }}>
                <div className="card-title-row">
                  <strong>岗位调配（直接指派上阵或驻扎建筑）</strong>
                  <div style={{ display: 'flex', gap: '5px', alignItems: 'center' }}>
                    {selectedLoc.kind === 'station' && u.baseUnlocked && (
                      <button
                        className="small"
                        onClick={() => gameController.setTab('base')}
                      >
                        🏗 前往基建 ↗
                      </button>
                    )}
                    {selectedLoc.kind !== 'idle' && (
                      <button
                        className="small danger"
                        onClick={() => gameController.doRecallOperative(selectedOp.id)}
                      >
                        撤回待命
                      </button>
                    )}
                  </div>
                </div>

                <div
                  style={{
                    display: 'grid',
                    gridTemplateColumns: 'repeat(auto-fit, minmax(250px, 1fr))',
                    gap: '8px',
                  }}
                >
                  {/* 编队指派 */}
                  <div style={{ display: 'flex', gap: '5px', alignItems: 'center', flexWrap: 'wrap' }}>
                    <span className="muted" style={{ fontSize: '11.5px', minWidth: '52px' }}>
                      编队上阵
                    </span>
                    {state.run.squads.length > 1 ? (
                      <select
                        style={{ fontSize: '11.5px' }}
                        value={targetSquad.id}
                        onChange={(e) => setDeploySquadId(e.target.value)}
                      >
                        {state.run.squads.map((sq) => (
                          <option key={sq.id} value={sq.id}>
                            {sq.name} ({sq.members.length}/{memberCap})
                          </option>
                        ))}
                      </select>
                    ) : (
                      <span className="pill muted" style={{ fontSize: '11px' }}>
                        {targetSquad.name} ({targetSquad.members.length}/{memberCap})
                      </span>
                    )}
                    <select
                      style={{ fontSize: '11.5px', flex: 1, minWidth: '115px' }}
                      value={deployPosKey}
                      onChange={(e) => setDeployPosKey(e.target.value)}
                    >
                      {POSITIONS.map((pos) => {
                        const occMember = targetSquad.members.find(
                          (m) => m.row === pos.row && m.slot === pos.slot,
                        );
                        const occOp = occMember
                          ? state.run.roster.find((o) => o.id === occMember.operativeId)
                          : null;
                        return (
                          <option key={`${pos.row}:${pos.slot}`} value={`${pos.row}:${pos.slot}`}>
                            {pos.label}{' '}
                            {occOp
                              ? occOp.id === selectedOp.id
                                ? '(本人)'
                                : `(替:${occOp.name})`
                              : '(空)'}
                          </option>
                        );
                      })}
                    </select>
                    <button
                      className={`small ${canAssignTargetSlot ? 'primary' : ''}`}
                      disabled={!canAssignTargetSlot}
                      onClick={() => {
                        gameController.doAssignSquadMember(
                          targetSquad.id,
                          selectedOp.id,
                          targetRow,
                          targetSlot,
                        );
                        gameController.setSelectedSquad(targetSquad.id);
                      }}
                    >
                      {isAlreadyAtTargetSlot
                        ? '已在位'
                        : !canAssignTargetSlot
                          ? '队满(选已有位可替)'
                          : '指派上阵'}
                    </button>
                  </div>

                  {/* 建筑驻扎 */}
                  <div style={{ display: 'flex', gap: '5px', alignItems: 'center', flexWrap: 'wrap' }}>
                    <span className="muted" style={{ fontSize: '11.5px', minWidth: '52px' }}>
                      建筑驻扎
                    </span>
                    {builtPlots.length > 0 && activePlot && activePlotDef ? (
                      <>
                        <select
                          style={{ fontSize: '11.5px', flex: 1, minWidth: '150px' }}
                          value={activePlot.plotId}
                          onChange={(e) => setDeployPlotId(e.target.value)}
                        >
                          {builtPlots.map((plot) => {
                            const fDef = plot.facilityId ? FACILITIES[plot.facilityId] : null;
                            if (!fDef) return null;
                            const bonus = getStationingBonusPct(selectedOp, fDef, state.hq);
                            const occName = plot.stationedOperativeId
                              ? state.run.roster.find((o) => o.id === plot.stationedOperativeId)?.name
                              : null;
                            return (
                              <option key={plot.plotId} value={plot.plotId}>
                                {fDef.name} Lv.{plot.level} (+{bonus}%)
                                {occName
                                  ? plot.stationedOperativeId === selectedOp.id
                                    ? ' [驻守中]'
                                    : ` [替:${occName}]`
                                  : ' [空]'}
                              </option>
                            );
                          })}
                        </select>
                        <button
                          className={`small ${!isAlreadyOnActivePlot ? 'affordable' : ''}`}
                          disabled={isAlreadyOnActivePlot}
                          onClick={() =>
                            gameController.doStationOperative(activePlot.plotId, selectedOp.id)
                          }
                        >
                          {isAlreadyOnActivePlot ? '驻守中' : `驻扎 (+${stationPreviewPct}%)`}
                        </button>
                      </>
                    ) : (
                      <span className="muted" style={{ fontSize: '11.5px' }}>
                        攻克「一环 · 老商业街口」并建造设施后可在此直接派遣驻守。
                      </span>
                    )}
                  </div>
                </div>
              </div>

              <div className="sub-tabs">
                <button
                  className={subTab === 'profile' ? 'primary' : ''}
                  onClick={() => setSubTab('profile')}
                >
                  面板与配装
                </button>
                <button
                  className={subTab === 'tree' ? 'primary' : ''}
                  onClick={() => setSubTab('tree')}
                >
                  星盘技能树与转职
                  {selectedOp.skillPoints > 0 ? ` (${selectedOp.skillPoints} SP)` : ''}
                </button>
              </div>

              {/* 1. 面板与配装 */}
              {subTab === 'profile' && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
                  <div className="card">
                    <div className="mono" style={{ display: 'flex', gap: '14px', flexWrap: 'wrap' }}>
                      <span>HP <strong>{selectedEff.maxHp}</strong></span>
                      <span>ATK <strong>{selectedEff.atk}</strong></span>
                      <span>DEF <strong>{selectedEff.def}</strong></span>
                      <span>SPD <strong>{selectedEff.spd}</strong></span>
                      <span style={{ color: 'var(--cyan)' }}>AP <strong>{selectedEff.maxAp}</strong></span>
                      <span style={{ color: 'var(--purple)' }}>PP <strong>{selectedEff.maxPp}</strong></span>
                      <span>战术槽 <strong>{selectedEff.gambitSlotCap}</strong></span>
                      <span className="muted" style={{ marginLeft: 'auto', fontSize: '11.5px' }}>
                        全职业精通 {selectedEff.totalMasteryStars}★ (+{selectedEff.totalMasteryStars * 2}%)
                      </span>
                    </div>
                  </div>

                  {/* CoC 九维 */}
                  <div className="coc-grid">
                    {COC_LABELS.map((item) => (
                      <GameTooltip
                        key={item.key}
                        as="div"
                        content={
                          <CoCStatTooltipCard
                            statKey={item.key}
                            value={selectedEff.coc[item.key]}
                            title={item.name}
                            detail={item.tip}
                          />
                        }
                      >
                        <div className="coc-cell">
                          <span className="muted" style={{ fontSize: '11.5px' }}>
                            {item.name}
                          </span>
                          <span className="mono" style={{ fontWeight: 700, fontSize: '13.5px' }}>
                            {selectedEff.coc[item.key]}
                          </span>
                        </div>
                      </GameTooltip>
                    ))}
                  </div>

                  {/* 词条徽章区 */}
                  <div className="card">
                    <div className="card-title-row">
                      <strong>词条与传承槽（悬停查详情）</strong>
                      <span className="muted mono" style={{ fontSize: '11px' }}>
                        传承槽 {selectedOp.equippedTraitIds.length}/{selectedEff.evilitySlotCap}
                      </span>
                    </div>
                    <div style={{ display: 'flex', flexWrap: 'wrap', gap: '5px' }}>
                      {selectedOp.innateTraitIds.map((tid) => {
                        const tr = TRAITS[tid];
                        return tr ? (
                          <GameTooltip key={tid} content={<TraitTooltipCard traitId={tid} />}>
                            <span
                              className={`pill ${tr.isUnique ? 'amber' : 'cyan'}`}
                              style={{ cursor: 'help' }}
                            >
                              {tr.name}
                            </span>
                          </GameTooltip>
                        ) : null;
                      })}
                      {selectedOp.learnedTraitIds.map((tid) => {
                        const tr = TRAITS[tid];
                        const eqp = selectedOp.equippedTraitIds.includes(tid);
                        return tr ? (
                          <GameTooltip key={tid} content={<TraitTooltipCard traitId={tid} />}>
                            <button
                              className={`small ${eqp ? 'primary' : ''}`}
                              onClick={() =>
                                gameController.doToggleEquippedTrait(selectedOp.id, tid)
                              }
                            >
                              {eqp ? '✓ ' : '+ '}
                              {tr.name}
                            </button>
                          </GameTooltip>
                        ) : null;
                      })}
                    </div>
                  </div>

                  {/* 装备槽与背包 */}
                  <div className="card">
                    <div className="card-title-row">
                      <strong>个人配装与随身装备库 ({state.run.inventory.length}/24)</strong>
                      <select
                        value={state.run.autoSalvageMaxRarity}
                        onChange={(e) =>
                          gameController.doSetAutoSalvage(
                            e.target.value as 'none' | 'common' | 'uncommon',
                          )
                        }
                      >
                        <option value="none">不自动分解</option>
                        <option value="common">自动分解常规</option>
                        <option value="uncommon">自动分解特制以下</option>
                      </select>
                    </div>

                    <div className="row-grid">
                      {EQ_SLOTS.map(({ slot, label }) => {
                        const eq = selectedOp.equipment[slot];
                        const tpl = eq ? EQUIPMENT_TEMPLATES[eq.templateId] : null;
                        return (
                          <div key={slot} className="gear-tile">
                            <span className="muted" style={{ fontSize: '10.5px' }}>
                              {label}
                            </span>
                            {eq && tpl ? (
                              <>
                                <GameTooltip content={<EquipmentTooltipCard eq={eq} />}>
                                  <strong
                                    className={`rarity-${eq.rarity}`}
                                    style={{ cursor: 'help', fontSize: '12px' }}
                                  >
                                    {tpl.name}
                                  </strong>
                                </GameTooltip>
                                <button
                                  className="small"
                                  onClick={() => gameController.doUnequipItem(selectedOp.id, slot)}
                                >
                                  卸下
                                </button>
                              </>
                            ) : (
                              <span className="dim">空槽</span>
                            )}
                          </div>
                        );
                      })}
                    </div>

                    <div className="bag-grid" style={{ marginTop: '6px' }}>
                      {state.run.inventory.map((eq) => {
                        const tpl = EQUIPMENT_TEMPLATES[eq.templateId];
                        if (!tpl) return null;
                        const vaulted = vaultMarkedUids.includes(eq.uid);
                        return (
                          <GameTooltip
                            key={eq.uid}
                            as="div"
                            content={<EquipmentTooltipCard eq={eq} />}
                          >
                            <div className="gear-tile">
                              <div className="card-title-row">
                                <strong className={`rarity-${eq.rarity}`} style={{ fontSize: '11.5px' }}>
                                  {tpl.name}
                                </strong>
                                <span className="mono muted" style={{ fontSize: '10.5px' }}>
                                  +{eq.bonusRollPct}%
                                </span>
                              </div>
                              <div style={{ display: 'flex', gap: '4px' }}>
                                <button
                                  className="small primary"
                                  style={{ flex: 1 }}
                                  onClick={() => gameController.doEquipItem(selectedOp.id, eq.uid)}
                                >
                                  穿戴
                                </button>
                                <button
                                  className={`small ${vaulted ? 'primary' : ''}`}
                                  title={`总部封存 (${vaultMarkedUids.length}/${vaultCap})`}
                                  onClick={() => gameController.doToggleVaultMark(eq.uid)}
                                >
                                  {vaulted ? '★' : '☆'}
                                </button>
                                <button
                                  className="small"
                                  title="分解"
                                  onClick={() => gameController.doSalvageItem(eq.uid)}
                                >
                                  ♻
                                </button>
                              </div>
                            </div>
                          </GameTooltip>
                        );
                      })}
                    </div>
                  </div>
                </div>
              )}

              {/* 2. SVG 星盘技能树与转职 */}
              {subTab === 'tree' && selectedTrees && selectedClass && (
                <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
                  {selectedTrees.personalNodes.length > 0 && (
                    <SkillTreeGraph
                      title="SSR 专属个人星盘"
                      nodes={selectedTrees.personalNodes}
                      unlockedIds={selectedOp.unlockedNodeIds}
                      availableSp={selectedOp.skillPoints}
                      viewBoxWidth={480}
                      viewBoxHeight={200}
                      onUnlockNode={(nid) => gameController.doUnlockTreeNode(selectedOp.id, nid)}
                    />
                  )}

                  <SkillTreeGraph
                    title={`${selectedClass.icon} ${selectedClass.name} · 职业星盘（悬停节点查详情，绿圈可学习）`}
                    nodes={selectedTrees.classNodes}
                    unlockedIds={selectedOp.unlockedNodeIds}
                    availableSp={selectedOp.skillPoints}
                    origin={{
                      name: selectedClass.name,
                      icon: selectedClass.icon,
                      x: 58,
                      y: 120,
                    }}
                    viewBoxWidth={600}
                    viewBoxHeight={240}
                    onUnlockNode={(nid) => gameController.doUnlockTreeNode(selectedOp.id, nid)}
                  />

                  <div className="card">
                    <div className="card-title-row">
                      <strong>转职与职业精通（悬停查适性与 ★3 传承词条）</strong>
                      <span className="pill muted mono">转职耗 {RECLASS_MANUAL_COST} 手札</span>
                    </div>
                    <div className="class-bar">
                      {CLASS_IDS.map((cid) => {
                        const cdef = CLASSES[cid]!;
                        const mxp = selectedOp.classMastery[cid] ?? 0;
                        const stars = getClassMasteryStars(mxp);
                        const pct = getClassMasteryProgressPct(mxp);
                        const isCur = selectedOp.classId === cid;
                        return (
                          <GameTooltip
                            key={cid}
                            as="div"
                            content={<ClassTooltipCard classId={cid} stars={stars} />}
                          >
                            <div className={`card ${isCur ? 'selected' : ''}`} style={{ padding: '6px 8px' }}>
                              <div className="card-title-row">
                                <strong style={{ fontSize: '12px' }}>
                                  {cdef.icon} {cdef.name}
                                </strong>
                                <span className="mono" style={{ color: 'var(--amber)', fontSize: '10.5px' }}>
                                  {renderStars(stars)}
                                </span>
                              </div>
                              <div className="progress-track">
                                <div className="progress-fill" style={{ width: `${pct}%` }} />
                              </div>
                              {!isCur ? (
                                <button
                                  className="small"
                                  disabled={state.run.resources.manuals < RECLASS_MANUAL_COST}
                                  onClick={() =>
                                    gameController.doReclassOperative(selectedOp.id, cid)
                                  }
                                >
                                  转职
                                </button>
                              ) : (
                                <span className="pill cyan" style={{ alignSelf: 'flex-start' }}>
                                  当前职业
                                </span>
                              )}
                            </div>
                          </GameTooltip>
                        );
                      })}
                    </div>
                  </div>
                </div>
              )}
            </>
          )}
        </section>
      </div>
    </div>
  );
}
