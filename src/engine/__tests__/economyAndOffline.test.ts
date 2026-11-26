import { describe, expect, it } from 'vitest';
import {
  assaultDistrict,
  assignSquadMember,
  buildOnPlot,
  recallOperativeToIdle,
  setAutoSalvageMaxRarity,
  startDojoInheritance,
  stationOperativeOnPlot,
  upgradePlot,
} from '../actions.ts';
import {
  MAX_INVENTORY_SIZE,
  computePowerGrid,
  getPassiveRatesPerSec,
  recomputePatrolBenchmarks,
  rollExpeditionLoot,
} from '../economy/baseAndPatrol.ts';
import { createInitialGameState } from '../init.ts';
import { settleOffline } from '../offline.ts';
import { getOperativeLocation } from '../operativeHelpers.ts';
import { tick } from '../tick.ts';

describe('Plot Base Building, Stationing, Dojo, Patrol & Offline', () => {
  it('manages plot power brownouts cleanly and resumes when generator plot upgrades', () => {
    let state = createInitialGameState(12345);
    state = {
      ...state,
      run: {
        ...state.run,
        activeModifierId: 'dense_fog',
        resources: { ...state.run.resources, materials: 600, parts: 100, crystals: 50 },
        districts: state.run.districts.map((d) =>
          d.id === 'district_1' || d.id === 'district_2'
            ? { ...d, captured: true }
            : d,
        ),
      },
    };
    // hq_p1 has diesel_gen Lv.1 (14 kW * 1.15 terrain = 16 kW)
    // Build tactical_relay (-7 kW), barracks_expansion (-6 kW), and archive_lab (-6 kW) -> demand 19 kW > 16 kW
    state = buildOnPlot(state, 'hq_p2', 'tactical_relay');
    state = buildOnPlot(state, 'hq_p3', 'barracks_expansion');
    // Add a district plot with archive_lab (-6 kW)
    state = {
      ...state,
      run: {
        ...state.run,
        plots: [
          ...state.run.plots,
          {
            plotId: 'd1_p2',
            sourceId: 'district_1',
            terrainLabel: 'test',
            bonusCategory: 'research',
            bonusPct: 20,
            facilityId: 'archive_lab',
            level: 1,
            enabled: true,
            powered: false,
            stationedOperativeId: null,
          },
        ],
      },
    };
    const grid1 = computePowerGrid(state);
    state = grid1.nextState;
    expect(grid1.summary.generated).toBe(16);
    expect(grid1.summary.demand).toBe(19);
    expect(grid1.summary.brownedOutPlotIds.length).toBe(1);

    // Upgrade diesel_gen on hq_p1 -> all plots turn back on
    state = upgradePlot(state, 'hq_p1');
    const grid2 = computePowerGrid(state);
    expect(grid2.summary.generated).toBeGreaterThan(25);
    expect(grid2.summary.brownedOutPlotIds).toEqual([]);
  });

  it('clamps 2-tier materials->parts conversion and applies CoC stationing bonuses', () => {
    let state = createInitialGameState(555);
    // Capture district_1 to unlock parts_workshop
    const assRes = assaultDistrict(state, 'squad_1', 'district_1')!;
    state = assRes.nextState;
    expect(state.hq.story.unfolding.baseUnlocked).toBe(true);

    // Build parts_workshop on hq_p2 with 0 materials left to verify non-negative clamping
    state = {
      ...state,
      run: {
        ...state.run,
        resources: { ...state.run.resources, materials: 100 },
      },
    };
    state = buildOnPlot(state, 'hq_p2', 'parts_workshop');
    const baseRates = getPassiveRatesPerSec(state);

    // Station high-INT/DEX operative 苏扳手 on parts_workshop -> boosts output & removes from squad
    const suId = state.run.roster[1]!.id;
    state = stationOperativeOnPlot(state, 'hq_p2', suId);
    const boostedRates = getPassiveRatesPerSec(state);
    expect(boostedRates.parts).toBeGreaterThan(baseRates.parts);
    expect(state.run.squads[0]!.members.some((m) => m.operativeId === suId)).toBe(false);

    // Drain materials to 0 -> ticking should never drive materials negative
    state = {
      ...state,
      run: {
        ...state.run,
        resources: { ...state.run.resources, materials: 0, parts: 0 },
      },
    };
    const ticked = tick(state, 5);
    expect(ticked.nextState.run.resources.materials).toBeGreaterThanOrEqual(0);
  });

  it('completes Disgaea Dojo skill inheritance during tick and offline settlement', () => {
    let state = createInitialGameState(888);
    state = {
      ...state,
      run: {
        ...state.run,
        resources: { materials: 500, parts: 100, crystals: 50, manuals: 20, intel: 20 },
        districts: state.run.districts.map((d) =>
          d.id === 'district_1' || d.id === 'district_2' ? { ...d, captured: true } : d,
        ),
      },
    };
    state = upgradePlot(state, 'hq_p1');
    state = buildOnPlot(state, 'hq_p2', 'training_dojo');

    const mentor = state.run.roster[0]!; // 陈巡街 has shield_bash, guard_cover
    const student = state.run.roster[1]!; // 苏扳手 lacks guard_cover
    expect(student.learnedSkillIds).not.toContain('guard_cover');

    state = startDojoInheritance(state, mentor.id, student.id, 'skill', 'guard_cover');
    expect(state.run.dojo).not.toBeNull();

    const offlineRes = settleOffline(state, 40);
    expect(offlineRes.nextState.run.dojo).toBeNull();
    expect(offlineRes.summary.dojoCompletedText).toContain('警戒掩护');
    const updatedStudent = offlineRes.nextState.run.roster.find((o) => o.id === student.id)!;
    expect(updatedStudent.learnedSkillIds).toContain('guard_cover');
  });

  it('settles offline patrol cycles deterministically and respects autoSalvageMaxRarity', () => {
    let state = createInitialGameState(777);
    state = {
      ...state,
      run: {
        ...state.run,
        clearedDepths: { depth_1: true },
        squads: state.run.squads.map((s) =>
          s.id === 'squad_1' ? { ...s, assignedDepthId: 'depth_1', patrolProgressSec: 0 } : s,
        ),
      },
    };
    state = recomputePatrolBenchmarks(state);
    const bench = state.run.patrolBenchmarks['squad_1']!;
    const dt = bench.durationSec * 10 + 3.5;

    const res1 = settleOffline(state, dt);
    const res2 = settleOffline(state, dt);
    expect(res1.summary.squadReports[0]?.completedCycles).toBe(10);
    expect(res1.nextState.run.squads[0]?.patrolProgressSec).toBeCloseTo(3.5, 4);
    expect(res1.summary.newEquipment).toEqual(res2.summary.newEquipment);

    state = setAutoSalvageMaxRarity(state, 'common');
    const lootRes = rollExpeditionLoot(state, 'depth_1', 1.0, 200, 0.1);
    expect(lootRes.nextState.run.inventory.length).toBeLessThanOrEqual(MAX_INVENTORY_SIZE);
    for (const eq of lootRes.droppedEquipment) {
      expect(eq.rarity).not.toBe('common');
    }
    expect(lootRes.salvagedCount).toBeGreaterThan(0);
  });

  it('transfers operatives cleanly between squad, plot stationing, and idle recall', () => {
    let state = createInitialGameState(9001);
    const opId = state.run.roster[1]!.id;
    state = {
      ...state,
      run: {
        ...state.run,
        clearedDepths: { depth_1: true },
        resources: { ...state.run.resources, materials: 400 },
        districts: state.run.districts.map((d) =>
          d.id === 'district_1' ? { ...d, captured: true } : d,
        ),
      },
    };
    state = buildOnPlot(state, 'hq_p2', 'salvage_yard');
    state = {
      ...state,
      run: {
        ...state.run,
        squads: state.run.squads.map((s) =>
          s.id === 'squad_1' ? { ...s, assignedDepthId: 'depth_1' } : s,
        ),
      },
    };
    state = recomputePatrolBenchmarks(state);

    // 1. Directly station squad member onto plot -> removed from squad & clears patrol if squad emptied or recalculates benchmark
    const squadMembersBefore = state.run.squads[0]!.members.length;
    state = stationOperativeOnPlot(state, 'hq_p2', opId);
    expect(getOperativeLocation(state, opId)).toMatchObject({ kind: 'station', plotId: 'hq_p2' });
    expect(state.run.plots.find((p) => p.plotId === 'hq_p2')?.stationedOperativeId).toBe(opId);
    expect(state.run.squads[0]!.members).toHaveLength(squadMembersBefore - 1);
    expect(
      state.run.patrolBenchmarks['squad_1']?.sampleReport.unitStats.filter((u) => u.side === 'ally'),
    ).toHaveLength(squadMembersBefore - 1);

    // 2. Assign back directly to squad -> vacates plot automatically
    state = assignSquadMember(state, 'squad_1', opId, 'back', 1);
    expect(getOperativeLocation(state, opId)).toMatchObject({ kind: 'squad', squadId: 'squad_1' });
    expect(state.run.plots.find((p) => p.plotId === 'hq_p2')?.stationedOperativeId).toBeNull();
    expect(state.run.squads[0]!.members).toHaveLength(squadMembersBefore);
    expect(
      state.run.patrolBenchmarks['squad_1']?.sampleReport.unitStats.filter((u) => u.side === 'ally'),
    ).toHaveLength(squadMembersBefore);

    // 3. Recall operative back to idle -> leaves squad cleanly and updates patrol benchmark
    state = recallOperativeToIdle(state, opId);
    expect(getOperativeLocation(state, opId)).toEqual({ kind: 'idle', label: '待命' });
    expect(state.run.squads[0]!.members.some((m) => m.operativeId === opId)).toBe(false);
    expect(state.run.plots.some((p) => p.stationedOperativeId === opId)).toBe(false);
    expect(
      state.run.patrolBenchmarks['squad_1']?.sampleReport.unitStats.filter((u) => u.side === 'ally'),
    ).toHaveLength(squadMembersBefore - 1);
  });
});
