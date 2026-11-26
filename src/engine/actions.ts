import { CLASSES } from '../content/classesAndTrees.ts';
import { EQUIPMENT_TEMPLATES } from '../content/equipment.ts';
import { FACILITIES, getFacilityUpgradeCost } from '../content/facilities.ts';
import {
  HQ_DEPARTMENTS,
  OTHERWORLD_MODIFIERS,
  getDepartmentUpgradeCost,
} from '../content/hqAndModifiers.ts';
import { SKILLS, TACTIC_BLUEPRINTS, TRAITS } from '../content/skillsAndTraits.ts';
import { DISTRICTS, OTHERWORLD_DEPTHS } from '../content/worldNodes.ts';
import type { ClassId, CombatReport, GambitRule, RowType, SlotIndex } from '../types/combat.ts';
import type { EquipmentInstance, GameState, OperativeInstance } from '../types/game.ts';
import { simulateExpedition } from './combat/simulator.ts';
import {
  buildEnemyUnitInput,
  buildSquadCombatInputs,
  computePowerGrid,
  generateRecruitPool,
  getLiaisonBonuses,
  getPoweredFacilitySupportData,
  recomputePatrolBenchmarks,
  rollExpeditionLoot,
  syncSquadCountForDistricts,
} from './economy/baseAndPatrol.ts';
import {
  getEvilitySlotCap,
  getOperativeTreeNodes,
  getSquadLuckDropBonus,
  getSquadMemberCap,
  grantOperativeXpAndMastery,
} from './operativeHelpers.ts';
import { randomInt } from './prng.ts';
import { appendTicker, syncUnfoldingFlags } from './unfolding.ts';

export function isFacilityAvailable(state: GameState, facilityId: string): boolean {
  const def = FACILITIES[facilityId];
  if (!def) return false;
  if (def.starterAvailable) return true;
  if (def.unlockDistrictId) {
    const dist = state.run.districts.find((d) => d.id === def.unlockDistrictId);
    return Boolean(dist?.captured);
  }
  if (def.requiresBlueprint) {
    return state.run.selectedLoadout.blueprints.includes(facilityId);
  }
  return false;
}

export function canBuildFacilityOnBase(state: GameState, facilityId: string): boolean {
  if (!isFacilityAvailable(state, facilityId)) return false;
  const def = FACILITIES[facilityId];
  if (!def) return false;
  if (!def.maxPerBase) return true;
  const count = state.run.plots.filter((p) => p.facilityId === facilityId).length;
  return count < def.maxPerBase;
}

function unassignOperativeEverywhere(state: GameState, operativeId: string): GameState {
  const nextSquads = state.run.squads.map((s) => {
    const filtered = s.members.filter((m) => m.operativeId !== operativeId);
    return {
      ...s,
      members: filtered,
      assignedDepthId: filtered.length === 0 ? null : s.assignedDepthId,
    };
  });
  const nextPlots = state.run.plots.map((p) =>
    p.stationedOperativeId === operativeId ? { ...p, stationedOperativeId: null } : p,
  );
  const nextDojo =
    state.run.dojo &&
    (state.run.dojo.mentorId === operativeId || state.run.dojo.studentId === operativeId)
      ? null
      : state.run.dojo;

  return {
    ...state,
    run: {
      ...state.run,
      squads: nextSquads,
      plots: nextPlots,
      dojo: nextDojo,
    },
  };
}

export function buildOnPlot(
  state: GameState,
  plotId: string,
  facilityId: string,
): GameState {
  const plot = state.run.plots.find((p) => p.plotId === plotId);
  const def = FACILITIES[facilityId];
  if (!plot || plot.facilityId !== null || !def || !canBuildFacilityOnBase(state, facilityId)) {
    return state;
  }
  const cost = getFacilityUpgradeCost(def, 0);
  if (
    state.run.resources.materials < cost.materials ||
    state.run.resources.parts < cost.parts ||
    state.run.resources.crystals < cost.crystals
  ) {
    return state;
  }

  const nextState: GameState = {
    ...state,
    run: {
      ...state.run,
      resources: {
        ...state.run.resources,
        materials: state.run.resources.materials - cost.materials,
        parts: state.run.resources.parts - cost.parts,
        crystals: state.run.resources.crystals - cost.crystals,
      },
      plots: state.run.plots.map((p) =>
        p.plotId === plotId
          ? { ...p, facilityId, level: 1, enabled: true }
          : p,
      ),
      eventTicker: appendTicker(state.run.eventTicker, `建成「${def.name}」`),
    },
  };

  return syncUnfoldingFlags(
    recomputePatrolBenchmarks(computePowerGrid(nextState).nextState),
  );
}

