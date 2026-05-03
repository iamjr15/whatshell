#!/usr/bin/env node
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const { installAgentSkills } = require("./install-skills");

const root = path.resolve(__dirname, "..");
const exe = process.platform === "win32" ? ".exe" : "";
const source = path.join(root, "target", "release", `whatshell${exe}`);
const dest = path.join(root, "npm", "bin", `whatshell-${process.platform}-${process.arch}${exe}`);

if (process.env.WHATSHELL_SKIP_BUILD !== "1") {
  const cargo = spawnSync("cargo", ["build", "--release"], {
    cwd: root,
    stdio: "inherit",
    env: process.env
  });

  if (cargo.error) {
    console.error("Failed to run cargo. Install Rust from https://rustup.rs or install with cargo directly.");
    console.error(cargo.error.message);
    process.exit(1);
  }
  if (cargo.status !== 0) {
    process.exit(cargo.status ?? 1);
  }

  fs.copyFileSync(source, dest);
  fs.chmodSync(dest, 0o755);
}

try {
  installAgentSkills({ root });
} catch (error) {
  console.error(`Failed to install Whatshell agent skills: ${error.message}`);
  process.exit(1);
}
