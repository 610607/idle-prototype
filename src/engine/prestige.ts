import { MODIFIER_IDS } from '../content/hqAndModifiers.ts';
import { DISTRICTS } from '../content/worldNodes.ts';
import type { BuildingPlot, EquipmentInstance, GameState, OperativeInstance } from '../types/game.ts';
import {
  computePowerGrid,
  generateRecruitPool,
  makeSpecialOperativeInstance,
  recomputePatrolBenchmarks,
  syncSquadCountForDistricts,
} from './economy/baseAndPatrol.ts';
import { getLoadoutCapacity, getVaultCapacity } from './operativeHelpers.ts';
import { randomInt } from './prng.ts';
import { syncUnfoldingFlags } from './unfolding.ts';

export function buildStarterHQPlots(fastRespLevel = 0): BuildingPlot[] {
  return [
    {
      plotId: 'hq_p1',
      sourceId: 'hq_base',
      terrainLabel: '总部后院机房（发电设施 +15%）',
      bonusCategory: 'power',
      bonusPct: 15,
      facilityId: 'diesel_gen',
      level: 1 + fastRespLevel,
      enabled: true,
      powered: true,
      stationedOperativeId: null,
    },
    {
      plotId: 'hq_p2',
      sourceId: 'hq_base',
      terrainLabel: '总部临街仓库空地（经济产出 +15%）',
      bonusCategory: 'economy',
      bonusPct: 15,
      facilityId: fastRespLevel > 0 ? 'salvage_yard' : null,
      level: fastRespLevel > 0 ? 1 : 0,
      enabled: true,
      powered: fastRespLevel > 0,
      stationedOperativeId: null,
    },
    {
      plotId: 'hq_p3',
      sourceId: 'hq_base',
      terrainLabel: '总部值班阁楼空地（研究研习 +10%）',
      bonusCategory: 'research',
      bonusPct: 10,
      facilityId: null,
      level: 0,
      enabled: true,
      powered: false,
      stationedOperativeId: null,
    },
  ];
}

export function calculatePendingOrgExp(state: GameState): number {
  let districtScore = 0;
  for (const d of state.run.districts) {
    if (!d.captured) continue;
    const def = DISTRICTS.find((x) => x.id === d.id);
    districtScore += def ? def.ring * 2 : 1;
  }

  let depthScore = 0;
  const depthWeights: Record<string, number> = {
    depth_1: 4,
    depth_2: 8,
    depth_3: 14,
    depth_4: 22,
    depth_5: 32,
  };
  for (const [id, cleared] of Object.entries(state.run.clearedDepths)) {
    if (cleared) {
      depthScore += depthWeights[id] ?? 4;
    }
  }

  const intelScore = Math.floor(Math.sqrt(Math.max(0, state.run.resources.intel)) * 1.6);
  const raw = districtScore + depthScore + intelScore;
  if (districtScore === 0 && depthScore === 0 && state.run.resources.intel < 5) {
    return 0;
  }
  return Math.max(1, raw);
}