export function upgradePlot(state: GameState, plotId: string): GameState {
  const plot = state.run.plots.find((p) => p.plotId === plotId);
  if (!plot || !plot.facilityId) return state;
  const def = FACILITIES[plot.facilityId];
  if (!def || plot.level >= def.maxLevel) return state;

  const cost = getFacilityUpgradeCost(def, plot.level);
  if (
    state.run.resources.materials < cost.materials ||
    state.run.resources.parts < cost.parts ||
    state.run.resources.crystals < cost.crystals
  ) {
    return state;
  }

  const nextState: GameState = {
    ...state,
    run: {
      ...state.run,
      resources: {
        ...state.run.resources,
        materials: state.run.resources.materials - cost.materials,
        parts: state.run.resources.parts - cost.parts,
        crystals: state.run.resources.crystals - cost.crystals,
      },
      plots: state.run.plots.map((p) =>
        p.plotId === plotId ? { ...p, level: p.level + 1, enabled: true } : p,
      ),
    },
  };

  return recomputePatrolBenchmarks(computePowerGrid(nextState).nextState);
}

export function demolishPlot(state: GameState, plotId: string): GameState {
  const plot = state.run.plots.find((p) => p.plotId === plotId);
  if (!plot || !plot.facilityId) return state;
  const nextState: GameState = {
    ...state,
    run: {
      ...state.run,
      plots: state.run.plots.map((p) =>
        p.plotId === plotId
          ? {
              ...p,
              facilityId: null,
              level: 0,
              enabled: true,
              powered: false,
              stationedOperativeId: null,
            }
          : p,
      ),
    },
  };
  return recomputePatrolBenchmarks(computePowerGrid(nextState).nextState);
}

export function togglePlotPower(state: GameState, plotId: string): GameState {
  const plot = state.run.plots.find((p) => p.plotId === plotId);
  if (!plot || !plot.facilityId) return state;
  const nextState: GameState = {
    ...state,
    run: {
      ...state.run,
      plots: state.run.plots.map((p) =>
        p.plotId === plotId ? { ...p, enabled: !p.enabled } : p,
      ),
    },
  };
  return recomputePatrolBenchmarks(computePowerGrid(nextState).nextState);
}

export function stationOperativeOnPlot(
  state: GameState,
  plotId: string,
  operativeId: string | null,
): GameState {
  const plot = state.run.plots.find((p) => p.plotId === plotId);
  if (!plot || !plot.facilityId) return state;

  let next = state;
  if (operativeId !== null) {
    if (!state.run.roster.some((o) => o.id === operativeId)) return state;
    next = unassignOperativeEverywhere(next, operativeId);
  }

  next = {
    ...next,
    run: {
      ...next.run,
      plots: next.run.plots.map((p) =>
        p.plotId === plotId ? { ...p, stationedOperativeId: operativeId } : p,
      ),
    },
  };
  return recomputePatrolBenchmarks(computePowerGrid(next).nextState);
}

export function craftEquipmentWithParts(
  state: GameState,
  templateId: string,
): GameState {
  const tpl = EQUIPMENT_TEMPLATES[templateId];
  if (!tpl || !tpl.craftCost) return state;
  const hasWorkshop = state.run.plots.some(
    (p) => p.facilityId === 'parts_workshop' && p.level > 0 && p.powered,
  );
  if (!hasWorkshop) return state;
  if (
    state.run.resources.materials < tpl.craftCost.materials ||
    state.run.resources.parts < tpl.craftCost.parts ||
    state.run.resources.crystals < tpl.craftCost.crystals
  ) {
    return state;
  }

  const [bonusRoll, nextSeed] = randomInt(state.run.prngState, 4, 18);
  const newItem: EquipmentInstance = {
    uid: `craft_${tpl.id}_${(nextSeed >>> 0).toString(36)}`,
    templateId: tpl.id,
    rarity: tpl.rarity,
    bonusRollPct: bonusRoll,
  };

  return {
    ...state,
    run: {
      ...state.run,
      prngState: nextSeed,
      resources: {
        ...state.run.resources,
        materials: state.run.resources.materials - tpl.craftCost.materials,
        parts: state.run.resources.parts - tpl.craftCost.parts,
        crystals: state.run.resources.crystals - tpl.craftCost.crystals,
      },
      inventory: [...state.run.inventory, newItem],
      eventTicker: appendTicker(state.run.eventTicker, `工坊打造完成：「${tpl.name}」`),
    },
  };
}

