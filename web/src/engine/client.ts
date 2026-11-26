// The only place the web client talks to the Rust engine. Everything is JSON across the wasm
// boundary; this wrapper parses it into the types in ../types.ts and owns saving and debug time.

import init, { WasmEngine } from './pkg/idle_engine.js';
import wasmUrl from './pkg/idle_engine_bg.wasm?url';
import type {
  BattleReport,
  Command,
  ContentView,
  DryRun,
  GarrisonOption,
  RoutePreview,
  View,
} from '../types';

const SAVE_KEY = 'idle-proto/v1/save';
const OFFSET_KEY = 'idle-proto/v1/time-offset';

export interface Ack {
  ok: boolean;
  error?: string;
  shape?: string;
}

function readOffset(): number {
  try {
    return Number(localStorage.getItem(OFFSET_KEY) ?? '0') || 0;
  } catch {
    return 0;
  }
}

export class Game {
  private engine: WasmEngine;
  /** Debug fast-forward: seconds added to the wall clock. Stored with the save. */
  offset: number;
  readonly content: ContentView;
  loadError: string | null = null;

  private constructor(engine: WasmEngine, offset: number, content: ContentView) {
    this.engine = engine;
    this.offset = offset;
    this.content = content;
  }

  static async boot(): Promise<Game> {
    await init({ module_or_path: wasmUrl });
    const content = JSON.parse(WasmEngine.content()) as ContentView;
    const offset = readOffset();
    let saved: string | null = null;
    try {
      saved = localStorage.getItem(SAVE_KEY);
    } catch {
      saved = null;
    }
    let loadError: string | null = null;
    if (saved) {
      try {
        const g = new Game(WasmEngine.load(saved), offset, content);
        g.tick();
        return g;
      } catch (e) {
        loadError = String(e instanceof Error ? e.message : e);
      }
    }
    const g = new Game(Game.fresh(offset), offset, content);
    g.loadError = loadError;
    g.persist();
    return g;
  }

  private static fresh(offset: number): WasmEngine {
    const seed = Math.floor(Math.random() * 2 ** 31);
    return new WasmEngine(seed, Math.floor(Date.now() / 1000) + offset, 'web');
  }

  now(): number {
    return Math.floor(Date.now() / 1000) + this.offset;
  }

  /** Catch the simulation up to now. Returns the number of new events. */
  tick(): number {
    return this.engine.advance(this.now());
  }

  view(): View {
    return JSON.parse(this.engine.view()) as View;
  }

  command(cmd: Command): Ack {
    this.tick();
    const ack = JSON.parse(this.engine.command(JSON.stringify(cmd))) as Ack;
    if (ack.ok) this.persist();
    return ack;
  }

  dryRun(cmd: Command): DryRun {
    this.tick();
    return JSON.parse(this.engine.dry_run(JSON.stringify(cmd))) as DryRun;
  }

  garrisonOptions(tile: number): GarrisonOption[] {
    return JSON.parse(this.engine.garrison_options(tile)) as GarrisonOption[];
  }

  /** op: undefined = current setup, null = nobody, number = that operative. */
  previewSurge(tile: number, op: number | null | undefined): BattleReport | null {
    const code = op === undefined ? -1 : op === null ? 0 : op;
    return JSON.parse(this.engine.preview_surge(tile, code)) as BattleReport | null;
  }

  previewRoute(squad: number, route: number[]): RoutePreview {
    return JSON.parse(this.engine.preview_route(squad, JSON.stringify(route))) as RoutePreview;
  }

  report(id: number): BattleReport | null {
    return JSON.parse(this.engine.report(id)) as BattleReport | null;
  }

  tripBattle(squad: number, tile: number): BattleReport | null {
    return JSON.parse(this.engine.trip_battle(squad, tile)) as BattleReport | null;
  }

  persist(): void {
    try {
      localStorage.setItem(SAVE_KEY, this.engine.save());
      localStorage.setItem(OFFSET_KEY, String(this.offset));
    } catch {
      // Storage can be unavailable (private mode); the game still runs in memory.
    }
  }

  exportSave(): string {
    return this.engine.save();
  }

  importSave(text: string): string | null {
    try {
      const e = WasmEngine.load(text.trim());
      this.engine.free();
      this.engine = e;
      this.tick();
      this.persist();
      return null;
    } catch (e) {
      return String(e instanceof Error ? e.message : e);
    }
  }

  reset(): void {
    this.engine.free();
    this.offset = 0;
    this.engine = Game.fresh(0);
    this.persist();
  }

  /** Debug: jump the clock forward. Offline catch-up runs the same discrete-event path. */
  skip(seconds: number): number {
    this.offset += seconds;
    const n = this.tick();
    this.persist();
    return n;
  }
}
