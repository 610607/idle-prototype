import { CLASSES, CLASS_IDS, SPECIAL_OPERATIVES } from '../../content/classesAndTrees.ts';
import { EQUIPMENT_TEMPLATES } from '../../content/equipment.ts';
import { FACILITIES } from '../../content/facilities.ts';
import { OTHERWORLD_MODIFIERS } from '../../content/hqAndModifiers.ts';
import {
  ORDINARY_TRAIT_IDS,
  SKILLS,
  TACTIC_BLUEPRINTS,
} from '../../content/skillsAndTraits.ts';
import { OTHERWORLD_DEPTHS } from '../../content/worldNodes.ts';
import type {
  ClassId,
  CoCStats,
  CombatUnitInput,
  GambitRule,
  OperativeRarity,
  SlotIndex,
} from '../../types/combat.ts';
import type {
  BuildingPlot,
  EnemyUnitDef,
  EquipmentInstance,
  GameState,
  OperativeInstance,
  PatrolBenchmark,
  PowerGridSummary,
  RecruitCandidate,
  ResourceBundle,
} from '../../types/game.ts';
import { simulateExpedition } from '../combat/simulator.ts';
import {
  buildCombatUnitFromOperative,
  ensureSkillInGambits,
  getMaxSquadCount,
  getSquadMemberCap,
  getStationingBonusPct,
} from '../operativeHelpers.ts';
import { nextRandom, pickWeighted, randomInt, shuffleWithSeed } from '../prng.ts';

export const MAX_INVENTORY_SIZE = 24;
const RARITY_ORDER = { common: 0, uncommon: 1, rare: 2 } as const;

export function buildEnemyUnitInput(def: EnemyUnitDef, id: string): CombatUnitInput {
  return {
    id,
    name: def.name,
    side: 'enemy',
    row: def.row,
    slot: def.slot,
    maxHp: def.hp,
    atk: def.atk,
    def: def.def,
    spd: def.spd,
    maxAp: def.ap,
    maxPp: def.pp,
    traitIds: def.traitIds ?? [],
    gambits: def.gambits.map((g) => ({ ...g })),
    skillIds: [...def.skillIds],
    initialTags: def.initialTags,
  };
}

export function getPlotEfficiencyMultiplier(
  state: GameState,
  plot: BuildingPlot,
): number {
  if (!plot.facilityId || plot.level <= 0) return 1;
  const def = FACILITIES[plot.facilityId];
  if (!def) return 1;
  let pct = 0;
  if (plot.bonusCategory && plot.bonusCategory === def.category) {
    pct += plot.bonusPct;
  }
  if (plot.stationedOperativeId) {
    const op = state.run.roster.find((o) => o.id === plot.stationedOperativeId);
    if (op) {
      pct += getStationingBonusPct(op, def, state.hq);
    }
  }
  return Number((1 + pct / 100).toFixed(3));
}

