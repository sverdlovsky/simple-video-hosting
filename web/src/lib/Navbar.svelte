<script lang="ts">
  import type { Video } from "$lib/stores";
  import { videos } from "$lib/stores";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";

  let domain: string = window.location.hostname;
  let title: string = window.location.hostname.split(".")[0];
  title = title.charAt(0).toUpperCase() + title.slice(1);

  let search: string = $state("");
  let kind = $derived(page.url.searchParams.get("kind"));

  function getState(): string {
    const params = page.url.searchParams;

    return JSON.stringify({
      search: params.get("search"),
      tag: params.get("tag"),
      user: params.get("user"),
      app: params.get("app"),
      kind: params.get("kind"),
    });
  }

  function delKind(): void {
    const url: URL = page.url;

    url.searchParams.delete("kind");

    goto(url, { replaceState: true });
  }

  function setKind(value: string): void {
    const url: URL = page.url;

    url.searchParams.set("kind", value);

    goto(url, { replaceState: true });
  }

  let prevState: string;

  $effect(() => {
    const curState: string = getState();
    if (curState === prevState) {
      return;
    }
    prevState = curState;

    const url = new URL(`https://api.${domain}/video`);
    url.search = page.url.search;
    url.searchParams.set(
      "random",
      (url.searchParams.get("kind") === null).toString(),
    );

    const debounceTimeout = setTimeout(async () => {
      try {
        const res = await fetch(url, {
          method: "GET",
          credentials: "include",
        });
        if (!res.ok) {
          console.error("Request error");
          videos.set([]);
          return;
        }
        videos.set((await res.json()) as Video[]);
      } catch (err) {
        console.error("Network error", err);
        videos.set([]);
      }
    }, 300);

    return () => clearTimeout(debounceTimeout);
  });
</script>

<div class="navbar">
  <div class="navbox">
    <div class="top">
      <a href="/" class="title">
        <svg viewBox="0 0 24 24">
          <path
            d="M7 8.69951V15.3005C7 16.8255 8.63823 17.7894 9.97129 17.0488L9.48564 16.1746L9.97129 17.0488L15.9122 13.7483C17.2838 12.9863 17.2838 11.0137 15.9122 10.2517L9.97129 6.9512C8.63822 6.21061 7 7.17455 7 8.69951Z"
          />
        </svg>
        <p>{title}</p>
      </a>
      <div class="actions">
        <button class="add">
          <svg viewBox="0 0 24 24">
            <path
              d="M13 11h7a1 1 0 0 1 0 2h-7v7a1 1 0 0 1-2 0v-7H4a1 1 0 0 1 0-2h7V4a1 1 0 0 1 2 0v7z"
            />
          </svg>
          <p>Add</p>
        </button>
        <a href={`https://auth.${domain}/?next=https://${domain}`} class="sign">
          <svg viewBox="0 0 24 24">
            <path d="M20 21v-2a4 4 0 00-4-4H8a4 4 0 00-4 4v2" />
            <circle cx="12" cy="7" r="4" />
          </svg>
          <p>Sign in</p>
        </a>
      </div>
    </div>
    <div class="search">
      <svg viewBox="0 0 24 24">
        <path
          d="M20.207 18.793L16.6 15.184a7.027 7.027 0 1 0-1.416 1.416l3.609 3.609a1 1 0 0 0 1.414-1.416zM6 11a5 5 0 1 1 5 5 5.006 5.006 0 0 1-5-5z"
        />
      </svg>
      <input type="text" bind:value={search} placeholder="Search..." />
    </div>
    <div class="categories">
      <button class:active={!kind} onclick={() => delKind()}> All </button>
      <button class:active={kind === "full"} onclick={() => setKind("full")}>
        Fulls
      </button>
      <button class:active={kind === "clip"} onclick={() => setKind("clip")}>
        Clips
      </button>
      <button class:active={kind === "short"} onclick={() => setKind("short")}>
        Shorts
      </button>
    </div>
  </div>