export function researchTacticWithManuals(
  state: GameState,
  tacticId: string,
): GameState {
  const def = TACTIC_BLUEPRINTS[tacticId];
  if (!def || !def.researchCostManuals) return state;
  if (state.hq.unlockedTactics.includes(tacticId)) return state;
  const hasArchive = state.run.plots.some(
    (p) => p.facilityId === 'archive_lab' && p.level > 0 && p.powered,
  );
  if (!hasArchive) return state;
  if (state.run.resources.manuals < def.researchCostManuals) return state;

  return {
    ...state,
    hq: {
      ...state.hq,
      unlockedTactics: [...state.hq.unlockedTactics, tacticId],
    },
    run: {
      ...state.run,
      resources: {
        ...state.run.resources,
        manuals: state.run.resources.manuals - def.researchCostManuals,
      },
      selectedLoadout: {
        ...state.run.selectedLoadout,
        tactics: Array.from(new Set([...state.run.selectedLoadout.tactics, tacticId])),
      },
      eventTicker: appendTicker(state.run.eventTicker, `研习破译战术：「${def.name}」`),
    },
  };
}

export function startDojoInheritance(
  state: GameState,
  mentorId: string,
  studentId: string,
  targetKind: 'skill' | 'trait' | 'tactic',
  targetId: string,
): GameState {
  if (mentorId === studentId) return state;
  const hasDojo = state.run.plots.some(
    (p) => p.facilityId === 'training_dojo' && p.level > 0 && p.powered,
  );
  if (!hasDojo) return state;
  const mentor = state.run.roster.find((o) => o.id === mentorId);
  const student = state.run.roster.find((o) => o.id === studentId);
  if (!mentor || !student) return state;

  let next = unassignOperativeEverywhere(state, mentorId);
  next = unassignOperativeEverywhere(next, studentId);

  const requiredSec = targetKind === 'trait' ? 35 : 25;
  next = {
    ...next,
    run: {
      ...next.run,
      dojo: {
        mentorId,
        studentId,
        targetKind,
        targetId,
        progressSec: 0,
        requiredSec,
      },
    },
  };
  return recomputePatrolBenchmarks(computePowerGrid(next).nextState);
}

export function cancelDojoInheritance(state: GameState): GameState {
  return {
    ...state,
    run: {
      ...state.run,
      dojo: null,
    },
  };
}

export function getBoostOperativeManualCost(level: number): number {
  return Math.max(2, Math.round(2 * Math.pow(1.3, Math.max(0, level - 1))));
}

export function boostOperativeWithManuals(
  state: GameState,
  operativeId: string,
): GameState {
  const idx = state.run.roster.findIndex((o) => o.id === operativeId);
  if (idx < 0) return state;
  const op = state.run.roster[idx]!;
  if (op.level >= 10) return state;
  const cost = getBoostOperativeManualCost(op.level);
  if (state.run.resources.manuals < cost) return state;

  const xpGain = 55;
  const masteryGain = 35;
  const res = grantOperativeXpAndMastery(op, xpGain, masteryGain);
  const nextRoster = [...state.run.roster];
  nextRoster[idx] = res.nextOp;

  let ticker = state.run.eventTicker;
  if (res.leveledUp) {
    ticker = appendTicker(ticker, `${op.name} 研读手札升至 Lv.${res.nextOp.level}`);
  }
  if (res.unlockedMasteryTraitId) {
    ticker = appendTicker(
      ticker,
      `${op.name} 领悟传承词条「${TRAITS[res.unlockedMasteryTraitId]?.name}」`,
    );
  }

  return recomputePatrolBenchmarks({
    ...state,
    run: {
      ...state.run,
      resources: {
        ...state.run.resources,
        manuals: state.run.resources.manuals - cost,
      },
      roster: nextRoster,
      eventTicker: ticker,
    },
  });
}

