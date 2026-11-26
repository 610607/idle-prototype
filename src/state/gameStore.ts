import { useSyncExternalStore } from 'react';
import * as workerTimers from 'worker-timers';
import {
  assaultDistrict,
  assaultOtherworld,
  assignSquadMember,
  assignSquadPatrol,
  boostOperativeWithManuals,
  buildOnPlot,
  cancelDojoInheritance,
  craftEquipmentWithParts,
  debugUnlockAllStages,
  demolishPlot,
  hireRecruit,
  reclassOperative,
  recallOperativeToIdle,
  refreshRecruitPool,
  removeSquadMember,
  researchTacticWithManuals,
  setAutoSalvageMaxRarity,
  startDojoInheritance,
  stationOperativeOnPlot,
  toggleEquippedTrait,
  togglePlotPower,
  unlockSkillTreeNode,
  updateOperativeGambits,
  upgradeHQDepartment,
  upgradePlot,
  type AssaultActionResult,
} from '../engine/actions.ts';
import {
  computePowerGrid,
  equipItemOnOperative,
  recomputePatrolBenchmarks,
  salvageEquipmentFromInventory,
  unequipItemFromOperative,
} from '../engine/economy/baseAndPatrol.ts';
import { createInitialGameState } from '../engine/init.ts';
import { settleOffline } from '../engine/offline.ts';
import { getVaultCapacity } from '../engine/operativeHelpers.ts';
import { executeGreatInfiltration } from '../engine/prestige.ts';
import {
  clearLocalStorageSaves,
  deserializeSave,
  loadFromLocalStorage,
  saveToLocalStorage,
  serializeSave,
} from '../engine/save/serializer.ts';
import { tick } from '../engine/tick.ts';
import { syncUnfoldingFlags } from '../engine/unfolding.ts';
import type {
  ClassId,
  CombatReport,
  GambitRule,
  RowType,
  SlotIndex,
} from '../types/combat.ts';
import type {
  EquipmentSlot,
  GameState,
  OfflineSummary,
  PowerGridSummary,
} from '../types/game.ts';

export type ActiveTab = 'expedition' | 'formation' | 'operatives' | 'base' | 'hq';

export interface StoreSnapshot {
  state: GameState;
  powerGrid: PowerGridSummary;
  activeTab: ActiveTab;
  selectedSquadId: string;
  selectedOperativeId: string | null;
  activeReport: CombatReport | null;
  offlineSummary: OfflineSummary | null;
  vaultMarkedUids: string[];
  toast: string | null;
}

function initInitialSnapshot(): StoreSnapshot {
  const loaded = loadFromLocalStorage();
  let state = loaded.state ?? createInitialGameState();
  let offlineSummary: OfflineSummary | null = null;

  if (loaded.state) {
    const elapsedSec = Math.max(0, (Date.now() - loaded.state.savedAt) / 1000);
    if (elapsedSec > 60) {
      const res = settleOffline(loaded.state, elapsedSec);
      state = res.nextState;
      offlineSummary = res.summary;
    }
  }

  state = syncUnfoldingFlags(recomputePatrolBenchmarks(computePowerGrid(state).nextState));
  const powerGrid = computePowerGrid(state).summary;

  return {
    state,
    powerGrid,
    activeTab: 'expedition',
    selectedSquadId: state.run.squads[0]?.id ?? 'squad_1',
    selectedOperativeId: state.run.roster[0]?.id ?? null,
    activeReport: state.run.lastBattleReport,
    offlineSummary,
    vaultMarkedUids: state.hq.vaultEquipment.map((e) => e.uid),
    toast: loaded.error ? `读档回退：${loaded.error}` : null,
  };
}

class GameController {
  private snapshot: StoreSnapshot;
  private listeners = new Set<() => void>();
  private timerId: number | null = null;
  private usingWorkerTimer = false;
  private lastTickMs = Date.now();
  private ticksSinceSave = 0;

