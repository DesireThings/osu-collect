<script lang="ts">
  import { onMount } from "svelte";
  import Setup from "./lib/Setup.svelte";
  import Collections from "./lib/Collections.svelte";
  import Import from "./lib/Import.svelte";
  import SettingsPage from "./lib/Settings.svelte";
  import { detectInstalls, loadSettings, type Installs, type Settings } from "./lib/api";
  import { initDownloads, store } from "./lib/store.svelte";

  type Page = "setup" | "collections" | "import" | "settings";

  const steps: { id: Page; label: string; hint: string }[] = [
    { id: "setup", label: "Set up", hint: "Your osu! and a download folder" },
    { id: "collections", label: "Add collections", hint: "Paste osu!collector links" },
    { id: "import", label: "Import into osu!", hint: "Get everything into the game" },
  ];

  let page = $state<Page>("setup");
  let settings = $state<Settings | null>(null);
  let installs = $state<Installs | null>(null);
  let loadError = $state<string | null>(null);

  /** Sidebar hint for step 2 while something downloads, so it shows from any step. */
  const downloadingHint = $derived.by(() => {
    const running = Object.values(store.downloads).find(
      (s) => s.phase === "downloading" || s.phase === "preparing",
    );
    if (!running) return null;
    if (running.phase === "preparing") return `Getting ready: ${running.name}`;
    const settled = running.downloaded + running.skipped + running.failed;
    return `Downloading ${settled}/${running.total}: ${running.name}`;
  });

  onMount(async () => {
    try {
      [settings, installs] = await Promise.all([
        loadSettings(),
        detectInstalls(),
        initDownloads(),
      ]);
    } catch (e) {
      loadError = String(e);
    }
  });
</script>

<div class="shell">
  <nav class="sidebar">
    <div class="brand">osu-<span>collect</span></div>
    <ol class="steps">
      {#each steps as step, i (step.id)}
        <li>
          <button
            class="step"
            class:active={page === step.id}
            onclick={() => (page = step.id)}
          >
            <span class="num">{i + 1}</span>
            <span class="text">
              <span class="label">{step.label}</span>
              {#if step.id === "collections" && downloadingHint}
                <span class="hint live">{downloadingHint}</span>
              {:else}
                <span class="hint">{step.hint}</span>
              {/if}
            </span>
          </button>
        </li>
      {/each}
    </ol>
    <button
      class="step settings-link"
      class:active={page === "settings"}
      onclick={() => (page = "settings")}
    >
      <span class="num" aria-hidden="true">⚙</span>
      <span class="text">
        <span class="label">Settings</span>
        <span class="hint">Mirrors, downloads, osu! API app</span>
      </span>
    </button>
    <p class="unofficial">Unofficial tool, not made by ppy (osu!) or osu!Collector.</p>
  </nav>

  <main class="content">
    {#if loadError}
      <div class="card error">
        <h2>Couldn't start</h2>
        <p>{loadError}</p>
      </div>
    {:else if !settings || !installs}
      <p class="muted">Loading…</p>
    {:else if page === "setup"}
      <Setup bind:settings {installs} onDone={() => (page = "collections")} />
    {:else if page === "collections"}
      <Collections onNext={() => (page = "import")} />
    {:else if page === "import"}
      <Import {settings} />
    {:else}
      <SettingsPage />
    {/if}
  </main>
</div>
