<script lang="ts">
  import { onMount } from "svelte";

  import store from "$lib/store.svelte";
  import { listen } from "@tauri-apps/api/event";
  import {
    Block,
    Button,
    f7,
    Icon,
    Link,
    List,
    ListItem,
    Message,
    Messagebar,
    Messages,
    MessagesTitle,
    Navbar,
    NavRight,
    NavTitle,
    Page,
    Popover,
    Popup,
    Preloader,
  } from "framework7-svelte";

  let messageText = $state("");

  onMount(() => {
    const unlisten = listen("token", (event) => {
      store.streaming(event.payload as string);
    });

    return () => {
      unlisten.then((f) => f());
    };
  });

  const dateFormatter = new Intl.DateTimeFormat("en-US", {
    weekday: "long",
    month: "short",
    day: "numeric",
  });

  const timeFormatter = new Intl.DateTimeFormat("en-US", {
    hour12: false,
    hour: "2-digit",
    minute: "2-digit",
  });

  const currentDate = new Date();
  const currentDay = dateFormatter.format(currentDate);
  const currentTime = timeFormatter.format(currentDate);

  console.log(store.models);
</script>

<Page>
  <Navbar>
    <NavTitle>Aide Chat</NavTitle>
    <NavRight>
      <Link iconIos="f7:gear" popoverOpen=".menu"></Link>
    </NavRight>
  </Navbar>

  <Messagebar
    placeholder="Message"
    value={messageText}
    onInput={(e) => (messageText = e.target.value)}
  >
    {#snippet innerEnd()}
      <a
        class="link icon-only"
        onclick={() => {
          store.sendMessage(messageText);
          messageText = "";
        }}
      >
        <Icon f7="arrow_up_circle" />
      </a>
    {/snippet}
  </Messagebar>

  <Messages>
    <MessagesTitle>
      <b>{currentDay}</b> , {currentTime}
    </MessagesTitle>

    {#each store.messages as message, index (index)}
      {#if message.type === "received"}
        {#if message.text}
          <Message type={message.type} name={message.name} text={message.text}>
            {#if message.avatar}
              {#snippet avatar()}
                <img
                  alt="avatar"
                  class="w-8 h-8 rounded-full"
                  src={message.avatar}
                />
              {/snippet}
            {/if}
          </Message>
        {:else}
          <p
            class="inline-block bg-linear-to-r from-slate-500 via-white/80 to-slate-500 bg-size[200%_auto] bg-clip-text text-transparent animate-shimmer"
          >
            Thinking...
          </p>
        {/if}
      {:else}
        <Message type={message.type} name={message.name} text={message.text} />
      {/if}
    {/each}
  </Messages>

  <Popover class="menu">
    <List>
      <ListItem title="Memory" link="/memory" popoverClose noChevron />
      <ListItem title="System" link="/system" popoverClose noChevron />
      <ListItem
        title="Clear Data"
        popoverClose
        onClick={() => {
          f7.dialog.confirm("Are you sure you want to clear all data?", () => {
            // We'll handle this via a global state or a direct call
            import("@tauri-apps/api/core").then((tauri) => {
              tauri.invoke("clear_data", { target: "all" }).then(() => {
                window.location.reload();
              });
            });
          });
        }}
      />
    </List>
  </Popover>

  <Popup opened={!store.isModelLoaded}>
    <Page>
      <Navbar title="Setup Aide" />
      <Block strong>
        <p>
          Aide needs a local AI model to run. Please select one to download.
        </p>
        {#if store.systemInfo}
          <p>
            <b>System:</b>
            {store.systemInfo.os}, {store.systemInfo.memory_total}GB RAM
          </p>
        {/if}
      </Block>
      <List strong>
        {#each store.models as model}
          <ListItem
            title={model.name}
            subtitle={model.description}
            text={"Quality: " + model.quality_score + "/10"}
          >
            {#snippet after()}
              <div style="display: flex; align-items: center; gap: 8px;">
                <span style="font-size: 12px; opacity: 0.6"
                  >{model.size_gb} GB</span
                >
                <Button
                  fill
                  small
                  disabled={store.isDownloading}
                  onClick={() => store.downloadModel(model.filename)}
                >
                  {model.is_downloaded ? "Select" : "Download"}
                </Button>
              </div>
            {/snippet}
          </ListItem>
        {/each}
      </List>
      {#if store.isDownloading}
        <Block class="text-align-center">
          <Preloader />
          <p>Downloading model, please wait...</p>
        </Block>
      {/if}
    </Page>
  </Popup>
</Page>
