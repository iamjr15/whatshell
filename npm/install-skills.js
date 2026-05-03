const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

function boolEnv(name) {
  return process.env[name] === "1" || process.env[name] === "true";
}

function skillHome() {
  return process.env.WHATSHELL_SKILL_HOME || os.homedir();
}

function copyDir(source, destination) {
  const parent = path.dirname(destination);
  const temporary = path.join(parent, `.whatshell-${process.pid}-${Date.now()}.tmp`);

  fs.mkdirSync(parent, { recursive: true });
  fs.rmSync(temporary, { recursive: true, force: true });
  fs.cpSync(source, temporary, {
    recursive: true,
    dereference: true,
    errorOnExist: false,
    force: true
  });
  fs.rmSync(destination, { recursive: true, force: true });
  fs.renameSync(temporary, destination);
}

function installAgentSkills(options = {}) {
  if (boolEnv("WHATSHELL_SKIP_SKILL_INSTALL")) {
    return [];
  }

  const root = options.root || path.resolve(__dirname, "..");
  const source = path.join(root, "skills", "whatshell");
  const entrypoint = path.join(source, "SKILL.md");
  if (!fs.existsSync(entrypoint)) {
    throw new Error(`missing skill entrypoint at ${entrypoint}`);
  }

  const home = skillHome();
  if (!home) {
    throw new Error("could not determine a home directory for global agent skills");
  }

  const targets = [
    {
      label: "Claude Code",
      path: path.join(home, ".claude", "skills", "whatshell")
    },
    {
      label: "Agent Skills",
      path: path.join(home, ".agents", "skills", "whatshell")
    }
  ];

  if (boolEnv("WHATSHELL_INSTALL_CODEX_HOME_SKILL")) {
    const codexHome = process.env.CODEX_HOME || path.join(home, ".codex");
    targets.push({
      label: "Codex home",
      path: path.join(codexHome, "skills", "whatshell")
    });
  }

  if (boolEnv("WHATSHELL_INSTALL_OPENCODE_SKILL")) {
    const opencodeConfig = process.env.OPENCODE_CONFIG_DIR || path.join(home, ".config", "opencode");
    targets.push({
      label: "OpenCode native",
      path: path.join(opencodeConfig, "skills", "whatshell")
    });
  }

  const installed = [];
  for (const target of targets) {
    copyDir(source, target.path);
    installed.push(target);
  }

  if (!boolEnv("WHATSHELL_SKILL_INSTALL_QUIET") && !options.quiet) {
    const names = installed.map((target) => target.label).join(", ");
    console.log(`Installed Whatshell agent skill for: ${names}`);
  }

  return installed;
}

module.exports = { installAgentSkills };

if (require.main === module) {
  try {
    installAgentSkills();
  } catch (error) {
    console.error(`Failed to install Whatshell agent skills: ${error.message}`);
    process.exit(1);
  }
}
