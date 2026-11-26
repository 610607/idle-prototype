import { HQ_DEPARTMENT_IDS, MODIFIER_IDS } from '../content/hqAndModifiers.ts';
import { TACTIC_BLUEPRINTS } from '../content/skillsAndTraits.ts';
import { DISTRICTS } from '../content/worldNodes.ts';
import type { GameState } from '../types/game.ts';
import { computePowerGrid, generateRecruitPool } from './economy/baseAndPatrol.ts';
import { buildStarterHQPlots, buildStarterVolunteers } from './prestige.ts';
import { randomInt } from './prng.ts';

export const CURRENT_SAVE_VERSION = 2;

export function createInitialGameState(seed = 20261126): GameState {
  const [modIdx, s1] = randomInt(seed, 0, MODIFIER_IDS.length - 1);
  const modifierId = MODIFIER_IDS[modIdx] ?? 'dense_fog';
  const { pool, nextSeed } = generateRecruitPool(s1, 1);

  const departments: Record<string, number> = {};
  for (const id of HQ_DEPARTMENT_IDS) {
    departments[id] = 0;
  }

  const starterTactics = Object.values(TACTIC_BLUEPRINTS)
    .filter((t) => t.starterUnlocked)
    .map((t) => t.id);

  const volunteers = buildStarterVolunteers(0);

  const state: GameState = {
    version: CURRENT_SAVE_VERSION,
    savedAt: Date.now(),
    totalPlayTimeSec: 0,
    hq: {
      orgExp: 0,
      departments,
      unlockedBlueprints: [],
      unlockedTactics: starterTactics,
      vaultEquipment: [],
      specialOperatives: [],
      story: {
        cycleCount: 0,
        maxDistrictRingEver: 0,
        maxOtherworldDepthEver: 0,
        seenFlavorIds: [],
        truthFlags: {},
        unfolding: {
          baseUnlocked: false,
          workshopUnlocked: false,
          otherworldUnlocked: false,
          dojoUnlocked: false,
          tacticsGuided: false,
          hqUnlocked: false,
        },
      },
    },
    run: {
      runSeed: seed,
      prngState: nextSeed,
      activeModifierId: modifierId,
      selectedLoadout: {
        blueprints: [],
        tactics: starterTactics,
      },
      resources: {
        materials: 60,
        parts: 0,
        crystals: 0,
        manuals: 0,
        intel: 0,
      },
      districts: DISTRICTS.map((d) => ({
        id: d.id,
        captured: false,
      })),
      plots: buildStarterHQPlots(0),
      dojo: null,
      roster: volunteers,
      recruitPool: pool,
      recruitRefreshCost: 25,
      inventory: [
        {
          uid: 'starter_carbine',
          templateId: 'rust_carbine',
          rarity: 'common',
          bonusRollPct: 5,
        },
      ],
      autoSalvageMaxRarity: 'none',
      squads: [
        {
          id: 'squad_1',
          name: '第 1 小队',
          members: [
            { operativeId: volunteers[0]!.id, row: 'front', slot: 0 },
            { operativeId: volunteers[1]!.id, row: 'back', slot: 0 },
          ],
          assignedDepthId: null,
          patrolProgressSec: 0,
        },
      ],
      clearedDepths: {},
      patrolBenchmarks: {},
      lastBattleReport: null,
      eventTicker: ['自治小队已在老街区集结，准备夺回一环商业街口。'],
    },
  };

  return computePowerGrid(state).nextState;
}
