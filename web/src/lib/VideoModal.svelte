<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";

  let domain = window.location.hostname;

  let id = $derived(page.url.searchParams.get("v"));

  function vDel() {
    let url = page.url;
    url.searchParams.delete("v");
    goto(url);
  }
</script>

{#if id}
  <button onclick={() => vDel()}>
    <video controls autoplay>
      <source
        src={`https://media.${domain}/video/${id}.mp4`}
        type="video/mp4"
      />
      <track kind="captions" />
    </video>
  </button>
{/if}

<style>
  button {
    z-index: 10;
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 24px;
    padding: 24px;
    background-color: #000000bf;
    outline: none;
    border: none;
  }

  video {
    max-width: 96%;
    max-height: 96%;
  }
</style>
