<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import DownloadProgress from "./DownloadProgress.svelte";
  import type { Settings, UpdateCollection } from "./api";
  import {
    cancel,
    downloadPickedUpdates,
    isActive,
    latestUpdateRun,
    pickedSetIds,
    reincludeKey,
    scanForUpdates,
    store,
  } from "./store.svelte";

  let { settings, onImport }: { settings: Settings; onImport: () => void } = $props();

  const u = store.updates;
  /** Collections whose map list is open. */
  let open = $state<number[]>([]);

  const report = $derived(u.report);
  const run = $derived(latestUpdateRun());
  const running = $derived(isActive(run));
  /** This scan's maps are downloaded (or downloading); a new download needs a new check. */
  const spent = $derived(
    u.downloaded && !!run && run.phase !== "failed" && run.phase !== "cancelled",
  );
  const toDownload = $derived(pickedSetIds().size);

  const newCount = (c: UpdateCollection) => c.missing.filter((m) => !m.previously_deleted).length;
  const deletedCount = (c: UpdateCollection) =>
    c.missing.filter((m) => m.previously_deleted).length;
  const withNew = $derived(report ? report.collections.filter((c) => newCount(c) > 0) : []);
  const totalNew = $derived(
    new Set(withNew.flatMap((c) => c.missing.filter((m) => !m.previously_deleted).map((m) => m.beatmapset_id)))
      .size,
  );
  const heldBack = $derived(report ? report.collections.reduce((n, c) => n + deletedCount(c), 0) : 0);

  const clientName = (client: string) => (client === "lazer" ? "osu!lazer" : "osu! stable");
  const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

  function togglePicked(id: number) {
    u.picked = u.picked.includes(id) ? u.picked.filter((p) => p !== id) : [...u.picked, id];
  }

  function toggleReincluded(collectionId: number, setId: number) {
    const key = reincludeKey(collectionId, setId);
    u.reincluded = u.reincluded.includes(key)
      ? u.reincluded.filter((k) => k !== key)
      : [...u.reincluded, key];
    // Picking a deleted set again only means something with its collection ticked.
    if (u.reincluded.includes(key) && !u.picked.includes(collectionId)) togglePicked(collectionId);
  }

  function toggleOpen(id: number) {
    open = open.includes(id) ? open.filter((o) => o !== id) : [...open, id];
  }

  /** Collections that can be ticked: something new, or a deleted set picked again. */
  const pickable = (c: UpdateCollection) =>
    newCount(c) > 0 ||
    c.missing.some((m) => u.reincluded.includes(reincludeKey(c.id, m.beatmapset_id)));

  function pickAll(on: boolean) {
    u.picked = on ? report!.collections.filter(pickable).map((c) => c.id) : [];
  }

  const progressText = $derived.by(() => {
    const p = u.progress;
    if (!p || p.step === "reading_library") return "Reading your osu! library…";
    return `Found ${p.local_sets.toLocaleString()} map sets. Checking ${plural(p.collections, "collection", "collections")} on osu!collector…`;
  });
</script>

<header class="page-head">
  <h1>Update collections</h1>
  <p class="muted">
    Check the collections you downloaded with osu-shelf for maps added on osu!collector since, and
    download only what's new.
  </p>
</header>

