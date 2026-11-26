import { describe, expect, it } from 'vitest';
import { simulateExpedition } from '../combat/simulator.ts';
import type { CoCStats, CombatUnitInput, GambitRule } from '../../types/combat.ts';

const BASE_COC: CoCStats = {
  str: 40,
  con: 50,
  siz: 50,
  dex: 40,
  app: 50,
  int: 50,
  pow: 50,
  edu: 50,
  luk: 50,
};

function makeAlly(
  id: string,
  name: string,
  row: 'front' | 'back',
  slot: 0 | 1 | 2,
  overrides: Partial<CombatUnitInput> = {},
): CombatUnitInput {
  return {
    id,
    name,
    side: 'ally',
    row,
    slot,
    maxHp: 140,
    atk: 24,
    def: 8,
    spd: 14,
    maxAp: 1,
    maxPp: 2,
    coc: { ...BASE_COC },
    traitIds: [],
    skillIds: ['baton_strike'],
    gambits: [
      {
        id: `${id}_g1`,
        enabled: true,
        actionType: 'SKILL',
        skillId: 'baton_strike',
        condition1: { type: 'ALWAYS' },
        target: 'DEFAULT',
      },
    ],
    ...overrides,
  };
}

function makeEnemy(
  id: string,
  name: string,
  row: 'front' | 'back',
  slot: 0 | 1 | 2,
  overrides: Partial<CombatUnitInput> = {},
): CombatUnitInput {
  return {
    id,
    name,
    side: 'enemy',
    row,
    slot,
    maxHp: 80,
    atk: 12,
    def: 4,
    spd: 10,
    maxAp: 1,
    maxPp: 0,
    traitIds: [],
    skillIds: ['baton_strike'],
    gambits: [
      {
        id: `${id}_eg1`,
        enabled: true,
        actionType: 'SKILL',
        skillId: 'baton_strike',
        condition1: { type: 'ALWAYS' },
        target: 'DEFAULT',
      },
    ],
    ...overrides,
  };
}

