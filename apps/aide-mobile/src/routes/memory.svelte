<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { Page, Navbar, BlockTitle, Block } from "framework7-svelte";

  let memory = $state("Loading...");

  onMount(async () => {
    try {
      memory = await invoke<string>("get_memory");
    } catch (e) {
      memory = "Error: " + e;
    }
  });
</script>

<Page>
  <Navbar backLink="Back" title="Aide's Memory" />
  <BlockTitle>What Aide knows about you</BlockTitle>
  <Block strong inset>
    <p>{memory}</p>
  </Block>
  <Block>
    <p>Aide learns from your conversations to be more helpful over time.</p>
  </Block>
</Page>