export function computePowerGrid(state: GameState): {
  nextState: GameState;
  summary: PowerGridSummary;
} {
  const modifier = OTHERWORLD_MODIFIERS[state.run.activeModifierId];
  let generated = modifier?.powerGenerationBonus ?? 0;

  const plots = state.run.plots.map((p) => ({ ...p }));

  // Pass 1: generators
  for (const plot of plots) {
    const def = plot.facilityId ? FACILITIES[plot.facilityId] : undefined;
    if (!def || plot.level <= 0 || !plot.enabled) {
      plot.powered = false;
      continue;
    }
    if (def.powerDeltaPerLevel > 0) {
      const mult = getPlotEfficiencyMultiplier(state, plot);
      generated += Math.round(def.powerDeltaPerLevel * plot.level * mult);
      plot.powered = true;
    }
  }

  let remaining = generated;
  let demand = 0;
  const brownedOutPlotIds: string[] = [];

  const priorityOrder = [
    'barracks_expansion',
    'salvage_yard',
    'parts_workshop',
    'liaison_office',
    'archive_lab',
    'training_dojo',
    'tactical_relay',
    'cryo_booster',
    'erosion_analyzer',
  ];

  // Prioritize consumers by facility priority, then plot index
  const consumerIndices = plots
    .map((p, idx) => ({ p, idx }))
    .filter(({ p }) => {
      const def = p.facilityId ? FACILITIES[p.facilityId] : undefined;
      return Boolean(def && def.powerDeltaPerLevel < 0 && p.level > 0 && p.enabled);
    })
    .sort((a, b) => {
      const pa = priorityOrder.indexOf(a.p.facilityId ?? '');
      const pb = priorityOrder.indexOf(b.p.facilityId ?? '');
      const ra = pa === -1 ? 99 : pa;
      const rb = pb === -1 ? 99 : pb;
      return ra !== rb ? ra - rb : a.idx - b.idx;
    });

  for (const { p } of consumerIndices) {
    const def = FACILITIES[p.facilityId!]!;
    const cost = Math.abs(def.powerDeltaPerLevel) * p.level;
    demand += cost;
    if (remaining >= cost) {
      remaining -= cost;
      p.powered = true;
    } else {
      p.powered = false;
      brownedOutPlotIds.push(p.plotId);
    }
  }

  return {
    nextState: {
      ...state,
      run: {
        ...state.run,
        plots,
      },
    },
    summary: {
      generated,
      demand,
      net: generated - demand,
      brownedOutPlotIds,
    },
  };
}

export function getPoweredFacilitySupportData(state: GameState): {
  levels: Record<string, number>;
  bonusPct: Record<string, number>;
} {
  const levels: Record<string, number> = {};
  const bonusPct: Record<string, number> = {};
  for (const plot of state.run.plots) {
    if (!plot.facilityId || plot.level <= 0 || !plot.powered) continue;
    levels[plot.facilityId] = (levels[plot.facilityId] ?? 0) + plot.level;
    const effMult = getPlotEfficiencyMultiplier(state, plot);
    bonusPct[plot.facilityId] = Math.round((effMult - 1) * 100);
  }
  return { levels, bonusPct };
}

export function getDojoSpeedMultiplier(state: GameState): number {
  const plot = state.run.plots.find(
    (p) => p.facilityId === 'training_dojo' && p.level > 0 && p.powered,
  );
  if (!plot) return 0;
  return (1 + (plot.level - 1) * 0.25) * getPlotEfficiencyMultiplier(state, plot);
}

export function getLiaisonBonuses(state: GameState): {
  rarityBoostPct: number;
  costDiscountPct: number;
} {
  const plot = state.run.plots.find(
    (p) => p.facilityId === 'liaison_office' && p.level > 0 && p.powered,
  );
  if (!plot) return { rarityBoostPct: 0, costDiscountPct: 0 };
  const mult = getPlotEfficiencyMultiplier(state, plot);
  return {
    rarityBoostPct: Math.min(55, Math.round((plot.level * 12 + 8) * mult)),
    costDiscountPct: Math.min(40, Math.round(plot.level * 6 * mult)),
  };
}

