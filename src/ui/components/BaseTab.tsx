import { useMemo, useState } from 'react';
import { CLASSES } from '../../content/classesAndTrees.ts';
import { CRAFTABLE_EQUIPMENT_IDS, EQUIPMENT_TEMPLATES } from '../../content/equipment.ts';
import { FACILITIES, FACILITY_IDS, getFacilityUpgradeCost } from '../../content/facilities.ts';
import { SKILLS, TACTIC_BLUEPRINTS, TRAITS } from '../../content/skillsAndTraits.ts';
import { DISTRICTS } from '../../content/worldNodes.ts';
import { canBuildFacilityOnBase } from '../../engine/actions.ts';
import {
  getDojoSpeedMultiplier,
  getLiaisonBonuses,
  getPlotEfficiencyMultiplier,
} from '../../engine/economy/baseAndPatrol.ts';
import {
  getOperativeEffectiveStats,
  getOperativeLocation,
  getStationingBonusPct,
} from '../../engine/operativeHelpers.ts';
import { gameController, useGameStore } from '../../state/gameStore.ts';
import { GameTooltip } from './GameTooltip.tsx';
import {
  EquipmentTooltipCard,
  FacilityTooltipCard,
  SkillTooltipCard,
  TraitTooltipCard,
} from './TooltipCards.tsx';

type WorkbenchTab = 'recruit' | 'craft' | 'research' | 'dojo';

const FAC_ICON: Record<string, string> = {
  diesel_gen: '🛢',
  substation_array: '⚡',
  resonance_reactor: '⚛',
  salvage_yard: '♻',
  parts_workshop: '🔨',
  liaison_office: '📋',
  archive_lab: '📡',
  training_dojo: '🥋',
  barracks_expansion: '⛺',
  tactical_relay: '📻',
  cryo_booster: '❄',
  erosion_analyzer: '🔮',
};