export function unlockSkillTreeNode(
  state: GameState,
  operativeId: string,
  nodeId: string,
): GameState {
  const idx = state.run.roster.findIndex((o) => o.id === operativeId);
  if (idx < 0) return state;
  const op = state.run.roster[idx]!;
  if (op.unlockedNodeIds.includes(nodeId)) return state;

  const { allNodes } = getOperativeTreeNodes(op);
  const node = allNodes.find((n) => n.id === nodeId);
  if (!node) return state;
  if (op.skillPoints < node.costSp) return state;
  if (!node.prereqNodeIds.every((pid) => op.unlockedNodeIds.includes(pid))) return state;

  const nextLearnedTraits =
    node.grantTraitId && !op.learnedTraitIds.includes(node.grantTraitId)
      ? [...op.learnedTraitIds, node.grantTraitId]
      : op.learnedTraitIds;

  let nextGambits = op.gambits;
  if (node.grantSkillId) {
    const sk = SKILLS[node.grantSkillId];
    if (sk && !nextGambits.some((g) => g.skillId === sk.id)) {
      const rule: GambitRule = {
        id: `tree_${sk.id}_${Date.now()}`,
        enabled: true,
        actionType: 'SKILL',
        skillId: sk.id,
        condition1:
          sk.id === 'pursuit_shot'
            ? { type: 'ENEMY_HAS_TAG', tag: 'shocked' }
            : sk.id === 'mark_follow_up'
              ? { type: 'ENEMY_HAS_TAG', tag: 'marked' }
              : sk.id === 'overclock_echo'
                ? { type: 'SELF_HAS_TAG', tag: 'overclock' }
                : sk.id === 'emergency_aegis' || sk.id === 'guard_cover'
                  ? { type: 'ALLY_LOWEST_HP_BELOW', threshold: 50 }
                  : { type: 'ALWAYS' },
        target:
          sk.id === 'pursuit_shot' || sk.id === 'mark_follow_up'
            ? 'ENEMY_WITH_TAG'
            : sk.id === 'emergency_aegis' || sk.id === 'guard_cover'
              ? 'LOWEST_HP_ALLY'
              : 'DEFAULT',
      };
      nextGambits = sk.kind === 'passive' ? [rule, ...nextGambits] : [rule, ...nextGambits];
    }
  }

  let unlockedTactics = state.hq.unlockedTactics;
  let selectedTactics = state.run.selectedLoadout.tactics;
  if (node.grantTacticId) {
    if (!unlockedTactics.includes(node.grantTacticId)) {
      unlockedTactics = [...unlockedTactics, node.grantTacticId];
    }
    if (!selectedTactics.includes(node.grantTacticId)) {
      selectedTactics = [...selectedTactics, node.grantTacticId];
    }
  }

  const nextRoster = [...state.run.roster];
  nextRoster[idx] = {
    ...op,
    skillPoints: op.skillPoints - node.costSp,
    unlockedNodeIds: [...op.unlockedNodeIds, nodeId],
    learnedTraitIds: nextLearnedTraits,
    gambits: nextGambits,
  };

  return syncUnfoldingFlags(
    recomputePatrolBenchmarks({
      ...state,
      hq: {
        ...state.hq,
        unlockedTactics,
      },
      run: {
        ...state.run,
        selectedLoadout: {
          ...state.run.selectedLoadout,
          tactics: selectedTactics,
        },
        roster: nextRoster,
        eventTicker: appendTicker(
          state.run.eventTicker,
          `${op.name} 点亮技能树「${node.name}」`,
        ),
      },
    }),
  );
}

export const RECLASS_MANUAL_COST = 4;

export function reclassOperative(
  state: GameState,
  operativeId: string,
  newClassId: ClassId,
): GameState {
  const idx = state.run.roster.findIndex((o) => o.id === operativeId);
  if (idx < 0) return state;
  const op = state.run.roster[idx]!;
  if (op.classId === newClassId) return state;
  if (state.run.resources.manuals < RECLASS_MANUAL_COST) return state;

  const oldClassDef = CLASSES[op.classId];
  const newClassDef = CLASSES[newClassId];
  if (!oldClassDef || !newClassDef) return state;

  // Refund SP spent on old class tree nodes while keeping personal tree nodes intact
  const oldClassNodeMap = new Map(oldClassDef.treeNodes.map((n) => [n.id, n]));
  let refundedSp = 0;
  const keptNodeIds: string[] = [];
  for (const nid of op.unlockedNodeIds) {
    const oldNode = oldClassNodeMap.get(nid);
    if (oldNode) {
      refundedSp += oldNode.costSp;
    } else {
      keptNodeIds.push(nid);
    }
  }

  // If old class reached at least ★2 (200 mastery), permanently retain its primary starter skill
  let nextLearnedSkills = [...op.learnedSkillIds];
  if ((op.classMastery[op.classId] ?? 0) >= 200) {
    const sig = oldClassDef.starterSkillIds[0];
    if (sig && !nextLearnedSkills.includes(sig)) {
      nextLearnedSkills.push(sig);
    }
  }

  const nextRoster = [...state.run.roster];
  nextRoster[idx] = {
    ...op,
    classId: newClassId,
    title: op.isSpecial ? op.title : newClassDef.name,
    preferredRow: newClassDef.preferredRow,
    skillPoints: op.skillPoints + refundedSp,
    unlockedNodeIds: keptNodeIds,
    learnedSkillIds: nextLearnedSkills,
  };

  return recomputePatrolBenchmarks({
    ...state,
    run: {
      ...state.run,
      resources: {
        ...state.run.resources,
        manuals: state.run.resources.manuals - RECLASS_MANUAL_COST,
      },
      roster: nextRoster,
      eventTicker: appendTicker(
        state.run.eventTicker,
        `${op.name} 转职为「${newClassDef.name}」（返还 ${refundedSp} SP）`,
      ),
    },
  });
}