export function getPassiveBreakdown(state: GameState): {
  gross: ResourceBundle;
  materialsConsume: number;
  net: ResourceBundle;
} {
  const logisticsMult = 1 + (state.hq.departments['dept_logistics'] ?? 0) * 0.15;
  const modifier = OTHERWORLD_MODIFIERS[state.run.activeModifierId];
  const crystalMult = modifier?.crystalYieldMultiplier ?? 1;

  let materials = 0;
  let materialsConsume = 0;
  let parts = 0;
  let crystals = 0;
  let manuals = 0;
  let intel = 0;

  for (const plot of state.run.plots) {
    if (!plot.facilityId || !plot.powered || plot.level <= 0) continue;
    const def = FACILITIES[plot.facilityId];
    if (!def) continue;
    const plotMult = getPlotEfficiencyMultiplier(state, plot);
    const milestoneMult = Math.pow(1.38, Math.floor(plot.level / def.breakthroughEvery));
    const scale = plot.level * milestoneMult * plotMult;

    if (def.materialsPerSecPerLevel) materials += def.materialsPerSecPerLevel * scale;
    if (def.materialsConsumePerSecPerLevel) {
      materialsConsume += def.materialsConsumePerSecPerLevel * plot.level * plotMult;
    }
    if (def.partsPerSecPerLevel) parts += def.partsPerSecPerLevel * scale;
    if (def.crystalsPerSecPerLevel) crystals += def.crystalsPerSecPerLevel * scale;
    if (def.manualsPerSecPerLevel) manuals += def.manualsPerSecPerLevel * scale;
    if (def.intelPerSecPerLevel) intel += def.intelPerSecPerLevel * scale;
  }

  const grossMat = Number((materials * logisticsMult).toFixed(3));
  const grossParts = Number((parts * logisticsMult).toFixed(3));
  const grossCry = Number((crystals * logisticsMult * crystalMult).toFixed(3));
  const grossManuals = Number(manuals.toFixed(3));
  const grossIntel = Number(intel.toFixed(3));

  return {
    gross: {
      materials: grossMat,
      parts: grossParts,
      crystals: grossCry,
      manuals: grossManuals,
      intel: grossIntel,
    },
    materialsConsume: Number(materialsConsume.toFixed(3)),
    net: {
      materials: Number((grossMat - materialsConsume).toFixed(3)),
      parts: grossParts,
      crystals: grossCry,
      manuals: grossManuals,
      intel: grossIntel,
    },
  };
}

export function getPassiveRatesPerSec(state: GameState): ResourceBundle {
  return getPassiveBreakdown(state).net;
}

export function buildSquadCombatInputs(
  state: GameState,
  squadId: string,
): CombatUnitInput[] {
  const squad = state.run.squads.find((s) => s.id === squadId);
  if (!squad) return [];
  const cap = getSquadMemberCap(state);
  const rosterById = new Map(state.run.roster.map((op) => [op.id, op]));

  const units: CombatUnitInput[] = [];
  for (const member of squad.members.slice(0, cap)) {
    const op = rosterById.get(member.operativeId);
    if (!op) continue;
    units.push(buildCombatUnitFromOperative(op, state.hq, member.row, member.slot));
  }
  return units;
}

export function recomputePatrolBenchmarks(state: GameState): GameState {
  const support = getPoweredFacilitySupportData(state);
  const logisticsMult = 1 + (state.hq.departments['dept_logistics'] ?? 0) * 0.15;
  const modifier = OTHERWORLD_MODIFIERS[state.run.activeModifierId];
  const crystalMult = modifier?.crystalYieldMultiplier ?? 1;

  const nextBenchmarks: Record<string, PatrolBenchmark> = {};

  for (const squad of state.run.squads) {
    if (!squad.assignedDepthId) continue;
    if (!state.run.clearedDepths[squad.assignedDepthId]) continue;
    const depthDef = OTHERWORLD_DEPTHS.find((d) => d.id === squad.assignedDepthId);
    if (!depthDef) continue;

    const allies = buildSquadCombatInputs(state, squad.id);
    if (allies.length === 0) continue;

    const enemiesByWave = depthDef.waves.map((wave, wIdx) =>
      wave.map((e, eIdx) => buildEnemyUnitInput(e, `${depthDef.id}_w${wIdx}_e${eIdx}`)),
    );

    const report = simulateExpedition({
      allies,
      enemiesByWave,
      baseTravelSec: depthDef.baseTravelSec,
      poweredFacilities: support.levels,
      supportBonusPct: support.bonusPct,
      modifierId: state.run.activeModifierId,
    });

    const guaranteedYield: ResourceBundle = {
      materials: Math.round(depthDef.baseYield.materials * report.yieldFactor * logisticsMult),
      parts: Math.round(depthDef.baseYield.parts * report.yieldFactor * logisticsMult),
      crystals: Math.round(
        depthDef.baseYield.crystals *
          report.yieldFactor *
          logisticsMult *
          crystalMult *
          (1 + report.erosionBonusPct / 100),
      ),
      manuals: Math.max(
        report.yieldFactor > 0.3 ? 1 : 0,
        Math.round(depthDef.baseYield.manuals * report.yieldFactor),
      ),
      intel: Math.max(1, Math.round(depthDef.baseYield.intel * report.yieldFactor)),
    };

    nextBenchmarks[squad.id] = {
      depthId: depthDef.id,
      squadId: squad.id,
      durationSec: report.durationSec,
      outcome: report.outcome,
      wavesCleared: report.wavesCleared,
      totalWaves: report.totalWaves,
      yieldFactor: report.yieldFactor,
      dropRateFactor: report.dropRateFactor,
      erosionBonusPct: report.erosionBonusPct,
      guaranteedYield,
      sampleReport: report,
    };
  }

  return {
    ...state,
    run: {
      ...state.run,
      patrolBenchmarks: nextBenchmarks,
    },
  };
}

