# Smart guides and snapping

DesignCraft already snaps a selection move and a frame-tool drag onto page edges, centers, margins, columns, ruler guides, and other items' edges and centers. One tolerance of 5 screen pixels is hardcoded, `Session::with_ctx` always passes `snap: true`, and the Properties smart-guides control only stores a bool. Resize, rotate, pen, and anchor drags do not snap. A Shift-constrained move zeroes one axis and then adds the snap delta, so the locked axis can still jump.

This spec replaces that with one runner and six passes. The work matches the snapping model observed in Adobe InDesign 2026 on a synthetic document, plus the documented switch behavior for the cases that were not dragged. It is behavior, not an implementation plan.

## Passes

Each pass is its own function. A runner calls them in the order below, once per free axis. The runner contains no snap math. The first pass that hits an axis wins, and later passes do not move that axis. Inside a pass, the closest candidate within the zone wins.

A candidate is in range when its distance is at most the snap zone. The zone is a view setting in screen pixels, converted to spread points with `ToolContext::tol`.

1. **Document grid**, when Snap to Document Grid is on. It still runs when the grid is hidden. The interval is the major spacing divided by the subdivision count. A count below 1 counts as 1, so every major line is a target. The pass calls `designcraft_geom::snap_to_grid`. It does not build a line per cell.
2. **Baseline grid**, when Snap to Guides is on. It still runs when the baseline grid is hidden, and the baseline view threshold does not suppress it. The lines are the ones the canvas paints: `page.y0 + start + n * increment`, across that page. An increment of 0 or less is no hit. `relative_to` is unused, matching the canvas.
3. **Ruler guides, margins, and columns**, when Snap to Guides is on and Show Guides is on. A ruler guide qualifies only when `Guide::visible_in` is true. A page guide attracts within that page. A spread guide attracts across the spread. Margins and columns of the pages on the spread are included. Locked guides still attract.
4. **Object alignment**, when Smart Guides is on. Page edges are candidates when Align to Object Edges is on. Page centers are candidates when Align to Object Centers is on. The same two switches gate item edges and item centers. Targets are the axis-aligned visible bounds: top-level items on the spread, and parent-page items when the page's `show_parent_items` is set. A group's box is the union of its children's visible bounds, in the group's transformed space. Hidden items are skipped. Locked items still attract.
5. **Equal spacing**, on moves only, when Smart Guides and Smart Spacing are on. See Gestures for which drags count as moves.
6. **Dimensions**, on create, resize, and rotate, when Smart Guides and Smart Dimensions are on. Position gestures compare a length. Rotation compares an angle. See Gestures.

Every pass shares one exclude list. A plain move excludes the items that are moving, including children of a selected group. An Alt-drag copy excludes nothing in the document, because the originals stay put and the copy is still only a preview.

Parent items are collected from the page's applied parent, following `based_on` with a seen set. A cycle ends the walk. While a parent spread is itself being edited, its items are ordinary items and are not added a second time.

### Visible bounds and own size

Alignment and spacing use `Item::visible_bounds`: a centered stroke grows the box by half the weight, an outside stroke by the whole weight, and an inside stroke by nothing. A 1 pt centered stroke therefore attracts at half a point outside the path.

Dimensions use each other item's own size: take the visible bounds after removing rotation and keeping scale. A rectangle whose sides are 200 by 40, rotated 30 degrees, contributes 200 and 40. The axis-aligned box, about 193.2 by 134.6, stays an alignment target only. A length being edited may match either the width or the height.

### Spacing

Spacing runs on one axis at a time. Consider top-level visible boxes whose ranges overlap on the other axis. The moving box has a nearest neighbor on each side, and the gap is the positive distance between them. Stationary gaps are the nearest-neighbor gaps between stationary boxes that overlap on the other axis. A candidate shifts the moving box so one of its gaps equals one stationary gap. The shift must fall inside the zone. The closest shift wins. Overlapping boxes produce no gap. A zero gap is left to alignment, which has already had its turn.

## Gestures

The runner is called from the tool during preview. The committed command receives the adjusted delta, the same way a move does today. Snap never becomes a separate undo step.