  constructor() {
    this.snapshot = initInitialSnapshot();
  }

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    if (this.listeners.size === 1) {
      this.startLoop();
    }
    return () => {
      this.listeners.delete(listener);
      if (this.listeners.size === 0) {
        this.stopLoop();
      }
    };
  };

  getSnapshot = (): StoreSnapshot => this.snapshot;

  private emit() {
    for (const listener of this.listeners) {
      listener();
    }
  }

  private updateState(
    nextState: GameState,
    extra: Partial<Omit<StoreSnapshot, 'state' | 'powerGrid'>> = {},
    persistNow = false,
  ) {
    const grid = computePowerGrid(syncUnfoldingFlags(nextState));
    this.snapshot = {
      ...this.snapshot,
      ...extra,
      state: grid.nextState,
      powerGrid: grid.summary,
    };
    if (persistNow) {
      saveToLocalStorage(this.snapshot.state);
      this.ticksSinceSave = 0;
    }
    this.emit();
  }

  private startLoop() {
    if (this.timerId !== null || typeof window === 'undefined') return;
    this.lastTickMs = Date.now();
    const step = () => {
      const now = Date.now();
      const dtSec = Math.max(0, (now - this.lastTickMs) / 1000);
      this.lastTickMs = now;

      if (dtSec > 60) {
        const res = settleOffline(this.snapshot.state, dtSec);
        this.updateState(res.nextState, { offlineSummary: res.summary }, true);
        return;
      }

      const tickRes = tick(this.snapshot.state, dtSec);
      this.ticksSinceSave++;
      const shouldSave =
        this.ticksSinceSave >= 75 || Boolean(tickRes.events.dojoCompletedText);
      this.updateState(
        tickRes.nextState,
        tickRes.events.dojoCompletedText ? { toast: tickRes.events.dojoCompletedText } : {},
        shouldSave,
      );
    };

    try {
      if (typeof Worker !== 'undefined') {
        this.timerId = workerTimers.setInterval(step, 200);
        this.usingWorkerTimer = true;
        return;
      }
    } catch {
      // fallback
    }
    this.timerId = window.setInterval(step, 200);
    this.usingWorkerTimer = false;
  }

  private stopLoop() {
    if (this.timerId === null) return;
    try {
      if (this.usingWorkerTimer) {
        workerTimers.clearInterval(this.timerId);
      } else {
        window.clearInterval(this.timerId);
      }
    } catch {
      // ignore
    }
    this.timerId = null;
  }

  setTab(tab: ActiveTab) {
    this.snapshot = { ...this.snapshot, activeTab: tab };
    this.emit();
  }

  setSelectedSquad(squadId: string) {
    this.snapshot = { ...this.snapshot, selectedSquadId: squadId };
    this.emit();
  }

  setSelectedOperative(operativeId: string | null) {
    this.snapshot = { ...this.snapshot, selectedOperativeId: operativeId };
    this.emit();
  }

  openOperativeDossier(operativeId: string) {
    this.snapshot = {
      ...this.snapshot,
      activeTab: 'operatives',
      selectedOperativeId: operativeId,
    };
    this.emit();
  }

  openSquadFormation(squadId?: string, operativeId?: string) {
    this.snapshot = {
      ...this.snapshot,
      activeTab: 'formation',
      selectedSquadId: squadId ?? this.snapshot.selectedSquadId,
      selectedOperativeId: operativeId ?? this.snapshot.selectedOperativeId,
    };
    this.emit();
  }

  setActiveReport(report: CombatReport | null, switchToExpedition = false) {
    this.snapshot = {
      ...this.snapshot,
      activeReport: report,
      activeTab: switchToExpedition ? 'expedition' : this.snapshot.activeTab,
    };
    this.emit();
  }

  dismissOfflineModal() {
    this.snapshot = { ...this.snapshot, offlineSummary: null };
    this.emit();
  }

  clearToast() {
    this.snapshot = { ...this.snapshot, toast: null };
    this.emit();
  }

  showToast(msg: string) {
    this.snapshot = { ...this.snapshot, toast: msg };
    this.emit();
  }

  runSimulateOffline(seconds: number) {
    const res = settleOffline(this.snapshot.state, seconds);
    this.updateState(res.nextState, { offlineSummary: res.summary }, true);
  }

  debugAddResources() {
    const s = this.snapshot.state;
    this.updateState(
      {
        ...s,
        run: {
          ...s.run,
          resources: {
            materials: s.run.resources.materials + 150,
            parts: s.run.resources.parts + 40,
            crystals: s.run.resources.crystals + 25,
            manuals: s.run.resources.manuals + 20,
            intel: s.run.resources.intel + 20,
          },
        },
      },
      { toast: '调试注入：废材+150 / 零件+40 / 原晶+25 / 手札+20 / 情报+20' },
      true,
    );
  }

  doDebugUnlockAll() {
    this.updateState(
      debugUnlockAllStages(this.snapshot.state),
      { toast: '已展开全部渐进阶段页签与面板' },
      true,
    );
  }

  doEnableGuidedTactics() {
    const s = this.snapshot.state;
    this.updateState(
      {
        ...s,
        hq: {
          ...s.hq,
          story: {
            ...s.hq.story,
            unfolding: {
              ...s.hq.story.unfolding,
              tacticsGuided: true,
            },
          },
        },
      },
      {},
      true,
    );
  }

  doAssaultDistrict(squadId: string, districtId: string): AssaultActionResult | null {
    const res = assaultDistrict(this.snapshot.state, squadId, districtId);
    if (!res) return null;
    const msg = res.newlyCaptured
      ? '夺回街区成功！已开放新地块与据点设施。'
      : res.report.fullClear
        ? '演练全胜！队员获得经验与职业熟练度。'
        : '攻坚未能全清，请查看战果摘要并调整职业技能树或战术。';
    this.updateState(res.nextState, { activeReport: res.report, toast: msg }, true);
    return res;
  }

  doAssaultOtherworld(squadId: string, depthId: string): AssaultActionResult | null {
    const res = assaultOtherworld(this.snapshot.state, squadId, depthId);
    if (!res) return null;
    const parts: string[] = [];
    if (res.newlyClearedDepth) parts.push('首次打通该异界深度，已开放挂机巡逻！');
    if (res.droppedItemNames.length > 0) parts.push(`掉落装备 ×${res.droppedItemNames.length}`);
    if (res.unlockedBlueprints.length > 0) parts.push(`解锁新蓝图 ×${res.unlockedBlueprints.length}`);
    if (res.unlockedTactics.length > 0) parts.push(`解锁新战术 ×${res.unlockedTactics.length}`);
    if (res.unlockedSpecialOperatives.length > 0) parts.push('找回 SSR 特别队员！');
    if (parts.length === 0) {
      parts.push(
        res.report.outcome === 'PLANNED'
          ? '异界探索凯旋。'
          : res.report.outcome === 'EMERGENCY'
            ? '触发紧急撤退，带回 60% 物资与折半掉率。'
            : '小队战败，抢救 30% 基础物资撤回。',
      );
    }
    this.updateState(
      res.nextState,
      { activeReport: res.report, toast: parts.join(' ') },
      true,
    );
    return res;
  }

  doBuildOnPlot(plotId: string, facilityId: string) {
    this.updateState(buildOnPlot(this.snapshot.state, plotId, facilityId), {}, true);
  }

  doUpgradePlot(plotId: string) {
    this.updateState(upgradePlot(this.snapshot.state, plotId), {}, true);
  }

  doDemolishPlot(plotId: string) {
    this.updateState(demolishPlot(this.snapshot.state, plotId), {}, true);
  }

  doTogglePlotPower(plotId: string) {
    this.updateState(togglePlotPower(this.snapshot.state, plotId), {}, true);
  }

  doStationOperative(plotId: string, operativeId: string | null) {
    this.updateState(stationOperativeOnPlot(this.snapshot.state, plotId, operativeId), {}, true);
  }

  doCraftEquipment(templateId: string) {
    this.updateState(
      craftEquipmentWithParts(this.snapshot.state, templateId),
      { toast: '装备锻造完成，已放入装备库。' },
      true,
    );
  }

  doResearchTactic(tacticId: string) {
    this.updateState(
      researchTacticWithManuals(this.snapshot.state, tacticId),
      { toast: '战术芯片破译完成，现可在战术表使用！' },
      true,
    );
  }

  doStartDojo(
    mentorId: string,
    studentId: string,
    targetKind: 'skill' | 'trait' | 'tactic',
    targetId: string,
  ) {
    this.updateState(
      startDojoInheritance(this.snapshot.state, mentorId, studentId, targetKind, targetId),
      { toast: '已开启联合演武传授，随挂机进度自动完成。' },
      true,
    );
  }

  doCancelDojo() {
    this.updateState(cancelDojoInheritance(this.snapshot.state), {}, true);
  }

  doHireRecruit(candidateId: string) {
    const next = hireRecruit(this.snapshot.state, candidateId);
    const added = next.run.roster[next.run.roster.length - 1];
    this.updateState(
      next,
      {
        selectedOperativeId: added?.id ?? this.snapshot.selectedOperativeId,
        toast: added ? `已招募 [${added.rarity}] ${added.name}` : undefined,
      },
      true,
    );
  }

  doRefreshRecruitPool() {
    this.updateState(refreshRecruitPool(this.snapshot.state), {}, true);
  }

  doBoostOperative(operativeId: string) {
    this.updateState(boostOperativeWithManuals(this.snapshot.state, operativeId), {}, true);
  }

  doUnlockTreeNode(operativeId: string, nodeId: string) {
    this.updateState(unlockSkillTreeNode(this.snapshot.state, operativeId, nodeId), {}, true);
  }

  doReclassOperative(operativeId: string, newClassId: ClassId) {
    this.updateState(reclassOperative(this.snapshot.state, operativeId, newClassId), {}, true);
  }

  doToggleEquippedTrait(operativeId: string, traitId: string) {
    this.updateState(toggleEquippedTrait(this.snapshot.state, operativeId, traitId), {}, true);
  }

  doAssignSquadMember(squadId: string, operativeId: string, row: RowType, slot: SlotIndex) {
    this.updateState(
      assignSquadMember(this.snapshot.state, squadId, operativeId, row, slot),
      {},
      true,
    );
  }

  doRemoveSquadMember(squadId: string, operativeId: string) {
    this.updateState(removeSquadMember(this.snapshot.state, squadId, operativeId), {}, true);
  }

  doRecallOperative(operativeId: string) {
    this.updateState(recallOperativeToIdle(this.snapshot.state, operativeId), {}, true);
  }

  doUpdateGambits(operativeId: string, gambits: GambitRule[]) {
    this.updateState(updateOperativeGambits(this.snapshot.state, operativeId, gambits), {}, true);
  }

  doAssignPatrol(squadId: string, depthId: string | null) {
    this.updateState(assignSquadPatrol(this.snapshot.state, squadId, depthId), {}, true);
  }

  doEquipItem(operativeId: string, equipmentUid: string) {
    this.updateState(equipItemOnOperative(this.snapshot.state, operativeId, equipmentUid), {}, true);
  }

  doUnequipItem(operativeId: string, slot: EquipmentSlot) {
    this.updateState(unequipItemFromOperative(this.snapshot.state, operativeId, slot), {}, true);
  }

  doSalvageItem(equipmentUid: string) {
    this.updateState(
      salvageEquipmentFromInventory(this.snapshot.state, equipmentUid),
      {
        vaultMarkedUids: this.snapshot.vaultMarkedUids.filter((id) => id !== equipmentUid),
      },
      true,
    );
  }

  doSetAutoSalvage(rarity: 'none' | 'common' | 'uncommon') {
    this.updateState(setAutoSalvageMaxRarity(this.snapshot.state, rarity), {}, true);
  }

  doToggleVaultMark(equipmentUid: string) {
    const cap = getVaultCapacity(this.snapshot.state.hq);
    const exists = this.snapshot.vaultMarkedUids.includes(equipmentUid);
    if (exists) {
      this.snapshot = {
        ...this.snapshot,
        vaultMarkedUids: this.snapshot.vaultMarkedUids.filter((id) => id !== equipmentUid),
      };
      this.emit();
      return;
    }
    if (this.snapshot.vaultMarkedUids.length >= cap) {
      this.showToast(`总部封存格已满（上限 ${cap} 件），请先取消一件或升级铅封库。`);
      return;
    }
    this.snapshot = {
      ...this.snapshot,
      vaultMarkedUids: [...this.snapshot.vaultMarkedUids, equipmentUid],
    };
    this.emit();
  }

  doUpgradeDepartment(deptId: string) {
    this.updateState(upgradeHQDepartment(this.snapshot.state, deptId), {}, true);
  }

  doGreatInfiltration(chosenLoadout: { blueprints: string[]; tactics: string[] }) {
    const next = executeGreatInfiltration(
      this.snapshot.state,
      chosenLoadout,
      this.snapshot.vaultMarkedUids,
    );
    this.updateState(
      next,
      {
        selectedSquadId: next.run.squads[0]?.id ?? 'squad_1',
        selectedOperativeId: next.run.roster[0]?.id ?? null,
        activeReport: null,
        vaultMarkedUids: next.hq.vaultEquipment.map((e) => e.uid),
        activeTab: 'expedition',
        toast: `大渗透重开完成！进入第 ${next.hq.story.cycleCount} 轮。`,
      },
      true,
    );
  }

  exportSaveString(): string {
    return serializeSave(this.snapshot.state);
  }

  importSaveString(encoded: string): { ok: boolean; error?: string } {
    try {
      const parsed = deserializeSave(encoded);
      this.updateState(
        parsed,
        {
          selectedSquadId: parsed.run.squads[0]?.id ?? 'squad_1',
          selectedOperativeId: parsed.run.roster[0]?.id ?? null,
          activeReport: parsed.run.lastBattleReport,
          vaultMarkedUids: parsed.hq.vaultEquipment.map((e) => e.uid),
          toast: '存档导入成功！',
        },
        true,
      );
      return { ok: true };
    } catch (err) {
      return { ok: false, error: err instanceof Error ? err.message : '导入失败' };
    }
  }

  resetAllSave() {
    clearLocalStorageSaves();
    const fresh = createInitialGameState(Date.now() & 0x7fffffff);
    this.updateState(
      fresh,
      {
        selectedSquadId: fresh.run.squads[0]?.id ?? 'squad_1',
        selectedOperativeId: fresh.run.roster[0]?.id ?? null,
        activeReport: null,
        offlineSummary: null,
        vaultMarkedUids: [],
        activeTab: 'expedition',
        toast: '已重置为全新初始存档。',
      },
      true,
    );
  }
}

export const gameController = new GameController();

export function useGameStore(): StoreSnapshot {
  return useSyncExternalStore(
    gameController.subscribe,
    gameController.getSnapshot,
    gameController.getSnapshot,
  );
}
