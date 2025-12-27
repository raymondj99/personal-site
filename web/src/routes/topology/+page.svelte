<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { browser } from '$app/environment';
    import { BG_PALETTE, BG_PIXELS, BG_GROUND } from '$lib/scene';
    import {
        BG_WIDTH,
        BG_HEIGHT,
        getHeight,
        sampleFlowField,
        sampleNormals
    } from '$lib/world';

    // -----------------------------------------------------------------------------
    // State
    // -----------------------------------------------------------------------------

    let container: HTMLDivElement;
    let heightScale = 1.0;
    let showWireframe = false;
    let showGround = false;
    let showFlow = false;
    let showNormals = false;

    let THREE: any;
    let scene: any;
    let camera: any;
    let renderer: any;
    let controls: any;
    let terrain: any;
    let wireframeMesh: any;
    let flowLines: any;
    let normalLines: any;
    let animationId: number;

    // Materials
    let textureMat: any;
    let groundMat: any;

    // Original height data for scaling
    let basePositions: Float32Array;

    // -----------------------------------------------------------------------------
    // Coordinate Conversion
    // -----------------------------------------------------------------------------

    const WORLD_WIDTH = 2;
    const WORLD_HEIGHT = WORLD_WIDTH * BG_HEIGHT / BG_WIDTH;

    function pixelToWorld(px: number, py: number, h: number): [number, number, number] {
        const wx = (px / BG_WIDTH - 0.5) * WORLD_WIDTH;
        const wz = (py / BG_HEIGHT - 0.5) * WORLD_HEIGHT;
        const wy = h * 0.5;
        return [wx, wy, wz];
    }

    // -----------------------------------------------------------------------------
    // Geometry Builders
    // -----------------------------------------------------------------------------

    function buildFlowLines(): any {
        const samples = sampleFlowField(8);
        const positions: number[] = [];
        const colors: number[] = [];

        for (const s of samples) {
            const [x, y, z] = pixelToWorld(s.x, s.y, s.height);
            const len = 0.03 + s.speed * 0.06;

            positions.push(x, y + 0.01, z);
            positions.push(x + s.dir.x * len, y + 0.01, z + s.dir.y * len);

            // Color by speed
            const c = Math.min(s.speed * 4, 1);
            colors.push(0.3, 0.6 + c * 0.3, 0.9, 0.3, 0.6 + c * 0.3, 0.9);
        }

        const geo = new THREE.BufferGeometry();
        geo.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
        geo.setAttribute('color', new THREE.Float32BufferAttribute(colors, 3));
        return new THREE.LineSegments(geo, new THREE.LineBasicMaterial({ vertexColors: true }));
    }

    function buildNormalLines(): any {
        const samples = sampleNormals(12);
        const positions: number[] = [];
        const len = 0.04;

        for (const s of samples) {
            const [x, y, z] = pixelToWorld(s.x, s.y, s.height);
            positions.push(x, y + 0.01, z);
            positions.push(x + s.normal.x * len, y + 0.01 + s.normal.y * len, z + s.normal.z * len);
        }

        const geo = new THREE.BufferGeometry();
        geo.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
        return new THREE.LineSegments(geo, new THREE.LineBasicMaterial({ color: 0xff8844 }));
    }

    function buildTexture(showGroundMask: boolean): Uint8Array {
        const data = new Uint8Array(BG_WIDTH * BG_HEIGHT * 4);

        for (let y = 0; y < BG_HEIGHT; y++) {
            for (let x = 0; x < BG_WIDTH; x++) {
                const i = (y * BG_WIDTH + x) * 4;
                const c = BG_PALETTE[BG_PIXELS[y][x]];
                let r = parseInt(c.slice(1, 3), 16);
                let g = parseInt(c.slice(3, 5), 16);
                let b = parseInt(c.slice(5, 7), 16);

                if (showGroundMask) {
                    if (BG_GROUND[y][x]) {
                        r = Math.min(255, r * 0.5 + 50);
                        g = Math.min(255, g * 0.7 + 100);
                        b = Math.min(255, b * 0.5 + 30);
                    } else {
                        r *= 0.3; g *= 0.3; b *= 0.3;
                    }
                }

                data[i] = r; data[i+1] = g; data[i+2] = b; data[i+3] = 255;
            }
        }

        return data;
    }

    // -----------------------------------------------------------------------------
    // Scene Setup
    // -----------------------------------------------------------------------------

    async function initScene() {
        THREE = await import('three');
        const { OrbitControls } = await import('three/examples/jsm/controls/OrbitControls.js');

        // Scene
        scene = new THREE.Scene();
        scene.background = new THREE.Color(0x1a1a2e);

        // Camera
        camera = new THREE.PerspectiveCamera(60, window.innerWidth / window.innerHeight, 0.1, 100);
        camera.position.set(0, 1.5, 2);

        // Renderer
        renderer = new THREE.WebGLRenderer({ antialias: true });
        renderer.setSize(window.innerWidth, window.innerHeight);
        renderer.setPixelRatio(window.devicePixelRatio);
        container.appendChild(renderer.domElement);

        // Controls
        controls = new OrbitControls(camera, renderer.domElement);
        controls.enableDamping = true;

        // Lighting
        scene.add(new THREE.AmbientLight(0x404040, 0.5));
        const light = new THREE.DirectionalLight(0xffffff, 1);
        light.position.set(1, 2, 1);
        scene.add(light);

        // Terrain mesh
        const geo = new THREE.PlaneGeometry(WORLD_WIDTH, WORLD_HEIGHT, BG_WIDTH - 1, BG_HEIGHT - 1);
        const pos = geo.attributes.position.array as Float32Array;

        // Set heights
        for (let y = 0; y < BG_HEIGHT; y++) {
            for (let x = 0; x < BG_WIDTH; x++) {
                const i = y * BG_WIDTH + x;
                pos[i * 3 + 2] = getHeight(x, y) * 0.5;
            }
        }
        basePositions = new Float32Array(pos);
        geo.attributes.position.needsUpdate = true;
        geo.computeVertexNormals();

        // Textures
        const tex = new THREE.DataTexture(buildTexture(false), BG_WIDTH, BG_HEIGHT, THREE.RGBAFormat);
        tex.flipY = true; tex.needsUpdate = true;
        const gTex = new THREE.DataTexture(buildTexture(true), BG_WIDTH, BG_HEIGHT, THREE.RGBAFormat);
        gTex.flipY = true; gTex.needsUpdate = true;

        textureMat = new THREE.MeshStandardMaterial({ map: tex, side: THREE.DoubleSide });
        groundMat = new THREE.MeshStandardMaterial({ map: gTex, side: THREE.DoubleSide });

        terrain = new THREE.Mesh(geo, textureMat);
        terrain.rotation.x = -Math.PI / 2;
        scene.add(terrain);

        // Wireframe
        wireframeMesh = new THREE.Mesh(
            geo.clone(),
            new THREE.MeshBasicMaterial({ color: 0x00ff88, wireframe: true })
        );
        wireframeMesh.rotation.x = -Math.PI / 2;
        wireframeMesh.visible = false;
        scene.add(wireframeMesh);

        // Flow field
        flowLines = buildFlowLines();
        flowLines.visible = false;
        scene.add(flowLines);

        // Normals
        normalLines = buildNormalLines();
        normalLines.visible = false;
        scene.add(normalLines);

        // Grid
        const grid = new THREE.GridHelper(3, 30, 0x444444, 0x222222);
        grid.position.y = -0.01;
        scene.add(grid);

        window.addEventListener('resize', onResize);
        window.addEventListener('keydown', onKey);
        animate();
    }

    // -----------------------------------------------------------------------------
    // Updates
    // -----------------------------------------------------------------------------

    function updateHeightScale() {
        if (!terrain || !basePositions) return;

        const pos = terrain.geometry.attributes.position.array;
        const wfPos = wireframeMesh.geometry.attributes.position.array;

        for (let i = 0; i < basePositions.length / 3; i++) {
            const z = basePositions[i * 3 + 2] * heightScale;
            pos[i * 3 + 2] = z;
            wfPos[i * 3 + 2] = z;
        }

        terrain.geometry.attributes.position.needsUpdate = true;
        terrain.geometry.computeVertexNormals();
        wireframeMesh.geometry.attributes.position.needsUpdate = true;
    }

    function updateMaterial() {
        if (terrain) terrain.material = showGround ? groundMat : textureMat;
    }

    // -----------------------------------------------------------------------------
    // Events
    // -----------------------------------------------------------------------------

    function onResize() {
        camera.aspect = window.innerWidth / window.innerHeight;
        camera.updateProjectionMatrix();
        renderer.setSize(window.innerWidth, window.innerHeight);
    }

    function onKey(e: KeyboardEvent) {
        const k = e.key.toLowerCase();
        if (k === 'r') { camera.position.set(0, 1.5, 2); controls.target.set(0, 0, 0); }
        if (k === 'w') { showWireframe = !showWireframe; wireframeMesh.visible = showWireframe; }
        if (k === 'g') { showGround = !showGround; updateMaterial(); }
        if (k === 'f') { showFlow = !showFlow; flowLines.visible = showFlow; }
        if (k === 'n') { showNormals = !showNormals; normalLines.visible = showNormals; }
    }

    function animate() {
        animationId = requestAnimationFrame(animate);
        controls?.update();
        renderer?.render(scene, camera);
    }

    // -----------------------------------------------------------------------------
    // Lifecycle
    // -----------------------------------------------------------------------------

    onMount(() => { if (browser) initScene(); });

    onDestroy(() => {
        if (!browser) return;
        if (animationId) cancelAnimationFrame(animationId);
        window.removeEventListener('resize', onResize);
        window.removeEventListener('keydown', onKey);
        renderer?.dispose();
    });

    $: if (browser && terrain) updateHeightScale();
    $: if (browser && wireframeMesh) wireframeMesh.visible = showWireframe;
    $: if (browser && flowLines) flowLines.visible = showFlow;
    $: if (browser && normalLines) normalLines.visible = showNormals;