export function toggleEquippedTrait(
  state: GameState,
  operativeId: string,
  traitId: string,
): GameState {
  const idx = state.run.roster.findIndex((o) => o.id === operativeId);
  if (idx < 0) return state;
  const op = state.run.roster[idx]!;
  if (!op.learnedTraitIds.includes(traitId)) return state;

  const cap = getEvilitySlotCap(op.rarity);
  const exists = op.equippedTraitIds.includes(traitId);
  let nextEquipped: string[];
  if (exists) {
    nextEquipped = op.equippedTraitIds.filter((id) => id !== traitId);
  } else {
    if (op.equippedTraitIds.length >= cap) return state;
    nextEquipped = [...op.equippedTraitIds, traitId];
  }

  const nextRoster = [...state.run.roster];
  nextRoster[idx] = { ...op, equippedTraitIds: nextEquipped };

  return recomputePatrolBenchmarks({
    ...state,
    run: {
      ...state.run,
      roster: nextRoster,
    },
  });
}

export function hireRecruit(state: GameState, candidateId: string): GameState {
  const candIdx = state.run.recruitPool.findIndex((c) => c.candidateId === candidateId);
  if (candIdx < 0) return state;
  const cand = state.run.recruitPool[candIdx]!;
  if (state.run.resources.materials < cand.hireCostMaterials) return state;

  const newOp: OperativeInstance = {
    id: `op_${cand.candidateId}`,
    archetypeId: 'ordinary_recruit',
    name: cand.name,
    title: cand.title,
    rarity: cand.rarity,
    isSpecial: false,
    flavor: cand.flavor,
    coc: { ...cand.coc },
    classId: cand.classId,
    classMastery: {},
    level: 1,
    xp: 0,
    skillPoints: 1,
    unlockedNodeIds: [],
    learnedSkillIds: [],
    innateTraitIds: [...cand.innateTraitIds],
    learnedTraitIds: [],
    equippedTraitIds: [],
    preferredRow: cand.preferredRow,
    baseStats: { ...cand.baseStats },
    equipment: {},
    gambits: cand.defaultGambits.map((g) => ({ ...g })),
  };

  const nextPool = [...state.run.recruitPool];
  nextPool.splice(candIdx, 1);

  return {
    ...state,
    run: {
      ...state.run,
      resources: {
        ...state.run.resources,
        materials: state.run.resources.materials - cand.hireCostMaterials,
      },
      roster: [...state.run.roster, newOp],
      recruitPool: nextPool,
      eventTicker: appendTicker(
        state.run.eventTicker,
        `招募 [${cand.rarity}] ${cand.name}（${cand.title}）`,
      ),
    },
  };
}

export function refreshRecruitPool(state: GameState): GameState {
  if (state.run.resources.materials < state.run.recruitRefreshCost) return state;
  let maxRing = 1;
  for (const d of state.run.districts) {
    if (!d.captured) continue;
    const def = DISTRICTS.find((x) => x.id === d.id);
    if (def && def.ring > maxRing) maxRing = def.ring;
  }

  const liaison = getLiaisonBonuses(state);
  const { pool, nextSeed } = generateRecruitPool(state.run.prngState, maxRing, liaison);
  return {
    ...state,
    run: {
      ...state.run,
      prngState: nextSeed,
      resources: {
        ...state.run.resources,
        materials: state.run.resources.materials - state.run.recruitRefreshCost,
      },
      recruitPool: pool,
    },
  };
}

export function assignSquadMember(
  state: GameState,
  squadId: string,
  operativeId: string,
  row: RowType,
  slot: SlotIndex,
): GameState {
  if (!state.run.roster.some((o) => o.id === operativeId)) return state;
  const targetSquad = state.run.squads.find((s) => s.id === squadId);
  if (!targetSquad) return state;

  const cap = getSquadMemberCap(state);
  const cleanedState = unassignOperativeEverywhere(state, operativeId);

  const nextSquads = cleanedState.run.squads.map((s) => {
    if (s.id !== squadId) return s;
    const withoutOccupied = s.members.filter((m) => !(m.row === row && m.slot === slot));
    if (withoutOccupied.length >= cap) return s;
    return {
      ...s,
      members: [...withoutOccupied, { operativeId, row, slot }],
    };
  });

  return recomputePatrolBenchmarks(
    computePowerGrid({
      ...cleanedState,
      run: {
        ...cleanedState.run,
        squads: nextSquads,
      },
    }).nextState,
  );
}

