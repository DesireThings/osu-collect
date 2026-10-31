# Mirrors

Downloads rotate across every enabled mirror.

## Built-in mirrors

| Mirror | Host | Default | Notes |
|---|---|---|---|
| osu!direct | `osu.direct` | on | |
| Nerinyan | `api.nerinyan.moe` | on | |
| Sayobot | `dl.sayobot.cn` | on | |
| Nekoha | `mirror.nekoha.moe` | on | Serves the size estimates for find results |
| Beatconnect | `beatconnect.io` | on | |
| osu!dl | `osudl.org` | on | Covers ranked / approved / loved sets only; rotation backfills the rest |
| catboy.best | `catboy.best` | on | |
| Hinamizawa | `mirror.hinamizawa.ai` | off | Races the other mirrors server-side, enable if you're hitting rate limits |
| nzbasic CDN | `direct.nzbasic.com` | off | A solo developer's free instance; coverage is what its backend has cached |

## Managing mirrors

The config tab is the mirror editor: toggle each built-in on or off, reorder the try-order with <kbd>⇧</kbd>+<kbd>↑</kbd>/<kbd>↓</kbd>, or press <kbd>r</kbd> to re-probe latency (shown per mirror next to each row). The get maps tab shows the enabled count with the latency range.

## Rate-limit handling

- Requests to each mirror are spaced out; the spacing widens when a mirror pushes back, then decays once it calms down.
- A throttled mirror sits out its cooldown while the rest keep downloading. Per-map countdowns show in the open run.
- A throttled map goes back in the queue rather than getting dropped; one that stays throttled past the auto-defer delay (60s of actual waiting by default, `download.rate_limit_skip_secs`) re-queues on its own.
- In an open run, <kbd>s</kbd> defers the currently stuck maps yourself; <kbd>S</kbd> drops them for the rest of the run.

## Custom mirrors

Add your own on the config tab: each URL template must contain the `{id}` placeholder (for example `https://example.com/d/{id}`). A new empty row appears as you type; clearing a row removes it. Custom mirrors are tried after the built-ins, in list order. The matching config key is `mirror.urls` ([Configuration](Configuration#mirror)).

## osu! official

osu-shelf doesn't download from osu!'s own servers and never asks for your osu! login. osu! only lets its own lazer client download beatmaps, and osu! hasn't given other apps permission to sign in as that client. Earlier versions did; if one of them left a login behind, it is deleted on the next start.
