import { describe, expect, it } from 'vitest';
import {
  assaultDistrict,
  hireRecruit,
  reclassOperative,
  toggleEquippedTrait,
  unlockSkillTreeNode,
  upgradeHQDepartment,
} from '../actions.ts';
import { makeSpecialOperativeInstance } from '../economy/baseAndPatrol.ts';
import { createInitialGameState } from '../init.ts';
import { grantOperativeXpAndMastery } from '../operativeHelpers.ts';
import { calculatePendingOrgExp, executeGreatInfiltration } from '../prestige.ts';
import {
  SAVE_PREFIX,
  deserializeSave,
  migrateSavePayload,
  serializeSave,
} from '../save/serializer.ts';

describe('Skill Trees, Reclassing, Great Infiltration & V2 Saves', () => {
  it('supports skill tree unlocks, class mastery ★3 evility unlocks, and SP-refunding reclassing', () => {
    let state = createInitialGameState(101);
    state = {
      ...state,
      run: {
        ...state.run,
        resources: { ...state.run.resources, manuals: 20 },
      },
    };
    const opId = state.run.roster[0]!.id; // vanguard with 1 SP

    // Unlock vg_1 node in vanguard tree
    state = unlockSkillTreeNode(state, opId, 'vg_1');
    let op = state.run.roster[0]!;
    expect(op.unlockedNodeIds).toContain('vg_1');
    expect(op.skillPoints).toBe(0);

    // Grant 300 mastery XP -> reaches ★3 and unlocks evility_bulwark
    const masteryRes = grantOperativeXpAndMastery(op, 0, 300);
    expect(masteryRes.unlockedMasteryTraitId).toBe('evility_bulwark');
    state = {
      ...state,
      run: {
        ...state.run,
        roster: [masteryRes.nextOp, ...state.run.roster.slice(1)],
      },
    };

    // Equip unlocked evility
    state = toggleEquippedTrait(state, opId, 'evility_bulwark');
    expect(state.run.roster[0]!.equippedTraitIds).toContain('evility_bulwark');

    // Reclass from vanguard to striker -> refunds 1 SP, preserves mastery & learned skills/traits
    state = reclassOperative(state, opId, 'striker');
    op = state.run.roster[0]!;
    expect(op.classId).toBe('striker');
    expect(op.skillPoints).toBe(1);
    expect(op.unlockedNodeIds).toEqual([]);
    expect(op.classMastery['vanguard']).toBe(300);
    expect(op.learnedSkillIds).toContain('shield_bash');
    expect(op.learnedTraitIds).toContain('evility_bulwark');
  });

  it('tracks unfolding milestones and preserves Special Operative mastery & learned skills across Great Infiltration', () => {
    let state = createInitialGameState(2026);
    expect(state.hq.story.unfolding.baseUnlocked).toBe(false);

    // Capture District 1 -> unfolds base & workshop
    const assRes = assaultDistrict(state, 'squad_1', 'district_1')!;
    state = assRes.nextState;
    expect(state.hq.story.unfolding.baseUnlocked).toBe(true);

    const yaYu = makeSpecialOperativeInstance('ya_yu')!;
    yaYu.level = 5;
    yaYu.classMastery = { marksman: 250 };
    yaYu.learnedSkillIds = ['guard_cover'];
    yaYu.learnedTraitIds = ['evility_deadeye'];

    const candidate = state.run.recruitPool[0]!;
    state = {
      ...state,
      hq: {
        ...state.hq,
        orgExp: 50,
        unlockedBlueprints: ['cryo_booster'],
        unlockedTactics: [...state.hq.unlockedTactics, 'tac_cond_tag_check'],
        specialOperatives: [{ archetypeId: 'ya_yu', unlockedAtCycle: 0, timesDeployed: 1 }],
      },
      run: {
        ...state.run,
        resources: { materials: 300, parts: 50, crystals: 40, manuals: 15, intel: 36 },
        clearedDepths: { depth_1: true },
        inventory: [
          {
            uid: 'vault_rare_1',
            templateId: 'overclock_plug',
            rarity: 'rare',
            bonusRollPct: 18,
          },
          {
            uid: 'unvaulted_common_1',
            templateId: 'rust_carbine',
            rarity: 'common',
            bonusRollPct: 2,
          },
        ],
        roster: [...state.run.roster, yaYu],
      },
    };
    state = hireRecruit(state, candidate.candidateId);
    state = upgradeHQDepartment(state, 'dept_fast_response');

    const pendingExp = calculatePendingOrgExp(state);
    expect(pendingExp).toBeGreaterThan(0);

    const next = executeGreatInfiltration(
      state,
      { blueprints: ['cryo_booster'], tactics: ['tac_cond_tag_check'] },
      ['vault_rare_1'],
    );

    expect(next.hq.story.cycleCount).toBe(1);
    expect(next.hq.vaultEquipment.map((e) => e.uid)).toEqual(['vault_rare_1']);
    expect(next.run.inventory.map((e) => e.uid)).toEqual(['vault_rare_1']);
    expect(next.run.roster.some((o) => o.name === candidate.name)).toBe(false);

    // SSR Special Operative keeps Disgaea reincarnation carryovers while resetting level to 1
    const restoredYaYu = next.run.roster.find((o) => o.archetypeId === 'ya_yu')!;
    expect(restoredYaYu).toBeDefined();
    expect(restoredYaYu.level).toBe(1);
    expect(restoredYaYu.classMastery['marksman']).toBe(250);
    expect(restoredYaYu.learnedSkillIds).toContain('guard_cover');
    expect(restoredYaYu.learnedTraitIds).toContain('evility_deadeye');

    // Fast response auto-reclaimed district_1 + created district_1 plots + upgraded diesel_gen
    expect(next.run.districts.find((d) => d.id === 'district_1')?.captured).toBe(true);
    expect(next.run.plots.find((p) => p.plotId === 'hq_p1')?.level).toBe(2);
    expect(next.run.plots.length).toBe(5);
  });

  it('serializes and deserializes V2 saves with lz-string + zod, rejects corruption, and migrates old versions', () => {
    const initial = createInitialGameState(4242);
    const encoded = serializeSave(initial);
    expect(encoded.startsWith(SAVE_PREFIX)).toBe(true);

    const restored = deserializeSave(encoded);
    expect(restored.hq).toEqual(initial.hq);
    expect(restored.run.resources).toEqual(initial.run.resources);
    expect(restored.run.plots.length).toBe(initial.run.plots.length);

    expect(() => deserializeSave('INVALID_HEADER:abc')).toThrow();
    expect(() => deserializeSave(`${SAVE_PREFIX}corrupted_base64_@@@`)).toThrow();

    const migrated = migrateSavePayload({ version: 1, customFlag: true });
    expect(migrated.version).toBe(2);
    expect(migrated.customFlag).toBe(true);
  });
});
