const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { installAgentSkills } = require("./install-skills");

const root = path.resolve(__dirname, "..");
const tempHome = fs.mkdtempSync(path.join(os.tmpdir(), "whatshell-skills-"));

const previous = {
  WHATSHELL_SKILL_HOME: process.env.WHATSHELL_SKILL_HOME,
  WHATSHELL_SKILL_INSTALL_QUIET: process.env.WHATSHELL_SKILL_INSTALL_QUIET,
  WHATSHELL_INSTALL_OPENCODE_SKILL: process.env.WHATSHELL_INSTALL_OPENCODE_SKILL,
  WHATSHELL_INSTALL_CODEX_HOME_SKILL: process.env.WHATSHELL_INSTALL_CODEX_HOME_SKILL
};

try {
  process.env.WHATSHELL_SKILL_HOME = tempHome;
  process.env.WHATSHELL_SKILL_INSTALL_QUIET = "1";
  delete process.env.WHATSHELL_INSTALL_OPENCODE_SKILL;
  delete process.env.WHATSHELL_INSTALL_CODEX_HOME_SKILL;

  const installed = installAgentSkills({ root, quiet: true });
  assert.deepEqual(
    installed.map((target) => target.label),
    ["Claude Code", "Agent Skills"]
  );

  const claudeSkill = path.join(tempHome, ".claude", "skills", "whatshell", "SKILL.md");
  const agentSkill = path.join(tempHome, ".agents", "skills", "whatshell", "SKILL.md");
  const opencodeSkill = path.join(tempHome, ".config", "opencode", "skills", "whatshell", "SKILL.md");

  assert.equal(fs.existsSync(claudeSkill), true);
  assert.equal(fs.existsSync(agentSkill), true);
  assert.equal(fs.existsSync(opencodeSkill), false);

  const contents = fs.readFileSync(agentSkill, "utf8");
  assert.match(contents, /^---\nname: whatshell\n/m);
  assert.match(contents, /description: /);
  assert.match(contents, /whatshell doctor --json/);

  process.env.WHATSHELL_INSTALL_OPENCODE_SKILL = "1";
  process.env.WHATSHELL_INSTALL_CODEX_HOME_SKILL = "1";
  const installedWithOptIns = installAgentSkills({ root, quiet: true });
  assert.deepEqual(
    installedWithOptIns.map((target) => target.label),
    ["Claude Code", "Agent Skills", "Codex home", "OpenCode native"]
  );
  assert.equal(fs.existsSync(opencodeSkill), true);
  assert.equal(fs.existsSync(path.join(tempHome, ".codex", "skills", "whatshell", "SKILL.md")), true);
} finally {
  for (const [name, value] of Object.entries(previous)) {
    if (value === undefined) {
      delete process.env[name];
    } else {
      process.env[name] = value;
    }
  }
  fs.rmSync(tempHome, { recursive: true, force: true });
}
