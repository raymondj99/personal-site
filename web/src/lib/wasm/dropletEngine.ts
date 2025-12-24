type RainWorldInstance = {
    tick(): void;
    output_ptr(): number;
    output_len(): number;
    width(): number;
    height(): number;
    resize(width: number, height: number): void;
    clear(): void;
    droplet_count(): number;
    free(): void;
};

let wasmMemory: WebAssembly.Memory | null = null;
let RainWorldClass: (new (width: number, height: number) => RainWorldInstance) | null = null;

export async function initDropletEngine(): Promise<void> {
    if (RainWorldClass) return;

    // Dynamic import to avoid SSR issues
    const module = await import("droplet-engine");
    const wasm = await module.default();

    wasmMemory = wasm.memory;
    RainWorldClass = module.RainWorld as any;
}

export async function createWorld(width: number, height: number): Promise<RainWorldInstance> {
    await initDropletEngine();
    return new RainWorldClass!(width, height);
}

export function getMemory(): WebAssembly.Memory | null {
    return wasmMemory;
}

export type RainWorld = RainWorldInstance;
