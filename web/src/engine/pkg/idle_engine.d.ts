/* tslint:disable */
/* eslint-disable */

export class WasmEngine {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Catch up to `now`; returns how many new events happened.
     */
    advance(now: number): number;
    static balance(): string;
    command(json: string): string;
    static content(): string;
    dry_run(json: string): string;
    garrison_options(tile: number): string;
    static load(save: string): WasmEngine;
    constructor(seed: number, now: number, device: string);
    /**
     * Parse-check a command without running it (lets the client validate saved macros).
     */
    static parse_command(json: string): boolean;
    preview_route(squad: number, route_json: string): string;
    /**
     * `op`: -1 = current setup, 0 = nobody, otherwise an operative id.
     */
    preview_surge(tile: number, op: number): string;
    report(id: number): string;
    revision(): number;
    save(): string;
    time(): number;
    trip_battle(squad: number, tile: number): string;
    view(): string;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_wasmengine_free: (a: number, b: number) => void;
    readonly wasmengine_advance: (a: number, b: number) => number;
    readonly wasmengine_balance: () => [number, number];
    readonly wasmengine_command: (a: number, b: number, c: number) => [number, number];
    readonly wasmengine_content: () => [number, number];
    readonly wasmengine_dry_run: (a: number, b: number, c: number) => [number, number];
    readonly wasmengine_garrison_options: (a: number, b: number) => [number, number];
    readonly wasmengine_load: (a: number, b: number) => [number, number, number];
    readonly wasmengine_new: (a: number, b: number, c: number, d: number) => number;
    readonly wasmengine_parse_command: (a: number, b: number) => number;
    readonly wasmengine_preview_route: (a: number, b: number, c: number, d: number) => [number, number];
    readonly wasmengine_preview_surge: (a: number, b: number, c: number) => [number, number];
    readonly wasmengine_report: (a: number, b: number) => [number, number];
    readonly wasmengine_revision: (a: number) => number;
    readonly wasmengine_save: (a: number) => [number, number];
    readonly wasmengine_time: (a: number) => number;
    readonly wasmengine_trip_battle: (a: number, b: number, c: number) => [number, number];
    readonly wasmengine_view: (a: number) => [number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
