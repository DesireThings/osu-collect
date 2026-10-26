<script lang="ts">
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { buildLazerImport, type LazerImportResult, type Settings } from "./api";
  import { isActive, store } from "./store.svelte";

  let { settings }: { settings: Settings } = $props();

  let building = $state(false);
  let result = $state<LazerImportResult | null>(null);
  let error = $state<string | null>(null);
  let copied = $state(false);

  async function build() {
    building = true;
    error = null;
    result = null;
    try {
      result = await buildLazerImport(settings.download_dir);
    } catch (e) {
      error = String(e);
    } finally {
      building = false;
    }
  }

  async function copyPath(path: string) {
    await navigator.clipboard.writeText(path);
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  const downloading = $derived(Object.values(store.downloads).some(isActive));
  const sets = $derived(result ? result.report.unpacked + result.report.already_unpacked : 0);
</script>

<header class="page-head">
  <h1>Import into osu!</h1>
</header>

{#if settings.client === "stable"}
  <section class="card">
    <h2>osu! stable</h2>
    <p>
      Adding maps and collections to stable automatically is coming. For now, drag the downloaded
      <code>.osz</code> files into osu!, and merge each folder's <code>collection.db</code> with
      Collection Manager.
    </p>
  </section>
{:else}
  <section class="card">
    <h2>Get every collection into osu!lazer in one go</h2>
    <p>
      osu-collect puts all your downloaded maps and collections into one folder. osu!lazer then
      imports that folder in a single step, however many collections you downloaded.
    </p>
    <p class="muted small">
      Looks in <code>{settings.download_dir}</code>. Running it again later only adds what's new.
    </p>
    {#if downloading}
      <p class="warn-text small">
        Some collections are still downloading. Wait for them to finish, or run this again
        afterwards to add the rest.
      </p>
    {/if}
    <div class="actions left">
      <button class="primary" onclick={build} disabled={building}>
        {building ? "Preparing… (big downloads take a while)" : "Prepare import folder"}
      </button>
    </div>
    {#if error}
      <p class="error-text">{error}</p>
    {/if}
  </section>

  {#if result}
    {#if result.report.runs === 0}
      <section class="card">
        <h2>Nothing to import yet</h2>
        <p>No downloaded collections were found in your download folder.</p>
      </section>
    {:else}
      <section class="card success">
        <h2>Ready: {sets} map sets and {result.report.collections} collections</h2>
        {#if result.report.failed.length > 0}
          <p class="warn-text small">
            {result.report.failed.length} download(s) were broken and skipped:
            {result.report.failed.map(([path]) => path.split(/[\\/]/).pop()).join(", ")}
          </p>
        {/if}

        <ol class="howto">
          <li>
            Open osu!lazer, go to <b>Settings → General</b> and click <b>Run setup wizard</b>.
          </li>
          <li>Click <b>Next</b> until you reach the <b>Import</b> screen.</li>
          <li>
            Click <b>previous osu! install</b> and choose this folder:
            <div class="path-row">
              <code class="path">{result.folder}</code>
              <button class="secondary" onclick={() => copyPath(result!.folder)}>
                {copied ? "Copied" : "Copy"}
              </button>
              <button class="secondary" onclick={() => revealItemInDir(result!.folder)}>
                Show folder
              </button>
            </div>
          </li>
          <li>
            Leave only <b>Beatmaps</b> and <b>Collections</b> ticked, then click
            <b>Import content from previous version</b>.
          </li>
        </ol>
        <p class="muted small">
          When osu! says the import finished, you can delete the folder to save space.
        </p>
      </section>
    {/if}
  {/if}
{/if}