export interface LootRollResult {
  nextState: GameState;
  droppedEquipment: EquipmentInstance[];
  salvagedCount: number;
  salvageYield: ResourceBundle;
  unlockedBlueprints: string[];
  unlockedTactics: string[];
  unlockedSpecialOperatives: string[];
}

export function makeSpecialOperativeInstance(
  archetypeId: string,
  progress?: {
    classId?: ClassId;
    classMastery?: Partial<Record<ClassId, number>>;
    learnedSkillIds?: string[];
    learnedTraitIds?: string[];
  },
): OperativeInstance | null {
  const tpl = SPECIAL_OPERATIVES[archetypeId];
  if (!tpl) return null;
  const classId = progress?.classId ?? tpl.defaultClassId;
  return {
    id: `spec_${archetypeId}`,
    archetypeId: tpl.archetypeId,
    name: tpl.name,
    title: tpl.title,
    rarity: tpl.rarity,
    isSpecial: true,
    flavor: tpl.flavor,
    coc: { ...tpl.coc },
    classId,
    classMastery: progress?.classMastery ? { ...progress.classMastery } : {},
    level: 1,
    xp: 0,
    skillPoints: 1,
    unlockedNodeIds: [],
    learnedSkillIds: progress?.learnedSkillIds ? [...progress.learnedSkillIds] : [],
    innateTraitIds: [...tpl.innateTraitIds],
    learnedTraitIds: progress?.learnedTraitIds ? [...progress.learnedTraitIds] : [],
    equippedTraitIds: [],
    preferredRow: tpl.preferredRow,
    baseStats: { ...tpl.baseStats },
    equipment: {},
    gambits: tpl.defaultGambits.map((g) => ({ ...g })),
  };
}

