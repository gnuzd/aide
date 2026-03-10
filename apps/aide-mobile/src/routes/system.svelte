<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import {
    Page,
    Navbar,
    List,
    ListItem,
    BlockTitle,
    Preloader,
  } from "framework7-svelte";
  import type { SystemInfo } from "$lib/types";

  let systemInfo = $state<SystemInfo | null>(null);

  onMount(async () => {
    try {
      systemInfo = await invoke<SystemInfo>("get_system_info");
    } catch (e) {
      console.error(e);
    }
  });
</script>

<Page>
  <Navbar backLink="Back" title="System Info" />
  <BlockTitle>Hardware Audit</BlockTitle>
  {#if !systemInfo}
    <div style="text-align: center; padding: 20px;">
      <Preloader />
    </div>
  {:else}
    <List strong inset>
      <ListItem title="OS" after={systemInfo.os} />
      <ListItem title="CPU" after={systemInfo.cpu} />
      <ListItem title="Total Memory" after={systemInfo.memory_total + " GB"} />
      <ListItem
        title="Available Memory"
        after={systemInfo.memory_available + " GB"}
      />
      <ListItem
        title="Compatible"
        after={systemInfo.is_compatible ? "Yes" : "No"}
      />
    </List>
    {#if systemInfo.warnings.length > 0}
      <BlockTitle>Warnings</BlockTitle>
      <List strong inset>
        {#each systemInfo.warnings as warning}
          <ListItem title={warning} />
        {/each}
      </List>
    {/if}
  {/if}
</Page>
