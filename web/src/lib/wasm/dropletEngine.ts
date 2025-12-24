import init, { DropletWorld } from "droplet-engine";
import type { InitOutput } from "droplet-engine";

let initialized: Promise<InitOutput> | null = null;

/**
 * Initialize the WASM module
 */
export async function initDropletEngine() {
    if (!initialized) {
        initialized = init();
    }
    return initialized;
}

/**
 * Create a new droplet world instance
 */
export async function createWorld(width: number, height: number): Promise<DropletWorld> {
    await initDropletEngine();
    return new DropletWorld(width, height);
}

export type { DropletWorld };