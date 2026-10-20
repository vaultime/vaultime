// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// Writes the notices of the software and fonts the app bundles, for the
// installers and Settings, About. Run by `npm run dev` and `npm run build`.
//   node scripts/third-party-notices.mjs
// Reads the Rust crates through `cargo metadata`, those every platform build
// links in, and the npm packages the frontend ships from package-lock.json.
// Each license text appears once, with every package that comes with it.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const desktop = join(root, "apps/desktop");
const output = join(desktop, "public/third-party-notices.txt");

// Packages the frontend build tools put parts of into the app: Tailwind's
// base styles and Vite's module loader.
const BUILD_OUTPUT_PACKAGES = ["tailwindcss", "vite"];
const LICENSE_FILE = /^(licen[cs]e|copying|notice)/i;

/** The license files of a package folder, each as trimmed text. */
function licenseTexts(folder) {
  if (!existsSync(folder)) return [];
  return readdirSync(folder, { withFileTypes: true })
    .filter((entry) => entry.isFile() && LICENSE_FILE.test(entry.name))
    .sort((a, b) => a.name.localeCompare(b.name))
    .map((entry) => readFileSync(join(folder, entry.name), "utf8").replace(/\r\n/g, "\n").trim())
    .filter(Boolean);
}

/** The crates the app links in on any platform, without build and dev dependencies. */
function crates() {
  const metadata = JSON.parse(
    execFileSync("cargo", ["metadata", "--format-version", "1", "--locked"], {
      cwd: join(desktop, "src-tauri"),
      encoding: "utf8",
      maxBuffer: 256 * 1024 * 1024,
    }),
  );
  const packages = new Map(metadata.packages.map((pkg) => [pkg.id, pkg]));
  const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
  const seen = new Set();
  const stack = [metadata.resolve.root];
  while (stack.length > 0) {
    const id = stack.pop();
    if (seen.has(id)) continue;
    seen.add(id);
    for (const dep of nodes.get(id).deps) {
      if (dep.dep_kinds.some((kind) => kind.kind === null)) stack.push(dep.pkg);
    }
  }
  seen.delete(metadata.resolve.root);
  return [...seen].map((id) => {
    const pkg = packages.get(id);
    return {
      name: pkg.name,
      version: pkg.version,
      license: pkg.license ?? "see its license file",
      home: pkg.repository ?? pkg.homepage ?? `https://crates.io/crates/${pkg.name}`,
      texts: licenseTexts(dirname(pkg.manifest_path)),
    };
  });
}

/** The npm packages the frontend ships, and the build tools whose output it carries. */
function npmPackages() {
  const lock = JSON.parse(readFileSync(join(desktop, "package-lock.json"), "utf8"));
  const shipped = Object.entries(lock.packages).filter(
    ([path, entry]) =>
      path.startsWith("node_modules/") &&
      (!entry.dev || BUILD_OUTPUT_PACKAGES.includes(path.slice("node_modules/".length))),
  );
  return shipped.map(([path, entry]) => {
    const folder = join(desktop, path);
    const manifest = existsSync(join(folder, "package.json"))
      ? JSON.parse(readFileSync(join(folder, "package.json"), "utf8"))
      : {};
    const repository = typeof manifest.repository === "string" ? manifest.repository : manifest.repository?.url;
    return {
      name: path.slice(path.lastIndexOf("node_modules/") + "node_modules/".length),
      version: entry.version,
      license: entry.license ?? manifest.license ?? "see its license file",
      home: (repository ?? manifest.homepage ?? `https://www.npmjs.com/package/${path.slice("node_modules/".length)}`)
        .replace(/^git\+/, "")
        .replace(/\.git$/, ""),
      texts: licenseTexts(folder),
    };
  });
}

const byName = (a, b) => a.name.localeCompare(b.name) || a.version.localeCompare(b.version);
const groups = [
  ["Fonts and frontend libraries", npmPackages().sort(byName)],
  ["Rust crates", crates().sort(byName)],
];

// Each text once, named by the packages that come with it.
const texts = new Map();
for (const [, packages] of groups) {
  for (const pkg of packages) {
    for (const text of pkg.texts) {
      const key = createHash("sha256").update(text).digest("hex");
      const known = texts.get(key) ?? { text, users: [] };
      known.users.push(`${pkg.name} ${pkg.version}`);
      texts.set(key, known);
    }
  }
}

const lines = [
  "Third-party notices for Vaultime",
  "",
  "Vaultime is free software under the GNU General Public License, version 3",
  "or later. It bundles the fonts and libraries below, each under its own",
  "license. Each package names its license and where it comes from, and the",
  "license texts the packages carry follow after the list. A package without",
  "a text of its own is under the license it names.",
  "",
];
for (const [title, packages] of groups) {
  lines.push(`${title} (${packages.length})`, "");
  for (const pkg of packages) lines.push(`${pkg.name} ${pkg.version}, ${pkg.license}, ${pkg.home}`);
  lines.push("");
}
lines.push(`License texts (${texts.size})`, "");
for (const { text, users } of texts.values()) {
  lines.push("-".repeat(72), `For ${users.join(", ")}:`, "", text, "");
}
writeFileSync(output, `${lines.join("\n").trimEnd()}\n`);
console.log(`third-party notices: ${groups.map(([title, packages]) => `${packages.length} ${title.toLowerCase()}`).join(", ")}, ${texts.size} license texts`);