</script>

<svelte:head><title>3D Topology</title></svelte:head>

<div bind:this={container} class="canvas"></div>

<div class="info">
    <h2>3D Topology</h2>
    <p><kbd>R</kbd> Reset | <kbd>W</kbd> Wire | <kbd>G</kbd> Ground</p>
    <p><kbd>F</kbd> Flow | <kbd>N</kbd> Normals</p>
</div>

<div class="controls">
    <label>Height: <input type="range" bind:value={heightScale} min="0.1" max="3" step="0.1"> {heightScale.toFixed(1)}</label>
    <label><input type="checkbox" bind:checked={showWireframe}> Wireframe</label>
    <label><input type="checkbox" bind:checked={showGround}> Ground</label>
    <label><input type="checkbox" bind:checked={showFlow}> Flow</label>
    <label><input type="checkbox" bind:checked={showNormals}> Normals</label>
</div>

<a href="/" class="back">Back</a>

<style>
    :global(body) { margin: 0; overflow: hidden; }
    .canvas { position: fixed; inset: 0; }
    .info, .controls {
        position: fixed;
        background: rgba(0,0,0,0.8);
        color: #fff;
        padding: 12px 16px;
        border-radius: 8px;
        font: 12px monospace;
        z-index: 10;
    }
    .info { top: 20px; left: 20px; }
    .info h2 { margin: 0 0 8px; font-size: 14px; }
    .info p { margin: 4px 0; color: #aaa; }
    kbd { background: rgba(255,255,255,0.2); padding: 2px 5px; border-radius: 3px; }
    .controls { bottom: 20px; left: 20px; }
    .controls label { display: block; margin: 6px 0; }
    .controls input[type="range"] { width: 100px; vertical-align: middle; }
    .back {
        position: fixed;
        top: 20px;
        right: 20px;
        background: rgba(0,0,0,0.8);
        color: #88f;
        padding: 8px 12px;
        border-radius: 6px;
        text-decoration: none;
        font: 12px monospace;
    }
</style>
