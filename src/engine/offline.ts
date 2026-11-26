import { TRAITS } from '../content/skillsAndTraits.ts';
import { OTHERWORLD_DEPTHS } from '../content/worldNodes.ts';
import type {
  EquipmentInstance,
  GameState,
  OfflineSquadReport,
  OfflineSummary,
  ResourceBundle,
} from '../types/game.ts';
import {
  getPassiveBreakdown,
  recomputePatrolBenchmarks,
  rollExpeditionLoot,
} from './economy/baseAndPatrol.ts';
import { getSquadLuckDropBonus, grantOperativeXpAndMastery } from './operativeHelpers.ts';
import { applyDojoProgress } from './tick.ts';
import { appendTicker, syncUnfoldingFlags } from './unfolding.ts';

const MAX_DIRECT_LOOT_SIM_ROLLS = 5000;

export function settleOffline(
  state: GameState,
  elapsedSec: number,
): { nextState: GameState; summary: OfflineSummary } {
  const safeElapsed = Math.max(0, elapsedSec);
  if (safeElapsed === 0) {
    return {
      nextState: state,
      summary: {
        elapsedSec: 0,
        passiveYield: { materials: 0, parts: 0, crystals: 0, manuals: 0, intel: 0 },
        squadReports: [],
        newEquipment: [],
        salvagedCount: 0,
        salvageYield: { materials: 0, parts: 0, crystals: 0, manuals: 0, intel: 0 },
        newBlueprints: [],
        newTactics: [],
        newSpecialOperatives: [],
      },
    };
  }

  const breakdown = getPassiveBreakdown(state);
  const availMat = Math.max(0, state.run.resources.materials + breakdown.gross.materials * safeElapsed);
  const desiredConsume = breakdown.materialsConsume * safeElapsed;
  const convertRatio = desiredConsume > 0 ? Math.min(1, availMat / desiredConsume) : 1;

  const passiveYield: ResourceBundle = {
    materials: Math.round(breakdown.gross.materials * safeElapsed - desiredConsume * convertRatio),
    parts: Math.round(breakdown.gross.parts * safeElapsed * convertRatio),
    crystals: Math.round(breakdown.gross.crystals * safeElapsed),
    manuals: Math.round(breakdown.gross.manuals * safeElapsed),
    intel: Math.round(breakdown.gross.intel * safeElapsed),
  };

  let materials = Math.max(0, availMat - desiredConsume * convertRatio);
  let parts = state.run.resources.parts + breakdown.gross.parts * safeElapsed * convertRatio;
  let crystals = state.run.resources.crystals + breakdown.gross.crystals * safeElapsed;
  let manuals = state.run.resources.manuals + breakdown.gross.manuals * safeElapsed;
  let intel = state.run.resources.intel + breakdown.gross.intel * safeElapsed;

  let currentState: GameState = {
    ...state,
    totalPlayTimeSec: state.totalPlayTimeSec + safeElapsed,
    run: {
      ...state.run,
      resources: { materials, parts, crystals, manuals, intel },
    },
  };

  const dojoRes = applyDojoProgress(currentState, safeElapsed);
  currentState = dojoRes.nextState;

  const squadReports: OfflineSquadReport[] = [];
  const newEquipment: EquipmentInstance[] = [];
  const newBlueprints: string[] = [];
  const newTactics: string[] = [];
  const newSpecialOperatives: string[] = [];
  let salvagedCount = 0;
  const salvageYield: ResourceBundle = {
    materials: 0,
    parts: 0,
    crystals: 0,
    manuals: 0,
    intel: 0,
  };

  const updatedSquads = currentState.run.squads.map((s) => ({ ...s }));
  const updatedRoster = [...currentState.run.roster];
  let ticker = currentState.run.eventTicker;
  let rosterDirty = false;

  for (const squad of updatedSquads) {
    if (!squad.assignedDepthId) continue;
    const benchmark = currentState.run.patrolBenchmarks[squad.id];
    if (!benchmark || benchmark.durationSec <= 0) continue;

    const totalSec = squad.patrolProgressSec + safeElapsed;
    const completedCycles = Math.floor(totalSec / benchmark.durationSec);
    squad.patrolProgressSec = totalSec % benchmark.durationSec;

    if (completedCycles <= 0) continue;

    const yieldEarned: ResourceBundle = {
      materials: benchmark.guaranteedYield.materials * completedCycles,
      parts: benchmark.guaranteedYield.parts * completedCycles,
      crystals: benchmark.guaranteedYield.crystals * completedCycles,
      manuals: benchmark.guaranteedYield.manuals * completedCycles,
      intel: benchmark.guaranteedYield.intel * completedCycles,
    };

    materials += yieldEarned.materials;
    parts += yieldEarned.parts;
    crystals += yieldEarned.crystals;
    manuals += yieldEarned.manuals;
    intel += yieldEarned.intel;

    const xpPerCycle = Math.round(14 * benchmark.yieldFactor);
    const masteryPerCycle = Math.round(12 * benchmark.yieldFactor);
    for (const m of squad.members) {
      const rIdx = updatedRoster.findIndex((o) => o.id === m.operativeId);
      if (rIdx >= 0) {
        const res = grantOperativeXpAndMastery(
          updatedRoster[rIdx]!,
          xpPerCycle * completedCycles,
          masteryPerCycle * completedCycles,
        );
        updatedRoster[rIdx] = res.nextOp;
        rosterDirty = true;
        if (res.unlockedMasteryTraitId) {
          const tname = TRAITS[res.unlockedMasteryTraitId]?.name ?? res.unlockedMasteryTraitId;
          ticker = appendTicker(ticker, `${res.nextOp.name} 领悟职业传承词条「${tname}」！`);
        }
      }
    }

    currentState = {
      ...currentState,
      run: {
        ...currentState.run,
        resources: { materials, parts, crystals, manuals, intel },
        roster: updatedRoster,
      },
    };

    const directRolls = Math.min(completedCycles, MAX_DIRECT_LOOT_SIM_ROLLS);
    const lootScale =
      completedCycles > MAX_DIRECT_LOOT_SIM_ROLLS
        ? completedCycles / MAX_DIRECT_LOOT_SIM_ROLLS
        : 1;

    const luckBonus = getSquadLuckDropBonus(currentState, squad.id);
    const lootRes = rollExpeditionLoot(
      currentState,
      benchmark.depthId,
      benchmark.dropRateFactor,
      directRolls,
      luckBonus,
    );
    currentState = lootRes.nextState;

    const extraSalvageMat = Math.round(lootRes.salvageYield.materials * (lootScale - 1));
    const extraSalvageParts = Math.round(lootRes.salvageYield.parts * (lootScale - 1));
    const extraSalvageCry = Math.round(lootRes.salvageYield.crystals * (lootScale - 1));
    const extraSalvageMan = Math.round(lootRes.salvageYield.manuals * (lootScale - 1));
    const extraSalvageInt = Math.round(lootRes.salvageYield.intel * (lootScale - 1));

    materials = currentState.run.resources.materials + extraSalvageMat;
    parts = currentState.run.resources.parts + extraSalvageParts;
    crystals = currentState.run.resources.crystals + extraSalvageCry;
    manuals = currentState.run.resources.manuals + extraSalvageMan;
    intel = currentState.run.resources.intel + extraSalvageInt;

    newEquipment.push(...lootRes.droppedEquipment);
    newBlueprints.push(...lootRes.unlockedBlueprints);
    newTactics.push(...lootRes.unlockedTactics);
    newSpecialOperatives.push(...lootRes.unlockedSpecialOperatives);
    salvagedCount += Math.round(lootRes.salvagedCount * lootScale);
    salvageYield.materials += lootRes.salvageYield.materials + extraSalvageMat;
    salvageYield.parts += lootRes.salvageYield.parts + extraSalvageParts;
    salvageYield.crystals += lootRes.salvageYield.crystals + extraSalvageCry;
    salvageYield.manuals += lootRes.salvageYield.manuals + extraSalvageMan;
    salvageYield.intel += lootRes.salvageYield.intel + extraSalvageInt;

    const depthName =
      OTHERWORLD_DEPTHS.find((d) => d.id === benchmark.depthId)?.name ?? benchmark.depthId;
    squadReports.push({
      squadId: squad.id,
      squadName: squad.name,
      depthName,
      completedCycles,
      outcome: benchmark.outcome,
      yieldEarned,
    });
  }

  currentState = {
    ...currentState,
    run: {
      ...currentState.run,
      resources: { materials, parts, crystals, manuals, intel },
      squads: updatedSquads,
      eventTicker: ticker,
    },
  };

  if (rosterDirty) {
    currentState = recomputePatrolBenchmarks(currentState);
  }
  currentState = syncUnfoldingFlags(currentState);

  return {
    nextState: currentState,
    summary: {
      elapsedSec: safeElapsed,
      passiveYield,
      squadReports,
      newEquipment,
      salvagedCount,
      salvageYield,
      newBlueprints,
      newTactics,
      newSpecialOperatives,
      dojoCompletedText: dojoRes.completedText,
    },
  };
}
