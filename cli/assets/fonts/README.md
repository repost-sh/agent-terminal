# Bundled fonts

`agent-terminal screenshot` draws the screen with these fonts, built into the binary, so a screenshot looks the same on every machine and needs no system fonts. The renderer tries them in this order for each character: JetBrains Mono (the cell's style), Symbols Nerd Font Mono, Noto Emoji; a character none of them has is drawn as `□`.

| File | Font | Version | Source | Licence |
| --- | --- | --- | --- | --- |
| `JetBrainsMonoNL-Regular.ttf`, `-Bold`, `-Italic`, `-BoldItalic` | JetBrains Mono NL (no ligatures: cells are drawn one by one) | 2.304 | https://github.com/JetBrains/JetBrainsMono/releases/tag/v2.304 | SIL Open Font License 1.1, [`OFL-JetBrainsMono.txt`](../../../licenses/OFL-JetBrainsMono.txt) |
| `SymbolsNerdFontMono-Regular.ttf` | Symbols Nerd Font Mono | 3.5.1 | https://github.com/ryanoasis/nerd-fonts/releases/tag/v3.5.1 (`NerdFontsSymbolsOnly.zip`) | MIT, [`LICENSE-SymbolsNerdFont.txt`](../../../licenses/LICENSE-SymbolsNerdFont.txt); its icon sets in [`licenses/README.md`](../../../licenses/README.md) |
| `NotoEmoji-VariableFont_wght.ttf` | Noto Emoji (monochrome) | 3.000 | https://github.com/google/fonts/tree/main/ofl/notoemoji | SIL Open Font License 1.1, [`OFL-NotoEmoji.txt`](../../../licenses/OFL-NotoEmoji.txt) |

All three licences allow bundling the fonts with software and redistributing them unchanged, provided the copyright notice and licence travel with them: the licence texts are in `licenses/` at the root of the repository, which every release archive and npm platform package ships next to the binary. The OFL fonts are not sold on their own and are not modified.
