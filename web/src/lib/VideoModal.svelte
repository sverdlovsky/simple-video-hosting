<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";

  let domain = window.location.hostname;

  let id = $derived(page.url.searchParams.get("v"));
  let videoUrl = $state<string | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);

  let qual = $derived(page.url.searchParams.get("qual") ?? "high");

  $effect(() => {
    if (!id) {
      videoUrl = null;
      return;
    }

    loading = true;
    error = null;
    videoUrl = null;

    fetch(`https://api.${domain}/video/${id}/${qual}`, {
      method: "GET",
      credentials: "include",
    })
      .then((res) => {
        if (!res.ok) throw new Error(`Failed to fetch video: ${res.status}`);
        return res.json();
      })
      .then((data) => {
        videoUrl = data.url;
      })
      .catch((e) => {
        error = e.message;
      })
      .finally(() => {
        loading = false;
      });
  });

  function vDel() {
    let url = page.url;
    url.searchParams.delete("v");
    goto(url);
  }
</script>

{#if id}
  <button onclick={() => vDel()}>
    {#if loading}
      <p>Загрузка...</p>
    {:else if error}
      <p>Ошибка: {error}</p>
    {:else if videoUrl}
      <video controls autoplay>
        <source src={videoUrl} type="video/mp4" />
        <track kind="captions" />
      </video>
    {/if}

    <!--
    <h3>{title}</h3>

    <p>{cat}</p>

    <p>{desc}</p>

    <div class="users">
      {#each users as user, i}
        <button
          class="user_button"
          onclick={() => queryAdd("user", user.id)}
        >
          <img
            src={`https://media.${domain}/avatars/${user.id}.png`}
            alt={user.email}
            class="user_icon"
            loading="lazy"
          />
        </button>
        <h2>{user.name}</h2>
        <p>{user.desc}</p>
      {/each}
    </div>

    <div class="apps">
      {#each apps as app, i}
        <button
          class="app_button"
          onclick={() => queryAdd("app", app.id)}
        >
          <img
            src={`https://media.${domain}/apps/${app.id}.png`}
            alt={app.title}
            class="app_icon"
            loading="lazy"
          />
        </button>
        <h2>{app.name}</h2>
        <p>{app.desc}</p>
      {/each}
    </div>
    -->
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
