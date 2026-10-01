# DesignCraft roadmap

DesignCraft aims at full Adobe InDesign parity — and to be better: faster, open (documented JSON format + IDML), scriptable by agents (MCP), and available on the web.

## Status (2026-10-01)

**Working today**
- Document model: facing/non-facing spreads, parent pages with page-number markers, sections, layers, frames (text / graphic / unassigned), threaded stories, paragraph/character/object styles with based-on, swatches (process/spot/tints/gradients), corner options, text wrap, drop shadow, opacity/blend.
- Text engine: Knuth–Plass paragraph composer and single-line composer, hyphenation, justification, columns, threading across frames and spreads, tabs, bullets/numbering, rules, shading, baseline grid, first-baseline options, vertical justification, wrap exclusions, overset detection, caret/hit testing.
- Rendering: vello_cpu (SIMD, multithreaded) — pages, items, images with mip levels, gradients, composed text.
- UI (egui): application bar + menus, Control panel (object and text modes), Tools panel with flyouts, document tabs, rulers, pasteboard, guides (margins, columns, bleed, baseline grid), frame edges, selection handles, text ports and threads, Properties / Pages / Layers dock, Swatches, Styles, Character, Paragraph, Stroke, Text Wrap panels, New Document dialog, ⌘K palette, 4 brightness themes.
- Tools: Selection (click, marquee, move, Alt-duplicate, resize handles), Direct Selection (basic), Type (draw frame, click caret, select, type), Rectangle/Ellipse/Polygon (+ frame variants), Line, Hand, Zoom.
- ~100 commands, all reachable through the JSON control channel; headless CLI rendering to PNG.

**Next (in order):** MCP server · PDF export (krilla, PDF/X-4) · IDML import/export · native format as zip · web build · Pen tool & Direct Selection anchors · Story Editor · Find/Change (+GREP) · tables · Liang hyphenation · smart guides · Links panel + relink · Preflight · TOC/index/footnotes · EPUB.

## Milestones

| # | Milestone | State |
|---|---|---|
| M0 | Skeleton + vertical slice | ✅ |
| M1 | Selection, transform, layers, pages, MCP | in progress |
| M2 | Type I (Type tool, threading, Character/Paragraph, composer) | in progress |
| M3 | Styles (nested/GREP, bullets, keeps, span columns) | started |
| M4 | Color & effects | started |
| M5 | Graphics & links | started |
| M6 | Files & export (native, IDML, PDF, PNG/JPEG) | next |
| M7 | Long documents (sections, TOC, index, footnotes, books) | |
| M8 | Tables | |
| M9 | Performance (MT composition, tiles) | |
| M10 | Find/Change, spelling, Preflight | |
| M11 | Layout power features (liquid/alternate layouts, data merge) | |
| M12 | Interactive & digital (EPUB, HTML, interactive PDF) | |
| M13 | Automation (scripts, batch) | |
| M14 | 1.0 polish & packaging | |
