<script lang="ts">
    import AsciiCanvas from "$lib/components/AsciiCanvas.svelte";

    let canvasRef: any;
    let spawnRate = 0.3;
    let gravity = 0.8;
    let fps = 24;
    let isPaused = false;

    function togglePause() {
        if (canvasRef) {
            isPaused = !isPaused;
            isPaused ? canvasRef.pause() : canvasRef.play();
        }
    }

    function clearDroplets() {
        if (canvasRef) {
            canvasRef.clear();
        }
    }
</script>

<svelte:head>
    <title>ASCII Droplet Engine Demo</title>
</svelte:head>

<main>
    <h1>ASCII Rain Engine</h1>
    <p>Rust + WASM + Svelte</p>

    <div class="demo-container">
        <AsciiCanvas
            bind:this={canvasRef}
            width={80}
            height={30}
            bind:spawnRate
            bind:gravity
            bind:fps
        />
    </div>

    <div class="controls">
        <h3>Controls</h3>

        <div class="control-group">
            <label>
                Spawn Rate: {spawnRate.toFixed(2)}
                <input type="range" min="0" max="1" step="0.05" bind:value={spawnRate} />
            </label>
        </div>

        <div class="control-group">
            <label>
                Gravity: {gravity.toFixed(2)}
                <input type="range" min="0" max="1" step="0.1" bind:value={gravity} />
            </label>
        </div>

        <div class="control-group">
            <label>
                FPS: {fps}
                <input type="range" min="1" max="60" step="1" bind:value={fps} />
            </label>
        </div>

        <div class="button-group">
            <button on:click={togglePause}>
                {isPaused ? 'Play' : 'Pause'}
            </button>
            <button on:click={clearDroplets}>Clear</button>
        </div>
    </div>
</main>

<style>
    main {
        max-width: 1200px;
        margin: 0 auto;
        padding: 2rem;
        font-family: system-ui, sans-serif;
    }

    h1 {
        font-size: 2.5rem;
        margin-bottom: 0.5rem;
        background: linear-gradient(135deg, #00ff41, #00cc33);
        -webkit-background-clip: text;
        background-clip: text;
        -webkit-text-fill-color: transparent;
    }

    .demo-container {
        display: flex;
        gap: 2rem;
        margin-top: 2rem;
        flex-wrap: wrap;
    }

    .controls {
        flex: 1;
        min-width: 250px;
        padding: 1.5rem;
        background: #1a1a1a;
        border-radius: 8px;
        color: #fff;
    }

    h3 {
        margin-top: 0;
        color: #00ff41;
    }

    .control-group {
        margin-bottom: 1.5rem;
    }

    label {
        display: block;
        font-size: 0.9rem;
        margin-bottom: 0.5rem;
        color: #aaa;
    }

    input[type="range"] {
        width: 100%;
        height: 6px;
        border-radius: 3px;
        background: #333;
        outline: none;
    }

    input[type="range"]::-webkit-slider-thumb {
        width: 18px;
        height: 18px;
        border-radius: 50%;
        background: #00ff41;
        cursor: pointer;
    }

    input[type="range"]::-webkit-range-thumb {
        width: 18px;
        height: 18px;
        border-radius: 50%;
        background: #00ff41;
        cursor: pointer;
        border: none;
    }

    .button-group {
        display: flex;
        gap: 0.5rem;
    }

    button {
        flex: 1;
        padding: 0.75rem 1rem;
        color: #000;
        border: none;
        border-radius: 4px;
        font-weight: 600;
        cursor: pointer;
        transition: all 0.2s;
    }

    button:hover {
        background: #00cc33;
        transform: translateY(-1px);
    }

    button:active {
        transform: translateY(0);
    }
</style>