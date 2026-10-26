// Prints the licence section for the JavaScript packages bundled into the
// desktop app's window, for scripts/third-party-licenses.sh --desktop.
//
// Which packages count comes from the bundle itself: the script reads the
// source maps of a `vite build --sourcemap` and keeps every package that
// contributed a module, so it never lists a dev tool that isn't shipped.
//
// usage: node scripts/js-bundle-licenses.mjs <gui dir> <source-map build dir>
import { readFileSync, readdirSync, existsSync } from "node:fs";
import { join } from "node:path";

const [guiDir, mapDir] = process.argv.slice(2);
if (!guiDir || !mapDir) {
  console.error("usage: js-bundle-licenses.mjs <gui dir> <source-map build dir>");
  process.exit(1);
}

// pnpm paths look like node_modules/.pnpm/<id>/node_modules/<name>/...;
// the last node_modules/<name> (or @scope/<name>) is the package.
const PACKAGE = /node_modules\/((?:@[^/]+\/)?[^/]+)\/(?!.*node_modules\/)/;

function bundledPackages() {
  const assets = join(mapDir, "assets");
  const names = new Set();
  for (const file of readdirSync(assets).filter((f) => f.endsWith(".map"))) {
    const { sources } = JSON.parse(readFileSync(join(assets, file), "utf8"));
    for (const source of sources) {
      const match = PACKAGE.exec(source);
      if (match) names.add(match[1]);
    }
  }
  if (names.size === 0) {
    throw new Error(`no packages found in the source maps under ${assets}`);
  }
  return [...names].sort();
}

// Where a package offers a choice, list MIT, matching about.toml's order.
function pickLicence(spdx) {
  const options = spdx.replace(/[()]/g, "").split(/\s+OR\s+/);
  return options.includes("MIT") ? "MIT" : spdx;
}

const LICENCE_FILE = /^(licen[cs]e|copying)/i;

function licenceText(dir, chosen) {
  const files = readdirSync(dir).filter((f) => LICENCE_FILE.test(f) && !f.endsWith(".spdx"));
  // With one file per licence (LICENSE-MIT, LICENSE-APACHE-2.0), take the
  // chosen one; a single LICENSE file is the text whatever its name says.
  const file =
    files.find((f) => f.toUpperCase().includes(chosen.toUpperCase())) ??
    (files.length === 1 ? files[0] : undefined);
  if (file) return readFileSync(join(dir, file), "utf8").trim();

  // No licence text at all, only an SPDX file (the Tauri plugin packages).
  // MIT is the same text for everyone, so it can be written out with the
  // copyright line the package declares; anything else has to be added by hand.
  const spdxFile = join(dir, "LICENSE.spdx");
  const copyright =
    existsSync(spdxFile) &&
    /^PackageCopyrightText:\s*(.+)$/m.exec(readFileSync(spdxFile, "utf8"))?.[1];
  if (chosen === "MIT" && copyright) return mitText(`Copyright (c) ${copyright.trim()}`);
  throw new Error(`no licence text in ${dir}`);
}

const mitText = (copyright) => `MIT License

${copyright}

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.`;

const rule = "=".repeat(80);
const out = [
  "JavaScript packages",
  "-------------------",
  "",
  "Bundled into the desktop app's window. Source code: https://www.npmjs.com/package/<name>",
];
for (const name of bundledPackages()) {
  const dir = join(guiDir, "node_modules", name);
  const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
  const chosen = pickLicence(pkg.license);
  const offered = chosen === pkg.license ? "" : ` (offered as ${pkg.license})`;
  out.push("", rule, `${name} ${pkg.version}: ${chosen}${offered}`, rule, "");
  out.push(licenceText(dir, chosen));
}
console.log(out.join("\n"));
