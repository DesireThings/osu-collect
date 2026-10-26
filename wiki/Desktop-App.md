# Desktop app

The desktop app is a window with buttons and text boxes instead of keyboard shortcuts. It uses the same download engine, mirrors and `config.toml` as the terminal app, so settings changed in one show up in the other.

It's new and only runs from source for now; see [Installation](Installation#desktop-app-from-source). It covers downloading collections and getting them into osu!lazer. **find** and **update** are still terminal-only.

The sidebar has three steps, plus Settings. You can go back to any step at any time.

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

## Settings

Mirror and download changes apply once you press **Save changes**; **Undo changes** puts back what's saved. The API app saves on its own with **Check and save**, after osu! accepts it.

| Card | What's in it |
|---|---|
| osu! API app | Your own osu! OAuth app (client ID and secret), only needed for the terminal app's find. Collections never need it. See [Finding maps](Finding-Maps#your-osu-api-app). |
| Mirrors | Turn the built-in mirrors on or off and change the order they're tried in; add your own mirror links (with `{id}` where the map set number goes). osu!'s own servers aren't offered: they need a login this app doesn't do. |
| Downloads | Background videos, skip maps already in osu!, maps at once (automatic or 1–100), how long a map set waits on a busy mirror before it goes back in the queue to try later, and how strictly downloaded files are checked. |
| About | The app's version, and **Show third-party licences**: the licences of everything built into the app (release builds only). |

The same settings are in `config.toml`; see [Configuration](Configuration).
