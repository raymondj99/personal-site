<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { browser } from "$app/environment";
    import { createWorld, getMemory, type RainWorld } from "$lib/wasm/dropletEngine";
    import { BG_WIDTH, BG_HEIGHT, BG_PALETTE, BG_PIXELS, BG_DEPTH } from "$lib/background";

    let canvas: HTMLCanvasElement;
    let ctx: CanvasRenderingContext2D;
    let world: RainWorld;
    let animationId: number;

    // Pre-rendered canvases
    let bgCanvas: HTMLCanvasElement | null = null;
    let depthCanvas: HTMLCanvasElement | null = null;

    // Visualization controls
    let showDepth = false;
    let depthOpacity = 0.5;

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

        preRenderBackground();
        preRenderDepth();
        await resize();
        window.addEventListener('resize', resize);
        loop();
    });

    function preRenderBackground() {
        bgCanvas = document.createElement('canvas');
        bgCanvas.width = BG_WIDTH;
        bgCanvas.height = BG_HEIGHT;
        const bgCtx = bgCanvas.getContext('2d')!;

        for (let y = 0; y < BG_HEIGHT; y++) {
            for (let x = 0; x < BG_WIDTH; x++) {
                const colorIdx = BG_PIXELS[y][x];
                bgCtx.fillStyle = BG_PALETTE[colorIdx];
                bgCtx.fillRect(x, y, 1, 1);
            }
        }
    }

    function preRenderDepth() {
        depthCanvas = document.createElement('canvas');
        depthCanvas.width = BG_WIDTH;
        depthCanvas.height = BG_HEIGHT;
        const depthCtx = depthCanvas.getContext('2d')!;

        const imageData = depthCtx.createImageData(BG_WIDTH, BG_HEIGHT);
        const data = imageData.data;

        for (let y = 0; y < BG_HEIGHT; y++) {
            for (let x = 0; x < BG_WIDTH; x++) {
                const depth = BG_DEPTH[y][x];
                const idx = (y * BG_WIDTH + x) * 4;
                const t = depth / 255;

                let r: number, g: number, b: number;
                if (t < 0.25) {
                    const s = t / 0.25;
                    r = 0; g = Math.floor(s * 255); b = 255;
                } else if (t < 0.5) {
                    const s = (t - 0.25) / 0.25;
                    r = 0; g = 255; b = Math.floor((1 - s) * 255);
                } else if (t < 0.75) {
                    const s = (t - 0.5) / 0.25;
                    r = Math.floor(s * 255); g = 255; b = 0;
                } else {
                    const s = (t - 0.75) / 0.25;
                    r = 255; g = Math.floor((1 - s) * 255); b = 0;
                }

                data[idx] = r;
                data[idx + 1] = g;
                data[idx + 2] = b;
                data[idx + 3] = 255;
            }
        }

        depthCtx.putImageData(imageData, 0, 0);
    }

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

        ctx.imageSmoothingEnabled = false;

        // Draw background
        if (bgCanvas) {
            ctx.drawImage(bgCanvas, 0, 0, window.innerWidth, window.innerHeight);
        } else {
            ctx.fillStyle = '#0a0a12';
            ctx.fillRect(0, 0, window.innerWidth, window.innerHeight);
        }

        // Optionally overlay depth map
        if (showDepth && depthCanvas) {
            ctx.globalAlpha = depthOpacity;
            ctx.drawImage(depthCanvas, 0, 0, window.innerWidth, window.innerHeight);
            ctx.globalAlpha = 1.0;
        }

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

        // Draw legend if depth is shown
        if (showDepth) {
            drawDepthLegend();
        }
    }

    function drawDepthLegend() {
        const legendX = 20;
        const legendY = 20;
        const legendW = 200;
        const legendH = 20;

        ctx.fillStyle = 'rgba(0, 0, 0, 0.7)';
        ctx.fillRect(legendX - 10, legendY - 30, legendW + 20, legendH + 50);

        ctx.fillStyle = '#fff';
        ctx.font = '14px monospace';
        ctx.fillText('MiDaS Depth Map', legendX, legendY - 10);

        const gradient = ctx.createLinearGradient(legendX, 0, legendX + legendW, 0);
        gradient.addColorStop(0, 'rgb(0, 0, 255)');
        gradient.addColorStop(0.25, 'rgb(0, 255, 255)');
        gradient.addColorStop(0.5, 'rgb(0, 255, 0)');
        gradient.addColorStop(0.75, 'rgb(255, 255, 0)');
        gradient.addColorStop(1, 'rgb(255, 0, 0)');

        ctx.fillStyle = gradient;
        ctx.fillRect(legendX, legendY, legendW, legendH);

        ctx.fillStyle = '#fff';
        ctx.font = '12px monospace';
        ctx.fillText('Far', legendX, legendY + legendH + 15);
        ctx.fillText('Close', legendX + legendW - 30, legendY + legendH + 15);
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'd' || e.key === 'D') {
            showDepth = !showDepth;
        } else if (e.key === 'ArrowUp' && showDepth) {
            depthOpacity = Math.min(1, depthOpacity + 0.1);
        } else if (e.key === 'ArrowDown' && showDepth) {
            depthOpacity = Math.max(0, depthOpacity - 0.1);
        }
    }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if browser}
    <canvas bind:this={canvas}></canvas>
    <div class="controls">
        <p>Press <kbd>D</kbd> to toggle depth overlay</p>
        {#if showDepth}
            <p>Press <kbd>Up/Down</kbd> to adjust opacity</p>
        {/if}
    </div>
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

    .controls {
        position: fixed;
        bottom: 20px;
        right: 20px;
        background: rgba(0, 0, 0, 0.7);
        padding: 10px 15px;
        border-radius: 8px;
        color: #fff;
        font-family: monospace;
        font-size: 12px;
        z-index: 10;
    }

    .controls p {
        margin: 5px 0;
    }

    kbd {
        background: rgba(255, 255, 255, 0.2);
        padding: 2px 6px;
        border-radius: 3px;
        font-family: inherit;
    }
</style>