export function rollExpeditionLoot(
  state: GameState,
  depthId: string,
  dropRateFactor: number,
  cycles = 1,
  squadLuckBonus = 0,
): LootRollResult {
  const depthDef = OTHERWORLD_DEPTHS.find((d) => d.id === depthId);
  if (!depthDef || dropRateFactor <= 0 || cycles <= 0) {
    return {
      nextState: state,
      droppedEquipment: [],
      salvagedCount: 0,
      salvageYield: { materials: 0, parts: 0, crystals: 0, manuals: 0, intel: 0 },
      unlockedBlueprints: [],
      unlockedTactics: [],
      unlockedSpecialOperatives: [],
    };
  }

  const modifier = OTHERWORLD_MODIFIERS[state.run.activeModifierId];
  const modDropMult = (modifier?.dropRateMultiplier ?? 1) * (1 + squadLuckBonus);

  let prng = state.run.prngState;
  const inventory = [...state.run.inventory];
  const unlockedBlueprints = new Set(state.hq.unlockedBlueprints);
  const unlockedTactics = new Set(state.hq.unlockedTactics);
  const specialOps = [...state.hq.specialOperatives];
  const roster = [...state.run.roster];

  const droppedEquipment: EquipmentInstance[] = [];
  const newlyUnlockedBlueprints: string[] = [];
  const newlyUnlockedTactics: string[] = [];
  const newlyUnlockedSpecials: string[] = [];
  let salvagedCount = 0;
  const salvageYield: ResourceBundle = {
    materials: 0,
    parts: 0,
    crystals: 0,
    manuals: 0,
    intel: 0,
  };

  const eqEntries = depthDef.lootTable.filter((e) => e.kind === 'equipment');
  const eqChance = Math.min(0.9, 0.42 * dropRateFactor * modDropMult);
  const discoveryChance = Math.min(0.75, 0.26 * dropRateFactor * modDropMult);

  const salvageThreshold =
    state.run.autoSalvageMaxRarity === 'none'
      ? -1
      : RARITY_ORDER[state.run.autoSalvageMaxRarity];

  for (let i = 0; i < cycles; i++) {
    const [eqRoll, s1] = nextRandom(prng);
    prng = s1;
    if (eqRoll < eqChance && eqEntries.length > 0) {
      const [picked, s2] = pickWeighted(prng, eqEntries);
      prng = s2;
      if (picked) {
        const tpl = EQUIPMENT_TEMPLATES[picked.refId];
        if (tpl) {
          const [bonusRoll, s3] = randomInt(prng, 0, 20);
          prng = s3;
          const uid = `eq_${tpl.id}_${(prng >>> 0).toString(36)}_${i}`;
          const item: EquipmentInstance = {
            uid,
            templateId: tpl.id,
            rarity: tpl.rarity,
            bonusRollPct: bonusRoll,
          };

          if (RARITY_ORDER[item.rarity] <= salvageThreshold) {
            salvagedCount++;
            salvageYield.materials += tpl.salvageYield.materials;
            salvageYield.parts += tpl.salvageYield.parts;
            salvageYield.crystals += tpl.salvageYield.crystals;
            salvageYield.manuals += tpl.salvageYield.manuals;
            salvageYield.intel += tpl.salvageYield.intel;
          } else if (inventory.length < MAX_INVENTORY_SIZE) {
            inventory.push(item);
            droppedEquipment.push(item);
          } else {
            let worstIdx = -1;
            let worstRank = RARITY_ORDER[item.rarity];
            for (let idx = 0; idx < inventory.length; idx++) {
              const existing = inventory[idx];
              if (!existing) continue;
              const rank = RARITY_ORDER[existing.rarity];
              if (rank < worstRank) {
                worstRank = rank;
                worstIdx = idx;
              }
            }
            if (worstIdx >= 0) {
              const displaced = inventory[worstIdx]!;
              const dispTpl = EQUIPMENT_TEMPLATES[displaced.templateId];
              if (dispTpl) {
                salvagedCount++;
                salvageYield.materials += dispTpl.salvageYield.materials;
                salvageYield.parts += dispTpl.salvageYield.parts;
                salvageYield.crystals += dispTpl.salvageYield.crystals;
                salvageYield.manuals += dispTpl.salvageYield.manuals;
                salvageYield.intel += dispTpl.salvageYield.intel;
              }
              inventory[worstIdx] = item;
              droppedEquipment.push(item);
            } else {
              salvagedCount++;
              salvageYield.materials += tpl.salvageYield.materials;
              salvageYield.parts += tpl.salvageYield.parts;
              salvageYield.crystals += tpl.salvageYield.crystals;
              salvageYield.manuals += tpl.salvageYield.manuals;
              salvageYield.intel += tpl.salvageYield.intel;
            }
          }
        }
      }
    }

    const discoveryCandidates = depthDef.lootTable.filter((e) => {
      if (e.kind === 'blueprint_facility') return !unlockedBlueprints.has(e.refId);
      if (e.kind === 'blueprint_tactic') return !unlockedTactics.has(e.refId);
      if (e.kind === 'special_operative') {
        return !specialOps.some((s) => s.archetypeId === e.refId);
      }
      return false;
    });

    if (discoveryCandidates.length > 0) {
      const [discRoll, s4] = nextRandom(prng);
      prng = s4;
      if (discRoll < discoveryChance) {
        const [picked, s5] = pickWeighted(prng, discoveryCandidates);
        prng = s5;
        if (picked) {
          if (picked.kind === 'blueprint_facility' && FACILITIES[picked.refId]) {
            unlockedBlueprints.add(picked.refId);
            newlyUnlockedBlueprints.push(picked.refId);
          } else if (picked.kind === 'blueprint_tactic' && TACTIC_BLUEPRINTS[picked.refId]) {
            unlockedTactics.add(picked.refId);
            newlyUnlockedTactics.push(picked.refId);
          } else if (picked.kind === 'special_operative' && SPECIAL_OPERATIVES[picked.refId]) {
            specialOps.push({
              archetypeId: picked.refId,
              unlockedAtCycle: state.hq.story.cycleCount,
              timesDeployed: 1,
            });
            newlyUnlockedSpecials.push(picked.refId);
            const newOp = makeSpecialOperativeInstance(picked.refId);
            if (newOp && !roster.some((r) => r.id === newOp.id)) {
              roster.push(newOp);
            }
          }
        }
      }
    }
  }

  const currentLoadoutBlueprints = Array.from(
    new Set([...state.run.selectedLoadout.blueprints, ...newlyUnlockedBlueprints]),
  );
  const currentLoadoutTactics = Array.from(
    new Set([...state.run.selectedLoadout.tactics, ...newlyUnlockedTactics]),
  );

  const nextState: GameState = {
    ...state,
    hq: {
      ...state.hq,
      unlockedBlueprints: Array.from(unlockedBlueprints),
      unlockedTactics: Array.from(unlockedTactics),
      specialOperatives: specialOps,
    },
    run: {
      ...state.run,
      prngState: prng,
      selectedLoadout: {
        blueprints: currentLoadoutBlueprints,
        tactics: currentLoadoutTactics,
      },
      resources: {
        materials: state.run.resources.materials + salvageYield.materials,
        parts: state.run.resources.parts + salvageYield.parts,
        crystals: state.run.resources.crystals + salvageYield.crystals,
        manuals: state.run.resources.manuals + salvageYield.manuals,
        intel: state.run.resources.intel + salvageYield.intel,
      },
      inventory,
      roster,
    },
  };

  return {
    nextState,
    droppedEquipment,
    salvagedCount,
    salvageYield,
    unlockedBlueprints: newlyUnlockedBlueprints,
    unlockedTactics: newlyUnlockedTactics,
    unlockedSpecialOperatives: newlyUnlockedSpecials,
  };
}

