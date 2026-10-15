use super::install::Install;

pub(super) const NPM_INSTALLED_BINARY: &str =
    "prefix/lib/node_modules/@repost/agent-terminal/node_modules/@repost/agent-terminal-linux-x64/bin/agent-terminal";

pub(super) const REINSTALL: &str =
    "mkdir -p \"$GLOBAL_BIN\"\nln -sf \"$NEW_BINARY\" \"$GLOBAL_BIN/agent-terminal\"\necho 'changed 1 package'";

pub(super) const PRINT_PREFIX: &str = "echo \"${GLOBAL_BIN%/bin}\"";

pub(super) const PRINT_BIN: &str = "echo \"$GLOBAL_BIN\"";

pub(super) const NEWER_ON_NPM: &[(&str, &str)] = &[
    ("view @repost/agent-terminal version", "echo 9.9.9"),
    ("install -g @repost/agent-terminal@latest", REINSTALL),
    ("prefix -g", PRINT_PREFIX),
];

pub(super) const NEWER_ON_PNPM: &[(&str, &str)] = &[
    ("view @repost/agent-terminal version", "echo 9.9.9"),
    ("add -g @repost/agent-terminal@latest", REINSTALL),
    ("bin -g", PRINT_BIN),
];

pub(super) const EVERY_INSTALLER: &str = "  npm install -g @repost/agent-terminal@latest
  pnpm add -g @repost/agent-terminal@latest
  yarn global add @repost/agent-terminal@latest
  bun add -g @repost/agent-terminal@latest
  brew upgrade repost-sh/tap/agent-terminal";

pub(super) struct Installed {
    pub(super) installer: &'static str,
    pub(super) binary: &'static str,
    pub(super) manager: &'static str,
    pub(super) answers: &'static [(&'static str, &'static str)],
    pub(super) calls: &'static str,
}

pub(super) const INSTALLS: &[Installed] = &[
    Installed {
        installer: "npm",
        binary: NPM_INSTALLED_BINARY,
        manager: "npm",
        answers: NEWER_ON_NPM,
        calls: "npm view @repost/agent-terminal version\nnpm install -g @repost/agent-terminal@latest\nnpm prefix -g\n",
    },
    Installed {
        installer: "pnpm",
        binary: "store/v11/links/@repost/agent-terminal-linux-x64/0.0.1/3f9c/node_modules/@repost/agent-terminal-linux-x64/bin/agent-terminal",
        manager: "pnpm",
        answers: NEWER_ON_PNPM,
        calls: "pnpm view @repost/agent-terminal version\npnpm add -g @repost/agent-terminal@latest\npnpm bin -g\n",
    },
    Installed {
        installer: "pnpm",
        binary: "global/5/node_modules/.pnpm/@repost+agent-terminal-linux-x64@0.0.1/node_modules/@repost/agent-terminal-linux-x64/bin/agent-terminal",
        manager: "pnpm",
        answers: NEWER_ON_PNPM,
        calls: "pnpm view @repost/agent-terminal version\npnpm add -g @repost/agent-terminal@latest\npnpm bin -g\n",
    },
    Installed {
        installer: "yarn",
        binary: ".config/yarn/global/node_modules/@repost/agent-terminal-linux-x64/bin/agent-terminal",
        manager: "yarn",
        answers: &[
            ("info @repost/agent-terminal version --silent", "echo 9.9.9"),
            ("global add @repost/agent-terminal@latest", REINSTALL),
            ("global bin", PRINT_BIN),
        ],
        calls: "yarn info @repost/agent-terminal version --silent\nyarn global add @repost/agent-terminal@latest\nyarn global bin\n",
    },
    Installed {
        installer: "bun",
        binary: "bun/install/global/node_modules/@repost/agent-terminal-linux-x64/bin/agent-terminal",
        manager: "bun",
        answers: &[
            ("pm view @repost/agent-terminal version", "echo 9.9.9"),
            ("add -g @repost/agent-terminal@latest", REINSTALL),
            ("pm bin -g", PRINT_BIN),
        ],
        calls: "bun pm view @repost/agent-terminal version\nbun add -g @repost/agent-terminal@latest\nbun pm bin -g\n",
    },
    Installed {
        installer: "Homebrew",
        binary: "homebrew/Cellar/agent-terminal/0.0.1/bin/agent-terminal",
        manager: "brew",
        answers: &[
            ("update", "echo 'Already up-to-date.'"),
            (
                "info --json=v2 repost-sh/tap/agent-terminal",
                r#"echo '{"formulae":[{"full_name":"repost-sh/tap/agent-terminal","versions":{"stable":"9.9.9","head":null,"bottle":true}}],"casks":[]}'"#,
            ),
            ("upgrade repost-sh/tap/agent-terminal", REINSTALL),
            ("--prefix repost-sh/tap/agent-terminal", PRINT_PREFIX),
        ],
        calls: "brew update\nbrew info --json=v2 repost-sh/tap/agent-terminal\nbrew upgrade repost-sh/tap/agent-terminal\nbrew --prefix repost-sh/tap/agent-terminal\n",
    },
];

pub(super) const RELOCATED_BINARY: &str = "global/node_modules/@repost/agent-terminal-linux-x64/bin/agent-terminal";

pub(super) fn global_listings(install: &Install, npm_lists: &str) {
    install.answering("npm", &[("ls -g --depth=0", &format!("echo '/usr/local/lib'\necho '└── {npm_lists}'"))]);
    install.answering("pnpm", &[("ls -g --depth=0", "echo 'Legend: production dependency, optional only, dev only'")]);
    install.answering(
        "yarn",
        &[
            ("global list", "echo 'info \"@repost/agent-terminal@0.0.1\" has binaries:'\necho '   - agent-terminal'"),
            ("info @repost/agent-terminal version --silent", "echo 9.9.9"),
            ("global add @repost/agent-terminal@latest", REINSTALL),
            ("global bin", PRINT_BIN),
        ],
    );
    install.answering(
        "bun",
        &[("pm ls -g", "echo '/root/.bun/install/global node_modules (1)'\necho '└── typescript@5.6.3'")],
    );
}