**Move**, including an Alt-drag copy. Passes: grid, baseline, guides, alignment, spacing. The submitted shape is the union of the selection's visible bounds. Snap is computed on the spread under the pointer, including a drag that crosses spreads. `Mods.cmd` (Command on macOS, Ctrl elsewhere) suspends the runner for that drag. Shift drops the smaller axis before any pass runs, and that axis is not offered to the runner.

**Resize**. The eight selection handles and the Scale tool. Passes: grid, baseline, guides, alignment, dimensions. A side handle submits the edge it moves. A corner submits both edges. Shift keeps the proportions: the free edge snaps, and the other edge follows the ratio. `Mods.cmd` still means scale the content, and the runner still runs. Space with more than one selected item is Live Distribute, and the runner does not run.

**Create**. The frame tools, on the rectangle or line being drawn. Same passes as resize. Spacing does not run. Shift constrains the shape, and the free edges still snap. A line's moving endpoint takes the position passes, and its length takes the dimensions pass.

**Rotate**. The selection rotate gesture and the Rotate tool. Only the dimensions pass runs, comparing the angle with other visible items' rotation angles, including 0. The angular tolerance is the angle subtended by the zone length at the pointer's distance from the rotation center. The zone length is `zone_px` converted with `tol`. Below a radius of 1 pt, the pass does not hit. Shift quantizes to 45 degrees, and while it is held the pass does not run.

**Points**. A pen snap follows the cursor and the click places the snapped point. A direct-selection anchor drag snaps during the drag. Both submit a point to the grid, baseline, guides, and alignment passes. No spacing and no dimensions.

Direction handles, the Shear tool, the Gap tool, the pencil, and the gradient tools do not snap.

## Switches

These are view settings on `UiState`. They are not stored in the document.

Three UI commands live under View > Grids & Guides, beside the existing show and hide items, and they are reachable from the menu, the control channel, and MCP like the other UI commands:

- `view.snapToGuides`, label "Snap to Guides", default on, shortcut Cmd+Shift+;.
- `view.snapToDocumentGrid`, label "Snap to Document Grid", default off, no shortcut.
- `view.smartGuides`, label "Smart Guides", default on, no shortcut.

The Properties smart-guides icon calls `view.smartGuides`. It no longer writes the bool itself.

The Guides & Pasteboard preferences page gains five controls, stored on `UiState` rather than in `document.preferences`:

- Align to Object Edges, default on.
- Align to Object Centers, default on.
- Smart Dimensions, default on.
- Smart Spacing, default on.
- Snap to Zone, in screen pixels, default 4.

A zone below 0 is saved as 0. A zone of 0 produces no hit. A non-finite value leaves the stored zone unchanged.

`ViewInfo` carries a `SnapView` and the document unit. `SnapView` holds the three switches, the four categories, the zone, and whether Show Guides is on. `view_info` copies `UiState.units` into that unit. `Session::with_ctx` copies both onto `ToolContext` and stops hardcoding `snap: true`. The old `snap: bool` is removed. `SnapView::default` and `ViewInfo::default` are the factory above, with Show Guides on and the unit in points, and both are const-constructible so existing `ViewInfo { zoom: 1.0 }` values can adopt the factory fields and keep compiling. `ui.pointer` does not grow parameters. It uses the app's current view.

A category runs only when Smart Guides is on and that category is on. Snap to Guides gates the baseline pass and the ruler, margin, and column pass. Show Guides also gates the ruler, margin, and column pass. Snap to Document Grid gates the grid pass. Hiding the document grid or the baseline grid does not turn those snaps off.

New labels are added to the UI localization table in the same change. The order is German, French, Spanish, Japanese:

- Snap to Guides: An Hilfslinien ausrichten; Coller aux repères; Ajustar a las guías; ガイドにスナップ.
- Snap to Document Grid: Am Dokumentraster ausrichten; Coller à la grille du document; Ajustar a la cuadrícula del documento; ドキュメントグリッドにスナップ.
- Smart Guides: Smarte Hilfslinien; Repères intelligents; Guías inteligentes; スマートガイド.
- Align to Object Edges: An Objektkanten ausrichten; Aligner sur les bords des objets; Alinear a los bordes del objeto; オブジェクトの端に揃える.
- Align to Object Centers: An Objektmitten ausrichten; Aligner sur les centres des objets; Alinear a los centros del objeto; オブジェクトの中心に揃える.
- Smart Dimensions: Smarte Maße; Mesures intelligentes; Dimensiones inteligentes; スマートサイズ.
- Smart Spacing: Smarte Abstände; Espacements intelligents; Espaciado inteligente; スマート間隔.
- Snap to Zone: Fangzone; Zone de magnétisme; Zona de ajuste; スナップゾーン.

