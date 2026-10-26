# Desktop app

The desktop app is a window with buttons and text boxes instead of keyboard shortcuts. It uses the same download engine, mirrors and `config.toml` as the terminal app, so settings changed in one show up in the other.

Installers for Windows, macOS (Apple Silicon) and Linux are on the [latest release](https://github.com/DesireThings/osu-collect/releases/latest); see [Installation](Installation#desktop-app). It covers downloading collections, updating the ones you downloaded, and getting them into osu!lazer. **find** is still terminal-only.

The sidebar has three steps, then **Update collections** and Settings. You can go back to any of them at any time.

## 1. Set up

- **Which osu! do you play?** osu!lazer (which covers Torii and other lazer builds) or osu! stable. Your osu! folder is found automatically when it's in the usual place; otherwise use **Choose folder…**. For lazer, that's the folder with `client.realm` in it (in osu!lazer: Settings → Maintenance → Open osu! folder).
- **Where should downloaded maps go?** Each collection gets its own folder inside this one.

**Save and continue** writes both to `config.toml`.

## 2. Add collections

Paste a collection link from osucollector.com (or just its number) and press **Add** or <kbd>Enter</kbd>. The collection is looked up straight away and shows its name, uploader and map count. Add as many as you like.

- **Download** starts one collection; **Download all** starts every collection that isn't downloaded yet. Collections download one at a time, in order; each one already uses every download thread.
- The card shows progress, how many map sets are downloading, and the size so far.
- **waiting for a free mirror**: every mirror that has those map sets is rate-limiting right now. They're tried again on their own (the card counts down), or **Skip these for now** drops them from this run. Skipped sets show as "skipped for now"; press **Check again** later to fetch them.
- **Cancel** stops a download. **Try again** / **Check again** runs the collection again and only downloads what's missing.
- Map sets that couldn't be downloaded are listed under the card, with the reason.

Downloads keep running while you look at other steps.

## 3. Import into osu!

**osu!lazer**: **Prepare import folder** puts every downloaded collection into one folder, so lazer imports all of them in one go:

1. In osu!lazer, open **Settings → General** and click **Run setup wizard**.
2. Click **Next** until the **Import** screen.
3. Click **previous osu! install** and choose the folder the app shows (**Copy** and **Show folder** help here).
4. Leave only **Beatmaps** and **Collections** ticked, then click **Import content from previous version**.

Running **Prepare import folder** again after more downloads only adds what's new. Delete the folder once osu! says the import finished. This is the same folder `osu-collect lazer-import` builds; see [Importing into osu!](Importing-into-osu#several-collections-at-once).

**osu! stable**: not automatic yet. Drag the downloaded `.osz` files into osu!, then merge each folder's `collection.db` with [Collection Manager](https://github.com/Piotrekol/CollectionManager).

## Update collections

Checks the collections you downloaded with osu-collect for maps added on osu!collector since, and downloads only those. It reads the osu! install from **Set up**.

1. **Check for new maps** reads your osu! library, finds the collections osu-collect downloaded (their names end in the osu!collector number, like `Tiebreakers-18951`), and looks each one up on osu!collector.
2. Every collection with something new is ticked. Its card shows how many maps are new, how many it holds in osu!, and how many osu!collector removed. **Show maps** lists the missing map sets; each one opens its osu! page.
3. **Download N map sets** fetches the new maps of the ticked collections, each map set once even if several collections miss it. The download shows the same progress, waiting and skip controls as a collection.
4. Then import as usual (step 3) and **Check again**.

A map set you deleted from osu! after downloading stays left out, so it doesn't come back on its own. Its row in **Show maps** says so; tick **Download it again** to include it in this download.

Map sets that failed to download before are left out too. The terminal app's update view can recheck those, and mark a map you already have as installed when the check keeps calling it missing; see [Updating collections](Updating-Collections). A check doesn't change anything in osu!; it only saves what it saw, so the next check can tell which maps you deleted.

## Settings

Mirror and download changes apply once you press **Save changes**; **Undo changes** puts back what's saved. The API app saves on its own with **Check and save**, after osu! accepts it.

| Card | What's in it |
|---|---|
| osu! API app | Your own osu! OAuth app (client ID and secret), only needed for the terminal app's find. Collections never need it. See [Finding maps](Finding-Maps#your-osu-api-app). |
| Mirrors | Turn the built-in mirrors on or off and change the order they're tried in; add your own mirror links (with `{id}` where the map set number goes). osu!'s own servers aren't offered: they need a login this app doesn't do. |
| Downloads | Background videos, skip maps already in osu!, maps at once (automatic or 1–100), how long a map set waits on a busy mirror before it goes back in the queue to try later, and how strictly downloaded files are checked. |
| About | The app's version, and **Show third-party licences**: the licences of everything built into the app (release builds only). |

The same settings are in `config.toml`; see [Configuration](Configuration).