export function removeSquadMember(
  state: GameState,
  squadId: string,
  operativeId: string,
): GameState {
  const nextSquads = state.run.squads.map((s) => {
    if (s.id !== squadId) return s;
    const remaining = s.members.filter((m) => m.operativeId !== operativeId);
    return {
      ...s,
      members: remaining,
      assignedDepthId: remaining.length === 0 ? null : s.assignedDepthId,
    };
  });
  return recomputePatrolBenchmarks({
    ...state,
    run: {
      ...state.run,
      squads: nextSquads,
    },
  });
}

export function recallOperativeToIdle(state: GameState, operativeId: string): GameState {
  const next = unassignOperativeEverywhere(state, operativeId);
  return recomputePatrolBenchmarks(computePowerGrid(next).nextState);
}

export function updateOperativeGambits(
  state: GameState,
  operativeId: string,
  gambits: GambitRule[],
): GameState {
  const nextRoster = state.run.roster.map((op) =>
    op.id === operativeId ? { ...op, gambits: gambits.map((g) => ({ ...g })) } : op,
  );
  return recomputePatrolBenchmarks({
    ...state,
    run: {
      ...state.run,
      roster: nextRoster,
    },
  });
}

export function assignSquadPatrol(
  state: GameState,
  squadId: string,
  depthId: string | null,
): GameState {
  if (depthId !== null && !state.run.clearedDepths[depthId]) return state;
  const nextSquads = state.run.squads.map((s) => {
    if (s.id !== squadId) return s;
    if (depthId !== null && s.members.length === 0) return s;
    return {
      ...s,
      assignedDepthId: depthId,
      patrolProgressSec: 0,
    };
  });
  return recomputePatrolBenchmarks({
    ...state,
    run: {
      ...state.run,
      squads: nextSquads,
    },
  });
}

export interface AssaultActionResult {
  nextState: GameState;
  report: CombatReport;
  newlyCaptured: boolean;
  newlyClearedDepth: boolean;
  droppedItemNames: string[];
  unlockedBlueprints: string[];
  unlockedTactics: string[];
  unlockedSpecialOperatives: string[];
}

