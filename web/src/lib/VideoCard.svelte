<script lang="ts">
  import { page } from "$app/state";
  import { goto } from "$app/navigation";

  const domain: string = window.location.hostname;

  export let id: string;
  export let title: string;
  export let tags: {
    id: string;
    title: string;
    color: number;
  }[];
  export let users: {
    id: string;
    email: string;
    role: string;
  }[];
  export let apps: { id: string; title: string; link?: string }[];

  function queryAdd(key: string, value: string): void {
    let url: URL = page.url;
    url.searchParams.set(key, value);
    goto(url);
  }
</script>

<div class="videocard">
  <button class="preview" onclick={() => queryAdd("v", id)}>
    <svg viewBox="0 0 24 24">
      <path
        d="M14.2647 15.9377L12.5473 14.2346C11.758 13.4519 11.3633 13.0605 10.9089 12.9137C10.5092 12.7845 10.079 12.7845 9.67922 12.9137C9.22485 13.0605 8.83017 13.4519 8.04082 14.2346L4.04193 18.2622M14.2647 15.9377L14.606 15.5991C15.412 14.7999 15.8149 14.4003 16.2773 14.2545C16.6839 14.1262 17.1208 14.1312 17.5244 14.2688C17.9832 14.4253 18.3769 14.834 19.1642 15.6515L20 16.5001M14.2647 15.9377L18.22 19.9628M12 4H7.2C6.07989 4 5.51984 4 5.09202 4.21799C4.7157 4.40973 4.40973 4.71569 4.21799 5.09202C4 5.51984 4 6.0799 4 7.2V16.8C4 17.4466 4 17.9066 4.04193 18.2622M4.04193 18.2622C4.07264 18.5226 4.12583 18.7271 4.21799 18.908C4.40973 19.2843 4.7157 19.5903 5.09202 19.782C5.51984 20 6.07989 20 7.2 20H16.8C17.9201 20 18.4802 20 18.908 19.782C19.2843 19.5903 19.5903 19.2843 19.782 18.908C20 18.4802 20 17.9201 20 16.8V12M16 3L18.5 5.5M18.5 5.5L21 8M18.5 5.5L21 3M18.5 5.5L16 8"
      />
    </svg>
    <img
      src={`https://media.${domain}/previews/${id}.jpg`}
      alt=""
      loading="lazy"
    />
  </button>
  <!--
  <div class="tags">
    {#each tags as tag}
      <button
        class="tag"
        onclick={() => queryAdd("tag", tag.id)}
        style="background-color: #{tag.color.toString(16)}"
      >
        {tag.title}
      </button>
    {/each}
  </div>
  -->
  <div class="users">
    {#each (users || []).slice(0, 3) as user, i}
      <button
        class="user_button"
        onclick={() => queryAdd("user", user.id)}
        style="z-index: {users.length - i}"
      >
        <img
          src={`https://media.${domain}/avatars/${user.id}.png`}
          alt={user.email}
          class="user_icon"
          loading="lazy"
        />
      </button>
    {/each}
  </div>
  <p>{title}</p>
  <div class="apps">
    {#each (apps || []).slice(0, 3) as app, i}
      <button
        class="app_button"
        onclick={() => queryAdd("app", app.id)}
        style="z-index: {apps.length - i}"
      >
        <img
          src={`https://media.${domain}/apps/${app.id}.png`}
          alt={app.title}
          class="app_icon"
          loading="lazy"
        />
      </button>
    {/each}
  </div>
</div>

<style>
  button {
    outline: none;
    border: none;
    cursor: pointer;
  }

  p {
    grid-area: title;
    line-height: 0px;
    text-align: center;
    color: var(--color-white);
  }

  .videocard {
    display: grid;
    grid-template-columns: 1fr 6fr 1fr;
    grid-template-rows: auto auto auto;
    grid-template-areas:
      "preview preview preview"
      "tags tags tags"
      "users title apps";
    gap: 4px;
  }

  .preview {
    grid-area: preview;
    position: relative;
    width: 100%;
    aspect-ratio: 16 / 9;
    border-radius: 16px;
    background-color: color-mix(
      in oklab,
      var(--color-zinc-900) 50%,
      transparent
    );
    overflow: hidden;
  }

  .preview svg {
    z-index: 1;
    width: 25%;
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    fill: none;
    stroke: var(--color-black);
    stroke-width: 2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .preview img {
    z-index: 2;
    width: 100%;
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
  }

  .tags {
    grid-area: tags;
  }

  .tag {
    padding: 2px 6px;
    border-radius: 6px;
  }

  .users {
    grid-area: users;
    display: flex;
    flex-direction: row;
    justify-content: start;
  }

  .user_button {
    border: none;
    background: none;
    padding: 0;
    cursor: pointer;
    margin-right: -16px;
    transition: margin-right 0.3s ease;
    -webkit-tap-highlight-color: transparent;
  }

  .users:hover .user_button {
    margin: 0;
  }

  .user_icon {
    width: 32px;
    height: 32px;
    border-radius: calc(infinity * 1px);
  }

  .apps {
    grid-area: apps;
    display: flex;
    flex-direction: row-reverse;
    justify-content: end;
  }

  .app_button {
    border: none;
    background: none;
    padding: 0;
    cursor: pointer;
    margin-left: -16px;
    transition: margin-right 0.3s ease;
    -webkit-tap-highlight-color: transparent;
  }

  .apps:hover .app_button {
    margin: 0;
  }

  .app_icon {
    height: 32px;
    width: 32px;
    border-radius: 8px;
    margin-left: -16px;
  }
</style>
