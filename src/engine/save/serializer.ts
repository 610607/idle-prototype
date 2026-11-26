import LZString from 'lz-string';
import type { GameState } from '../../types/game.ts';
import { computePowerGrid, recomputePatrolBenchmarks } from '../economy/baseAndPatrol.ts';
import { CURRENT_SAVE_VERSION } from '../init.ts';
import { GameSaveSchemaV2 } from './schema.ts';

export const SAVE_PREFIX = 'IDLE_PROTO_V2:';
const PRIMARY_STORAGE_KEY = 'idle_prototype_save_v2_main';
const BACKUP_STORAGE_KEYS = [
  'idle_prototype_save_v2_bak_0',
  'idle_prototype_save_v2_bak_1',
  'idle_prototype_save_v2_bak_2',
];
const BACKUP_CURSOR_KEY = 'idle_prototype_save_v2_bak_cursor';

export type SaveMigrationFn = (raw: Record<string, unknown>) => Record<string, unknown>;

export const SAVE_MIGRATIONS: Record<number, SaveMigrationFn> = {
  0: (raw) => ({
    ...raw,
    version: 2,
  }),
  1: (raw) => ({
    ...raw,
    version: 2,
  }),
};

export function migrateSavePayload(
  raw: Record<string, unknown>,
  migrations: Record<number, SaveMigrationFn> = SAVE_MIGRATIONS,
): Record<string, unknown> {
  let current = { ...raw };
  let version = typeof current.version === 'number' ? current.version : 0;
  let guard = 0;

  while (version < CURRENT_SAVE_VERSION && guard < 20) {
    guard++;
    const fn = migrations[version];
    if (!fn) {
      throw new Error(`Missing save migration for version ${version}`);
    }
    current = fn(current);
    version = typeof current.version === 'number' ? current.version : version + 1;
  }

  return current;
}

export function serializeSave(state: GameState): string {
  const payload: GameState = {
    ...state,
    version: CURRENT_SAVE_VERSION,
    savedAt: Date.now(),
  };
  const json = JSON.stringify(payload);
  const compressed = LZString.compressToBase64(json);
  return `${SAVE_PREFIX}${compressed}`;
}

export function deserializeSave(
  encoded: string,
  migrations: Record<number, SaveMigrationFn> = SAVE_MIGRATIONS,
): GameState {
  const trimmed = encoded.trim();
  if (!trimmed.startsWith(SAVE_PREFIX)) {
    throw new Error('存档格式无效：缺少 IDLE_PROTO_V2 前缀');
  }
  const body = trimmed.slice(SAVE_PREFIX.length);
  const json = LZString.decompressFromBase64(body);
  if (!json) {
    throw new Error('存档解压失败：字符串可能已损坏或截断');
  }

  let parsedRaw: unknown;
  try {
    parsedRaw = JSON.parse(json);
  } catch {
    throw new Error('存档 JSON 解析失败');
  }

  if (!parsedRaw || typeof parsedRaw !== 'object' || Array.isArray(parsedRaw)) {
    throw new Error('存档顶层结构无效');
  }

  const migrated = migrateSavePayload(parsedRaw as Record<string, unknown>, migrations);
  const result = GameSaveSchemaV2.safeParse(migrated);
  if (!result.success) {
    throw new Error(`存档校验失败：${result.error.issues[0]?.message ?? '结构不匹配'}`);
  }

  let validated = result.data as GameState;
  validated = computePowerGrid(validated).nextState;
  validated = recomputePatrolBenchmarks(validated);
  return validated;
}

export function saveToLocalStorage(state: GameState): string {
  const serialized = serializeSave(state);
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem(PRIMARY_STORAGE_KEY, serialized);
    const cursorRaw = localStorage.getItem(BACKUP_CURSOR_KEY);
    const cursor = cursorRaw ? Number(cursorRaw) % BACKUP_STORAGE_KEYS.length : 0;
    const backupKey = BACKUP_STORAGE_KEYS[cursor] ?? BACKUP_STORAGE_KEYS[0]!;
    localStorage.setItem(backupKey, serialized);
    localStorage.setItem(BACKUP_CURSOR_KEY, String((cursor + 1) % BACKUP_STORAGE_KEYS.length));
  }
  return serialized;
}

export function loadFromLocalStorage(): { state: GameState | null; error?: string } {
  if (typeof localStorage === 'undefined') {
    return { state: null };
  }
  const primary = localStorage.getItem(PRIMARY_STORAGE_KEY);
  if (primary) {
    try {
      return { state: deserializeSave(primary) };
    } catch (err) {
      for (const bakKey of BACKUP_STORAGE_KEYS) {
        const bak = localStorage.getItem(bakKey);
        if (!bak) continue;
        try {
          return { state: deserializeSave(bak) };
        } catch {
          // continue searching backups
        }
      }
      return {
        state: null,
        error: err instanceof Error ? err.message : '读取本地存档失败',
      };
    }
  }
  return { state: null };
}

export function clearLocalStorageSaves(): void {
  if (typeof localStorage === 'undefined') return;
  localStorage.removeItem(PRIMARY_STORAGE_KEY);
  for (const bakKey of BACKUP_STORAGE_KEYS) {
    localStorage.removeItem(bakKey);
  }
  localStorage.removeItem(BACKUP_CURSOR_KEY);
}
