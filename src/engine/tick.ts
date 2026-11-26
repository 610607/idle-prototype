import { SKILLS, TACTIC_BLUEPRINTS, TRAITS } from '../content/skillsAndTraits.ts';
import type { GameState } from '../types/game.ts';
import {
  getDojoSpeedMultiplier,
  getPassiveBreakdown,
  recomputePatrolBenchmarks,
  rollExpeditionLoot,
} from './economy/baseAndPatrol.ts';
import { getSquadLuckDropBonus, grantOperativeXpAndMastery } from './operativeHelpers.ts';
import { appendTicker, syncUnfoldingFlags } from './unfolding.ts';

export interface TickEventSummary {
  patrolCompletedCount: number;
  droppedItemNames: string[];
  dojoCompletedText?: string;
}

export function applyDojoProgress(
  state: GameState,
  dtSec: number,
): { nextState: GameState; completedText?: string } {
  const dojo = state.run.dojo;
  if (!dojo) return { nextState: state };
  const speed = getDojoSpeedMultiplier(state);
  if (speed <= 0) return { nextState: state };

  const nextProgress = dojo.progressSec + dtSec * speed;
  if (nextProgress < dojo.requiredSec) {
    return {
      nextState: {
        ...state,
        run: {
          ...state.run,
          dojo: {
            ...dojo,
            progressSec: nextProgress,
          },
        },
      },
    };
  }

  // Completed! Transfer skill / trait / tactic to student
  const studentIdx = state.run.roster.findIndex((o) => o.id === dojo.studentId);
  const mentor = state.run.roster.find((o) => o.id === dojo.mentorId);
  let nextRoster = [...state.run.roster];
  let unlockedTactics = state.hq.unlockedTactics;
  let selectedTactics = state.run.selectedLoadout.tactics;
  let completedText = '演武传承完成';

  if (studentIdx >= 0) {
    const stu = nextRoster[studentIdx]!;
    if (dojo.targetKind === 'skill') {
      const skillName = SKILLS[dojo.targetId]?.name ?? dojo.targetId;
      if (!stu.learnedSkillIds.includes(dojo.targetId)) {
        nextRoster[studentIdx] = {
          ...stu,
          learnedSkillIds: [...stu.learnedSkillIds, dojo.targetId],
        };
      }
      completedText = `演武完成：${stu.name} 从 ${mentor?.name ?? '导师'} 习得技能「${skillName}」！`;
    } else if (dojo.targetKind === 'trait') {
      const traitName = TRAITS[dojo.targetId]?.name ?? dojo.targetId;
      if (!stu.learnedTraitIds.includes(dojo.targetId)) {
        nextRoster[studentIdx] = {
          ...stu,
          learnedTraitIds: [...stu.learnedTraitIds, dojo.targetId],
        };
      }
      completedText = `演武完成：${stu.name} 继承词条「${traitName}」！`;
    } else if (dojo.targetKind === 'tactic') {
      const tacName = TACTIC_BLUEPRINTS[dojo.targetId]?.name ?? dojo.targetId;
      if (!unlockedTactics.includes(dojo.targetId)) {
        unlockedTactics = [...unlockedTactics, dojo.targetId];
      }
      if (!selectedTactics.includes(dojo.targetId)) {
        selectedTactics = [...selectedTactics, dojo.targetId];
      }
      completedText = `演武破译完成：全队解锁战术「${tacName}」！`;
    }
  }

  return {
    nextState: recomputePatrolBenchmarks({
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
        dojo: null,
        eventTicker: appendTicker(state.run.eventTicker, completedText),
      },
    }),
    completedText,
  };
}

