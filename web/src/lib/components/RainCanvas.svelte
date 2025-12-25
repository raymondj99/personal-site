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

    // Encoding (must match Rust)
    const BUCKETS = 8;
    const TRAILS = 4;
    const SPLASH_OFF = 33;
    const SPLASH_CHARS = 8;

    const DROP_CHARS = ['|', ':', '.', '.'];
    const SPLASH_GLYPHS = ['.', '|', "'", '*', '\\', '/', '.', '.'];

    // Colors by depth bucket (0=far/dim, 7=near/bright)
    const DROP_COLORS = makeDropColors();
    const SPLASH_COLORS = makeSplashColors();

    function makeDropColors(): string[][] {
        const out: string[][] = [];
        for (let i = 0; i < BUCKETS; i++) {
            const t = i / (BUCKETS - 1);
            const base = 90 + t * 120;
            const alpha = 0.15 + t * 0.75;
            out.push([
                `rgba(${base}, ${base + 15}, ${base + 30}, ${alpha})`,
                `rgba(${base - 10}, ${base + 5}, ${base + 20}, ${alpha * 0.8})`,
                `rgba(${base - 20}, ${base - 5}, ${base + 10}, ${alpha * 0.6})`,
                `rgba(${base - 30}, ${base - 15}, ${base}, ${alpha * 0.4})`,
            ]);
        }
        return out;
    }

    function makeSplashColors(): string[] {
        const out: string[] = [];
        for (let i = 0; i < BUCKETS; i++) {
            const t = i / (BUCKETS - 1);
            const base = 100 + t * 110;
            const alpha = 0.2 + t * 0.7;
            out.push(`rgba(${base}, ${base + 15}, ${base + 30}, ${alpha})`);
        }
        return out;
    }

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

        const buf = new Uint8Array(memory.buffer, ptr, len);

        ctx.fillStyle = '#070709';
        ctx.fillRect(0, 0, window.innerWidth, window.innerHeight);

        // Render drops
        for (let bucket = 0; bucket < BUCKETS; bucket++) {
            for (let trail = 0; trail < TRAILS; trail++) {
                const target = bucket * TRAILS + trail + 1;
                ctx.fillStyle = DROP_COLORS[bucket][trail];

                for (let y = 0; y < h; y++) {
                    const row = y * w;
                    for (let x = 0; x < w; x++) {
                        if (buf[row + x] === target) {
                            ctx.fillText(DROP_CHARS[trail], x * CHAR_W, y * CHAR_H);
                        }
                    }
                }
            }
        }

        // Render splashes
        for (let bucket = 0; bucket < BUCKETS; bucket++) {
            ctx.fillStyle = SPLASH_COLORS[bucket];
            for (let c = 0; c < SPLASH_CHARS; c++) {
                const target = SPLASH_OFF + bucket * SPLASH_CHARS + c;

                for (let y = 0; y < h; y++) {
                    const row = y * w;
                    for (let x = 0; x < w; x++) {
                        if (buf[row + x] === target) {
                            ctx.fillText(SPLASH_GLYPHS[c], x * CHAR_W, y * CHAR_H);
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