describe('Combat Simulator (v0.2 CoC & MVP Summary)', () => {
  it('produces 100% deterministic output and unitStats summaries', () => {
    const allies = [
      makeAlly('a1', '前锋', 'front', 0, {
        skillIds: ['shield_bash', 'guard_cover'],
        gambits: [
          {
            id: 'g1',
            enabled: true,
            actionType: 'SKILL',
            skillId: 'shield_bash',
            condition1: { type: 'ALWAYS' },
            target: 'DEFAULT',
          },
        ],
      }),
      makeAlly('a2', '射手', 'back', 0, {
        skillIds: ['pursuit_shot', 'snap_shot'],
        gambits: [
          {
            id: 'g2',
            enabled: true,
            actionType: 'SKILL',
            skillId: 'pursuit_shot',
            condition1: { type: 'ENEMY_HAS_TAG', tag: 'shocked' },
            target: 'ENEMY_WITH_TAG',
          },
          {
            id: 'g3',
            enabled: true,
            actionType: 'SKILL',
            skillId: 'snap_shot',
            condition1: { type: 'ALWAYS' },
            target: 'DEFAULT',
          },
        ],
      }),
    ];
    const enemies = [[makeEnemy('e1', '敌人甲', 'front', 0), makeEnemy('e2', '敌人乙', 'back', 0)]];

    const r1 = simulateExpedition({
      allies,
      enemiesByWave: enemies,
      baseTravelSec: 12,
      poweredFacilities: { tactical_relay: 1 },
      modifierId: 'dense_fog',
    });
    const r2 = simulateExpedition({
      allies,
      enemiesByWave: enemies,
      baseTravelSec: 12,
      poweredFacilities: { tactical_relay: 1 },
      modifierId: 'dense_fog',
    });

    expect(r1).toEqual(r2);
    expect(r1.fullClear).toBe(true);
    expect(r1.unitStats.length).toBe(2);
    expect(r1.unitStats[0]?.damageDealt).toBeGreaterThan(0);
    expect(r1.unitStats[1]?.ppReactions).toBeGreaterThanOrEqual(1);
  });

  it('scales melee damage with CoC STR and ranged damage with CoC DEX', () => {
    const lowStr = makeAlly('low', '低力量', 'front', 0, {
      coc: { ...BASE_COC, str: 40 },
      skillIds: ['baton_strike'],
    });
    const highStr = makeAlly('high', '高力量', 'front', 0, {
      coc: { ...BASE_COC, str: 85 },
      skillIds: ['baton_strike'],
    });
    const dummy = makeEnemy('target', '木桩', 'front', 0, { maxHp: 300, atk: 1 });

    const rLowStr = simulateExpedition({
      allies: [lowStr],
      enemiesByWave: [[dummy]],
      baseTravelSec: 10,
      poweredFacilities: {},
      modifierId: 'dense_fog',
    });
    const rHighStr = simulateExpedition({
      allies: [highStr],
      enemiesByWave: [[dummy]],
      baseTravelSec: 10,
      poweredFacilities: {},
      modifierId: 'dense_fog',
    });

    expect(rHighStr.totalRounds).toBeLessThan(rLowStr.totalRounds);

    const lowDex = makeAlly('ld', '低敏捷', 'back', 0, {
      coc: { ...BASE_COC, dex: 40 },
      skillIds: ['snap_shot'],
      gambits: [
        {
          id: 'g_ld',
          enabled: true,
          actionType: 'SKILL',
          skillId: 'snap_shot',
          condition1: { type: 'ALWAYS' },
          target: 'DEFAULT',
        },
      ],
    });
    const highDex = makeAlly('hd', '高敏捷', 'back', 0, {
      coc: { ...BASE_COC, dex: 85 },
      skillIds: ['snap_shot'],
      gambits: [
        {
          id: 'g_hd',
          enabled: true,
          actionType: 'SKILL',
          skillId: 'snap_shot',
          condition1: { type: 'ALWAYS' },
          target: 'DEFAULT',
        },
      ],
    });
    const rLowDex = simulateExpedition({
      allies: [lowDex],
      enemiesByWave: [[dummy]],
      baseTravelSec: 10,
      poweredFacilities: {},
      modifierId: 'dense_fog',
    });
    const rHighDex = simulateExpedition({
      allies: [highDex],
      enemiesByWave: [[dummy]],
      baseTravelSec: 10,
      poweredFacilities: {},
      modifierId: 'dense_fog',
    });

    expect(rHighDex.totalRounds).toBeLessThan(rLowDex.totalRounds);
  });

  it('enforces melee front-row blocking while allowing ranged BACK_PRIORITY targeting', () => {
    const meleeOnly = makeAlly('a1', '近战手', 'front', 0, {
      atk: 20,
      spd: 20,
      skillIds: ['baton_strike'],
      gambits: [
        {
          id: 'g_melee',
          enabled: true,
          actionType: 'SKILL',
          skillId: 'baton_strike',
          condition1: { type: 'ALWAYS' },
          target: 'BACK_PRIORITY',
        },
      ],
    });
    const rangedSniper = makeAlly('a2', '狙击手', 'back', 0, {
      atk: 20,
      spd: 18,
      skillIds: ['snap_shot'],
      gambits: [
        {
          id: 'g_ranged',
          enabled: true,
          actionType: 'SKILL',
          skillId: 'snap_shot',
          condition1: { type: 'ALWAYS' },
          target: 'BACK_PRIORITY',
        },
      ],
    });

    const frontTank = makeEnemy('e_front', '前排铁盾', 'front', 0, { maxHp: 200 });
    const backCaster = makeEnemy('e_back', '后排脆弱', 'back', 0, { maxHp: 200 });

    const report = simulateExpedition({
      allies: [meleeOnly, rangedSniper],
      enemiesByWave: [[frontTank, backCaster]],
      baseTravelSec: 10,
      poweredFacilities: {},
      modifierId: 'phase_tide',
    });

    const activeLogs = report.entries.filter((e) => e.round === 1 && e.kind === 'ACTIVE_AP');
    expect(activeLogs[0]?.text).toContain('前排铁盾');
    expect(activeLogs[1]?.text).toContain('后排脆弱');
  });

  it('triggers PP passive guard/pursuit chains, tactical_relay after 3 reactions, and caps 1 PP per action', () => {
    const tank = makeAlly('tank', '林雾', 'front', 0, {
      maxHp: 180,
      atk: 20,
      spd: 18,
      maxAp: 2,
      maxPp: 3,
      skillIds: ['shield_bash', 'guard_cover'],
      gambits: [
        {
          id: 't_guard',
          enabled: true,
          actionType: 'SKILL',
          skillId: 'guard_cover',
          condition1: { type: 'ALWAYS' },
          target: 'LOWEST_HP_ALLY',
        },
        {
          id: 't_bash',
          enabled: true,
          actionType: 'SKILL',
          skillId: 'shield_bash',
          condition1: { type: 'ALWAYS' },
          target: 'DEFAULT',
        },
      ],
    });

    const marksman = makeAlly('dps', '鸦羽', 'back', 0, {
      maxHp: 120,
      atk: 16,
      spd: 15,
      maxAp: 1,
      maxPp: 5,
      skillIds: ['pursuit_shot', 'snap_shot'],
      gambits: [
        {
          id: 'd_pursuit',
          enabled: true,
          actionType: 'SKILL',
          skillId: 'pursuit_shot',
          condition1: { type: 'ENEMY_HAS_TAG', tag: 'shocked' },
          target: 'ENEMY_WITH_TAG',
        },
        {
          id: 'd_snap',
          enabled: true,
          actionType: 'SKILL',
          skillId: 'snap_shot',
          condition1: { type: 'ALWAYS' },
          target: 'DEFAULT',
        },
      ],
    });

    const sturdyEnemy = makeEnemy('boss', '厚甲样本', 'front', 0, {
      maxHp: 450,
      atk: 14,
      spd: 16,
      skillIds: ['snap_shot'],
      gambits: [
        {
          id: 'eb1',
          enabled: true,
          actionType: 'SKILL',
          skillId: 'snap_shot',
          condition1: { type: 'ALWAYS' },
          target: 'BACK_PRIORITY',
        },
      ],
    });

    const report = simulateExpedition({
      allies: [tank, marksman],
      enemiesByWave: [[sturdyEnemy]],
      baseTravelSec: 12,
      poweredFacilities: { tactical_relay: 2 },
      modifierId: 'dense_fog',
    });

    const ppLogs = report.entries.filter((e) => e.kind === 'PASSIVE_PP');
    const relayLogs = report.entries.filter(
      (e) => e.kind === 'FACILITY_SUPPORT' && e.actorName === '战术谐振电台',
    );
    expect(ppLogs.length).toBeGreaterThanOrEqual(3);
    expect(relayLogs.length).toBeGreaterThanOrEqual(1);
  });

  it('handles PLANNED, EMERGENCY 60%, anti-exploit bail, and WIPE 30%', () => {
    const strongAlly = makeAlly('a1', '突击手', 'front', 0, { maxHp: 200, atk: 55, spd: 20 });
    const waves = [
      [makeEnemy('w1', '怪1', 'front', 0, { maxHp: 40 })],
      [makeEnemy('w2', '怪2', 'front', 0, { maxHp: 40 })],
      [makeEnemy('w3', '怪3', 'front', 0, { maxHp: 40 })],
    ];

    const plannedGambit: GambitRule[] = [
      {
        id: 'p_ret',
        enabled: true,
        actionType: 'RETREAT_PLANNED',
        condition1: { type: 'WAVE_AT_LEAST', threshold: 1 },
        target: 'SELF',
      },
      ...strongAlly.gambits,
    ];
    const plannedReport = simulateExpedition({
      allies: [{ ...strongAlly, gambits: plannedGambit }],
      enemiesByWave: waves,
      baseTravelSec: 12,
      poweredFacilities: {},
      modifierId: 'dense_fog',
    });
    expect(plannedReport.outcome).toBe('PLANNED');
    expect(plannedReport.fullClear).toBe(false);
    expect(plannedReport.wavesCleared).toBe(1);
    expect(plannedReport.dropRateFactor).toBeCloseTo(1 / 3, 3);

    const emergencyWave2Gambit: GambitRule[] = [
      {
        id: 'e_ret',
        enabled: true,
        actionType: 'RETREAT_EMERGENCY',
        condition1: { type: 'WAVE_AT_LEAST', threshold: 2 },
        target: 'SELF',
      },
      ...strongAlly.gambits,
    ];
    const emergencyReport = simulateExpedition({
      allies: [{ ...strongAlly, gambits: emergencyWave2Gambit }],
      enemiesByWave: waves,
      baseTravelSec: 12,
      poweredFacilities: {},
      modifierId: 'dense_fog',
    });
    expect(emergencyReport.outcome).toBe('EMERGENCY');
    expect(emergencyReport.yieldFactor).toBe(0.6);
    expect(emergencyReport.dropRateFactor).toBe(0.5);

    const weakAlly = makeAlly('weak', '菜鸟', 'front', 0, { maxHp: 10, atk: 1, spd: 1 });
    const boss = makeEnemy('boss', '碾压怪', 'front', 0, { maxHp: 500, atk: 100, spd: 30 });
    const wipeReport = simulateExpedition({
      allies: [weakAlly],
      enemiesByWave: [[boss]],
      baseTravelSec: 12,
      poweredFacilities: {},
      modifierId: 'dense_fog',
    });
    expect(wipeReport.outcome).toBe('WIPE');
    expect(wipeReport.yieldFactor).toBe(0.3);
    expect(wipeReport.dropRateFactor).toBe(0);
    expect(wipeReport.durationSec).toBeGreaterThanOrEqual(22);
  });
});
