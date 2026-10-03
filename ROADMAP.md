# DesignCraft roadmap

DesignCraft aims at full Adobe InDesign parity — and to be better: faster, open (documented JSON format + IDML), scriptable by agents (MCP), and available on the web.

## Status (2026-10-01)

**Working today**
- Document model: facing/non-facing spreads, parent pages with page-number markers, sections, layers, frames (text / graphic / unassigned), threaded stories, paragraph/character/object styles with based-on, swatches (process/spot/tints/gradients), corner options, text wrap, drop shadow, opacity/blend.
- Text engine: Knuth–Plass paragraph composer and single-line composer, dictionary hyphenation (public-domain Moby list + our own trained Liang patterns), justification with word/letter/glyph-scaling ranges, keep options (keep with next, keep lines together, widows/orphans), balance ragged lines, optical margin alignment, hyphenation zone/limit, columns, threading across frames and spreads, tabs, bullets/numbering, rules, shading, baseline grid, first-baseline options, vertical justification, wrap exclusions, overset detection, caret/hit testing.
- Rendering: vello_cpu (SIMD, multithreaded) — pages, items, images with mip levels, gradients, composed text.
- UI (egui): application bar + menus, Control panel (object and text modes), Tools panel with flyouts, document tabs, rulers, pasteboard, guides (margins, columns, bleed, baseline grid), frame edges, selection handles, text ports and threads, Properties / Pages / Layers dock, Swatches, Styles, Character, Paragraph, Stroke, Text Wrap panels, New Document and Preferences dialogs, Quick Apply (⌘Return), 4 brightness themes.
- Pen tool, Direct Selection anchor/handle editing, rotate from corners, Rotate/Scale/Shear tools, Eyedropper, snapping & smart guides, Align/Distribute, Find/Change (text + GREP), Story Editor, Paragraph Style Options, Effects panel, hidden characters, native macOS menu bar, measured InDesign 2026 look (Medium Dark): Properties sections per selection state, 21 pt spinner fields with arithmetic, Contextual Task Bar, inverse (black) text selection, Pages panel with drag-and-drop reorder / parent apply, Layers panel with per-object rows.
- Tools: Selection (click, marquee, move, Alt-duplicate, resize handles), Direct Selection, Type (draw frame, click caret, select, type), Rectangle/Ellipse/Polygon (+ frame variants), Line, Hand, Zoom.
- PDF export (krilla): real selectable text with embedded font subsets, DeviceCMYK/RGB + spot Separations, bleed boxes, crop/bleed marks + page info, pages or spreads, PDF/A-2b (PDF/X-4 output intent pending) — `file.exportPdf`, File › Export PDF…, `designcraft-cli run --export out.pdf`.
- IDML interchange (`designcraft-idml`): export and import of swatches, styles, fonts, preferences, parent spreads, spreads/pages, frames, groups, images (embedded or linked), formatted threaded stories — opens in InDesign 2026 and round-trips InDesign-exported files (`file.exportIdml`, `file.openIdml`, File → Export IDML…, CLI `--in x.idml` / `--export x.idml`).
- Tables (M8): tables anchored in stories (header/footer/body rows, merged cells, per-edge strokes, fills + alternating fills, insets, vertical justification, at-least/exact row heights); composed into the text column (columns scale to fit, rows break across columns/frames with repeating headers/footers, overset); rendered, exported to PDF (real text) and IDML (export + import); Table menu, Table panel, Create Table dialog, caret/typing/Tab navigation and cell selection in cells; `table.*` commands.
- Hyperlinks (text or frames → URL / e-mail / page) and bookmarks, exported as PDF link annotations and outline.
- EPUB 3 (reflowable) export: stories in reading order, CSS from paragraph/character styles, images, navigation (`file.exportEpub`, CLI `--export x.epub`).
- Data Merge (CSV or JSON rows, `<<Field>>` placeholders), spell checking (public-domain Moby list + document dictionary, suggestions), Step and Repeat (count or grid), snippets, object styles, Numbering & Section Options.
- Footnotes: Type › Insert Footnote (caret moves into the note), Document Footnote Options (numbering style incl. symbols, start/restart per page/spread/section, prefix/suffix, reference position/character style, paragraph style, separator, spacing, first baseline, rule above); notes composed at the bottom of the referencing column with the body text making room; edited in place on the canvas; rendered, exported to PDF (real text) and IDML (InDesign's structure, verified opening in InDesign 2026) and imported from IDML; `footnote.*` commands.
- Cross-references and text anchors: Insert Cross-Reference (paragraph by style, or text anchor), 11 formats with InDesign's building blocks (full/partial paragraph, paragraph text/number, page number, anchor name, chapter, file name) plus user formats; resolved live at composition — never out of date, no Update step; unresolved destinations show `??`; PDF link annotations to the destination page; IDML export/import as InDesign cross-reference sources, hyperlinks, text destinations and formats (verified in InDesign 2026); `xref.*` / `anchor.*` commands.
- Index: page references (topic from the selection or up to 4 levels, sort keys; current page, to end of story, next n paragraphs, suppressed, See / See also), Generate / Update Index (section headings, nested or run-in, page ranges merged, Index Title / Section Head / Level 1–4 styles); IDML export/import as InDesign topics, page references and topic cross-references (verified in InDesign 2026); `index.*` commands.
- Text clipboard keeps formatting (character/paragraph formats, footnotes, cross-references, index markers, tables) and falls back to plain text when the system clipboard changed; Paste without Formatting (⇧⌘V); Change Case (UPPERCASE, lowercase, Title Case, Sentence case — formatting kept); Type › Insert Special Character / White Space / Break Character submenus.
- Anchored objects: items flow in the text inline (on the baseline, with Y offset; lines grow to fit) or above the line (left/center/right, space before/after); paste copied items into text to anchor them; Anchored Object Options and Release (back onto the page where shown); rendered, exported to PDF and to/from IDML (InDesign anchored object settings; verified in InDesign 2026); copy/paste and threading carry them; `anchored.*` commands.
- Ruler guides: drag out of the rulers (page guides, or spread guides on the pasteboard), drag to move, drop on a ruler to delete; Layout › Create Guides (rows/columns with gutters, fit to margins or page); Delete All Guides on Spread; `guide.*` commands.
- Links panel: every placed graphic with status (OK / Modified / Missing / Embedded, checked against the file on disk), page and effective PPI (low resolution flagged); Relink…, Go To, Update, Embed; `links.*` commands.
- Missing fonts: highlighted pink on screen (never in output), listed by Preflight; Type › Find/Replace Font (fonts in the document, missing first; replace in text, cells, footnotes and styles); `font.list` / `font.replace`.
- Damage-region repaint: after an edit the canvas re-renders only the frames whose composed text (or the items that) changed and patches its texture — pixel-identical to a full render; typing in a frame repaints in under a millisecond instead of a full-viewport render (22 ms measured on the sample).
- Crash recovery: unsaved documents are written to the recovery folder every 30 s and reopened (unsaved, remembering their files) after a crash; File › Revert and Save a Copy…; `file.recovery.*`, `file.revert`, `file.saveACopy`.
- Edit › Paste Into (copied objects become a frame's content, clipped by it; rendered, PDF and IDML both ways — verified in InDesign 2026); Polygon Settings (sides, star inset; double-click the Polygon tool).
- Color panel (fill/stroke proxy, CMYK/RGB/Lab sliders with channel ramps, tint slider for swatches, spectrum ramp, Add to Swatches) and Color Picker (double-click the proxy); mixed colours are unnamed (not listed in Swatches until added); `object.color`, `swatch.addToSwatches`, `swatch.addUnnamed`.
- Place text files: Word (.docx: styles by name with their attributes, bold/italic/underline/size/font, footnotes, tables, tabs, breaks), RTF and plain text, into the insertion point, the selected frame or a new frame; autoflow adds pages and threaded frames until the text fits; Remove Styles option (`designcraft-textimport`).
- Gradient Feather (⇧G tool, `object.gradientFeather`): objects fade along a linear or radial opacity gradient, on screen and as a soft mask in PDF.
- Script Label (`object.label`, `object.findByLabel`), alt text (`object.altText`), Isolate Blending / Knockout Group (`object.transparencyGroup`; on screen, PDF isolates), Table ▸ Sort (`table.sortRows`, numeric-aware, header/footer fixed).
- File ▸ Export Text (`file.exportText`): a story as Text Only or RTF (fonts, styles, alignment, indents, tables as tab-separated rows); Export EPUB now asks for a path in the UI.
- Tagged PDF (`file.exportPdf {tagged}`, on by default from the UI): structure tree with stories as paragraphs in reading order, figures carrying their alt text, parent-page items and printer's marks as artifacts.
- Global Light (`object.globalLight`, `globalLight` on drop/inner shadows; Use Global Light in the Effects panel) and Generate Static Caption (`object.caption`: template over name, path, alt text, label, effective ppi, dimensions, format; any side, offset, style).
- Control panel scale X/Y, rotation and shear fields (`transform.info`, absolute `transform.set {scaleX, scaleY, rotation, shear}` about the reference point) and Preferences › Transformations are Totals for nested objects and placed graphics.
- Variable fonts: each named instance is a style (Font menu, shaping with feature variations, metrics, outlines) and embeds at its axis settings in PDF; free axis sliders still to come.
- Object ▸ Select (first/next above, next/last below, container, content, previous/next in group), full Object ▸ Convert Shape (`object.convertShape`), Table ▸ Split Cell Horizontally/Vertically.
- Attributes panel (`object.attributes`: Overprint Fill/Stroke/Gap, Nonprinting), with Overprint Gap in Overprint Preview and IDML.
- Page Numbering View (Preferences: section or absolute): page labels everywhere follow it; Go to Page takes section names, numbers or `+n`.
- Preferences › Advanced Type (document `advancedType`: superscript/subscript size and position; IDML TextPreference); subscripts now drop 33.3% like InDesign.
- Preferences › Composition › Highlight: keep violations, H&J violations (three shades), custom tracking/kerning, substituted fonts.
- File ▸ Export HTML (`file.exportHtml`): one self-contained page in reading order, styles as CSS, images inline, alt text from Object Export Options (also in EPUB).
- Swatches panel colour groups (`swatch.newColorGroup`, `moveToGroup`, `ungroupColorGroup`, `renameColorGroup`; folders and a row context menu; IDML ColorGroup).
- Object ▸ Generate QR Code (`object.qrCode`: web link, text, SMS, email, business card; vector modules as a compound path, into a frame or re-encoded in place).
- Conditional text (`condition.new/options/delete/apply/list`, Conditional Text panel): hidden conditions drop their text from layout and exports, indicators underline on screen, IDML Condition/AppliedConditions.
- Endnotes (`endnote.insert/edit/delete/list/options`, Type ▸ Insert Endnote): references number through the document, the endnote frame on a new last page lists them under a heading; IDML keeps the listed text only.
- Editorial notes (`note.new/edit/delete/convertToText/list`, Notes panel): anchored in text, flagged on screen, never printed or exported.
- File ▸ Print Booklet (`file.printBooklet`): saddle-stitch or 2-up consecutive printer spreads as PDF, padded with blanks, with a gap between pages.
- Edit ▸ Transparency Blend Space (`edit.transparencyBlendSpace`; RGB for web/mobile documents; IDML TransparencyPreference) — stored; compositing is still RGB on screen and in PDF.
- High Contrast interface theme (fifth Interface Color Theme: black chrome, white text, yellow selection).
- Place Excel workbooks (.xlsx): the first worksheet's used range becomes a table (shared/inline strings, numbers, booleans).
- Object Library (`library.new/open/add/place/remove/list/json/close`, Library panel): `.dclib` files of snippets that keep stories, styles, swatches and images.
- Ink Manager (`ink.list`, `ink.options`; Swatches panel menu): All Spots to Process, per-ink conversion and ink aliases in PDF output; IDML ConvertToProcess/AliasInkName.
- Appearance of Black complete: Printing/Exporting rich black for RGB output (PNG), Overprint [Black] Swatch at 100% (document, IDML OverprintBlack).
- Hyperlinks and Bookmarks panels (Window ▸ Interactive): new from the selection, edit (`hyperlink.edit`), go to source (`hyperlink.goToSource`), rename bookmarks (`bookmark.rename`), go to page, delete.
- Nested line styles (`nestedLineStyles` paragraph attribute, Paragraph Style Options › Drop Caps and Nested Styles, IDML AllNestedLineStyles): the first lines restyle until the line breaks settle.
- User dictionary hyphenation exceptions (`hyphenation.addException/removeException/list`, Edit ▸ Spelling ▸ User Dictionary): `ex~am~ple` breaks only at `~`, a plain word never hyphenates.
- Frame Fitting Options (`object.fittingOptions`, Object ▸ Fitting ▸ Frame Fitting Options…): Auto-Fit refits on resize, fitting, Align From reference point, crop amounts; IDML FrameFittingOption.
- Paragraph borders (per-side weights, colour, tint, offsets) and shading offsets, drawn per column part of split paragraphs (also when overset); Paragraph panel controls; IDML ParagraphBorder*/ParagraphShading*Offset.
- Font menu: search, favourites (stars, Show Favorites Only; `favoriteFonts` preference) and each family previewed in its own face.
- Application preferences (engine `Prefs`) now persist across launches (desktop: prefs.json beside ui.json).
- View ▸ Fit Selection in Window (⌥⌘=) and Find/Change ▸ Object (`find.objects`, `find.changeObjects`: by fill, stroke, weight, opacity, kind, layer, label).
- Custom workspaces (`window.newWorkspace/deleteWorkspace/resetWorkspace`; workspace switcher shows the current one): bars and panel arrangement saved by name.
- Word/RTF Import Options (File ▸ Place with Import Options…, `place.styles`, `file.place {styleMap, styleConflicts}`): map imported styles onto the document's, use existing / redefine / auto-rename on conflicts, or remove formatting.
- Custom anchored objects (`anchored.insert/options {position: custom}`): placed relative to the anchor, frame, column, page margins or page edge with reference points and offsets, no space in the text, keep within column; IDML Anchored position.
- Drag and drop text editing: drag selected text to move it (Alt copies) with its formatting (`text.release`).
- Image Import Options: place any page of a multi-page PDF (`file.place {pdfPage}`; the app asks which page), shown on screen and embedded as that page in PDF export.
- Define Lists: named numbered lists that continue across stories in page order (`list.define`, paragraph `listName` / `startAt`), round-tripped through IDML; `null` in `type.para` / `type.char` now removes an override.
- View › Overprint Preview (⌥⇧⌘Y): overprinting fills, strokes and 100% [Black] text mix with what's beneath on screen.
- Type on a Path (⇧T tool, `type.onPath`): a story along any line, curve or shape on screen and in PDF; Type on a Path Options (start, flip, baseline/ascender/descender/centre) and Delete Type from Path. Round-trips through IDML as TextPath; on-path caret drawing is still to come.
- Preferences › Appearance of Black: 100% K on screen accurately (dark grey) or as rich black (`window.richBlack`); the printing/export side is still to come.
- Spreads of up to 10 pages: Add to Previous Spread and Allow Spread to Shuffle (Pages panel; `layout.pages.toSpread`, `layout.spreadShuffle`); inserted pages go around kept spreads.
- Page tool (⇧P) and `layout.pageSize`: pages of their own size (or a preset); the spine stays put and objects keep their place on their page.
- File › Print (⌘P): printer, copies, page range, spreads, marks, bleed; printed as PDF through the system spooler (`file.print`, `file.printers`, `dryRun` for agents). Shortcuts now act like choosing the menu item (dialogs open), and UI commands win shortcut ties (⌘N opens New Document).
- Nested styles (through/up to N sentences, words, characters, letters, digits, tabs, breaks, spaces or given characters) and GREP styles (regex → character style), edited in Paragraph Style Options and composed under local formatting; `ui.dialog.open` for agents. Nested line styles are still to come; nested and GREP styles round-trip through IDML.
- Table and cell styles (round-trip through IDML): cell styles (fill, insets, vertical justification, strokes, paragraph style) and table styles (region cell styles, border, alternating rows, spacing); apply from the Table panel, edits re-apply to every user; `style.cell.*`, `style.table.*`.
- Style groups: paragraph and character styles shown in group folders, Move to Group (new, existing, none); `style.group` renames every use (styles, stories, table cells, footnotes, object styles).
- Scrubby zoom: drag the Zoom tool left or right to zoom continuously around the press point.
- Guides belong to the active layer: hidden with it (or with its Show Guides off) and locked with it.
- Pencil tool (N): freehand strokes simplified to smooth paths (Alt closes); Smooth and Erase tools re-fit or remove the stretch you drag along (`path.smooth`, `path.erase`).
- Edit › Keyboard Shortcuts: every command, click and press the new keys, conflicts shown, per-command and global reset; `window.setShortcut`.
- Primary Text Frame (New Document) and Smart Text Reflow: pages are added while the primary story oversets and empty ones at the end removed, in the same undo step (Preferences › Type).
- Glyphs panel: every character of any font (rendered by our renderer), search by character or U+code, click to insert, recently used.
- UI scaling (Preferences › Interface, 50–200%; `window.uiScale`); the window title steps aside when the bar is crowded.
- Underline / Strikethrough Options: weight, offset, colour and tint (Character panel; screen, PDF and IDML).
- Relink to Folder and Relink File Extension (`links.relinkFolder`; missing links found by name).
- File › Package (⌥⇧⌘P): the document relinked to a Links folder, the placed files, an IDML copy, an optional PDF and a report (fonts, links, preflight, instructions); Copy Links To (`links.copyTo`).
- Load Swatches / Save Swatches for Exchange (.ase: CMYK, RGB, Lab, Gray; spot or process) from the Swatches panel menu; `swatch.load` / `swatch.save`.
- Pathfinder (Object › Pathfinder and a Pathfinder panel): Add, Subtract, Intersect, Exclude Overlap, Minus Back; curves stay curves (flo_curves); `object.pathfinder`.
- Make / Release Compound Path (⌘8; nested paths become holes), Create Outlines (⇧⌘O; one path per text colour), Scissors tool (C) with `path.split`.
- Transform Again / Individually / Sequence Again (⌥⌘3; a moved copy repeats like Step and Repeat), Clear Transformations; Layers: Merge, Delete Unused, Hide/Lock Others, Show/Unlock All; Break Link to Style (paragraph and character).
- Floating panels: tear a panel off the dock (its float button, or drag its header away) into a movable, resizable window; Dock puts it back; positions persist; `window.floatPanel` / `window.dockPanel`.
- Place SVG (vector in PDF export via krilla-svg, resvg on screen, text in the bundled fonts), Photoshop (.psd composite), BMP, and Illustrator files saved with PDF compatibility (.ai). New `designcraft-images` crate (L2) for sniffing, sizes and decoding. EPS is still missing.
- View › Display Performance: Fast (grey boxes, no effects, ⌥⇧⌘Z), Typical (72 ppi proxies, ⌥⌘Z), High Quality (⌥⌘H, our default — the renderer is fast enough).
- OpenType menu (Character panel): discretionary ligatures, fractions, ordinal, swash, titling, contextual alternates, slashed zero, the four figure styles and stylistic sets 1–20; `type.openType`; mapped to IDML's OTF attributes.
- Add Anchor Point (=), Delete Anchor Point (-) and Convert Direction Point (⇧C) tools (click toggles smooth/corner, drag pulls out handles), with `path.addAnchor`, `path.deleteAnchor`, `path.convertAnchor`.
- Gradients: the Gradient Swatch tool (G) drags a gradient's start and end across objects (Shift: 45°); the Gradient panel sets type, angle, reverse and edits stops on a ramp (drag, add, drag off to remove, location, colour). Edited gradients are unnamed swatches; `object.gradient`; the vector round-trips through IDML.
- Document Setup (⌥⌘P): intent, number of pages, start page # (an even start begins with a left page), facing pages, size presets and orientation, bleed and slug per edge; `layout.documentSetup {}` reports the setup.
- Stroke panel: cap, join, miter limit, alignment, type, Start/End arrowheads (12 kinds, drawn on screen and in PDF; the path is shortened under pointed heads) and gap colour under dashes and dots.
- Preferences (⌘K: General, Type, Units & Increments, Grids, Guides & Pasteboard, Display Performance) backed by `prefs.set` (application) and `document.preferences` (units, increments, grids — undoable, saved with the document); scaling applies to content and scales stroke weights (Include Stroke Weight), and X/Y/W/H measure the stroke's outer edge (Dimensions Include Stroke Weight). Quick Apply (⌘Return) finds paragraph, character and object styles as well as commands.
- Parent item overrides: Cmd+Shift-click a parent item on a page (or Override All Parent Page Items, ⌥⇧⌘L) makes an editable local copy that hides the parent's; Remove All Local Overrides and Detach All Objects from Parent in the Pages panel menu.
- Place PDF: placed PDF pages are sized by their crop box, drawn on screen through hayro (with mip levels) and embedded in exported PDFs as vector form XObjects (krilla), `<PDF>` in IDML.
- Menus expose the engine's features (TOC, numbering & sections, text variables, hyperlinks, cross-references, footnotes, step and repeat, spelling, fitting/content, effects, Data Merge, Preflight, EPUB); any menu command with parameters gets a dialog generated from its parameter documentation.
- Table of contents (generate/update, dot leaders) and text variables (running headers, last page number, chapter number, file name, dates, custom) resolved per page.
- Soft effects: blurred drop shadow, inner shadow, outer glow, basic feather.
- ~200 commands, all reachable through the JSON control channel; headless CLI rendering to PNG.
- MCP server (`designcraft-cli mcp [--connect PORT]`, docs/mcp.md): headless engine or the running app; commands, batch, document/story inspection, page renders as images, window screenshots, pointer/keyboard/dialog input.
- Web build (`apps/designcraft-web`, trunk): the same UI on WebGPU with a WebGL2 fallback; open/place via the browser file picker or drag-and-drop, save/export as downloads.

**Next (in order):** books · New Window / split view · Ink Manager and Separations Preview · XML structure, tags and export · buttons and forms · PDF/X-4 output intent (needs krilla support) · EPS place.

## How far from full parity (estimate, 2026-10-02, updated)

**Breadth: ~89% weighted** (P0 core 99%, P1 92%, P2 39%) over the 275 features of the InDesign catalogue, scored
row by row in [docs/parity.md](docs/parity.md) (`cargo xtask parity` recomputes it). Many features scored done still
lack some of InDesign's options or dialog details, so **overall parity including depth is about 72%**.

**Remaining work: about 355 wall-clock hours of a single Claude Opus 5.5 agent** (±30%), or roughly 95–130 hours with
four agents in parallel on separate crates:

| Work | Estimate |
|---|---|
| Open P0 (3: PDF/X-4 output intent and validation, EPS place, remaining Preferences sections) | 12 h |
| Open P1 (22: New Window, nested line styles, on-path caret, variable-font axes, blend-space compositing, …) | 50 h |
| Open P2 (54: liquid/alternate layouts, books, buttons & forms, XML, track changes, vertical/RTL type, …) | 150 h |
| Depth and pixel fidelity of every dialog, panel and menu against InDesign 2026 | 90 h |
| Performance (incremental composition, GPU raster) and interchange hardening (IDML/PDF corpus) | 55 h |

Basis: since the first estimate this effort landed about 45 arcs (from parent overrides, Preferences, Stroke panel,
gradients and Pathfinder to table/cell styles, nested/GREP styles and Print) in roughly 14 hours on a heavily loaded
machine — about 1–2 hours per catalogue row.

## Agents: CLI and MCP

Every command is reachable from `designcraft-cli` (`run`, `script` with `$N.path` result references, `app` for the
running window, `describe`, `commands`), from MCP (`designcraft-cli mcp`, tools including `execute` and `batch` with the
same references), and from the app's JSON control channel. See [docs/agents.md](docs/agents.md).

## Milestones

| # | Milestone | State |
|---|---|---|
| M0 | Skeleton + vertical slice | ✅ |
| M1 | Selection, transform, layers, pages, MCP | in progress |
| M2 | Type I (Type tool, threading, Character/Paragraph, composer) | in progress |
| M3 | Styles (nested/GREP, bullets, keeps, span columns) | started |
| M4 | Color & effects | started |
| M5 | Graphics & links | started |
| M6 | Files & export (native, IDML, PDF, PNG/JPEG) | in progress (IDML ✅, PDF export) |
| M7 | Long documents (sections, TOC, index, footnotes, books) | in progress (sections, TOC, text variables/running heads, hyperlinks, bookmarks, footnotes, cross-references, index) |
| M8 | Tables | ✅ core (table/cell styles, rotation, diagonal lines pending) |
| M9 | Performance (MT composition, tiles) | in progress (glyph path cache, culling, parallel compose 6.7× faster, perf harness) |
| M10 | Find/Change, spelling, Preflight | in progress (Find/Change ✅, spelling ✅, Preflight) |
| M11 | Layout power features (liquid/alternate layouts, data merge) | started (data merge, step & repeat, snippets) |
| M12 | Interactive & digital (EPUB, HTML, interactive PDF) | started (EPUB, PDF links/bookmarks) |
| M13 | Automation (scripts, batch) | |
| M14 | 1.0 polish & packaging | |