</div>

<style>
  button {
    outline: none;
    border: none;
    cursor: pointer;
  }

  .navbar {
    z-index: 5;
    position: fixed;
    width: 100%;
    display: flex;
    flex-direction: column;
    border-bottom: 1px solid var(--color-zinc-900);
    align-items: center;

    background: transparent;
    backdrop-filter: blur(32px) saturate(200%);
    -webkit-backdrop-filter: blur(32px) saturate(200%);
  }

  .navbox {
    width: calc(min(max(60%, 1440px), 100%) - 48px);
    display: flex;
    flex-direction: column;
    padding: 24px;
    gap: 32px;
  }

  .top {
    display: flex;
    flex-direction: row;
    justify-content: space-between;
  }

  .title {
    display: flex;
    flex-direction: row;
    gap: 8px;
    align-items: center;
    text-decoration: none;
  }

  .title svg {
    width: 48px;
    height: 48px;
    fill: none;
    stroke: var(--color-white);
    stroke-width: 2;
    stroke-linecap: round;
  }

  .title p {
    font-size: var(--text-2xl);
    line-height: 0px;
    font-weight: var(--font-weight-medium);
    color: var(--color-white);
  }

  .actions {
    display: flex;
    flex-direction: row;
    gap: 8px;
    align-items: center;
    text-decoration: none;
  }

  .add {
    height: 40px;
    display: flex;
    flex-direction: row;
    gap: 4px;
    align-items: center;
    background-color: var(--color-zinc-200);
    border: none;
    border-radius: calc(infinity * 1px);
    padding-left: 16px;
    padding-right: 24px;
  }

  .add svg {
    width: 16px;
    height: 16px;
    fill: var(--color-black);
  }

  .add p {
    color: var(--color-black);
    font-size: var(--text-base);
    font-weight: var(--font-weight-medium);
    line-height: 0px;
  }

  .sign {
    height: 40px;
    display: flex;
    flex-direction: row;
    gap: 4px;
    align-items: center;
    background-color: color-mix(
      in oklab,
      var(--color-zinc-900) 50%,
      transparent
    );
    border-radius: calc(infinity * 1px);
    padding-left: 16px;
    padding-right: 24px;
    text-decoration: none;
  }

  .sign svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: var(--color-zinc-400);
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .sign p {
    color: var(--color-zinc-400);
    font-size: var(--text-base);
    font-weight: var(--font-weight-medium);
    line-height: 0px;
  }

  .search {
    width: 100%;
    display: flex;
    flex-direction: row;
    justify-content: center;
    background-color: color-mix(
      in oklab,
      var(--color-zinc-900) 50%,
      transparent
    );
    border: 1px solid color-mix(in oklab, var(--ring) 50%, transparent);
    border-radius: calc(infinity * 1px);
    align-items: center;
  }

  .search svg {
    width: 20px;
    height: 20px;
    fill: var(--color-zinc-500);
    padding: 14px;
  }

  .search input {
    width: 100%;
    height: 46px;
    background-color: transparent;
    border: none;
    outline: none;
    font-size: 16px;
    color: var(--color-zinc-200);
  }

  .categories {
    display: flex;
    flex-direction: row;
    justify-content: start;
    overflow: scroll;
    gap: 8px;
  }

  .categories button {
    height: 40px;
    border: none;
    border-radius: calc(infinity * 1px);
    padding: 0px 24px;
    background-color: color-mix(
      in oklab,
      var(--color-zinc-900) 50%,
      transparent
    );
    color: var(--color-zinc-400);
    text-decoration: none;
    font-size: var(--text-sm);
    font-weight: var(--font-weight-medium);
    line-height: 0px;
  }

  .categories button:hover {
    background-color: var(--color-zinc-900);
    color: var(--color-white);
    transition:
      background-color 0.3s ease-in-out,
      color 0.3s ease-in-out;
  }

  .categories button.active {
    background-color: var(--color-white);
    color: var(--color-black);
  }
</style>