function rollCoCForClass(
  seedState: number,
  classId: ClassId,
  rarity: OperativeRarity,
): { coc: CoCStats; nextSeed: number } {
  const rarityBoost = rarity === 'SR' ? 12 : rarity === 'R' ? 5 : 0;
  let s = seedState;
  const rollOne = (bias: number): number => {
    const [r, ns] = randomInt(s, 35, 65);
    s = ns;
    return Math.min(90, r + bias + rarityBoost);
  };

  const coc: CoCStats = {
    str: rollOne(classId === 'vanguard' || classId === 'striker' ? 12 : 0),
    con: rollOne(classId === 'vanguard' ? 14 : 4),
    siz: rollOne(classId === 'vanguard' || classId === 'striker' ? 10 : 0),
    dex: rollOne(classId === 'marksman' || classId === 'striker' ? 14 : 0),
    app: rollOne(classId === 'medic' || classId === 'technician' ? 8 : 0),
    int: rollOne(classId === 'technician' || classId === 'occultist' ? 14 : 4),
    pow: rollOne(classId === 'occultist' || classId === 'medic' ? 12 : 0),
    edu: rollOne(classId === 'medic' || classId === 'technician' ? 14 : 0),
    luk: rollOne(4),
  };

  return { coc, nextSeed: s };
}

