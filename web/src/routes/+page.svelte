<script lang="ts">
    import RainCanvas from "$lib/components/RainCanvas.svelte";
</script>

<svelte:head>
    <title>Home</title>
</svelte:head>

<RainCanvas />

<main>
    <!-- Content will go here, overlaying the rain -->
</main>

<style>
    :global(html, body) {
        margin: 0;
        padding: 0;
        width: 100%;
        height: 100%;
        overflow-x: hidden;
        background: #070709;
    }

    main {
        position: relative;
        z-index: 1;
        min-height: 100vh;
    }
</style>
