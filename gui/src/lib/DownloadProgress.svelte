<script lang="ts">
  import type { DownloadStatus } from "./api";

  let { status }: { status: DownloadStatus } = $props();

  let showFailures = $state(false);

  const settled = $derived(status.downloaded + status.skipped + status.failed);
  const percent = $derived(status.total > 0 ? Math.min(100, (settled / status.total) * 100) : 0);

  function size(bytes: number): string {
    if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
    if (bytes >= 1e6) return `${Math.round(bytes / 1e6)} MB`;
    return `${Math.round(bytes / 1e3)} KB`;
  }

  const summary = $derived.by(() => {
    const parts = [`${status.downloaded} new`];
    if (status.skipped > 0) parts.push(`${status.skipped} you already had`);
    if (status.failed > 0) parts.push(`${status.failed} couldn't be downloaded`);
    return parts.join(" · ");
  });
</script>

<div class="progress" data-phase={status.phase}>
  {#if status.phase === "queued"}
    <p class="muted small">Waiting for the download above to finish…</p>
  {:else if status.phase === "preparing"}
    <div class="bar indeterminate"><span></span></div>
    <p class="muted small">Getting ready: checking which maps you already have…</p>
  {:else if status.phase === "downloading"}
    <div class="bar"><span style:width="{percent}%"></span></div>
    <p class="small">
      {settled} of {status.total} map sets
      {#if status.active > 0}· {status.active} downloading now{/if}
      {#if status.bytes_done > 0}· {size(status.bytes_done)}{/if}
    </p>
  {:else if status.phase === "done"}
    <div class="bar done"><span style:width="100%"></span></div>
    <p class="small {status.failed > 0 ? 'warn-text' : 'ok'}">Done: {summary}</p>
  {:else if status.phase === "cancelled"}
    <p class="muted small">Cancelled. {status.downloaded} map sets were downloaded before stopping.</p>
  {:else}
    <p class="error-text small">The download failed: {status.error ?? "unknown error"}</p>
  {/if}

  {#if status.warning}
    <p class="warn-text small">{status.warning}</p>
  {/if}

  {#if status.failures.length > 0}
    <button class="link" onclick={() => (showFailures = !showFailures)}>
      {showFailures ? "Hide" : "Show"} the {status.failures.length} that couldn't be downloaded
    </button>
    {#if showFailures}
      <ul class="failures small">
        {#each status.failures as failure (failure.beatmapset_id)}
          <li>
            {failure.title ?? `Map set ${failure.beatmapset_id}`}
            <span class="muted">({failure.reason})</span>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</div>
