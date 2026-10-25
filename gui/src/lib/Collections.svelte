<script lang="ts">
  import { previewCollection, type CollectionPreview } from "./api";

  let { onNext }: { onNext: () => void } = $props();

  type Entry =
    | { key: number; input: string; state: "loading" }
    | { key: number; input: string; state: "ready"; preview: CollectionPreview }
    | { key: number; input: string; state: "error"; message: string };

  let link = $state("");
  let entries = $state<Entry[]>([]);
  let nextKey = 0;

  async function add() {
    const input = link.trim();
    if (!input) return;
    link = "";
    const key = nextKey++;
    entries.push({ key, input, state: "loading" });
    let updated: Entry;
    try {
      const preview = await previewCollection(input);
      const duplicate = entries.some((e) => e.state === "ready" && e.preview.id === preview.id);
      updated = duplicate
        ? { key, input, state: "error", message: `"${preview.name}" is already in the list.` }
        : { key, input, state: "ready", preview };
    } catch (e) {
      updated = { key, input, state: "error", message: String(e) };
    }
    const index = entries.findIndex((e) => e.key === key);
    if (index !== -1) entries[index] = updated;
  }

  function remove(key: number) {
    entries = entries.filter((e) => e.key !== key);
  }
</script>

<header class="page-head">
  <h1>Add collections</h1>
  <p class="muted">
    Copy a collection's link from osucollector.com and paste it below. Add as many as you like.
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

{#if entries.length === 0}
  <p class="muted empty">No collections yet. Paste a link above to start.</p>
{:else}
  <ul class="entry-list">
    {#each entries as entry (entry.key)}
      <li class="card entry" class:entry-error={entry.state === "error"}>
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
        <button class="secondary" onclick={() => remove(entry.key)}>Remove</button>
      </li>
    {/each}
  </ul>
{/if}

<p class="note">
  Downloading from this window is the next thing being built. Until then, download with the
  terminal version, then use step 3 to import everything at once.
</p>

<div class="actions">
  <button class="primary" onclick={onNext}>Continue to import</button>
</div>