export function assaultDistrict(
  state: GameState,
  squadId: string,
  districtId: string,
): AssaultActionResult | null {
  const distDef = DISTRICTS.find((d) => d.id === districtId);
  const allies = buildSquadCombatInputs(state, squadId);
  if (!distDef || allies.length === 0) return null;

  const enemiesByWave = distDef.waves.map((wave, wIdx) =>
    wave.map((e, eIdx) => buildEnemyUnitInput(e, `${distDef.id}_w${wIdx}_e${eIdx}`)),
  );
  const support = getPoweredFacilitySupportData(state);

  const report = simulateExpedition({
    allies,
    enemiesByWave,
    baseTravelSec: 10,
    poweredFacilities: support.levels,
    supportBonusPct: support.bonusPct,
    modifierId: state.run.activeModifierId,
  });

  const existingDist = state.run.districts.find((d) => d.id === districtId);
  const newlyCaptured = report.fullClear && !existingDist?.captured;

  let nextState: GameState = {
    ...state,
    run: {
      ...state.run,
      lastBattleReport: report,
    },
  };

  if (report.fullClear) {
    // Award XP & mastery for squad members
    const squad = nextState.run.squads.find((s) => s.id === squadId);
    const updatedRoster = [...nextState.run.roster];
    let ticker = nextState.run.eventTicker;
    for (const m of squad?.members ?? []) {
      const rIdx = updatedRoster.findIndex((o) => o.id === m.operativeId);
      if (rIdx >= 0) {
        const res = grantOperativeXpAndMastery(
          updatedRoster[rIdx]!,
          newlyCaptured ? 32 : 14,
          newlyCaptured ? 35 : 15,
        );
        updatedRoster[rIdx] = res.nextOp;
        if (res.leveledUp) {
          ticker = appendTicker(ticker, `${res.nextOp.name} 升至 Lv.${res.nextOp.level}`);
        }
      }
    }
    nextState = {
      ...nextState,
      run: {
        ...nextState.run,
        roster: updatedRoster,
        eventTicker: ticker,
      },
    };
  }

  if (newlyCaptured) {
    const logisticsMult = 1 + (state.hq.departments['dept_logistics'] ?? 0) * 0.15;
    const newPlots = distDef.plotTemplates.map((pt) => ({
      plotId: pt.suffix,
      sourceId: distDef.id,
      terrainLabel: pt.terrainLabel,
      bonusCategory: pt.bonusCategory,
      bonusPct: pt.bonusPct,
      facilityId: null,
      level: 0,
      enabled: true,
      powered: false,
      stationedOperativeId: null,
    }));

    nextState = {
      ...nextState,
      hq: {
        ...nextState.hq,
        story: {
          ...nextState.hq.story,
          maxDistrictRingEver: Math.max(nextState.hq.story.maxDistrictRingEver, distDef.ring),
        },
      },
      run: {
        ...nextState.run,
        districts: nextState.run.districts.map((d) =>
          d.id === districtId ? { ...d, captured: true } : d,
        ),
        plots: [...nextState.run.plots, ...newPlots],
        resources: {
          materials:
            nextState.run.resources.materials +
            Math.round(distDef.firstClearReward.materials * logisticsMult),
          parts:
            nextState.run.resources.parts +
            Math.round(distDef.firstClearReward.parts * logisticsMult),
          crystals:
            nextState.run.resources.crystals +
            Math.round(distDef.firstClearReward.crystals * logisticsMult),
          manuals: nextState.run.resources.manuals + distDef.firstClearReward.manuals,
          intel: nextState.run.resources.intel + distDef.firstClearReward.intel,
        },
        eventTicker: appendTicker(
          nextState.run.eventTicker,
          `夺回「${distDef.name}」，开放 ${newPlots.length} 块街区建设地块！`,
        ),
      },
    };
    nextState = syncSquadCountForDistricts(nextState);
    nextState = computePowerGrid(nextState).nextState;
    nextState = recomputePatrolBenchmarks(nextState);
  }

  nextState = syncUnfoldingFlags(nextState);

  return {
    nextState,
    report,
    newlyCaptured,
    newlyClearedDepth: false,
    droppedItemNames: [],
    unlockedBlueprints: [],
    unlockedTactics: [],
    unlockedSpecialOperatives: [],
  };
}

