<script lang="ts">
  import DownloadProgress from "./DownloadProgress.svelte";
  import {
    addCollection,
    cancel,
    download,
    downloadFor,
    isActive,
    removeEntry,
    store,
  } from "./store.svelte";

  let { onNext }: { onNext: () => void } = $props();

  let link = $state("");

  function add() {
    const input = link.trim();
    if (!input) return;
    link = "";
    addCollection(input);
  }

  const ready = $derived(store.entries.filter((e) => e.state === "ready"));
  /** Ready collections not downloaded yet (or whose last try didn't finish). */
  const notStarted = $derived(
    ready.filter((e) => {
      const status = downloadFor(e.preview.id);
      return !status || status.phase === "cancelled" || status.phase === "failed";
    }),
  );
  const anyActive = $derived(ready.some((e) => isActive(downloadFor(e.preview.id))));
  const allDone = $derived(
    ready.length > 0 && ready.every((e) => downloadFor(e.preview.id)?.phase === "done"),
  );

  function downloadAll() {
    for (const entry of notStarted) download(entry);
  }
</script>

<header class="page-head">
  <h1>Add collections</h1>
  <p class="muted">
    Copy a collection's link from osucollector.com and paste it below. Add as many as you like,
    then download them all.
  </p>
</header>

<form
  class="card add-row"
  onsubmit={(e) => {
    e.preventDefault();
    add();
  }}
>
  <input
    class="text-input"
    bind:value={link}
    placeholder="https://osucollector.com/collections/12345"
    aria-label="osu!collector link"
  />
  <button class="primary" type="submit" disabled={!link.trim()}>Add</button>
</form>

{#if store.entries.length === 0}
  <p class="muted empty">No collections yet. Paste a link above to start.</p>
{:else}
  {#if notStarted.length > 1}
    <div class="actions left list-actions">
      <button class="primary" onclick={downloadAll}>Download all {notStarted.length}</button>
    </div>
  {/if}

  <ul class="entry-list">
    {#each store.entries as entry (entry.key)}
      {@const status = entry.state === "ready" ? downloadFor(entry.preview.id) : undefined}
      <li class="card entry-card" class:entry-error={entry.state === "error"}>
        <div class="entry">
          {#if entry.state === "loading"}
            <div class="entry-main">
              <span class="entry-title muted">Looking up {entry.input}…</span>
            </div>
          {:else if entry.state === "ready"}
            <div class="entry-main">
              <span class="entry-title">{entry.preview.name}</span>
              <span class="muted small">
                by {entry.preview.uploader} · {entry.preview.beatmapsets} map sets ·
                {entry.preview.beatmaps} difficulties
              </span>
            </div>
          {:else}
            <div class="entry-main">
              <span class="entry-title">Couldn't add {entry.input}</span>
              <span class="error-text small">{entry.message}</span>
            </div>
          {/if}

          <div class="entry-buttons">
            {#if entry.state === "ready"}
              {#if isActive(status)}
                <button class="secondary" onclick={() => cancel(status!)}>Cancel</button>
              {:else if status?.phase === "done"}
                <button class="secondary" onclick={() => download(entry)}>Check again</button>
              {:else}
                <button class="primary" onclick={() => download(entry)}>
                  {status ? "Try again" : "Download"}
                </button>
              {/if}
            {/if}
            {#if !isActive(status)}
              <button class="secondary" onclick={() => removeEntry(entry.key)}>Remove</button>
            {/if}
          </div>
        </div>

        {#if store.startErrors[entry.key]}
          <p class="error-text small">{store.startErrors[entry.key]}</p>
        {/if}
        {#if status}
          <DownloadProgress {status} />
        {/if}
      </li>
    {/each}
  </ul>
{/if}

{#if allDone}
  <section class="card success">
    <h2>All downloads finished</h2>
    <p>Next, get them into osu!: one step for every collection at once.</p>
    <div class="actions left">
      <button class="primary" onclick={onNext}>Continue to import</button>
    </div>
  </section>
{:else}
  <div class="actions">
    <button class="secondary" onclick={onNext}>
      {anyActive ? "Go to import (downloads keep running)" : "Continue to import"}
    </button>
  </div>
{/if}
