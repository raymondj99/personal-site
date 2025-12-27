<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { browser } from "$app/environment";
    import { createWorld, getMemory, type RainWorld } from "$lib/wasm/dropletEngine";
    import { BG_WIDTH, BG_HEIGHT, BG_PALETTE, BG_PIXELS, BG_DEPTH, BG_FLOW_X, BG_FLOW_Y, BG_SEGMENTS, BG_GROUND } from "$lib/background";

    let canvas: HTMLCanvasElement;
    let ctx: CanvasRenderingContext2D;
    let world: RainWorld;
    let animationId: number;

    // Pre-rendered canvases
    let bgCanvas: HTMLCanvasElement | null = null;
    let depthCanvas: HTMLCanvasElement | null = null;
    let flowCanvas: HTMLCanvasElement | null = null;
    let segmentCanvas: HTMLCanvasElement | null = null;
    let groundCanvas: HTMLCanvasElement | null = null;

    // Visualization controls
    let showDepth = false;
    let showFlow = false;
    let showSegments = false;
    let showGround = false;
    let overlayOpacity = 0.6;

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

        computeDetectedClasses();
        preRenderBackground();
        preRenderDepth();
        preRenderFlow();
        preRenderSegments();
        preRenderGround();
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

    function preRenderFlow() {
        // Flow arrows are rendered directly at screen resolution in renderFlowArrows()
        // This function is kept for compatibility but flowCanvas is not used
        flowCanvas = null;
    }

    function renderFlowArrows() {
        const screenW = window.innerWidth;
        const screenH = window.innerHeight;

        // Scale factors from background to screen
        const scaleX = screenW / BG_WIDTH;
        const scaleY = screenH / BG_HEIGHT;

        // Grid spacing in background pixels (samples every N background pixels)
        const bgSpacing = 8;
        const arrowLength = 20;
        const arrowHeadSize = 6;

        ctx.strokeStyle = 'rgba(100, 200, 255, 0.85)';
        ctx.fillStyle = 'rgba(100, 200, 255, 0.85)';
        ctx.lineWidth = 2;
        ctx.lineCap = 'round';

        for (let bgY = bgSpacing / 2; bgY < BG_HEIGHT; bgY += bgSpacing) {
            for (let bgX = bgSpacing / 2; bgX < BG_WIDTH; bgX += bgSpacing) {
                const iy = Math.floor(bgY);
                const ix = Math.floor(bgX);

                const fx = BG_FLOW_X[iy][ix] / 127.0;
                const fy = BG_FLOW_Y[iy][ix] / 127.0;
                const mag = Math.sqrt(fx * fx + fy * fy);

                // Skip if no significant flow
                if (mag < 0.1) continue;

                // Normalize direction
                const nx = fx / mag;
                const ny = fy / mag;

                // Arrow length proportional to magnitude
                const len = arrowLength * Math.min(mag * 1.5, 1);

                // Convert to screen coordinates
                const screenX = bgX * scaleX;
                const screenY = bgY * scaleY;

                // Arrow end point
                const endX = screenX + nx * len;
                const endY = screenY + ny * len;

                // Draw arrow line
                ctx.beginPath();
                ctx.moveTo(screenX, screenY);
                ctx.lineTo(endX, endY);
                ctx.stroke();

                // Draw arrowhead
                const angle = Math.atan2(ny, nx);
                const headAngle = Math.PI / 5; // 36 degrees

                ctx.beginPath();
                ctx.moveTo(endX, endY);
                ctx.lineTo(
                    endX - arrowHeadSize * Math.cos(angle - headAngle),
                    endY - arrowHeadSize * Math.sin(angle - headAngle)
                );
                ctx.lineTo(
                    endX - arrowHeadSize * Math.cos(angle + headAngle),
                    endY - arrowHeadSize * Math.sin(angle + headAngle)
                );
                ctx.closePath();
                ctx.fill();
            }
        }
    }

    function hslToRgb(h: number, s: number, l: number): [number, number, number] {
        let r: number, g: number, b: number;

        if (s === 0) {
            r = g = b = l;
        } else {
            const hue2rgb = (p: number, q: number, t: number) => {
                if (t < 0) t += 1;
                if (t > 1) t -= 1;
                if (t < 1/6) return p + (q - p) * 6 * t;
                if (t < 1/2) return q;
                if (t < 2/3) return p + (q - p) * (2/3 - t) * 6;
                return p;
            };

            const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
            const p = 2 * l - q;
            r = hue2rgb(p, q, h + 1/3);
            g = hue2rgb(p, q, h);
            b = hue2rgb(p, q, h - 1/3);
        }

        return [Math.round(r * 255), Math.round(g * 255), Math.round(b * 255)];
    }

    // ADE20K semantic class colors and names (150 classes)
    const ADE20K_CLASSES: Record<number, { name: string; color: [number, number, number] }> = {
        0: { name: 'wall', color: [120, 120, 120] },
        1: { name: 'building', color: [180, 120, 120] },
        2: { name: 'sky', color: [135, 206, 235] },
        3: { name: 'floor', color: [200, 180, 160] },
        4: { name: 'tree', color: [34, 139, 34] },
        5: { name: 'ceiling', color: [240, 240, 240] },
        6: { name: 'road', color: [100, 100, 100] },
        7: { name: 'bed', color: [200, 162, 200] },
        8: { name: 'windowpane', color: [140, 140, 180] },
        9: { name: 'grass', color: [124, 252, 0] },
        10: { name: 'cabinet', color: [139, 69, 19] },
        11: { name: 'sidewalk', color: [128, 128, 128] },
        12: { name: 'person', color: [255, 192, 203] },
        13: { name: 'earth', color: [139, 90, 43] },
        14: { name: 'door', color: [160, 82, 45] },
        15: { name: 'table', color: [205, 133, 63] },
        16: { name: 'mountain', color: [105, 105, 105] },
        17: { name: 'plant', color: [50, 205, 50] },
        18: { name: 'curtain', color: [255, 182, 193] },
        19: { name: 'chair', color: [165, 42, 42] },
        20: { name: 'car', color: [255, 0, 0] },
        21: { name: 'water', color: [65, 105, 225] },
        22: { name: 'painting', color: [255, 215, 0] },
        23: { name: 'sofa', color: [178, 34, 34] },
        24: { name: 'shelf', color: [210, 180, 140] },
        25: { name: 'house', color: [188, 143, 143] },
        26: { name: 'sea', color: [0, 119, 190] },
        27: { name: 'mirror', color: [192, 192, 192] },
        28: { name: 'rug', color: [128, 0, 128] },
        29: { name: 'field', color: [218, 165, 32] },
        30: { name: 'armchair', color: [139, 69, 19] },
        31: { name: 'seat', color: [160, 82, 45] },
        32: { name: 'fence', color: [139, 119, 101] },
        33: { name: 'desk', color: [205, 133, 63] },
        34: { name: 'rock', color: [128, 128, 128] },
        35: { name: 'wardrobe', color: [139, 90, 43] },
        36: { name: 'lamp', color: [255, 255, 0] },
        37: { name: 'bathtub', color: [173, 216, 230] },
        38: { name: 'railing', color: [105, 105, 105] },
        39: { name: 'cushion', color: [255, 182, 193] },
        40: { name: 'base', color: [128, 128, 128] },
        41: { name: 'box', color: [210, 180, 140] },
        42: { name: 'column', color: [192, 192, 192] },
        43: { name: 'signboard', color: [255, 165, 0] },
        44: { name: 'chest of drawers', color: [139, 69, 19] },
        45: { name: 'counter', color: [210, 180, 140] },
        46: { name: 'sand', color: [244, 164, 96] },
        47: { name: 'sink', color: [173, 216, 230] },
        48: { name: 'skyscraper', color: [70, 130, 180] },
        49: { name: 'fireplace', color: [178, 34, 34] },
        50: { name: 'refrigerator', color: [192, 192, 192] },
        51: { name: 'grandstand', color: [128, 128, 128] },
        52: { name: 'path', color: [160, 140, 100] },
        53: { name: 'stairs', color: [128, 128, 128] },
        54: { name: 'runway', color: [169, 169, 169] },
        55: { name: 'case', color: [139, 69, 19] },
        56: { name: 'pool table', color: [0, 100, 0] },
        57: { name: 'pillow', color: [255, 255, 255] },
        58: { name: 'screen door', color: [139, 119, 101] },
        59: { name: 'stairway', color: [128, 128, 128] },
        60: { name: 'river', color: [70, 130, 180] },
        61: { name: 'bridge', color: [128, 128, 128] },
        62: { name: 'bookcase', color: [139, 69, 19] },
        63: { name: 'blind', color: [245, 245, 220] },
        64: { name: 'coffee table', color: [139, 69, 19] },
        65: { name: 'toilet', color: [255, 255, 255] },
        66: { name: 'flower', color: [255, 105, 180] },
        67: { name: 'book', color: [139, 69, 19] },
        68: { name: 'hill', color: [85, 107, 47] },
        69: { name: 'bench', color: [139, 90, 43] },
        70: { name: 'countertop', color: [210, 180, 140] },
        71: { name: 'stove', color: [128, 128, 128] },
        72: { name: 'palm', color: [0, 100, 0] },
        73: { name: 'kitchen island', color: [210, 180, 140] },
        74: { name: 'computer', color: [128, 128, 128] },
        75: { name: 'swivel chair', color: [0, 0, 0] },
        76: { name: 'boat', color: [139, 69, 19] },
        77: { name: 'bar', color: [139, 69, 19] },
        78: { name: 'arcade machine', color: [128, 0, 128] },
        79: { name: 'hovel', color: [139, 90, 43] },
        80: { name: 'bus', color: [255, 215, 0] },
        81: { name: 'towel', color: [255, 255, 255] },
        82: { name: 'light', color: [255, 255, 0] },
        83: { name: 'truck', color: [255, 0, 0] },
        84: { name: 'tower', color: [128, 128, 128] },
        85: { name: 'chandelier', color: [255, 215, 0] },
        86: { name: 'awning', color: [255, 165, 0] },
        87: { name: 'streetlight', color: [255, 255, 0] },
        88: { name: 'booth', color: [139, 69, 19] },
        89: { name: 'television', color: [0, 0, 0] },
        90: { name: 'airplane', color: [192, 192, 192] },
        91: { name: 'dirt track', color: [139, 90, 43] },
        92: { name: 'apparel', color: [255, 105, 180] },
        93: { name: 'pole', color: [128, 128, 128] },
        94: { name: 'land', color: [160, 120, 60] },
        95: { name: 'bannister', color: [139, 69, 19] },
        96: { name: 'escalator', color: [192, 192, 192] },
        97: { name: 'ottoman', color: [139, 69, 19] },
        98: { name: 'bottle', color: [0, 128, 0] },
        99: { name: 'buffet', color: [139, 69, 19] },
        100: { name: 'poster', color: [255, 255, 255] },
        101: { name: 'stage', color: [139, 69, 19] },
        102: { name: 'van', color: [255, 255, 255] },
        103: { name: 'ship', color: [128, 128, 128] },
        104: { name: 'fountain', color: [65, 105, 225] },
        105: { name: 'conveyer belt', color: [128, 128, 128] },
        106: { name: 'canopy', color: [0, 128, 0] },
        107: { name: 'washer', color: [255, 255, 255] },
        108: { name: 'plaything', color: [255, 105, 180] },
        109: { name: 'swimming pool', color: [0, 191, 255] },
        110: { name: 'stool', color: [139, 69, 19] },
        111: { name: 'barrel', color: [139, 69, 19] },
        112: { name: 'basket', color: [210, 180, 140] },
        113: { name: 'waterfall', color: [100, 149, 237] },
        114: { name: 'tent', color: [255, 255, 255] },
        115: { name: 'bag', color: [139, 69, 19] },
        116: { name: 'minibike', color: [255, 0, 0] },
        117: { name: 'cradle', color: [255, 255, 255] },
        118: { name: 'oven', color: [128, 128, 128] },
        119: { name: 'ball', color: [255, 165, 0] },
        120: { name: 'food', color: [255, 165, 0] },
        121: { name: 'step', color: [128, 128, 128] },
        122: { name: 'tank', color: [128, 128, 128] },
        123: { name: 'trade name', color: [255, 0, 0] },
        124: { name: 'microwave', color: [128, 128, 128] },
        125: { name: 'pot', color: [139, 69, 19] },
        126: { name: 'animal', color: [139, 90, 43] },
        127: { name: 'bicycle', color: [255, 0, 0] },
        128: { name: 'lake', color: [30, 144, 255] },
        129: { name: 'dishwasher', color: [192, 192, 192] },
        130: { name: 'screen', color: [0, 0, 0] },
        131: { name: 'blanket', color: [255, 255, 255] },
        132: { name: 'sculpture', color: [192, 192, 192] },
        133: { name: 'hood', color: [128, 128, 128] },
        134: { name: 'sconce', color: [255, 215, 0] },
        135: { name: 'vase', color: [192, 192, 192] },
        136: { name: 'traffic light', color: [255, 255, 0] },
        137: { name: 'tray', color: [192, 192, 192] },
        138: { name: 'ashcan', color: [128, 128, 128] },
        139: { name: 'fan', color: [255, 255, 255] },
        140: { name: 'pier', color: [139, 69, 19] },
        141: { name: 'crt screen', color: [0, 0, 0] },
        142: { name: 'plate', color: [255, 255, 255] },
        143: { name: 'monitor', color: [0, 0, 0] },
        144: { name: 'bulletin board', color: [210, 180, 140] },
        145: { name: 'shower', color: [173, 216, 230] },
        146: { name: 'radiator', color: [255, 255, 255] },
        147: { name: 'glass', color: [173, 216, 230] },
        148: { name: 'clock', color: [255, 255, 255] },
        149: { name: 'flag', color: [255, 0, 0] },
    };

    // Compute detected classes once at startup
    let detectedClasses: number[] = [];
    function computeDetectedClasses() {
        const classSet = new Set<number>();
        for (let y = 0; y < BG_HEIGHT; y++) {
            for (let x = 0; x < BG_WIDTH; x++) {
                classSet.add(BG_SEGMENTS[y][x]);
            }
        }
        detectedClasses = Array.from(classSet).sort((a, b) => a - b);
    }

    function getSegmentColor(classId: number): [number, number, number] {
        if (classId in ADE20K_CLASSES) {
            return ADE20K_CLASSES[classId].color;
        }
        // Generate a pseudo-random color for unmapped classes
        const hue = (classId * 137.5) % 360;
        return hslToRgb(hue / 360, 0.7, 0.5);
    }

    function getClassName(classId: number): string {
        if (classId in ADE20K_CLASSES) {
            return ADE20K_CLASSES[classId].name;
        }
        return `class ${classId}`;
    }

    function preRenderSegments() {
        segmentCanvas = document.createElement('canvas');
        segmentCanvas.width = BG_WIDTH;
        segmentCanvas.height = BG_HEIGHT;
        const segCtx = segmentCanvas.getContext('2d')!;

        const imageData = segCtx.createImageData(BG_WIDTH, BG_HEIGHT);
        const data = imageData.data;

        for (let y = 0; y < BG_HEIGHT; y++) {
            for (let x = 0; x < BG_WIDTH; x++) {
                const classId = BG_SEGMENTS[y][x];
                const idx = (y * BG_WIDTH + x) * 4;

                const [r, g, b] = getSegmentColor(classId);

                data[idx] = r;
                data[idx + 1] = g;
                data[idx + 2] = b;
                data[idx + 3] = 200; // Semi-transparent
            }
        }

        segCtx.putImageData(imageData, 0, 0);
    }

    function preRenderGround() {
        groundCanvas = document.createElement('canvas');
        groundCanvas.width = BG_WIDTH;
        groundCanvas.height = BG_HEIGHT;
        const groundCtx = groundCanvas.getContext('2d')!;

        const imageData = groundCtx.createImageData(BG_WIDTH, BG_HEIGHT);
        const data = imageData.data;

        for (let y = 0; y < BG_HEIGHT; y++) {
            for (let x = 0; x < BG_WIDTH; x++) {
                const isGround = BG_GROUND[y][x];
                const idx = (y * BG_WIDTH + x) * 4;

                if (isGround) {
                    // Ground surface: green tint
                    data[idx] = 50;     // R
                    data[idx + 1] = 200; // G
                    data[idx + 2] = 80;  // B
                    data[idx + 3] = 180; // Alpha
                } else {
                    // Non-ground: transparent
                    data[idx] = 0;
                    data[idx + 1] = 0;
                    data[idx + 2] = 0;
                    data[idx + 3] = 0;
                }
            }
        }

        groundCtx.putImageData(imageData, 0, 0);
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
            ctx.globalAlpha = overlayOpacity;
            ctx.drawImage(depthCanvas, 0, 0, window.innerWidth, window.innerHeight);
            ctx.globalAlpha = 1.0;
        }

        // Optionally overlay flow field arrows
        if (showFlow) {
            ctx.globalAlpha = overlayOpacity;
            renderFlowArrows();
            ctx.globalAlpha = 1.0;
        }

        // Optionally overlay segments
        if (showSegments && segmentCanvas) {
            ctx.globalAlpha = overlayOpacity;
            ctx.drawImage(segmentCanvas, 0, 0, window.innerWidth, window.innerHeight);
            ctx.globalAlpha = 1.0;
        }

        // Optionally overlay ground mask
        if (showGround && groundCanvas) {
            ctx.globalAlpha = overlayOpacity;
            ctx.drawImage(groundCanvas, 0, 0, window.innerWidth, window.innerHeight);
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

        // Draw legend if overlays are shown
        if (showDepth) {
            drawDepthLegend();
        }
        if (showFlow) {
            drawFlowLegend();
        }
        if (showSegments) {
            drawSegmentLegend();
        }
        if (showGround) {
            drawGroundLegend();
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

    function drawFlowLegend() {
        const legendX = 20;
        const legendY = showDepth ? 100 : 20;
        const legendW = 180;
        const legendH = 50;

        ctx.fillStyle = 'rgba(0, 0, 0, 0.7)';
        ctx.fillRect(legendX - 10, legendY - 30, legendW + 20, legendH + 40);

        ctx.fillStyle = '#fff';
        ctx.font = '14px monospace';
        ctx.fillText('Flow Field', legendX, legendY - 10);

        ctx.font = '11px monospace';
        ctx.fillText('Arrows show water flow', legendX, legendY + 10);
        ctx.fillText('direction (downhill)', legendX, legendY + 24);

        // Draw example arrow
        ctx.strokeStyle = 'rgba(100, 200, 255, 0.9)';
        ctx.fillStyle = 'rgba(100, 200, 255, 0.9)';
        ctx.lineWidth = 2;

        const arrowX = legendX + 20;
        const arrowY = legendY + 45;
        const arrowLen = 25;

        ctx.beginPath();
        ctx.moveTo(arrowX, arrowY);
        ctx.lineTo(arrowX + arrowLen, arrowY);
        ctx.stroke();

        ctx.beginPath();
        ctx.moveTo(arrowX + arrowLen, arrowY);
        ctx.lineTo(arrowX + arrowLen - 6, arrowY - 4);
        ctx.lineTo(arrowX + arrowLen - 6, arrowY + 4);
        ctx.closePath();
        ctx.fill();

        ctx.fillStyle = '#fff';
        ctx.fillText('= flow direction', arrowX + arrowLen + 10, arrowY + 4);
    }

    function drawSegmentLegend() {
        const legendX = 20;
        let legendY = 20;
        if (showDepth) legendY += 80;
        if (showFlow) legendY += 100;

        const boxSize = 12;
        const rowHeight = 18;
        const numClasses = detectedClasses.length;
        const legendW = 180;
        const legendH = numClasses * rowHeight + 10;

        ctx.fillStyle = 'rgba(0, 0, 0, 0.7)';
        ctx.fillRect(legendX - 10, legendY - 30, legendW + 20, legendH + 40);

        ctx.fillStyle = '#fff';
        ctx.font = '14px monospace';
        ctx.fillText('Semantic Segmentation', legendX, legendY - 10);

        for (let i = 0; i < numClasses; i++) {
            const classId = detectedClasses[i];
            const y = legendY + i * rowHeight;
            const [r, g, b] = getSegmentColor(classId);
            const name = getClassName(classId);

            ctx.fillStyle = `rgb(${r}, ${g}, ${b})`;
            ctx.fillRect(legendX, y, boxSize, boxSize);

            ctx.fillStyle = '#fff';
            ctx.font = '11px monospace';
            ctx.fillText(name, legendX + boxSize + 8, y + 10);
        }
    }

    function drawGroundLegend() {
        const legendX = 20;
        let legendY = 20;
        if (showDepth) legendY += 80;
        if (showFlow) legendY += 100;
        if (showSegments) legendY += detectedClasses.length * 18 + 50;

        const legendW = 180;
        const legendH = 60;

        ctx.fillStyle = 'rgba(0, 0, 0, 0.7)';
        ctx.fillRect(legendX - 10, legendY - 30, legendW + 20, legendH + 40);

        ctx.fillStyle = '#fff';
        ctx.font = '14px monospace';
        ctx.fillText('Ground Mask', legendX, legendY - 10);

        // Ground color box
        ctx.fillStyle = 'rgb(50, 200, 80)';
        ctx.fillRect(legendX, legendY + 5, 12, 12);
        ctx.fillStyle = '#fff';
        ctx.font = '11px monospace';
        ctx.fillText('= ground surface', legendX + 20, legendY + 15);

        ctx.font = '10px monospace';
        ctx.fillStyle = '#aaa';
        ctx.fillText('Flow field computed on', legendX, legendY + 35);
        ctx.fillText('ground areas only', legendX, legendY + 48);
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'd' || e.key === 'D') {
            showDepth = !showDepth;
        } else if (e.key === 'f' || e.key === 'F') {
            showFlow = !showFlow;
        } else if (e.key === 's' || e.key === 'S') {
            showSegments = !showSegments;
        } else if (e.key === 'g' || e.key === 'G') {
            showGround = !showGround;
        } else if (e.key === 'ArrowUp' && (showDepth || showFlow || showSegments || showGround)) {
            overlayOpacity = Math.min(1, overlayOpacity + 0.1);
        } else if (e.key === 'ArrowDown' && (showDepth || showFlow || showSegments || showGround)) {
            overlayOpacity = Math.max(0, overlayOpacity - 0.1);
        }
    }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if browser}
    <canvas bind:this={canvas}></canvas>
    <div class="controls">
        <p>Press <kbd>D</kbd> to toggle depth overlay</p>
        <p>Press <kbd>F</kbd> to toggle flow field</p>
        <p>Press <kbd>S</kbd> to toggle segmentation</p>
        <p>Press <kbd>G</kbd> to toggle ground mask</p>
        {#if showDepth || showFlow || showSegments || showGround}
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