<section class="card">
  <div class="scan-row">
    <div class="scan-text">
      <span>{clientName(report?.client ?? settings.client)}</span>
      <span class="muted small">
        {#if report}
          {report.library} · {report.local_sets.toLocaleString()} map sets
        {:else}
          {settings.osu_path ?? "Found automatically"} · change it in Set up
        {/if}
      </span>
    </div>
    <button class="primary" onclick={scanForUpdates} disabled={u.scanning}>
      {u.scanning ? "Checking…" : report ? "Check again" : "Check for new maps"}
    </button>
  </div>
  {#if u.scanning}
    <div class="bar indeterminate"><span></span></div>
    <p class="muted small">{progressText}</p>
  {/if}
  {#if u.error}
    <p class="error-text small">{u.error}</p>
  {/if}
</section>

{#if report && !u.scanning}
  {#if report.collections.length === 0}
    <section class="card">
      <h2>No downloaded collections found</h2>
      <p>
        osu-shelf looks for collections whose name ends in their osu!collector number, the way it
        names every download (for example <code>Tiebreakers-18951</code>). None of the collections in
        your {clientName(report.client)} have one.
      </p>
      <p class="muted small">
        Downloaded but not imported yet? Import first, then check again. Renamed a collection? Put
        <code>-</code> and its number back at the end of the name.
      </p>
    </section>
  {:else}
    <section class="card summary">
      {#if totalNew > 0}
        <h2>
          {plural(totalNew, "new map set", "new map sets")} in {plural(withNew.length, "collection", "collections")}
        </h2>
      {:else}
        <h2>Everything's up to date</h2>
      {/if}
      <p class="muted small">
        Checked {plural(report.collections.length, "collection", "collections")}.
        {#if heldBack > 0}
          {plural(heldBack, "map set you deleted is", "map sets you deleted are")} left out; open a
          collection's list to get one again.
        {/if}
        {#if report.known_bad > 0}
          {plural(report.known_bad, "map set", "map sets")} that failed to download before
          {report.known_bad === 1 ? "is" : "are"} left out.
        {/if}
      </p>
    </section>

    {#if !spent}
      <div class="actions left list-actions">
        <button class="secondary" onclick={() => pickAll(true)}>Tick all</button>
        <button class="secondary" onclick={() => pickAll(false)}>Untick all</button>
      </div>
    {/if}

    <ul class="entry-list">
      {#each report.collections as collection (collection.id)}
        {@const fresh = newCount(collection)}
        {@const deleted = deletedCount(collection)}
        <li class="card entry-card">
          <div class="entry">
            <label class="pick" class:inert={!pickable(collection)}>
              <input
                type="checkbox"
                checked={u.picked.includes(collection.id)}
                disabled={!pickable(collection) || spent}
                onchange={() => togglePicked(collection.id)}
              />
              <span class="entry-main">
                <span class="entry-title">{collection.name}</span>
                <span class="muted small">
                  {fresh > 0 ? `${fresh} new` : "Nothing new"}
                  · {plural(collection.local_maps, "map", "maps")} in osu!
                  {#if deleted > 0}· {deleted} you deleted{/if}
                  {#if collection.removed_upstream > 0}
                    · {collection.removed_upstream} removed on osu!collector
                  {/if}
                </span>
              </span>
            </label>
            {#if collection.missing.length > 0}
              <div class="entry-buttons">
                <button class="secondary" onclick={() => toggleOpen(collection.id)}>
                  {open.includes(collection.id) ? "Hide maps" : "Show maps"}
                </button>
              </div>
            {/if}
          </div>

          {#if open.includes(collection.id)}
            <ul class="sets small">
              {#each collection.missing as set (set.beatmapset_id)}
                <li>
                  <button
                    class="link"
                    onclick={() => openUrl(`https://osu.ppy.sh/beatmapsets/${set.beatmapset_id}`)}
                  >
                    Map set {set.beatmapset_id}
                  </button>
                  {#if set.previously_deleted}
                    <label class="again">
                      <input
                        type="checkbox"
                        checked={u.reincluded.includes(reincludeKey(collection.id, set.beatmapset_id))}
                        disabled={spent}
                        onchange={() => toggleReincluded(collection.id, set.beatmapset_id)}
                      />
                      <span class="muted">You deleted this one. Download it again</span>
                    </label>
                  {:else}
                    <span class="muted">new</span>
                  {/if}
                </li>
              {/each}
            </ul>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
{/if}

{#if run && (running || u.downloaded)}
  <section class="card">
    <div class="entry">
      <div class="entry-main">
        <span class="entry-title">{run.name}</span>
      </div>
      {#if running}
        <div class="entry-buttons">
          <button class="secondary" onclick={() => cancel(run)}>Cancel</button>
        </div>
      {/if}
    </div>
    <DownloadProgress status={run} />
    {#if run.phase === "done"}
      <p class="muted small">
        Import to get the new maps into osu!, then check again to see the collections up to date.
      </p>
      <div class="actions left">
        <button class="primary" onclick={onImport}>Continue to import</button>
      </div>
    {/if}
  </section>
{/if}

{#if report && report.collections.length > 0 && !u.scanning && !spent}
  <div class="actions">
    {#if u.downloadError}
      <p class="error-text small">{u.downloadError}</p>
    {/if}
    <button class="primary" onclick={downloadPickedUpdates} disabled={toDownload === 0 || running}>
      Download {plural(toDownload, "map set", "map sets")}
    </button>
  </div>
{/if}

<style>
  .scan-row {
    display: flex;
    align-items: center;
    gap: 1rem;
  }
  .scan-text {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .summary h2 {
    margin-bottom: 0.25rem;
  }
  .pick {
    flex: 1;
    display: flex;
    align-items: flex-start;
    gap: 0.6rem;
    cursor: pointer;
    min-width: 0;
  }
  .pick input {
    margin-top: 0.3rem;
  }
  .pick.inert {
    cursor: default;
    opacity: 0.65;
  }
  .sets {
    list-style: none;
    margin: 0.75rem 0 0;
    padding: 0.5rem 0 0;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    max-height: 18rem;
    overflow-y: auto;
  }
  .sets li {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
  }
  .again {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    cursor: pointer;
  }
</style>