export function BaseTab() {
  const { state } = useGameStore();
  const [wbTab, setWbTab] = useState<WorkbenchTab>('recruit');
  const [buildingPlotId, setBuildingPlotId] = useState<string | null>(null);
  const [dojoMentorId, setDojoMentorId] = useState<string>(state.run.roster[0]?.id ?? '');
  const [dojoStudentId, setDojoStudentId] = useState<string>(state.run.roster[1]?.id ?? '');
  const [dojoChoice, setDojoChoice] = useState<string>('');

  const liaison = getLiaisonBonuses(state);
  const dojoSpeed = getDojoSpeedMultiplier(state);

  const hasWorkshop = state.run.plots.some(
    (p) => p.facilityId === 'parts_workshop' && p.level > 0 && p.powered,
  );
  const hasArchive = state.run.plots.some(
    (p) => p.facilityId === 'archive_lab' && p.level > 0 && p.powered,
  );
  const hasDojo = state.run.plots.some(
    (p) => p.facilityId === 'training_dojo' && p.level > 0 && p.powered,
  );

  const dojoOptions = useMemo(() => {
    const mentor = state.run.roster.find((o) => o.id === dojoMentorId);
    const student = state.run.roster.find((o) => o.id === dojoStudentId);
    if (!mentor || !student || mentor.id === student.id) return [];

    const mentorEff = getOperativeEffectiveStats(mentor, state.hq);
    const studentEff = getOperativeEffectiveStats(student, state.hq);
    const opts: Array<{
      value: string;
      label: string;
      kind: 'skill' | 'trait' | 'tactic';
      id: string;
    }> = [];

    for (const sid of mentorEff.skillIds) {
      if (!studentEff.skillIds.includes(sid) && !student.learnedSkillIds.includes(sid)) {
        opts.push({
          value: `skill:${sid}`,
          kind: 'skill',
          id: sid,
          label: `技能：${SKILLS[sid]?.name ?? sid}`,
        });
      }
    }

    const mentorTraits = Array.from(
      new Set([...mentor.innateTraitIds, ...mentor.learnedTraitIds]),
    );
    for (const tid of mentorTraits) {
      const tr = TRAITS[tid];
      if (!tr || tr.isUnique) continue;
      if (!student.innateTraitIds.includes(tid) && !student.learnedTraitIds.includes(tid)) {
        opts.push({
          value: `trait:${tid}`,
          kind: 'trait',
          id: tid,
          label: `词条：${tr.name}`,
        });
      }
    }

    for (const tb of Object.values(TACTIC_BLUEPRINTS)) {
      if (!tb.starterUnlocked && !state.hq.unlockedTactics.includes(tb.id)) {
        opts.push({
          value: `tactic:${tb.id}`,
          kind: 'tactic',
          id: tb.id,
          label: `战术：${tb.name}`,
        });
      }
    }
    return opts;
  }, [state.run.roster, state.hq, dojoMentorId, dojoStudentId]);

  const effectiveChoice =
    dojoOptions.find((o) => o.value === dojoChoice) ?? dojoOptions[0] ?? null;

  return (
    <div className="base-layout">
      {/* 左主舞台：紧凑地块房间格（悬停看详单） */}
      <div className="scroll-col">
        <section className="panel">
          <div className="panel-header">
            <h2>街区地块建设（悬停地块查产率公式与地形加成）</h2>
            <span className="pill cyan mono">{state.run.plots.length} 块地皮</span>
          </div>

          <div className="plot-grid">
            {state.run.plots.map((plot) => {
              const def = plot.facilityId ? FACILITIES[plot.facilityId] : null;
              const mult = getPlotEfficiencyMultiplier(state, plot);
              const cost = def ? getFacilityUpgradeCost(def, plot.level) : null;
              const canAfford = cost
                ? state.run.resources.materials >= cost.materials &&
                  state.run.resources.parts >= cost.parts &&
                  state.run.resources.crystals >= cost.crystals
                : false;

              const tile = (
                <div className="room-tile">
                  <div className="card-title-row">
                    <span className="muted" style={{ fontSize: '10.5px' }}>
                      {plot.terrainLabel.split('（')[0]}
                    </span>
                    {def ? (
                      <span className={`pill ${plot.powered ? 'emerald' : 'amber'}`}>
                        {plot.powered ? '⚡运转' : plot.enabled ? '欠电' : '停用'}
                      </span>
                    ) : (
                      <span className="pill muted">空地</span>
                    )}
                  </div>

                  {!def ? (
                    <button
                      className="small primary"
                      style={{ marginTop: 'auto' }}
                      onClick={() => setBuildingPlotId(plot.plotId)}
                    >
                      + 建造设施
                    </button>
                  ) : (
                    <>
                      <div className="card-title-row">
                        <strong style={{ fontSize: '13px' }}>
                          {FAC_ICON[def.id] ?? '🏛'} {def.name}{' '}
                          <span className="mono muted">Lv.{plot.level}</span>
                        </strong>
                        <span className="mono" style={{ color: 'var(--emerald)', fontSize: '11px' }}>
                          ×{mult.toFixed(2)}
                        </span>
                      </div>

                      <div style={{ display: 'flex', gap: '4px', alignItems: 'center' }}>
                        <select
                          style={{ fontSize: '11px', flex: 1, minWidth: 0 }}
                          value={plot.stationedOperativeId ?? ''}
                          onChange={(e) =>
                            gameController.doStationOperative(
                              plot.plotId,
                              e.target.value || null,
                            )
                          }
                        >
                          <option value="">
                            无人驻守 ({def.stationStats.map((s) => s.toUpperCase()).join('+')})
                          </option>
                          {state.run.roster.map((op) => {
                            const loc = getOperativeLocation(state, op.id);
                            const bonus = getStationingBonusPct(op, def, state.hq);
                            const isHere = plot.stationedOperativeId === op.id;
                            return (
                              <option key={op.id} value={op.id}>
                                {op.name} (+{bonus}%) {isHere ? '[驻]' : `[${loc.label}]`}
                              </option>
                            );
                          })}
                        </select>
                        {plot.stationedOperativeId && (
                          <button
                            className="small"
                            title="打开调查员档案"
                            onClick={() =>
                              gameController.openOperativeDossier(plot.stationedOperativeId!)
                            }
                          >
                            ↗
                          </button>
                        )}
                      </div>

                      <div style={{ display: 'flex', gap: '4px' }}>
                        <button
                          className={`small ${canAfford && plot.level < def.maxLevel ? 'affordable' : ''}`}
                          style={{ flex: 1 }}
                          disabled={plot.level >= def.maxLevel || !canAfford}
                          onClick={() => gameController.doUpgradePlot(plot.plotId)}
                        >
                          {plot.level >= def.maxLevel
                            ? '满级'
                            : `升级 材${cost?.materials}${cost && cost.parts > 0 ? `/件${cost.parts}` : ''}${cost && cost.crystals > 0 ? `/晶${cost.crystals}` : ''}`}
                        </button>
                        <button
                          className="small"
                          title="开关设施"
                          onClick={() => gameController.doTogglePlotPower(plot.plotId)}
                        >
                          ⚡
                        </button>
                        {plot.plotId !== 'hq_p1' && (
                          <button
                            className="small danger"
                            title="拆除"
                            onClick={() => gameController.doDemolishPlot(plot.plotId)}
                          >
                            ×
                          </button>
                        )}
                      </div>
                    </>
                  )}
                </div>
              );

              return def ? (
                <GameTooltip
                  key={plot.plotId}
                  as="div"
                  content={<FacilityTooltipCard plot={plot} state={state} />}
                >
                  {tile}
                </GameTooltip>
              ) : (
                <div key={plot.plotId}>{tile}</div>
              );
            })}
          </div>
        </section>
      </div>

      {/* 右工作台：选项卡收敛（招募 / 锻造 / 破译 / 演武） */}
      <div className="scroll-col">
        <section className="panel" style={{ flex: 1 }}>
          <div className="sub-tabs">
            <button
              className={`small ${wbTab === 'recruit' ? 'primary' : ''}`}
              onClick={() => setWbTab('recruit')}
            >
              📋 招募
            </button>
            <button
              className={`small ${wbTab === 'craft' ? 'primary' : ''}`}
              disabled={!hasWorkshop}
              title={!hasWorkshop ? '需先建造并供电「精密改装工坊」' : ''}
              onClick={() => setWbTab('craft')}
            >
              {hasWorkshop ? '🔨 锻造' : '🔒 锻造'}
            </button>
            <button
              className={`small ${wbTab === 'research' ? 'primary' : ''}`}
              disabled={!hasArchive}
              title={!hasArchive ? '需先建造并供电「档案研习室」' : ''}
              onClick={() => setWbTab('research')}
            >
              {hasArchive ? '📡 破译' : '🔒 破译'}
            </button>
            <button
              className={`small ${wbTab === 'dojo' ? 'primary' : ''}`}
              disabled={!hasDojo}
              title={!hasDojo ? '需夺回二环并建造「联合演武室」' : ''}
              onClick={() => setWbTab('dojo')}
            >
              {hasDojo ? '🥋 演武' : '🔒 演武'}
            </button>
          </div>

          {wbTab === 'recruit' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
              <div className="card-title-row">
                <strong>街区志愿招募</strong>
                <button
                  className="small"
                  disabled={state.run.resources.materials < state.run.recruitRefreshCost}
                  onClick={() => gameController.doRefreshRecruitPool()}
                >
                  刷新 ({state.run.recruitRefreshCost} 废材)
                </button>
              </div>
              {liaison.rarityBoostPct > 0 && (
                <div style={{ fontSize: '11px', color: 'var(--emerald)' }}>
                  联络处驻扎加成：高稀有 +{liaison.rarityBoostPct}% · 折扣 -{liaison.costDiscountPct}%
                </div>
              )}
              {state.run.recruitPool.map((cand) => {
                const canHire = state.run.resources.materials >= cand.hireCostMaterials;
                return (
                  <div key={cand.candidateId} className="card">
                    <div className="card-title-row">
                      <span>
                        <span className={`pill rarity-${cand.rarity}`} style={{ marginRight: '4px' }}>
                          {cand.rarity}
                        </span>
                        <strong>{cand.name}</strong> · <span className="muted">{cand.title}</span>
                      </span>
                      <span className="pill cyan">
                        {cand.preferredRow === 'front' ? '前' : '后'}
                      </span>
                    </div>
                    <div className="mono muted" style={{ fontSize: '11px' }}>
                      STR{cand.coc.str} CON{cand.coc.con} DEX{cand.coc.dex} INT{cand.coc.int}{' '}
                      POW{cand.coc.pow} EDU{cand.coc.edu} APP{cand.coc.app} LUK{cand.coc.luk}
                    </div>
                    <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap' }}>
                      {cand.innateTraitIds.map((tid) => (
                        <GameTooltip key={tid} content={<TraitTooltipCard traitId={tid} />}>
                          <span className="pill cyan" style={{ cursor: 'help' }}>
                            {TRAITS[tid]?.name ?? tid}
                          </span>
                        </GameTooltip>
                      ))}
                    </div>
                    <button
                      className={`small ${canHire ? 'affordable' : ''}`}
                      disabled={!canHire}
                      onClick={() => gameController.doHireRecruit(cand.candidateId)}
                    >
                      招募（{cand.hireCostMaterials} 废材）
                    </button>
                  </div>
                );
              })}
            </div>
          )}

          {wbTab === 'craft' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '7px' }}>
              <div className="card-title-row">
                <strong>改装工坊锻造（悬停查属性与技能）</strong>
                <span className="pill cyan mono">零件 {Math.floor(state.run.resources.parts)}</span>
              </div>
              {CRAFTABLE_EQUIPMENT_IDS.map((eid) => {
                const tpl = EQUIPMENT_TEMPLATES[eid]!;
                const cost = tpl.craftCost!;
                const ok =
                  state.run.resources.materials >= cost.materials &&
                  state.run.resources.parts >= cost.parts &&
                  state.run.resources.crystals >= cost.crystals;
                return (
                  <GameTooltip
                    key={eid}
                    as="div"
                    content={
                      <EquipmentTooltipCard
                        eq={{
                          uid: 'preview',
                          templateId: eid,
                          rarity: tpl.rarity,
                          bonusRollPct: 10,
                        }}
                      />
                    }
                  >
                    <div className="card" style={{ padding: '7px 9px' }}>
                      <div className="card-title-row">
                        <strong className={`rarity-${tpl.rarity}`}>{tpl.name}</strong>
                        <button
                          className={`small ${ok ? 'affordable' : ''}`}
                          disabled={!ok}
                          onClick={() => gameController.doCraftEquipment(eid)}
                        >
                          锻造 材{cost.materials}/件{cost.parts}
                          {cost.crystals > 0 ? `/晶${cost.crystals}` : ''}
                        </button>
                      </div>
                    </div>
                  </GameTooltip>
                );
              })}
            </div>
          )}

          {wbTab === 'research' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '7px' }}>
              <div className="card-title-row">
                <strong>档案研习 · 战术破译</strong>
                <span className="pill emerald mono">
                  手札 {Math.floor(state.run.resources.manuals)}
                </span>
              </div>
              {Object.values(TACTIC_BLUEPRINTS)
                .filter((tb) => !tb.starterUnlocked)
                .map((tb) => {
                  const unlocked = state.hq.unlockedTactics.includes(tb.id);
                  const cost = tb.researchCostManuals ?? 10;
                  const canRes = !unlocked && state.run.resources.manuals >= cost;
                  return (
                    <div key={tb.id} className="card">
                      <div className="card-title-row">
                        <strong>{tb.name}</strong>
                        {unlocked ? (
                          <span className="pill emerald">已解锁</span>
                        ) : (
                          <button
                            className={`small ${canRes ? 'affordable' : ''}`}
                            disabled={!canRes}
                            onClick={() => gameController.doResearchTactic(tb.id)}
                          >
                            破译 ({cost} 手札)
                          </button>
                        )}
                      </div>
                      <div className="muted" style={{ fontSize: '11.5px' }}>
                        {tb.description}
                      </div>
                    </div>
                  );
                })}
            </div>
          )}

          {wbTab === 'dojo' && (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
              <div className="card-title-row">
                <strong>联合演武偷学</strong>
                <span className="pill purple mono">速度 ×{dojoSpeed.toFixed(2)}</span>
              </div>

              {state.run.dojo ? (
                <div className="card" style={{ borderColor: 'var(--purple)' }}>
                  <div className="card-title-row">
                    <strong>
                      {state.run.roster.find((o) => o.id === state.run.dojo?.mentorId)?.name} ➔{' '}
                      {state.run.roster.find((o) => o.id === state.run.dojo?.studentId)?.name}
                    </strong>
                    <button className="small danger" onClick={() => gameController.doCancelDojo()}>
                      中止
                    </button>
                  </div>
                  <div className="mono" style={{ fontSize: '11.5px' }}>
                    {state.run.dojo.progressSec.toFixed(1)}s / {state.run.dojo.requiredSec}s
                  </div>
                  <div className="progress-track">
                    <div
                      className="progress-fill"
                      style={{
                        width: `${Math.min(
                          100,
                          Math.round(
                            (state.run.dojo.progressSec / state.run.dojo.requiredSec) * 100,
                          ),
                        )}%`,
                        background: 'var(--purple)',
                      }}
                    />
                  </div>
                </div>
              ) : (
                <div className="card">
                  <div className="gambit-controls">
                    <span className="muted">师：</span>
                    <select
                      value={dojoMentorId}
                      onChange={(e) => setDojoMentorId(e.target.value)}
                    >
                      {state.run.roster.map((op) => (
                        <option key={op.id} value={op.id}>
                          {op.name} ({CLASSES[op.classId]?.name})
                        </option>
                      ))}
                    </select>
                    <span className="muted">徒：</span>
                    <select
                      value={dojoStudentId}
                      onChange={(e) => setDojoStudentId(e.target.value)}
                    >
                      {state.run.roster.map((op) => (
                        <option key={op.id} value={op.id}>
                          {op.name} ({CLASSES[op.classId]?.name})
                        </option>
                      ))}
                    </select>
                  </div>

                  {dojoOptions.length === 0 ? (
                    <div className="muted" style={{ fontSize: '11.5px' }}>
                      请选择不同师徒，且师父需掌握学徒没有的技能/词条。
                    </div>
                  ) : (
                    <>
                      <GameTooltip
                        as="div"
                        content={
                          effectiveChoice?.kind === 'skill' ? (
                            <SkillTooltipCard skillId={effectiveChoice.id} />
                          ) : effectiveChoice?.kind === 'trait' ? (
                            <TraitTooltipCard traitId={effectiveChoice.id} />
                          ) : null
                        }
                        disabled={!effectiveChoice || effectiveChoice.kind === 'tactic'}
                      >
                        <select
                          style={{ width: '100%' }}
                          value={effectiveChoice?.value ?? ''}
                          onChange={(e) => setDojoChoice(e.target.value)}
                        >
                          {dojoOptions.map((opt) => (
                            <option key={opt.value} value={opt.value}>
                              {opt.label}
                            </option>
                          ))}
                        </select>
                      </GameTooltip>
                      <button
                        className="small primary"
                        disabled={!effectiveChoice}
                        onClick={() =>
                          effectiveChoice &&
                          gameController.doStartDojo(
                            dojoMentorId,
                            dojoStudentId,
                            effectiveChoice.kind,
                            effectiveChoice.id,
                          )
                        }
                      >
                        开始传授（调入演武室）
                      </button>
                    </>
                  )}
                </div>
              )}
            </div>
          )}
        </section>
      </div>

      {/* 地块建造选择器弹窗 */}
      {buildingPlotId && (
        <div className="modal-backdrop">
          <div className="modal-box">
            <div className="panel-header">
              <h2>选择建造设施</h2>
              <button className="small" onClick={() => setBuildingPlotId(null)}>
                关闭
              </button>
            </div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '7px' }}>
              {FACILITY_IDS.map((fid) => {
                const def = FACILITIES[fid]!;
                const allowed = canBuildFacilityOnBase(state, fid);
                const cost = getFacilityUpgradeCost(def, 0);
                const affordable =
                  allowed &&
                  state.run.resources.materials >= cost.materials &&
                  state.run.resources.parts >= cost.parts &&
                  state.run.resources.crystals >= cost.crystals;
                return (
                  <div key={fid} className="card">
                    <div className="card-title-row">
                      <strong>
                        {FAC_ICON[fid] ?? '🏛'} {def.name}
                      </strong>
                      <span className="mono muted" style={{ fontSize: '11px' }}>
                        {def.powerDeltaPerLevel > 0
                          ? `+${def.powerDeltaPerLevel}`
                          : def.powerDeltaPerLevel}{' '}
                        kW · {def.stationStats.map((s) => s.toUpperCase()).join('+')}
                      </span>
                    </div>
                    <div style={{ fontSize: '11.5px' }}>{def.effectSummary}</div>
                    {!allowed ? (
                      <span className="muted" style={{ fontSize: '11px' }}>
                        {def.maxPerBase &&
                        state.run.plots.filter((p) => p.facilityId === fid).length >=
                          def.maxPerBase
                          ? `已达建造上限 (${def.maxPerBase})`
                          : def.unlockDistrictId
                            ? `需夺回 ${DISTRICTS.find((d) => d.id === def.unlockDistrictId)?.name}`
                            : '需从异界解锁蓝图并在轮回中带入'}
                      </span>
                    ) : (
                      <button
                        className={`small ${affordable ? 'affordable' : ''}`}
                        disabled={!affordable}
                        onClick={() => {
                          gameController.doBuildOnPlot(buildingPlotId, fid);
                          setBuildingPlotId(null);
                        }}
                      >
                        建造：材{cost.materials}
                        {cost.parts > 0 ? ` 件${cost.parts}` : ''}
                        {cost.crystals > 0 ? ` 晶${cost.crystals}` : ''}
                      </button>
                    )}
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