export function generateRecruitPool(
  seedState: number,
  maxRingCaptured: number,
  liaisonBonuses = { rarityBoostPct: 0, costDiscountPct: 0 },
): { pool: RecruitCandidate[]; nextSeed: number } {
  const names = [
    '周铁', '顾青', '陈墨', '许灯', '唐砂',
    '沈桥', '林槐', '孟夏', '方砾', '陆哨',
  ];
  const [shuffledTraits, s1] = shuffleWithSeed(seedState, ORDINARY_TRAIT_IDS);
  const [shuffledNames, s2] = shuffleWithSeed(s1, names);
  const [shuffledClasses, s3] = shuffleWithSeed(s2, CLASS_IDS);
  let prng = s3;

  const pool: RecruitCandidate[] = [];
  const tierScale = 1 + Math.max(0, maxRingCaptured - 1) * 0.1;

  for (let i = 0; i < 3; i++) {
    const classId = shuffledClasses[i] ?? 'vanguard';
    const classDef = CLASSES[classId]!;
    const name = shuffledNames[i] ?? `志愿队员${i + 1}`;
    const [rarityRoll, ns1] = nextRandom(prng);
    prng = ns1;

    const srChance = 0.08 + (liaisonBonuses.rarityBoostPct / 100) * 0.35;
    const rChance = 0.35 + (liaisonBonuses.rarityBoostPct / 100) * 0.45;
    const rarity: OperativeRarity =
      rarityRoll < srChance ? 'SR' : rarityRoll < srChance + rChance ? 'R' : 'N';

    const traitCount = rarity === 'SR' ? 3 : rarity === 'R' ? 2 : 1;
    const innateTraitIds: string[] = [];
    for (let t = 0; t < traitCount; t++) {
      const tid = shuffledTraits[(i * 3 + t) % shuffledTraits.length];
      if (tid && !innateTraitIds.includes(tid)) innateTraitIds.push(tid);
    }

    const { coc, nextSeed } = rollCoCForClass(prng, classId, rarity);
    prng = nextSeed;

    const isFront = classDef.preferredRow === 'front';
    const defaultGambits: GambitRule[] = isFront
      ? [
          {
            id: `rec_${i}_1`,
            enabled: true,
            actionType: 'SKILL',
            skillId: classDef.starterSkillIds[0] ?? 'baton_strike',
            condition1: { type: 'ALWAYS' },
            target: 'DEFAULT',
          },
        ]
      : [
          {
            id: `rec_${i}_1`,
            enabled: true,
            actionType: 'SKILL',
            skillId: classDef.starterSkillIds[0] ?? 'snap_shot',
            condition1: { type: 'ALWAYS' },
            target: 'DEFAULT',
          },
          {
            id: `rec_${i}_2`,
            enabled: true,
            actionType: 'SKILL',
            skillId: classDef.starterSkillIds[1] ?? 'snap_shot',
            condition1: { type: 'ALWAYS' },
            target: 'DEFAULT',
          },
        ];

    const rawCost =
      (rarity === 'SR' ? 85 : rarity === 'R' ? 58 : 38) + maxRingCaptured * 14;
    const discountedCost = Math.max(
      20,
      Math.round(rawCost * (1 - liaisonBonuses.costDiscountPct / 100)),
    );

    pool.push({
      candidateId: `cand_${(prng >>> 0).toString(36)}_${i}`,
      name,
      title: classDef.name,
      rarity,
      flavor: classDef.flavor,
      coc,
      classId,
      preferredRow: classDef.preferredRow,
      baseStats: {
        hp: Math.round((isFront ? 96 : 78) * tierScale),
        atk: Math.round((isFront ? 15 : 17) * tierScale),
        def: Math.round((isFront ? 7 : 4) * tierScale),
        spd: isFront ? 10 : 13,
        ap: 1,
        pp: 1,
      },
      innateTraitIds,
      innateSkillIds: [...classDef.starterSkillIds],
      defaultGambits,
      hireCostMaterials: discountedCost,
    });
  }

  return { pool, nextSeed: prng };
}