export function buildStarterVolunteers(cycleCount: number): OperativeInstance[] {
  return [
    {
      id: `vol_vanguard_c${cycleCount}`,
      archetypeId: 'starter_vanguard',
      name: '陈巡街',
      title: '街头防暴卫',
      rarity: 'R',
      isSpecial: false,
      flavor: '原本在街口修卷闸门，现在拿着加固铝盾站到了最前排。',
      coc: {
        str: 64,
        con: 70,
        siz: 68,
        dex: 50,
        app: 48,
        int: 56,
        pow: 55,
        edu: 52,
        luk: 50,
      },
      classId: 'vanguard',
      classMastery: {},
      level: 1,
      xp: 0,
      skillPoints: 1,
      unlockedNodeIds: [],
      learnedSkillIds: ['guard_cover'],
      innateTraitIds: ['riot_veteran', 'iron_lung'],
      learnedTraitIds: [],
      equippedTraitIds: [],
      preferredRow: 'front',
      baseStats: { hp: 112, atk: 15, def: 8, spd: 11, ap: 1, pp: 1 },
      equipment: {},
      gambits: [
        {
          id: 'st_v1',
          enabled: true,
          actionType: 'SKILL',
          skillId: 'guard_cover',
          condition1: { type: 'ALLY_LOWEST_HP_BELOW', threshold: 90 },
          target: 'LOWEST_HP_ALLY',
        },
        {
          id: 'st_v2',
          enabled: true,
          actionType: 'SKILL',
          skillId: 'shield_bash',
          condition1: { type: 'ALWAYS' },
          target: 'DEFAULT',
        },
      ],
    },
    {
      id: `vol_marksman_c${cycleCount}`,
      archetypeId: 'starter_marksman',
      name: '苏扳手',
      title: '钟楼观测手',
      rarity: 'R',
      isSpecial: false,
      flavor: '用改装射钉枪和老式对讲机守住了两条半巷子。',
      coc: {
        str: 46,
        con: 54,
        siz: 50,
        dex: 70,
        app: 60,
        int: 66,
        pow: 58,
        edu: 64,
        luk: 62,
      },
      classId: 'marksman',
      classMastery: {},
      level: 1,
      xp: 0,
      skillPoints: 1,
      unlockedNodeIds: [],
      learnedSkillIds: ['pursuit_shot'],
      innateTraitIds: ['rooftop_watcher', 'radio_tinker'],
      learnedTraitIds: [],
      equippedTraitIds: [],
      preferredRow: 'back',
      baseStats: { hp: 82, atk: 18, def: 4, spd: 13, ap: 1, pp: 1 },
      equipment: {},
      gambits: [
        {
          id: 'st_m1',
          enabled: true,
          actionType: 'SKILL',
          skillId: 'pursuit_shot',
          condition1: { type: 'ENEMY_HAS_TAG', tag: 'shocked' },
          target: 'ENEMY_WITH_TAG',
        },
        {
          id: 'st_m2',
          enabled: true,
          actionType: 'SKILL',
          skillId: 'snap_shot',
          condition1: { type: 'ALWAYS' },
          target: 'DEFAULT',
        },
      ],
    },
  ];
}

export function collectAllRunEquipment(state: GameState): EquipmentInstance[] {
  const list: EquipmentInstance[] = [...state.run.inventory];
  for (const op of state.run.roster) {
    if (op.equipment.weapon) list.push(op.equipment.weapon);
    if (op.equipment.armor) list.push(op.equipment.armor);
    if (op.equipment.module) list.push(op.equipment.module);
  }
  return list;
}

