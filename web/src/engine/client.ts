// The only place the web client talks to the Rust engine. Everything is JSON across the wasm
// boundary; this wrapper parses it into the types in ../types.ts and owns saving and debug time.

import init, { WasmEngine } from './pkg/idle_engine.js';
import wasmUrl from './pkg/idle_engine_bg.wasm?url';
import type {
  Balance,
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
  /** Balance numbers, for text that quotes them (never write them into components). */
  readonly balance: Balance;

  private constructor(engine: WasmEngine, offset: number, content: ContentView, balance: Balance) {
    this.engine = engine;
    this.offset = offset;
    this.content = content;
    this.balance = balance;
  }

  /** Load the engine and the saved game. Null: nothing saved yet, so the player picks how to start
   *  (`start`). `bootError` then says why a save could not be read, if there was one. */
  static async boot(): Promise<Game | null> {
    await init({ module_or_path: wasmUrl });
    Game.content = JSON.parse(WasmEngine.content()) as ContentView;
    Game.balanceJson = JSON.parse(WasmEngine.balance()) as Balance;
    // Debug entry: `?map=tutorial` starts a new game on that fixed map (maps.json), replacing the
    // saved one; the parameter is dropped from the address so a reload keeps playing it. Static
    // hosts may drop query strings: the start screen offers the same choice in the page.
    const map = new URLSearchParams(window.location.search).get('map');
    if (map) {
      const url = new URL(window.location.href);
      url.searchParams.delete('map');
      window.history.replaceState(null, '', url.toString());
      return Game.start(map);
    }
    let saved: string | null = null;
    try {
      saved = localStorage.getItem(SAVE_KEY);
    } catch {
      saved = null;
    }
    if (saved) {
      try {
        const g = new Game(WasmEngine.load(saved), readOffset(), Game.content, Game.balanceJson);
        g.tick();
        return g;
      } catch (e) {
        Game.bootError = String(e instanceof Error ? e.message : e);
      }
    }
    return null;
  }

  private static content: ContentView;
  private static balanceJson: Balance;
  /** Why the saved game could not be loaded (the start screen says so). */
  static bootError: string | null = null;

  /** A new game: a fixed map by id (`tutorial`), or null for the random three-map choice. */
  static start(map: string | null): Game {
    const seed = Math.floor(Math.random() * 2 ** 31);
    const now = Math.floor(Date.now() / 1000);
    const e = map ? WasmEngine.new_map(seed, now, 'web', map) : new WasmEngine(seed, now, 'web');
    const g = new Game(e, 0, Game.content, Game.balanceJson);
    Game.bootError = null;
    g.persist();
    return g;
  }

  /** Drop this game and its save (back to the start screen). */
  discard(): void {
    this.engine.free();
    try {
      localStorage.removeItem(SAVE_KEY);
      localStorage.removeItem(OFFSET_KEY);
    } catch {
      // nothing saved
    }
  }

  now(): number {
    return Math.floor(Date.now() / 1000) + this.offset;
  }

  /** Catch the simulation up to now. Returns the number of new events. */
  tick(): number {
    return this.engine.advance(this.now());
  }

  view(): View {
    const v = JSON.parse(this.engine.view()) as View;
    // Locked blocks are absent; the forecast reads as empty.
    v.tide ??= [];
    return v;
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

  /** Debug: jump the clock forward. Offline catch-up runs the same discrete-event path. */
  skip(seconds: number): number {
    this.offset += seconds;
    const n = this.tick();
    this.persist();
    return n;
  }
}