## Overlays

The tool stores the overlays from the winning hits and returns them from `overlays`, as the selection tool and the frame tool already do. Only the winning pass on an axis draws.

The document grid, the baseline grid, ruler guides, margins, and columns draw nothing. The geometry jumps, and a guide that is already on the page is the feedback.

Object alignment uses `Overlay::Guide`. The canvas paints it at RGB 0, 200, 83. One segment per winning axis, extended 6 pt in spread space past the union of the two boxes, then transformed to canvas. Page edges and page centers use this same guide.

Equal spacing adds `Overlay::Gap { a, b, label }`. The canvas paints the same green line from `a` to `b` and a pill at the midpoint, using the existing measure-pill look. Every gap that shares the winning distance is drawn.

A width or height match uses `Overlay::Guide` along the matched side and `Overlay::Measure` with the length. A rotation match uses only `Overlay::Measure`, with the angle in degrees, no decimal when the angle is whole, and one decimal otherwise. The measure pill is anchored at the pointer. The canvas already offsets that pill. Lengths are formatted with `designcraft_geom::format_measure` in the unit carried on `ViewInfo`. This spec does not add a smart-guide color preference.

## Failure

A pass never fails the gesture and never returns an error. Non-finite coordinates, a zone of 0 or less, a grid spacing of 0 or less, and a baseline increment of 0 or less produce no hit. The drag keeps the pointer delta. No new code uses `unwrap`, `expect`, or `panic`. The grid and the baseline stay closed-form formulas. The parent walk stops on a repeated spread.

## Tests

Tests sit with the passes. A document is built only when the target comes from one.

- A guide and a closer object edge are both in range. The guide wins.
- A grid point and a closer guide are both in range. The grid wins.
- Shift locks an axis, and a target on that axis does not move it.
- `Mods.cmd` during a move leaves the delta unchanged. During a resize, the delta still snaps.
- A parent item with a 1 pt centered stroke attracts alignment at half a point outside the path.
- A resize against a rotated item matches that item's own side length. An item with sides 200 and 40 rotated 30 degrees matches 200 or 40, not its axis-aligned width.
- Spacing does not run while a frame is drawn. Dimensions do not run during a move.
- A hidden ruler guide is not a target. With Snap to Guides on, a hidden baseline still snaps. With Snap to Document Grid on, a hidden document grid still snaps.
- An Alt-drag copy can snap to the original item.
- The existing engine move onto the 36 pt margin still lands at x = 36.

## Code map

- `crates/tools/src/snap.rs`: one function per pass, and the runner.
- `crates/tools/src/lib.rs`: `SnapView`, `Overlay::Gap`, `ToolContext` loses `snap: bool`.
- `crates/tools/src/select.rs`, `frame.rs`, `xform.rs` (Scale and Rotate), `pen.rs`, direct-selection anchor drag: call the runner. Shear stays unsnapped.
- `crates/engine/src/tooling.rs`: `ViewInfo` gains `SnapView` and the unit. `with_ctx` copies them.
- `crates/ui-egui/src/menus.rs`: the three commands under Grids & Guides.
- `crates/ui-egui/src/lib.rs`: `UiState` fields and `view_info()`.
- `crates/ui-egui/src/dialogs.rs`: the five Guides & Pasteboard controls.
- `crates/ui-egui/src/panels/properties.rs`: the smart-guides icon calls the command.
- `crates/ui-egui/src/canvas.rs`: paint `Overlay::Gap` in the guide green, with a measure pill.
- `crates/ui-egui/src/i18n.rs`: the new labels.
- `crates/geom/src/snap.rs`: unchanged. The grid pass calls it.

Headless `ViewInfo` constants adopt the factory fields. `ui.pointer` is unchanged.

## Out of scope

Painting the document grid. A smart-guide color preference. Snapping for direction handles, Shear, the Gap tool, the pencil, or the gradient tools. Changing Live Distribute itself. Reworking resize math beyond giving the runner the edges and the length the handle is setting.
