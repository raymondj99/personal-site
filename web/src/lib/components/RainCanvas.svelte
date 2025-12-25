<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { browser } from "$app/environment";
    import { createWorld, getMemory, type RainWorld } from "$lib/wasm/dropletEngine";

    let canvas: HTMLCanvasElement;
    let ctx: CanvasRenderingContext2D;
    let world: RainWorld;
    let animationId: number;

    const CHAR_W = 8;
    const CHAR_H = 16;

    // Encoding constants (must match Rust)
    // Droplets: 1-32 = bucket * 4 + trail_pos + 1
    // Splashes: 33+ = 33 + bucket * 8 + char_type
    const DEPTH_BUCKETS = 8;
    const TRAIL_POSITIONS = 4;
    const SPLASH_OFFSET = 33;
    const SPLASH_CHAR_TYPES = 8;

    // Droplet characters by trail position
    const DROP_CHARS = ['|', ':', '.', '.'];

    // Splash characters by type
    const SPLASH_CHARS = ['.', '|', "'", '*', '\\', '/', '.', '.'];

    // Generate colors for 8 depth buckets
    // Bucket 0 = far (dim), Bucket 7 = near (bright)
    function makeDropColors(): string[][] {
        const colors: string[][] = [];
        for (let bucket = 0; bucket < DEPTH_BUCKETS; bucket++) {
            const t = bucket / (DEPTH_BUCKETS - 1);  // 0 = far, 1 = near
            const base = 90 + t * 120;  // 90-210
            const alpha = 0.15 + t * 0.75;  // 0.15-0.9
            colors.push([
                `rgba(${base}, ${base + 15}, ${base + 30}, ${alpha})`,
                `rgba(${base - 10}, ${base + 5}, ${base + 20}, ${alpha * 0.8})`,
                `rgba(${base - 20}, ${base - 5}, ${base + 10}, ${alpha * 0.6})`,
                `rgba(${base - 30}, ${base - 15}, ${base}, ${alpha * 0.4})`,
            ]);
        }
        return colors;
    }

    function makeSplashColors(): string[] {
        const colors: string[] = [];
        for (let bucket = 0; bucket < DEPTH_BUCKETS; bucket++) {
            const t = bucket / (DEPTH_BUCKETS - 1);
            const base = 100 + t * 110;
            const alpha = 0.2 + t * 0.7;
            colors.push(`rgba(${base}, ${base + 15}, ${base + 30}, ${alpha})`);
        }
        return colors;
    }

    const DROP_COLORS = makeDropColors();
    const SPLASH_COLORS = makeSplashColors();

    onMount(async () => {
        if (!canvas) return;
        ctx = canvas.getContext('2d', { alpha: false })!;
        if (!ctx) return;
        await resize();
        window.addEventListener('resize', resize);
        loop();
    });

    onDestroy(() => {
        if (!browser) return;
        if (animationId) cancelAnimationFrame(animationId);
        window.removeEventListener('resize', resize);
    });

    async function resize() {
        const dpr = window.devicePixelRatio || 1;
        const w = window.innerWidth;
        const h = window.innerHeight;

        canvas.width = w * dpr;
        canvas.height = h * dpr;
        canvas.style.width = `${w}px`;
        canvas.style.height = `${h}px`;

        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        ctx.font = '14px "JetBrains Mono", "Fira Code", monospace';
        ctx.textBaseline = 'top';

        const cols = Math.floor(w / CHAR_W);
        const rows = Math.floor(h / CHAR_H);

        if (world) {
            world.resize(cols, rows);
        } else {
            world = await createWorld(cols, rows);
        }
    }

    function loop() {
        world.tick();
        render();
        animationId = requestAnimationFrame(loop);
    }

    function render() {
        const w = world.width();
        const h = world.height();
        const ptr = world.output_ptr();
        const len = world.output_len();

        const memory = getMemory();
        if (!memory) return;

        const output = new Uint8Array(memory.buffer, ptr, len);

        ctx.fillStyle = '#070709';
        ctx.fillRect(0, 0, window.innerWidth, window.innerHeight);

        // Batch by encoded value for fewer style switches
        // Droplets: 1-32 = bucket * 4 + trail_pos + 1
        for (let bucket = 0; bucket < DEPTH_BUCKETS; bucket++) {
            for (let trail = 0; trail < TRAIL_POSITIONS; trail++) {
                const target = bucket * TRAIL_POSITIONS + trail + 1;
                ctx.fillStyle = DROP_COLORS[bucket][trail];

                for (let y = 0; y < h; y++) {
                    const row = y * w;
                    for (let x = 0; x < w; x++) {
                        if (output[row + x] === target) {
                            ctx.fillText(DROP_CHARS[trail], x * CHAR_W, y * CHAR_H);
                        }
                    }
                }
            }
        }

        // Splashes: 33+ = SPLASH_OFFSET + bucket * SPLASH_CHAR_TYPES + char_type
        for (let bucket = 0; bucket < DEPTH_BUCKETS; bucket++) {
            ctx.fillStyle = SPLASH_COLORS[bucket];
            for (let charType = 0; charType < SPLASH_CHAR_TYPES; charType++) {
                const target = SPLASH_OFFSET + bucket * SPLASH_CHAR_TYPES + charType;

                for (let y = 0; y < h; y++) {
                    const row = y * w;
                    for (let x = 0; x < w; x++) {
                        if (output[row + x] === target) {
                            ctx.fillText(SPLASH_CHARS[charType], x * CHAR_W, y * CHAR_H);
                        }
                    }
                }
            }
        }
    }
</script>

{#if browser}
    <canvas bind:this={canvas}></canvas>
{/if}

<style>
    canvas {
        position: fixed;
        top: 0;
        left: 0;
        width: 100vw;
        height: 100vh;
        z-index: 0;
    }
</style>
