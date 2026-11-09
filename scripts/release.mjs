#!/usr/bin/env node
// Bumps the version everywhere, commits "Release x.y.z", tags vx.y.z, pushes,
// and starts the Release workflow, which builds the installers and creates a
// draft GitHub release with them attached.
//
//   npm run release -- patch                      0.3.0 -> 0.3.1, all platforms
//   npm run release -- minor --platforms windows  only the Windows installer
//   npm run release -- 1.0.0 --platforms windows,linux
//   npm run release -- patch --dry-run            show what would happen
//
// Platforms: windows, linux, macos (default: all).

import { execSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const PLATFORMS = ["windows", "linux", "macos"];
const FILES = {
  tauri: "src/core/tauri.conf.json",
  cargo: "src/core/Cargo.toml",
  lock: "src/core/Cargo.lock",
};

const out = (cmd) => execSync(cmd, { encoding: "utf8" }).trim();
const run = (cmd) => execSync(cmd, { stdio: "inherit" });
const fail = (msg) => {
  console.error(`release: ${msg}`);
  process.exit(1);
};

// --- arguments ------------------------------------------------------------

const args = process.argv.slice(2);
const dryRun = args.includes("--dry-run");
const platformsArg = args.includes("--platforms") ? args[args.indexOf("--platforms") + 1] : "all";
const bump = args.find((a, i) => !a.startsWith("--") && args[i - 1] !== "--platforms");

if (!bump) fail("usage: npm run release -- <patch|minor|major|x.y.z> [--platforms windows,linux,macos] [--dry-run]");

const platforms = platformsArg === "all" ? PLATFORMS : platformsArg.split(",").map((p) => p.trim().toLowerCase());
const unknown = platforms.filter((p) => !PLATFORMS.includes(p));
if (unknown.length) fail(`unknown platform(s): ${unknown.join(", ")} (use ${PLATFORMS.join(", ")})`);

const current = JSON.parse(readFileSync("package.json", "utf8")).version;
const next = nextVersion(current, bump);
const tag = `v${next}`;

function nextVersion(version, kind) {
  if (/^\d+\.\d+\.\d+$/.test(kind)) return kind;
  const [major, minor, patch] = version.split(".").map(Number);
  if (kind === "major") return `${major + 1}.0.0`;
  if (kind === "minor") return `${major}.${minor + 1}.0`;
  if (kind === "patch") return `${major}.${minor}.${patch + 1}`;
  fail(`"${kind}" isn't patch, minor, major or a version like 1.2.3`);
}

// --- checks ---------------------------------------------------------------

if (out("git rev-parse --abbrev-ref HEAD") !== "main") fail("run it from the main branch");
if (out("git status --porcelain")) fail("commit or stash your changes first");
if (out(`git tag --list ${tag}`)) fail(`tag ${tag} already exists`);

console.log(`Release ${current} -> ${next} (${tag}) for ${platforms.join(", ")}`);
if (dryRun) {
  console.log("Dry run: nothing changed.");
  process.exit(0);
}

// --- bump -----------------------------------------------------------------

// package.json and package-lock.json
run(`npm version ${next} --no-git-tag-version`);

const replace = (file, pattern, replacement) => {
  const text = readFileSync(file, "utf8");
  const updated = text.replace(pattern, replacement);
  if (updated === text) fail(`couldn't find the version in ${file}`);
  writeFileSync(file, updated);
};
replace(FILES.tauri, /"version": "[^"]+"/, `"version": "${next}"`);
// The first `version =` in Cargo.toml is the [package] one.
replace(FILES.cargo, /^version = "[^"]+"/m, `version = "${next}"`);
replace(FILES.lock, /(name = "pulsar"\r?\nversion = )"[^"]+"/, `$1"${next}"`);

// --- commit, tag, push, build ---------------------------------------------

run(`git add package.json package-lock.json ${Object.values(FILES).join(" ")}`);
run(`git commit -m "Release ${next}"`);
run(`git tag -a ${tag} -m "Release ${next}"`);
run("git push origin main");
run(`git push origin ${tag}`);
run(`gh workflow run release.yml -f tag=${tag} -f platforms=${platforms.join(",")}`);

console.log(`\nBuilding ${tag}. The draft release shows up when the build finishes:`);
console.log(`  ${out("gh repo view --json url --jq .url")}/releases`);
console.log(`Publish it with: gh release edit ${tag} --draft=false`);
