<script lang="ts">
  import "../app.css";
  import Navbar from "$lib/Navbar.svelte";
  import VideoModal from "$lib/VideoModal.svelte";

  let title: string = window.location.hostname.split(".")[0];
  title = title.charAt(0).toUpperCase() + title.slice(1);
</script>

<svelte:head>
  <link rel="icon" href="/svg/favicon.png" />
  <title>{title}</title>
</svelte:head>

<VideoModal />

<Navbar />

<slot />