export function equipItemOnOperative(
  state: GameState,
  operativeId: string,
  equipmentUid: string,
): GameState {
  const itemIdx = state.run.inventory.findIndex((i) => i.uid === equipmentUid);
  const opIdx = state.run.roster.findIndex((o) => o.id === operativeId);
  if (itemIdx < 0 || opIdx < 0) return state;

  const item = state.run.inventory[itemIdx]!;
  const tpl = EQUIPMENT_TEMPLATES[item.templateId];
  const op = state.run.roster[opIdx]!;
  if (!tpl) return state;

  const nextInventory = [...state.run.inventory];
  nextInventory.splice(itemIdx, 1);

  const prevEquipped = op.equipment[tpl.slot];
  if (prevEquipped) {
    nextInventory.push(prevEquipped);
  }

  let nextGambits = op.gambits;
  if (tpl.grantedSkillId) {
    const skillDef = SKILLS[tpl.grantedSkillId];
    if (skillDef) {
      nextGambits = ensureSkillInGambits(
        op.gambits,
        tpl.grantedSkillId,
        skillDef.kind === 'passive',
      );
    }
  }

  const nextOp: OperativeInstance = {
    ...op,
    equipment: {
      ...op.equipment,
      [tpl.slot]: item,
    },
    gambits: nextGambits,
  };

  const nextRoster = [...state.run.roster];
  nextRoster[opIdx] = nextOp;

  return recomputePatrolBenchmarks({
    ...state,
    run: {
      ...state.run,
      inventory: nextInventory,
      roster: nextRoster,
    },
  });
}

export function unequipItemFromOperative(
  state: GameState,
  operativeId: string,
  slot: keyof OperativeInstance['equipment'],
): GameState {
  const opIdx = state.run.roster.findIndex((o) => o.id === operativeId);
  if (opIdx < 0) return state;
  const op = state.run.roster[opIdx]!;
  const item = op.equipment[slot];
  if (!item) return state;

  const nextEquipment = { ...op.equipment };
  delete nextEquipment[slot];

  const nextRoster = [...state.run.roster];
  nextRoster[opIdx] = { ...op, equipment: nextEquipment };

  return recomputePatrolBenchmarks({
    ...state,
    run: {
      ...state.run,
      inventory: [...state.run.inventory, item],
      roster: nextRoster,
    },
  });
}

export function salvageEquipmentFromInventory(
  state: GameState,
  equipmentUid: string,
): GameState {
  const idx = state.run.inventory.findIndex((i) => i.uid === equipmentUid);
  if (idx < 0) return state;
  const item = state.run.inventory[idx]!;
  const tpl = EQUIPMENT_TEMPLATES[item.templateId];
  if (!tpl) return state;

  const nextInventory = [...state.run.inventory];
  nextInventory.splice(idx, 1);

  return {
    ...state,
    run: {
      ...state.run,
      inventory: nextInventory,
      resources: {
        materials: state.run.resources.materials + tpl.salvageYield.materials,
        parts: state.run.resources.parts + tpl.salvageYield.parts,
        crystals: state.run.resources.crystals + tpl.salvageYield.crystals,
        manuals: state.run.resources.manuals + tpl.salvageYield.manuals,
        intel: state.run.resources.intel + tpl.salvageYield.intel,
      },
    },
  };
}

export function syncSquadCountForDistricts(state: GameState): GameState {
  const targetCount = getMaxSquadCount(state);
  if (state.run.squads.length >= targetCount) return state;

  const squads = [...state.run.squads];
  for (let i = squads.length; i < targetCount; i++) {
    squads.push({
      id: `squad_${i + 1}`,
      name: `第 ${i + 1} 小队`,
      members: [],
      assignedDepthId: null,
      patrolProgressSec: 0,
    });
  }
  return {
    ...state,
    run: {
      ...state.run,
      squads,
    },
  };
}

export function normalizeSlotIndex(slot: number): SlotIndex {
  if (slot <= 0) return 0;
  if (slot === 1) return 1;
  return 2;
}
