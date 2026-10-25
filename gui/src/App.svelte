<script lang="ts">
  import { onMount } from "svelte";
  import Setup from "./lib/Setup.svelte";
  import Collections from "./lib/Collections.svelte";
  import Import from "./lib/Import.svelte";
  import { detectInstalls, loadSettings, type Installs, type Settings } from "./lib/api";

  type Page = "setup" | "collections" | "import";

  const steps: { id: Page; label: string; hint: string }[] = [
    { id: "setup", label: "Set up", hint: "Your osu! and a download folder" },
    { id: "collections", label: "Add collections", hint: "Paste osu!collector links" },
    { id: "import", label: "Import into osu!", hint: "Get everything into the game" },
  ];

  let page = $state<Page>("setup");
  let settings = $state<Settings | null>(null);
  let installs = $state<Installs | null>(null);
  let loadError = $state<string | null>(null);

  onMount(async () => {
    try {
      [settings, installs] = await Promise.all([loadSettings(), detectInstalls()]);
    } catch (e) {
      loadError = String(e);
    }
  });
</script>

<div class="shell">
  <nav class="sidebar">
    <div class="brand">osu!<span>collect</span></div>
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
              <span class="hint">{step.hint}</span>
            </span>
          </button>
        </li>
      {/each}
    </ol>
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
    {:else}
      <Import {settings} />
    {/if}
  </main>
</div>
