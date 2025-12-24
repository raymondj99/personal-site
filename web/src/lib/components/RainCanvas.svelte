<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { browser } from "$app/environment";
    import { createWorld, getMemory, type RainWorld } from "$lib/wasm/dropletEngine";

    let canvas: HTMLCanvasElement;
    let ctx: CanvasRenderingContext2D;
    let world: RainWorld;
    let animationId: number;
    let charWidth = 8;
    let charHeight = 16;

    // Rain characters by trail position
    const CHARS = ['|', ':', '.', '.'];

    // Colors by layer and trail position [layer][trail_pos]
    // Far layer: very subtle
    // Mid layer: moderate
    // Near layer: bright
    const COLORS = [
        // Far (layer 0)
        ['rgba(100, 120, 140, 0.25)', 'rgba(90, 110, 130, 0.20)', 'rgba(80, 100, 120, 0.15)', 'rgba(70, 90, 110, 0.10)'],
        // Mid (layer 1)
        ['rgba(140, 160, 180, 0.50)', 'rgba(130, 150, 170, 0.40)', 'rgba(120, 140, 160, 0.30)', 'rgba(110, 130, 150, 0.20)'],
        // Near (layer 2)
        ['rgba(180, 195, 210, 0.85)', 'rgba(170, 185, 200, 0.70)', 'rgba(160, 175, 190, 0.50)', 'rgba(150, 165, 180, 0.35)'],
    ];

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
        const width = window.innerWidth;
        const height = window.innerHeight;

        canvas.width = width * dpr;
        canvas.height = height * dpr;
        canvas.style.width = `${width}px`;
        canvas.style.height = `${height}px`;

        // Reset transform and apply DPR scale
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        ctx.font = '14px "JetBrains Mono", "Fira Code", "Consolas", monospace';
        ctx.textBaseline = 'top';

        // Re-measure char dimensions after setting font
        const metrics = ctx.measureText('|');
        charWidth = Math.max(8, Math.ceil(metrics.width));
        charHeight = 16;

        const cols = Math.floor(width / charWidth);
        const rows = Math.floor(height / charHeight);

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

        // Zero-copy access to WASM memory
        const output = new Uint8Array(memory.buffer, ptr, len);

        // Clear with dark background
        ctx.fillStyle = '#070709';
        ctx.fillRect(0, 0, window.innerWidth, window.innerHeight);

        // Batch render by color to minimize state changes
        for (let layer = 0; layer < 3; layer++) {
            for (let trailPos = 0; trailPos < 4; trailPos++) {
                const targetValue = layer * 4 + trailPos + 1;
                ctx.fillStyle = COLORS[layer][trailPos];

                for (let y = 0; y < h; y++) {
                    for (let x = 0; x < w; x++) {
                        const value = output[y * w + x];
                        if (value === targetValue) {
                            ctx.fillText(
                                CHARS[trailPos],
                                x * charWidth,
                                y * charHeight
                            );
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
