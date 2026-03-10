<script lang="ts">
  import { onMount } from "svelte";
  import Framework7 from "framework7/lite-bundle";
  import Framework7Svelte, { App, View } from "framework7-svelte";
  import "framework7/css/bundle";
  import "framework7-icons";
  import store from "$lib/store.svelte";
  import "../app.css";

  import Chat from "./chat.svelte";
  import Memory from "./memory.svelte";
  import System from "./system.svelte";

  Framework7.use(Framework7Svelte);

  const f7params = {
    name: "Aide Mobile",
    theme: "ios",
    routes: [
      {
        path: "/",
        component: Chat,
      },
      {
        path: "/memory",
        component: Memory,
      },
      {
        path: "/system",
        component: System,
      },
    ],
  };

  onMount(() => {
    store.checkModel().then();
  });
</script>

<App {...f7params}>
  <View main url="/" />
</App>

<style>
  :global(body) {
    margin: 0;
  }
</style>
