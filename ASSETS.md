# Asset attribution

Every non-code asset in this repository (fonts, images, example art, presets) is listed here with its author, source and licence. `cargo xtask assets` (part of `cargo xtask ci`) fails if an asset file is missing from this table.

**Policy (mandatory):** DesignCraft contains **no Adobe iconography, images, artwork, presets, swatch libraries or ICC profiles**. Every asset is original work by DesignCraft contributors or third-party material under an open licence (OSI open source, public domain / CC0, or Creative Commons that allows redistribution). Screenshots of Adobe software are never committed.

Generated-in-code art is original and has no file to list: the UI icon set (`crates/ui-egui/src/icons.rs`), default swatches, and the sample magazine's imagery (`crates/engine/src/sample.rs`).

| Asset | Author | Source | Licence | Notes |
|---|---|---|---|---|
| `assets/hyphenation/en-us.dic` | Grady Ward (Moby Hyphenator II word list); converted by DesignCraft contributors | https://www.gutenberg.org/ebooks/3204 (file `https://www.gutenberg.org/files/3204/files/mhyph.txt`, SHA-256 `eeb30474c86b8af3469035ec1a0913e35905325ca885290db9dfed9881e230ac`) | Public Domain ("Public Domain material by grant from the author, January, 2001"; Project Gutenberg: "Public domain in the USA") | ~165k words with break points, front-coded + deflate; regenerate with `cargo run --release -p designcraft-compose --example hyphgen -- mhyph.txt assets/hyphenation` |
| `assets/hyphenation/en-us.pat` | DesignCraft contributors (generated) | trained from `en-us.dic`'s source list by `crates/compose/src/hyphen/patgen.rs` (`examples/hyphgen.rs`) | Public Domain (CC0-1.0) | Our own Liang hyphenation patterns (no TeX/hyph-utf8 patterns used) |
| `assets/fonts/Inter-Medium.ttf` | Rasmus Andersson / The Inter Project Authors | https://github.com/rsms/inter | OFL-1.1 (`assets/fonts/OFL-Inter.txt`) | Open-source typeface |
| `assets/fonts/Inter-Regular.ttf` | Rasmus Andersson / The Inter Project Authors | https://github.com/rsms/inter | OFL-1.1 (`assets/fonts/OFL-Inter.txt`) | Open-source typeface |
| `assets/fonts/Inter-SemiBold.ttf` | Rasmus Andersson / The Inter Project Authors | https://github.com/rsms/inter | OFL-1.1 (`assets/fonts/OFL-Inter.txt`) | Open-source typeface |
| `assets/fonts/JetBrainsMono-Regular.ttf` | The JetBrains Mono Project Authors | https://github.com/JetBrains/JetBrainsMono | OFL-1.1 (`assets/fonts/OFL-JetBrainsMono.txt`) | Open-source typeface |
| `assets/fonts/OFL-Inter.txt` | (licence text) | upstream project | — |  |
| `assets/fonts/OFL-JetBrainsMono.txt` | (licence text) | upstream project | — |  |
| `assets/fonts/OFL-SourceSans3.txt` | (licence text) | upstream project | — |  |
| `assets/fonts/OFL-SourceSerif4.txt` | (licence text) | upstream project | — |  |
| `assets/fonts/SourceSans3-Bold.ttf` | Paul D. Hunt / Adobe (released as open source) | https://github.com/adobe-fonts/source-sans | OFL-1.1 (`assets/fonts/OFL-SourceSans3.txt`) | Open-source typeface |
| `assets/fonts/SourceSans3-It.ttf` | Paul D. Hunt / Adobe (released as open source) | https://github.com/adobe-fonts/source-sans | OFL-1.1 (`assets/fonts/OFL-SourceSans3.txt`) | Open-source typeface |
| `assets/fonts/SourceSans3-Regular.ttf` | Paul D. Hunt / Adobe (released as open source) | https://github.com/adobe-fonts/source-sans | OFL-1.1 (`assets/fonts/OFL-SourceSans3.txt`) | Open-source typeface |
| `assets/fonts/SourceSans3-Semibold.ttf` | Paul D. Hunt / Adobe (released as open source) | https://github.com/adobe-fonts/source-sans | OFL-1.1 (`assets/fonts/OFL-SourceSans3.txt`) | Open-source typeface |
| `assets/fonts/SourceSerif4-Bold.ttf` | Frank Grießhammer / Adobe (released as open source) | https://github.com/adobe-fonts/source-serif | OFL-1.1 (`assets/fonts/OFL-SourceSerif4.txt`) | Open-source typeface |
| `assets/fonts/SourceSerif4-BoldIt.ttf` | Frank Grießhammer / Adobe (released as open source) | https://github.com/adobe-fonts/source-serif | OFL-1.1 (`assets/fonts/OFL-SourceSerif4.txt`) | Open-source typeface |
| `assets/fonts/SourceSerif4-It.ttf` | Frank Grießhammer / Adobe (released as open source) | https://github.com/adobe-fonts/source-serif | OFL-1.1 (`assets/fonts/OFL-SourceSerif4.txt`) | Open-source typeface |
| `assets/fonts/SourceSerif4-Regular.ttf` | Frank Grießhammer / Adobe (released as open source) | https://github.com/adobe-fonts/source-serif | OFL-1.1 (`assets/fonts/OFL-SourceSerif4.txt`) | Open-source typeface |
| `assets/fonts/SourceSerif4-Semibold.ttf` | Frank Grießhammer / Adobe (released as open source) | https://github.com/adobe-fonts/source-serif | OFL-1.1 (`assets/fonts/OFL-SourceSerif4.txt`) | Open-source typeface |
| `docs/images/page-2.png` | DesignCraft contributors | screenshot/render of DesignCraft itself (sample document generated in code) | MIT OR Apache-2.0 | Original; no Adobe UI |
| `docs/images/page-3.png` | DesignCraft contributors | screenshot/render of DesignCraft itself (sample document generated in code) | MIT OR Apache-2.0 | Original; no Adobe UI |
| `docs/images/page-cover.png` | DesignCraft contributors | screenshot/render of DesignCraft itself (sample document generated in code) | MIT OR Apache-2.0 | Original; no Adobe UI |
| `docs/images/ui-spread.png` | DesignCraft contributors | screenshot/render of DesignCraft itself (sample document generated in code) | MIT OR Apache-2.0 | Original; no Adobe UI |
| `docs/images/ui-typing.png` | DesignCraft contributors | screenshot/render of DesignCraft itself (sample document generated in code) | MIT OR Apache-2.0 | Original; no Adobe UI |