export function executeGreatInfiltration(
  state: GameState,
  chosenLoadout: { blueprints: string[]; tactics: string[] },
  vaultEquipmentUids: string[],
): GameState {
  const earnedOrgExp = calculatePendingOrgExp(state);
  const newCycle = state.hq.story.cycleCount + 1;

  let maxDistrictRing = state.hq.story.maxDistrictRingEver;
  for (const d of state.run.districts) {
    if (!d.captured) continue;
    const def = DISTRICTS.find((x) => x.id === d.id);
    if (def && def.ring > maxDistrictRing) maxDistrictRing = def.ring;
  }

  let maxOtherworldDepth = state.hq.story.maxOtherworldDepthEver;
  const depthNums: Record<string, number> = {
    depth_1: 1,
    depth_2: 2,
    depth_3: 3,
    depth_4: 4,
    depth_5: 5,
  };
  for (const [id, cleared] of Object.entries(state.run.clearedDepths)) {
    if (cleared) {
      const n = depthNums[id] ?? 0;
      if (n > maxOtherworldDepth) maxOtherworldDepth = n;
    }
  }

  // Persist Special Operatives' Disgaea reincarnation carryovers (classMastery, learnedSkillIds, learnedTraitIds)
  const updatedSpecialProgress = state.hq.specialOperatives.map((prog) => {
    const activeOp = state.run.roster.find(
      (o) => o.isSpecial && o.archetypeId === prog.archetypeId,
    );
    if (!activeOp) return prog;
    return {
      ...prog,
      timesDeployed: prog.timesDeployed + 1,
      classId: activeOp.classId,
      classMastery: { ...activeOp.classMastery },
      learnedSkillIds: [...activeOp.learnedSkillIds],
      learnedTraitIds: [...activeOp.learnedTraitIds],
    };
  });

  const vaultCap = getVaultCapacity(state.hq);
  const allAvailableEq = new Map(
    collectAllRunEquipment(state).map((eq) => [eq.uid, eq]),
  );
  const nextVault: EquipmentInstance[] = [];
  for (const uid of vaultEquipmentUids) {
    if (nextVault.length >= vaultCap) break;
    const eq = allAvailableEq.get(uid);
    if (eq && !nextVault.some((v) => v.uid === eq.uid)) {
      nextVault.push({ ...eq });
    }
  }

  const loadoutCap = getLoadoutCapacity(state.hq);
  const validBlueprints = chosenLoadout.blueprints
    .filter((id) => state.hq.unlockedBlueprints.includes(id))
    .slice(0, loadoutCap.blueprints);
  const validTactics = chosenLoadout.tactics
    .filter((id) => state.hq.unlockedTactics.includes(id))
    .slice(0, loadoutCap.tactics);

  const [modIdx, s1] = randomInt(
    state.run.prngState ^ (newCycle * 9973),
    0,
    MODIFIER_IDS.length - 1,
  );
  const nextModifierId = MODIFIER_IDS[modIdx] ?? MODIFIER_IDS[0]!;

  const fastRespLevel = state.hq.departments['dept_fast_response'] ?? 0;
  const nextDistricts = DISTRICTS.map((d) => ({
    id: d.id,
    captured: d.ring <= fastRespLevel,
  }));

  const nextPlots: BuildingPlot[] = buildStarterHQPlots(fastRespLevel);
  for (const d of DISTRICTS) {
    if (d.ring <= fastRespLevel) {
      for (const pt of d.plotTemplates) {
        nextPlots.push({
          plotId: pt.suffix,
          sourceId: d.id,
          terrainLabel: pt.terrainLabel,
          bonusCategory: pt.bonusCategory,
          bonusPct: pt.bonusPct,
          facilityId: null,
          level: 0,
          enabled: true,
          powered: false,
          stationedOperativeId: null,
        });
      }
    }
  }

  const restoredSpecials: OperativeInstance[] = [];
  for (const prog of updatedSpecialProgress) {
    const restored = makeSpecialOperativeInstance(prog.archetypeId, {
      classId: prog.classId,
      classMastery: prog.classMastery,
      learnedSkillIds: prog.learnedSkillIds,
      learnedTraitIds: prog.learnedTraitIds,
    });
    if (restored) restoredSpecials.push(restored);
  }

  const volunteers = buildStarterVolunteers(newCycle);
  const nextRoster = [...restoredSpecials, ...volunteers];

  const initialSquadMembers = nextRoster.slice(0, 2).map((op, idx) => ({
    operativeId: op.id,
    row: op.preferredRow,
    slot: (idx % 3) as 0 | 1 | 2,
  }));

  const { pool: nextRecruitPool, nextSeed } = generateRecruitPool(
    s1,
    Math.max(1, fastRespLevel),
  );

  let nextState: GameState = {
    ...state,
    hq: {
      ...state.hq,
      orgExp: state.hq.orgExp + earnedOrgExp,
      vaultEquipment: nextVault,
      specialOperatives: updatedSpecialProgress,
      story: {
        ...state.hq.story,
        cycleCount: newCycle,
        maxDistrictRingEver: maxDistrictRing,
        maxOtherworldDepthEver: maxOtherworldDepth,
        unfolding: {
          ...state.hq.story.unfolding,
          baseUnlocked: true,
          workshopUnlocked: true,
          hqUnlocked: true,
        },
      },
    },
    run: {
      runSeed: nextSeed,
      prngState: nextSeed,
      activeModifierId: nextModifierId,
      selectedLoadout: {
        blueprints: validBlueprints,
        tactics: validTactics,
      },
      resources: {
        materials: 60 + fastRespLevel * 45,
        parts: fastRespLevel * 10,
        crystals: fastRespLevel * 8,
        manuals: fastRespLevel * 2,
        intel: 0,
      },
      districts: nextDistricts,
      plots: nextPlots,
      dojo: null,
      roster: nextRoster,
      recruitPool: nextRecruitPool,
      recruitRefreshCost: 25,
      inventory: nextVault.map((eq) => ({ ...eq })),
      autoSalvageMaxRarity: state.run.autoSalvageMaxRarity,
      squads: [
        {
          id: 'squad_1',
          name: '第 1 小队',
          members: initialSquadMembers,
          assignedDepthId: null,
          patrolProgressSec: 0,
        },
      ],
      clearedDepths: {},
      patrolBenchmarks: {},
      lastBattleReport: null,
      eventTicker: [`开启第 ${newCycle} 轮：大渗透重洗了外围街区与异界修正`],
    },
  };

  nextState = syncSquadCountForDistricts(nextState);
  nextState = computePowerGrid(nextState).nextState;
  nextState = recomputePatrolBenchmarks(nextState);
  nextState = syncUnfoldingFlags(nextState);
  return nextState;
}