export function assaultOtherworld(
  state: GameState,
  squadId: string,
  depthId: string,
): AssaultActionResult | null {
  const depthDef = OTHERWORLD_DEPTHS.find((d) => d.id === depthId);
  const allies = buildSquadCombatInputs(state, squadId);
  if (!depthDef || allies.length === 0) return null;

  const enemiesByWave = depthDef.waves.map((wave, wIdx) =>
    wave.map((e, eIdx) => buildEnemyUnitInput(e, `${depthDef.id}_w${wIdx}_e${eIdx}`)),
  );
  const support = getPoweredFacilitySupportData(state);

  const report = simulateExpedition({
    allies,
    enemiesByWave,
    baseTravelSec: depthDef.baseTravelSec,
    poweredFacilities: support.levels,
    supportBonusPct: support.bonusPct,
    modifierId: state.run.activeModifierId,
  });

  const logisticsMult = 1 + (state.hq.departments['dept_logistics'] ?? 0) * 0.15;
  const modifier = OTHERWORLD_MODIFIERS[state.run.activeModifierId];
  const crystalMult = modifier?.crystalYieldMultiplier ?? 1;

  const newlyClearedDepth = report.fullClear && !state.run.clearedDepths[depthId];

  const baseMat = Math.round(depthDef.baseYield.materials * report.yieldFactor * logisticsMult);
  const baseParts = Math.round(depthDef.baseYield.parts * report.yieldFactor * logisticsMult);
  const baseCry = Math.round(
    depthDef.baseYield.crystals *
      report.yieldFactor *
      logisticsMult *
      crystalMult *
      (1 + report.erosionBonusPct / 100),
  );
  const baseMan = Math.max(
    report.yieldFactor > 0.3 ? 1 : 0,
    Math.round(depthDef.baseYield.manuals * report.yieldFactor),
  );
  const baseInt = Math.max(1, Math.round(depthDef.baseYield.intel * report.yieldFactor));

  const bonusMat = newlyClearedDepth
    ? Math.round(depthDef.firstClearBonus.materials * logisticsMult)
    : 0;
  const bonusParts = newlyClearedDepth
    ? Math.round(depthDef.firstClearBonus.parts * logisticsMult)
    : 0;
  const bonusCry = newlyClearedDepth
    ? Math.round(depthDef.firstClearBonus.crystals * logisticsMult * crystalMult)
    : 0;
  const bonusMan = newlyClearedDepth ? depthDef.firstClearBonus.manuals : 0;
  const bonusInt = newlyClearedDepth ? depthDef.firstClearBonus.intel : 0;

  // Squad XP & Class Mastery
  const squad = state.run.squads.find((s) => s.id === squadId);
  const updatedRoster = [...state.run.roster];
  let ticker = state.run.eventTicker;
  const xpGain = Math.round((newlyClearedDepth ? 45 : 20) * report.yieldFactor);
  const masteryGain = Math.round((newlyClearedDepth ? 40 : 18) * report.yieldFactor);

  for (const m of squad?.members ?? []) {
    const rIdx = updatedRoster.findIndex((o) => o.id === m.operativeId);
    if (rIdx >= 0) {
      const res = grantOperativeXpAndMastery(updatedRoster[rIdx]!, xpGain, masteryGain);
      updatedRoster[rIdx] = res.nextOp;
      if (res.leveledUp) {
        ticker = appendTicker(ticker, `${res.nextOp.name} 升至 Lv.${res.nextOp.level}`);
      }
      if (res.unlockedMasteryTraitId) {
        ticker = appendTicker(
          ticker,
          `${res.nextOp.name} 职业达到 ★3，领悟「${TRAITS[res.unlockedMasteryTraitId]?.name}」！`,
        );
      }
    }
  }

  let nextState: GameState = {
    ...state,
    hq: {
      ...state.hq,
      story: {
        ...state.hq.story,
        maxOtherworldDepthEver: newlyClearedDepth
          ? Math.max(state.hq.story.maxOtherworldDepthEver, depthDef.depth)
          : state.hq.story.maxOtherworldDepthEver,
      },
    },
    run: {
      ...state.run,
      clearedDepths: newlyClearedDepth
        ? { ...state.run.clearedDepths, [depthId]: true }
        : state.run.clearedDepths,
      resources: {
        materials: state.run.resources.materials + baseMat + bonusMat,
        parts: state.run.resources.parts + baseParts + bonusParts,
        crystals: state.run.resources.crystals + baseCry + bonusCry,
        manuals: state.run.resources.manuals + baseMan + bonusMan,
        intel: state.run.resources.intel + baseInt + bonusInt,
      },
      roster: updatedRoster,
      lastBattleReport: report,
      eventTicker: newlyClearedDepth
        ? appendTicker(ticker, `首通「${depthDef.name}」！现可指派小队挂机巡逻。`)
        : ticker,
    },
  };

  const luckBonus = getSquadLuckDropBonus(nextState, squadId);
  const lootRes = rollExpeditionLoot(
    nextState,
    depthId,
    report.dropRateFactor,
    1,
    luckBonus,
  );
  nextState = syncUnfoldingFlags(recomputePatrolBenchmarks(lootRes.nextState));

  return {
    nextState,
    report,
    newlyCaptured: false,
    newlyClearedDepth,
    droppedItemNames: lootRes.droppedEquipment.map((e) => e.templateId),
    unlockedBlueprints: lootRes.unlockedBlueprints,
    unlockedTactics: lootRes.unlockedTactics,
    unlockedSpecialOperatives: lootRes.unlockedSpecialOperatives,
  };
}

export function upgradeHQDepartment(state: GameState, deptId: string): GameState {
  const def = HQ_DEPARTMENTS[deptId];
  if (!def) return state;
  const curLevel = state.hq.departments[deptId] ?? 0;
  if (curLevel >= def.maxLevel) return state;
  const cost = getDepartmentUpgradeCost(def, curLevel);
  if (state.hq.orgExp < cost) return state;

  const nextState: GameState = {
    ...state,
    hq: {
      ...state.hq,
      orgExp: state.hq.orgExp - cost,
      departments: {
        ...state.hq.departments,
        [deptId]: curLevel + 1,
      },
    },
  };

  return recomputePatrolBenchmarks(computePowerGrid(nextState).nextState);
}

export function setAutoSalvageMaxRarity(
  state: GameState,
  rarity: 'none' | 'common' | 'uncommon',
): GameState {
  return {
    ...state,
    run: {
      ...state.run,
      autoSalvageMaxRarity: rarity,
    },
  };
}

export function debugUnlockAllStages(state: GameState): GameState {
  return {
    ...state,
    hq: {
      ...state.hq,
      story: {
        ...state.hq.story,
        unfolding: {
          baseUnlocked: true,
          workshopUnlocked: true,
          otherworldUnlocked: true,
          dojoUnlocked: true,
          tacticsGuided: true,
          hqUnlocked: true,
          unlockedAllDebug: true,
        },
      },
    },
  };
}
