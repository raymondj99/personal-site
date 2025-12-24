<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import {createWorld} from "$lib/wasm/dropletEngine";

    export let width = 80;
    export let height = 30;
    export let fps = 24;
    export let spawnRate = 0.3;
    export let gravity = 0.8;

    let frame = "";
    let world: any;
    let animationHandle: number;
    let isRunning = false;
    let dropletCount = 0;

    onMount(async () => {
        try {
            world = await createWorld(width, height);
            world.set_spawn_rate(spawnRate);
            world.set_gravity(gravity);

            // Initialize with empty frame
            frame = world.frame_string();
            startAnimation();
        } catch(err) {
            console.error("Failed to initialize WASM: ", err);
        }
    });

    onDestroy(() => {
        stopAnimation();
    })

    function startAnimation() {
        if (isRunning || !world) return;
        isRunning = true;

        console.log("Fps:", fps);
        const actualFps = fps > 0 ? fps : 24;
        const frameDuration = 1000 / actualFps;
        let lastFrame = performance.now();

        const loop = (now: number) => {
            if (now - lastFrame >= frameDuration) {
                world.step();
                frame = world.frame_string();
                dropletCount = world.droplet_count();
                lastFrame = now;
            }
            animationHandle = requestAnimationFrame(loop);
        }
        animationHandle = requestAnimationFrame(loop);
    }

    function stopAnimation() {
        isRunning = false;
        if (animationHandle) {
            cancelAnimationFrame(animationHandle);
        }
    }

    $: if (world) {
        world.set_spawn_rate(spawnRate);
        world.set_gravity(gravity);
    }

    export function pause() {
        stopAnimation();
    }

    export function play() {
        startAnimation();
    }

    export function clear() {
        if (world) {
            world.clear();
            frame = world.frame_string();
        }
    }
</script>

<div class="ascii-container">
    <pre class="ascii-canvas">{frame}</pre>

    {#if dropletCount > 0}
        <div class="debug-info">
            Droplets: {dropletCount} | FPS: {fps}
        </div>
    {/if}
</div>

<style>
    .ascii-container {
        position: relative;
        display: inline-block;
    }

    .ascii-canvas {
        font-family: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace;
        font-size: 14px;
        line-height: 1;
        margin: 0;
        padding: 1rem;
        background: #0a0a0a;
        color: #00ff41;
        white-space: pre;
        overflow: hidden;
        border-radius: 4px;
        text-shadow: 0 0 5px rgba(0, 255, 65, 0.5);
    }

    .debug-info {
        position: absolute;
        top: 0.5rem;
        right: 0.5rem;
        font-family: monospace;
        font-size: 10px;
        color: rgba(0, 255, 65, 0.5);
        pointer-events: none;
    }
</style>