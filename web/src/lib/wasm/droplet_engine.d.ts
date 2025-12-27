/* tslint:disable */
/* eslint-disable */

export class RainWorld {
  free(): void;
  [Symbol.dispose](): void;
  constructor(w: number, h: number);
  resize(w: number, h: number): void;
  tick(): void;
  output_ptr(): number;
  output_len(): number;
  width(): number;
  height(): number;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_rainworld_free: (a: number, b: number) => void;
  readonly rainworld_new: (a: number, b: number) => number;
  readonly rainworld_resize: (a: number, b: number, c: number) => void;
  readonly rainworld_tick: (a: number) => void;
  readonly rainworld_output_ptr: (a: number) => number;
  readonly rainworld_output_len: (a: number) => number;
  readonly rainworld_width: (a: number) => number;
  readonly rainworld_height: (a: number) => number;
  readonly __wbindgen_externrefs: WebAssembly.Table;
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