export function tick(state: GameState, dtSec: number): {
  nextState: GameState;
  events: TickEventSummary;
} {
  if (dtSec <= 0) {
    return { nextState: state, events: { patrolCompletedCount: 0, droppedItemNames: [] } };
  }

  const breakdown = getPassiveBreakdown(state);
  const availableMatForConvert = Math.max(
    0,
    state.run.resources.materials + breakdown.gross.materials * dtSec,
  );
  const desiredConsume = breakdown.materialsConsume * dtSec;
  const convertRatio =
    desiredConsume > 0 ? Math.min(1, availableMatForConvert / desiredConsume) : 1;

  let materials = Math.max(
    0,
    availableMatForConvert - desiredConsume * convertRatio,
  );
  let parts = state.run.resources.parts + breakdown.gross.parts * dtSec * convertRatio;
  let crystals = state.run.resources.crystals + breakdown.gross.crystals * dtSec;
  let manuals = state.run.resources.manuals + breakdown.gross.manuals * dtSec;
  let intel = state.run.resources.intel + breakdown.gross.intel * dtSec;

  let currentState: GameState = {
    ...state,
    totalPlayTimeSec: state.totalPlayTimeSec + dtSec,
    run: {
      ...state.run,
      resources: { materials, parts, crystals, manuals, intel },
    },
  };

  // Advance Dojo inheritance
  const dojoRes = applyDojoProgress(currentState, dtSec);
  currentState = dojoRes.nextState;
  materials = currentState.run.resources.materials;
  parts = currentState.run.resources.parts;
  crystals = currentState.run.resources.crystals;
  manuals = currentState.run.resources.manuals;
  intel = currentState.run.resources.intel;

  let patrolCompletedCount = 0;
  const droppedItemNames: string[] = [];
  let ticker = currentState.run.eventTicker;
  let rosterDirty = false;
  const updatedRoster = [...currentState.run.roster];

  const updatedSquads = currentState.run.squads.map((squad) => ({ ...squad }));

  for (let i = 0; i < updatedSquads.length; i++) {
    const squad = updatedSquads[i]!;
    if (!squad.assignedDepthId) continue;
    const benchmark = currentState.run.patrolBenchmarks[squad.id];
    if (!benchmark || benchmark.durationSec <= 0) continue;

    const totalSec = squad.patrolProgressSec + dtSec;
    const completed = Math.floor(totalSec / benchmark.durationSec);
    squad.patrolProgressSec = totalSec % benchmark.durationSec;

    if (completed > 0) {
      patrolCompletedCount += completed;
      materials += benchmark.guaranteedYield.materials * completed;
      parts += benchmark.guaranteedYield.parts * completed;
      crystals += benchmark.guaranteedYield.crystals * completed;
      manuals += benchmark.guaranteedYield.manuals * completed;
      intel += benchmark.guaranteedYield.intel * completed;

      // Grant XP & Class Mastery to squad members
      const xpPerCycle = Math.round(14 * benchmark.yieldFactor);
      const masteryPerCycle = Math.round(12 * benchmark.yieldFactor);
      for (const m of squad.members) {
        const rIdx = updatedRoster.findIndex((o) => o.id === m.operativeId);
        if (rIdx >= 0) {
          const res = grantOperativeXpAndMastery(
            updatedRoster[rIdx]!,
            xpPerCycle * completed,
            masteryPerCycle * completed,
          );
          updatedRoster[rIdx] = res.nextOp;
          rosterDirty = true;
          if (res.leveledUp) {
            ticker = appendTicker(
              ticker,
              `${res.nextOp.name} 升至 Lv.${res.nextOp.level}（+SP 技能点）`,
            );
          }
          if (res.unlockedMasteryTraitId) {
            const tname = TRAITS[res.unlockedMasteryTraitId]?.name ?? res.unlockedMasteryTraitId;
            ticker = appendTicker(ticker, `${res.nextOp.name} 职业达到 ★3，领悟「${tname}」！`);
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

      const luckBonus = getSquadLuckDropBonus(currentState, squad.id);
      const lootRes = rollExpeditionLoot(
        currentState,
        benchmark.depthId,
        benchmark.dropRateFactor,
        completed,
        luckBonus,
      );
      currentState = lootRes.nextState;
      materials = currentState.run.resources.materials;
      parts = currentState.run.resources.parts;
      crystals = currentState.run.resources.crystals;
      manuals = currentState.run.resources.manuals;
      intel = currentState.run.resources.intel;

      for (const eq of lootRes.droppedEquipment) {
        droppedItemNames.push(eq.templateId);
      }
      if (lootRes.unlockedSpecialOperatives.length > 0) {
        ticker = appendTicker(ticker, `巡逻小队从异界找回了新的特别队员！`);
      }
      if (lootRes.unlockedBlueprints.length > 0 || lootRes.unlockedTactics.length > 0) {
        ticker = appendTicker(ticker, `巡逻发现新的蓝图或战术芯片！`);
      }
    }
  }

  currentState = {
    ...currentState,
    run: {
      ...currentState.run,
      resources: { materials, parts, crystals, manuals, intel },
      squads: updatedSquads,
      roster: currentState.run.roster,
      eventTicker: ticker,
    },
  };

  if (rosterDirty) {
    currentState = recomputePatrolBenchmarks(currentState);
  }
  currentState = syncUnfoldingFlags(currentState);

  return {
    nextState: currentState,
    events: {
      patrolCompletedCount,
      droppedItemNames,
      dojoCompletedText: dojoRes.completedText,
    },
  };
}
