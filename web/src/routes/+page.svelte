<script lang="ts">
  import { videos } from "$lib/stores";
  import VideoCard from "$lib/VideoCard.svelte";
</script>

<main>
  <div class="videos">
    {#each $videos as video}
      <VideoCard {...video} />
    {/each}
  </div>
</main>

<style>
  main {
    z-index: 1;
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 24px;
    padding: 24px;

    overflow: scroll;
    scrollbar-width: none;
    -ms-overflow-style: none;
  }

  main::-webkit-scrollbar {
    display: none; /* Older Safari and Chromium */
  }

  .videos {
    max-width: calc(min(max(60%, 1440px), 100%) - 48px);
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(max(25%, 256px), 1fr));
    gap: 32px;
    padding-top: 230px;
    padding-left: 24px;
    padding-right: 24px;
  }
</style>
