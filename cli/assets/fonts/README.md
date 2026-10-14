# Bundled fonts

`agent-terminal screenshot` draws the screen with these fonts, built into the binary, so a screenshot looks the same on every machine and needs no system fonts. The renderer tries them in this order for each character: JetBrains Mono (the cell's style), Symbols Nerd Font Mono, Noto Emoji; a character none of them has is drawn as `□`.

| File | Font | Version | Source | Licence |
| --- | --- | --- | --- | --- |
| `JetBrainsMonoNL-Regular.ttf`, `-Bold`, `-Italic`, `-BoldItalic` | JetBrains Mono NL (no ligatures: cells are drawn one by one) | 2.304 | https://github.com/JetBrains/JetBrainsMono/releases/tag/v2.304 | SIL Open Font License 1.1, [`OFL-JetBrainsMono.txt`](OFL-JetBrainsMono.txt) |
| `SymbolsNerdFontMono-Regular.ttf` | Symbols Nerd Font Mono | 3.5.1 | https://github.com/ryanoasis/nerd-fonts/releases/tag/v3.5.1 (`NerdFontsSymbolsOnly.zip`) | MIT, [`LICENSE-SymbolsNerdFont.txt`](LICENSE-SymbolsNerdFont.txt); icon sets below |
| `NotoEmoji-VariableFont_wght.ttf` | Noto Emoji (monochrome) | 3.000 | https://github.com/google/fonts/tree/main/ofl/notoemoji | SIL Open Font License 1.1, [`OFL-NotoEmoji.txt`](OFL-NotoEmoji.txt) |

All three licences allow bundling the fonts with software and redistributing them unchanged, provided the copyright notice and licence travel with them. The OFL fonts are not sold on their own and are not modified.

Symbols Nerd Font Mono collects icons from these projects, each under its own licence (from the Nerd Fonts 3.5.1 release notes):

| Icon set | Upstream | Licence |
| --- | --- | --- |
| Codicons | https://github.com/microsoft/vscode-codicons | CC BY 4.0 |
| Devicons | https://github.com/devicons/devicon | MIT |
| extraglyphs | https://github.com/source-foundry/Hack | MIT |
| Font Awesome | https://github.com/FortAwesome/Font-Awesome | CC BY 4.0 |
| Font Awesome Extension | https://github.com/AndreLZGava/font-awesome-extension | MIT |
| Font Logos | https://github.com/lukas-w/font-logos | Unlicense |
| Material Design Icons | https://github.com/Templarian/MaterialDesign-Font | Apache 2.0 |
| Octicons | https://github.com/primer/octicons | MIT |
| Seti and original | https://github.com/jesseweed/seti-ui | MIT |
| Pomicons | https://github.com/gabrielelana/pomicons | SIL OFL 1.1 |
| Powerline Extra | https://github.com/ryanoasis/powerline-extra-symbols | MIT |
| Powerline Symbols | https://github.com/powerline/powerline | MIT |
| Power Symbols IEC | https://github.com/jloughry/Unicode | MIT |
| Weather Icons | https://github.com/erikflowers/weather-icons | SIL OFL 1.1 |
