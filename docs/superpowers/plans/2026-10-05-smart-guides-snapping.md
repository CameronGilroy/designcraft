# Smart guides and snapping implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the single always-on snap with the six-pass runner, view switches, and overlays in the spec.

**Architecture:** `SnapView` travels on `ViewInfo` into `ToolContext`. `designcraft_tools::snap::snap` runs six pass functions in order and keeps the first hit on each free axis. Tools submit a `SnapRequest` and preview the corrected delta. The UI only toggles `UiState` and paints the overlays.

**Tech Stack:** Rust, existing `designcraft_tools`, `designcraft_engine`, `designcraft_geom`, `designcraft_doc`, `designcraft_ui_egui`. No new crates.

**Spec:** `docs/superpowers/specs/2026-10-05-smart-guides-snapping-design.md`. Executors read the spec and this plan. The plan argues from the spec.

## Global Constraints

- A pass never fails a gesture and never returns an error. Non-finite input, a zone of 0 or less, a grid spacing of 0 or less, and a baseline increment of 0 or less produce no hit.
- No `unwrap`, `expect`, `panic`, or `unsafe` in non-test code. The tools crate already denies those lints.
- No new dependencies. Grid and baseline stay closed-form. Parent walks use a seen set.
- Factory: Snap to Guides on, Snap to Document Grid off, Smart Guides on, all four categories on, Show Guides on, zone 4 screen pixels.
- First pass that hits an axis wins. Inside a pass, the closest candidate wins. Pass order: document grid, baseline, ruler guides with margins and columns, alignment, spacing, dimensions.
- Do not paint the document grid. Do not add a smart-guide color preference. Do not snap direction handles, Shear, the Gap tool, the pencil, or the gradient tools.
- New menu and preference labels use the exact strings in the spec, including the German, French, Spanish, and Japanese rows.

## File map

- `crates/tools/src/lib.rs`: `SnapView`, `Gesture`, `SnapRequest`, `Overlay::Gap`, `ToolContext.snap` becomes `SnapView`, `ToolContext.unit`.
- `crates/tools/src/snap.rs`: the runner and the six passes. Tests live here.
- `crates/tools/src/select.rs`: move, resize, rotate, anchor drag.
- `crates/tools/src/frame.rs`: create.
- `crates/tools/src/xform.rs`: Scale and Rotate. Shear unchanged.
- `crates/tools/src/pen.rs`: point snap.
- `crates/tools/src/tests.rs`: the `ctx` helper's snap field.
- `crates/engine/src/tooling.rs`: `ViewInfo`.
- `crates/engine/src/lib.rs`: re-export `SnapView`.
- `crates/engine/src/tests.rs`: `ViewInfo::at_zoom`, gesture tests.
- `crates/mcp/src/headless.rs`: const view.
- `crates/ui-egui/src/lib.rs`: `UiState` fields and `view_info`.
- `crates/ui-egui/src/menus.rs`: three commands, menu, checked state, preference seed.
- `crates/ui-egui/src/dialogs.rs`: Guides & Pasteboard controls and apply.
- `crates/ui-egui/src/panels/properties.rs`: smart-guides icon.
- `crates/ui-egui/src/canvas.rs`: paint `Overlay::Gap`.
- `crates/ui-egui/src/i18n.rs`: the eight labels.
- `crates/geom/src/snap.rs`: unchanged. The grid pass calls `snap_to_grid`.

## Interfaces

Produced by Task 1 and consumed by every later task:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SnapView {
    pub snap_to_guides: bool,
    pub snap_to_document_grid: bool,
    pub show_guides: bool,
    pub smart_guides: bool,
    pub align_edges: bool,
    pub align_centers: bool,
    pub smart_dimensions: bool,
    pub smart_spacing: bool,
    pub zone_px: f64,
}

impl SnapView {
    pub const FACTORY: Self = Self {
        snap_to_guides: true,
        snap_to_document_grid: false,
        show_guides: true,
        smart_guides: true,
        align_edges: true,
        align_centers: true,
        smart_dimensions: true,
        smart_spacing: true,
        zone_px: 4.0,
    };
    pub const OFF: Self = Self {
        snap_to_guides: false,
        snap_to_document_grid: false,
        show_guides: true,
        smart_guides: false,
        align_edges: false,
        align_centers: false,
        smart_dimensions: false,
        smart_spacing: false,
        zone_px: 4.0,
    };
    pub fn any(self) -> bool {
        self.snap_to_guides || self.snap_to_document_grid || self.smart_guides
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture { Move, Resize, Create, Rotate, Point }

pub struct SnapRequest<'a> {
    pub spread: SpreadRef,
    pub gesture: Gesture,
    pub rect: Rect,
    /// Left, center, right. All false means the axis is locked.
    pub x_edges: [bool; 3],
    /// Top, center, bottom.
    pub y_edges: [bool; 3],
    pub exclude: &'a [ItemId],
    pub copying: bool,
    /// Proposed width and height for the dimensions pass. None when that length is not being set.
    pub lengths: [Option<f64>; 2],
    /// Proposed rotation in the same degrees `transform.rotate` already receives. None unless rotating.
    pub angle: Option<f64>,
    pub radius: f64,
    pub pointer: Point,
}

#[derive(Clone, Debug, Default)]
pub struct Snap {
    pub delta: Vec2,
    pub angle: Option<f64>,
    pub guides: Vec<Overlay>,
}

pub fn snap(cx: &ToolContext, req: SnapRequest<'_>) -> Snap
```

`ViewInfo` gains `pub snap: SnapView` and `pub unit: Unit`, plus `pub const fn at_zoom(zoom: f64) -> Self`. `ToolContext` stores the same two fields. `Overlay::Gap { a, b, label }` is the spacing segment in canvas coordinates.

### Task 1: SnapView reaches the tool

**Files:**
- Modify: `crates/tools/src/lib.rs` (SnapView, Gesture, SnapRequest, Snap, Overlay::Gap, ToolContext)
- Modify: `crates/tools/src/snap.rs` (temporary `snap` that delegates to today's `snap_rect` when `SnapView::any` is true)
- Modify: `crates/tools/src/select.rs` around the `if cx.snap` move check
- Modify: `crates/tools/src/frame.rs` around both `cx.snap` checks
- Modify: `crates/tools/src/tests.rs` helper `ctx`
- Modify: `crates/engine/src/tooling.rs`
- Modify: `crates/engine/src/lib.rs`
- Modify: `crates/engine/src/tests.rs` (five `ViewInfo { zoom: 1.0 }` sites)
- Modify: `crates/mcp/src/headless.rs`
- Modify: `crates/ui-egui/src/lib.rs` `view_info`
- Modify: `crates/ui-egui/src/canvas.rs` match arm
- Test: `crates/tools/src/snap.rs`

**Interfaces:**
- Consumes: nothing new
- Produces: the signatures in the Interfaces section. `snap` still uses the old closest-target behavior until Task 2.

- [ ] **Step 1: Write the failing test**

Add this to `crates/tools/src/snap.rs` inside `#[cfg(test)]`:

```rust
#[test]
fn factory_zone_is_four_pixels() {
    assert!(SnapView::FACTORY.snap_to_guides);
    assert!(!SnapView::FACTORY.snap_to_document_grid);
    assert!(SnapView::FACTORY.smart_guides);
    assert_eq!(SnapView::FACTORY.zone_px, 4.0);
    assert!(!SnapView::OFF.any());
    assert!(SnapView::FACTORY.any());
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p designcraft-tools factory_zone_is_four_pixels`

Expected: FAIL because `SnapView` does not exist.

- [ ] **Step 3: Add the types and keep today's snap behavior**

Put `SnapView`, `Gesture`, `SnapRequest`, and `Snap` in `crates/tools/src/lib.rs` and `pub use` them from `snap.rs` consumers. Replace `ToolContext.snap: bool` with `snap: SnapView` and add `unit: Unit`.

`snap` in `snap.rs` builds the old targets and returns a delta only when `cx.snap.any()` is true. A locked axis (`x_edges` or `y_edges` all false) contributes `0.0` on that axis. Non-finite `req.rect` returns `Snap::default()`.

Replace `if cx.snap` in `select.rs` and `cx.snap` in `frame.rs` with `cx.snap.any()`.

`ViewInfo`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ViewInfo {
    pub zoom: f64,
    pub snap: SnapView,
    pub unit: Unit,
}

impl ViewInfo {
    pub const fn at_zoom(zoom: f64) -> Self {
        Self { zoom, snap: SnapView::FACTORY, unit: Unit::Points }
    }
}

impl Default for ViewInfo {
    fn default() -> Self { Self::at_zoom(1.0) }
}
```

`with_ctx` copies `view.snap` and `view.unit` and still clamps zoom with `max(1e-6)`. Replace every `ViewInfo { zoom: 1.0 }` and the mcp `const VIEW` with `ViewInfo::at_zoom(1.0)`.

`view_info` returns `ViewInfo { zoom, snap: SnapView::FACTORY, unit: self.ui.units }` until Task 10 reads the real switches. The tools test helper uses `SnapView::OFF` and `Unit::Points`.

Add the canvas arm so the match stays exhaustive. Paint it in the guide green, RGB 0, 200, 83, and a pill using the same layout as `Overlay::Measure`, anchored at the segment midpoint with no extra 14 px offset:

```rust
Overlay::Gap { a, b, label } => {
    let (a, b) = (xf.to_screen(a), xf.to_screen(b));
    painter.line_segment([a, b], Stroke::new(1.0, Color32::from_rgb(0, 200, 83)));
    let s = a + (b - a) * 0.5;
    let g = painter.layout_no_wrap(label, egui::FontId::proportional(11.0), Color32::WHITE);
    let r = Rect::from_min_size(s, g.size() + vec2(10.0, 6.0));
    painter.rect_filled(r, 3.0, Color32::from_rgba_unmultiplied(70, 70, 70, 230));
    painter.galley(r.min + vec2(5.0, 3.0), g, Color32::WHITE);
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p designcraft-tools factory_zone_is_four_pixels`

Run: `cargo test -p designcraft-engine tool_gesture_creates_one_undo_step`

Expected: both PASS. The margin move still lands at x = 36 because the factory zone is 4 and that drag finishes 2 pt away.

- [ ] **Step 5: Commit**

```bash
git add crates/tools/src/lib.rs crates/tools/src/snap.rs crates/tools/src/select.rs crates/tools/src/frame.rs crates/tools/src/tests.rs crates/engine/src/tooling.rs crates/engine/src/lib.rs crates/engine/src/tests.rs crates/mcp/src/headless.rs crates/ui-egui/src/lib.rs crates/ui-egui/src/canvas.rs
git commit -m "Snap: carry SnapView on the tool context"
```

### Task 2: Guides and alignment, first hit wins

**Files:**
- Modify: `crates/tools/src/snap.rs`
- Test: `crates/tools/src/snap.rs`

**Interfaces:**
- Consumes: `snap`, `SnapRequest`, `SnapView`
- Produces: guides pass and alignment pass inside `snap`. Guide hits draw nothing. Alignment hits push `Overlay::Guide` extended 6 pt in spread space past the union of the two boxes, then transformed with `cx.layout.xf(spread)`.

- [ ] **Step 1: Write the failing tests**

```rust
fn doc_with_items(moving: Rect, other: Rect, guide_x: f64) -> (Document, ItemId, ItemId) {
    let mut doc = Document::new(&NewDocument::default());
    let layer = doc.default_layer();
    let make = |doc: &mut Document, rect: Rect| {
        let id = ItemId(doc.alloc());
        let item = Item::new(id, layer, Shape::Rectangle, shapes::rectangle(rect));
        doc.insert_item(SpreadRef::Doc(0), item, None).unwrap();
        id
    };
    let moving_id = make(&mut doc, moving);
    let other_id = make(&mut doc, other);
    let page = &mut Arc::make_mut(&mut doc.spreads[0]).pages[0];
    page.guides.push(Guide {
        orientation: Orientation::Vertical,
        position: guide_x,
        spread: false,
        locked: false,
        layer: None,
        liquid: false,
    });
    (doc, moving_id, other_id)
}

#[test]
fn guide_beats_a_closer_object_edge() {
    // Right edge at 100.5. Object edge at 100 (0.5 away). Guide at 104 (3.5 away). Zone is 4.
    let (doc, moving, _other) = doc_with_items(
        Rect::new(80.0, 100.0, 100.5, 140.0),
        Rect::new(90.0, 100.0, 100.0, 160.0),
        104.0,
    );
    let cache = Cache::new();
    let layout = CanvasLayout::new(&doc, false);
    let sel = Selection::default();
    let mut cx = ctx_on(&doc, &sel, &cache, &layout);
    cx.snap.zone_px = 4.0;
    let hit = snap(&cx, SnapRequest {
        spread: SpreadRef::Doc(0),
        gesture: Gesture::Move,
        rect: Rect::new(80.0, 100.0, 100.5, 140.0),
        x_edges: [true, true, true],
        y_edges: [false, false, false],
        exclude: &[moving],
        copying: false,
        lengths: [None, None],
        angle: None,
        radius: 0.0,
        pointer: Point::new(0.0, 0.0),
    });
    assert!((hit.delta.x - 3.5).abs() < 1e-6);
    assert!(hit.guides.is_empty());
}

#[test]
fn object_edge_snaps_when_guide_snap_is_off() {
    let (doc, moving, _other) = doc_with_items(
        Rect::new(80.0, 100.0, 100.5, 140.0),
        Rect::new(90.0, 100.0, 100.0, 160.0),
        104.0,
    );
    let cache = Cache::new();
    let layout = CanvasLayout::new(&doc, false);
    let sel = Selection::default();
    let mut cx = ctx_on(&doc, &sel, &cache, &layout);
    cx.snap.snap_to_guides = false;
    let hit = snap(&cx, SnapRequest {
        spread: SpreadRef::Doc(0),
        gesture: Gesture::Move,
        rect: Rect::new(80.0, 100.0, 100.5, 140.0),
        x_edges: [true, true, true],
        y_edges: [false, false, false],
        exclude: &[moving],
        copying: false,
        lengths: [None, None],
        angle: None,
        radius: 0.0,
        pointer: Point::new(0.0, 0.0),
    });
    assert!((hit.delta.x - -0.5).abs() < 1e-6);
    assert!(matches!(hit.guides.first(), Some(Overlay::Guide { .. })));
}

#[test]
fn hidden_layer_guide_is_not_a_target() {
    let (mut doc, moving, _) = doc_with_items(
        Rect::new(80.0, 100.0, 102.0, 140.0),
        Rect::new(300.0, 100.0, 340.0, 140.0),
        104.0,
    );
    let layer = doc.default_layer();
    doc.layers.iter_mut().find(|l| l.id == layer).unwrap().visible = false;
    Arc::make_mut(&mut doc.spreads[0]).pages[0].guides[0].layer = Some(layer);
    let cache = Cache::new();
    let layout = CanvasLayout::new(&doc, false);
    let sel = Selection::default();
    let cx = ctx_on(&doc, &sel, &cache, &layout);
    let hit = snap(&cx, SnapRequest {
        spread: SpreadRef::Doc(0),
        gesture: Gesture::Move,
        rect: Rect::new(80.0, 100.0, 102.0, 140.0),
        x_edges: [false, false, true],
        y_edges: [false, false, false],
        exclude: &[moving],
        copying: false,
        lengths: [None, None],
        angle: None,
        radius: 0.0,
        pointer: Point::new(0.0, 0.0),
    });
    assert_eq!(hit.delta.x, 0.0);
}
```

```rust
fn ctx_on<'a>(doc: &'a Document, sel: &'a Selection, cache: &'a Cache, layout: &'a CanvasLayout) -> ToolContext<'a> {
    ToolContext {
        doc, selection: sel, cache, layout,
        zoom: 1.0, layer: doc.default_layer(),
        snap: SnapView::FACTORY, unit: Unit::Points,
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p designcraft-tools guide_beats_a_closer_object_edge`

Expected: FAIL. Today's closest-target snap returns about -0.5.

- [ ] **Step 3: Split guides and alignment**

Replace the body of `snap`. For each free axis, try the guides pass, then the alignment pass. Later passes are empty functions that return `None` until their tasks.

Guides pass, only when `snap_to_guides && show_guides`: vertical ruler guides that `Guide::visible_in` accepts, page `margin_rect` left and right, and each `column_rects` left and right. A page guide's perpendicular range is that page. A `spread: true` guide uses the spread bounds. Locked guides are included. No overlay.

Alignment pass, only when `smart_guides`: page `bounds` edges when `align_edges`, page centers when `align_centers`, and the same for each top-level item that is not excluded and not hidden. Use `Item::visible_bounds` for items. Emit `Overlay::Guide`.

Centers are offered by the request's middle flag and still dropped when the matching category is off. Page edges follow `align_edges`. Page centers follow `align_centers`.

`snap_point` calls `snap` with a zero-size rect and all six edge flags true.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p designcraft-tools guide_beats_a_closer_object_edge object_edge_snaps_when_guide_snap_is_off hidden_layer_guide_is_not_a_target`

Run: `cargo test -p designcraft-engine tool_gesture_creates_one_undo_step`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/tools/src/snap.rs
git commit -m "Snap: guides beat object alignment"
```

### Task 3: Move honors Shift, Command, and Alt-drag

**Files:**
- Modify: `crates/tools/src/select.rs` `Drag::Move` arm
- Modify: `crates/tools/src/snap.rs` (ignore `exclude` when `copying` is true)
- Test: `crates/engine/src/tests.rs`
- Test: `crates/tools/src/snap.rs`

**Interfaces:**
- Consumes: `snap`, `SnapRequest`
- Produces: the move arm submits locked axes as `[false, false, false]`, skips `snap` entirely when `ev.mods.cmd`, and sets `copying: ev.mods.alt`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn shift_move_does_not_take_the_other_axis() {
    let mut s = session();
    s.execute("frame.create", &json!({"rect": [80.0, 100.0, 140.0, 140.0], "content": "unassigned"})).unwrap();
    s.execute("guide.add", &json!({"orientation": "horizontal", "position": 102.0, "page": 0})).unwrap();
    s.set_tool("selection");
    let v = ViewInfo::at_zoom(1.0);
    let shift = designcraft_tools::Mods { shift: true, ..Default::default() };
    s.pointer(&PointerEvent::new(PointerKind::Down, 100.0, 120.0).with_mods(shift), v).unwrap();
    s.pointer(&PointerEvent::new(PointerKind::Drag, 160.0, 121.0).with_mods(shift), v).unwrap();
    s.pointer(&PointerEvent::new(PointerKind::Up, 160.0, 121.0).with_mods(shift), v).unwrap();
    let b = s.doc().unwrap().doc.spreads[0].items[0].bounds();
    assert!((b.y0 - 100.0).abs() < 1e-6, "locked axis jumped to {b:?}");
    assert!(b.x0 > 80.0);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p designcraft-engine shift_move_does_not_take_the_other_axis`

Expected: FAIL. `y0` becomes 102 because snap runs after Shift zeroes the axis.

- [ ] **Step 3: Build the request after the Shift clamp**

In the move arm, after Shift zeroes one component of `d`, set that axis's edge flags to `[false, false, false]`. If `ev.mods.cmd`, do not call `snap`. Otherwise call `snap` with `copying: ev.mods.alt` and add `hit.delta` only on axes whose flags are not all false. Store `hit.guides`.

Add this test in `crates/tools/src/snap.rs`. The original is excluded and `copying` is true, so the pass must still target it:

```rust
#[test]
fn alt_copy_snaps_to_the_original() {
    let mut doc = Document::new(&NewDocument::default());
    let layer = doc.default_layer();
    let id = ItemId(doc.alloc());
    let item = Item::new(id, layer, Shape::Rectangle, shapes::rectangle(Rect::new(100.0, 40.0, 160.0, 80.0)));
    doc.insert_item(SpreadRef::Doc(0), item, None).unwrap();
    let cache = Cache::new();
    let layout = CanvasLayout::new(&doc, false);
    let sel = Selection::default();
    let mut cx = ctx_on(&doc, &sel, &cache, &layout);
    cx.snap.snap_to_guides = false;
    let hit = snap(&cx, SnapRequest {
        spread: SpreadRef::Doc(0),
        gesture: Gesture::Move,
        rect: Rect::new(162.0, 40.0, 222.0, 80.0),
        x_edges: [true, false, false],
        y_edges: [false, false, false],
        exclude: &[id],
        copying: true,
        lengths: [None, None],
        angle: None,
        radius: 0.0,
        pointer: Point::new(0.0, 0.0),
    });
    assert!((hit.delta.x - -2.0).abs() < 1e-6);
}
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p designcraft-engine shift_move_does_not_take_the_other_axis tool_gesture_creates_one_undo_step`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/tools/src/select.rs crates/tools/src/snap.rs crates/engine/src/tests.rs
git commit -m "Snap: Shift and Command on a move"
```

### Task 4: Document grid and baseline

**Files:**
- Modify: `crates/tools/src/snap.rs`
- Test: `crates/tools/src/snap.rs`

**Interfaces:**
- Consumes: `designcraft_geom::snap_to_grid`, `Document::grid` (`horizontal`, `vertical`, `subdivisions`), `Document` baseline grid (`start`, `increment`). Do not read `relative_to` or `view_threshold`.
- Produces: grid pass first, baseline pass second. Neither pushes an overlay.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn grid_beats_a_closer_guide() {
    let mut doc = Document::new(&NewDocument::default());
    doc.grid.horizontal = 72.0;
    doc.grid.vertical = 72.0;
    doc.grid.subdivisions = 8; // 9 pt
    let layer = doc.default_layer();
    let id = ItemId(doc.alloc());
    let item = Item::new(id, layer, Shape::Rectangle, shapes::rectangle(Rect::new(10.0, 10.0, 40.0, 40.0)));
    doc.insert_item(SpreadRef::Doc(0), item, None).unwrap();
    // Guide 1 pt away at x = 11. Subdivision line at 9 is 1 pt the other way. Grid must win.
    Arc::make_mut(&mut doc.spreads[0]).pages[0].guides.push(Guide {
        orientation: Orientation::Vertical, position: 11.0, spread: true,
        locked: false, layer: None, liquid: false,
    });
    let cache = Cache::new();
    let layout = CanvasLayout::new(&doc, false);
    let sel = Selection::default();
    let mut cx = ctx_on(&doc, &sel, &cache, &layout);
    cx.snap.snap_to_document_grid = true;
    let hit = snap(&cx, move_req(&doc, Rect::new(10.0, 10.0, 40.0, 40.0), &[id]));
    assert!((hit.delta.x - -1.0).abs() < 1e-6);
    assert!(hit.guides.is_empty());
}

#[test]
fn hidden_baseline_still_snaps_and_zero_increment_does_not() {
    let mut doc = Document::new(&NewDocument::default());
    doc.baseline_grid.start = 36.0;
    doc.baseline_grid.increment = 12.0;
    let cache = Cache::new();
    let layout = CanvasLayout::new(&doc, false);
    let sel = Selection::default();
    let cx = ctx_on(&doc, &sel, &cache, &layout);
    // Top at 37 wants the line at 36.
    let hit = snap(&cx, y_req(Rect::new(100.0, 37.0, 140.0, 80.0)));
    assert!((hit.delta.y - -1.0).abs() < 1e-6);
    let mut dead = doc.clone();
    dead.baseline_grid.increment = 0.0;
    let layout = CanvasLayout::new(&dead, false);
    let cx = ctx_on(&dead, &sel, &cache, &layout);
    let hit = snap(&cx, y_req(Rect::new(100.0, 37.0, 140.0, 80.0)));
    assert_eq!(hit.delta.y, 0.0);
}
```

```rust
fn y_req(rect: Rect) -> SnapRequest<'static> {
    SnapRequest {
        spread: SpreadRef::Doc(0), gesture: Gesture::Move, rect,
        x_edges: [false, false, false], y_edges: [true, true, true],
        exclude: &[], copying: false, lengths: [None, None],
        angle: None, radius: 0.0, pointer: Point::new(0.0, 0.0),
    }
}

fn move_req<'a>(rect: Rect, exclude: &'a [ItemId]) -> SnapRequest<'a> {
    SnapRequest {
        spread: SpreadRef::Doc(0), gesture: Gesture::Move, rect,
        x_edges: [true, true, true], y_edges: [false, false, false],
        exclude, copying: false, lengths: [None, None],
        angle: None, radius: 0.0, pointer: Point::new(0.0, 0.0),
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p designcraft-tools grid_beats_a_closer_guide hidden_baseline_still_snaps`

Expected: FAIL. Those passes still return None.

- [ ] **Step 3: Implement the two passes**

Grid, when `snap_to_document_grid`: subdivision count below 1 counts as 1. Interval is `horizontal / count` and `vertical / count`. Origin is the page bounds origin of the page that contains the rect center. Call `snap_to_grid`. A spacing of 0 or less skips that axis. No overlay. This pass does not look at a "grid shown" flag.

Baseline, when `snap_to_guides`: lines at `page.y0 + start + n * increment` for n >= 0 while the line is within the page. Increment of 0 or less skips the pass. Ignore `view_threshold` and `relative_to`. No overlay.

Both passes consider only the edge flags that are true.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p designcraft-tools grid_beats_a_closer_guide hidden_baseline_still_snaps guide_beats_a_closer_object_edge`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/tools/src/snap.rs
git commit -m "Snap: document grid and baseline passes"
```

### Task 5: Parent items, groups, and stroke

**Files:**
- Modify: `crates/tools/src/snap.rs` alignment target collection
- Test: `crates/tools/src/snap.rs`

**Interfaces:**
- Consumes: `Document::parent_page_for`, `Page::show_parent_items`, `ParentInfo::based_on`, `Item::visible_bounds`
- Produces: alignment targets include parent items and group children. A 1 pt centered stroke attracts at half a point outside the path.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn parent_centered_stroke_attracts_at_the_outer_edge() {
    let mut doc = Document::new(&NewDocument::default());
    let pid = doc.add_parent("A", "A-Parent", 1, 12.0, Margins::uniform(36.0));
    doc.apply_parent(&[0], Some(pid)).unwrap();
    let layer = doc.default_layer();
    let id = ItemId(doc.alloc());
    let mut item = Item::new(id, layer, Shape::Rectangle, shapes::rectangle(Rect::new(72.0, 140.0, 252.0, 300.0)));
    item.stroke = Stroke::default(); // 1 pt, center
    doc.insert_item(SpreadRef::Parent(0), item, None).unwrap();
    let cache = Cache::new();
    let layout = CanvasLayout::new(&doc, false);
    let sel = Selection::default();
    let mut cx = ctx_on(&doc, &sel, &cache, &layout);
    cx.snap.snap_to_guides = false;
    // Moving left edge at 253. Visible parent right edge is 252.5.
    let hit = snap(&cx, SnapRequest {
        spread: SpreadRef::Doc(0),
        gesture: Gesture::Move,
        rect: Rect::new(253.0, 140.0, 353.0, 240.0),
        x_edges: [true, false, false],
        y_edges: [false, false, false],
        exclude: &[],
        copying: false,
        lengths: [None, None],
        angle: None,
        radius: 0.0,
        pointer: Point::new(0.0, 0.0),
    });
    assert!((hit.delta.x - -0.5).abs() < 1e-6, "delta {}", hit.delta.x);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p designcraft-tools parent_centered_stroke_attracts_at_the_outer_edge`

Expected: FAIL. Parent items are not targets, so the delta is 0.

- [ ] **Step 3: Collect parent and group targets**

When gathering alignment targets for a document page, if `show_parent_items` is set, resolve `parent_page_for`. Walk `based_on` with a `HashSet<SpreadId>`. Stop when an id repeats. Translate parent-space bounds into document-spread space the same way `ToolContext::hit_parent_on_page` does: add `page.x - parent_page.x` to x, and leave y. Skip ids in `page.overridden`.

For a group, the target box is the union of the children's `visible_bounds`, transformed by the group's transform. Do not use the group's own stroke box when it has children.

While `layout` was built for parent editing (`editing_parents`), parent items are ordinary spread items. Do not add them again.

- [ ] **Step 4: Run the test**

Run: `cargo test -p designcraft-tools parent_centered_stroke_attracts_at_the_outer_edge guide_beats_a_closer_object_edge`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/tools/src/snap.rs
git commit -m "Snap: parent items use the stroked edge"
```

### Task 6: Dimensions on resize, scale, and create

**Files:**
- Modify: `crates/tools/src/snap.rs`
- Modify: `crates/tools/src/select.rs` resize arm
- Modify: `crates/tools/src/xform.rs` scale arm
- Modify: `crates/tools/src/frame.rs` drag arm
- Test: `crates/tools/src/snap.rs`
- Test: `crates/engine/src/tests.rs`

**Interfaces:**
- Consumes: `designcraft_geom::decompose`, `SnapRequest::lengths`, `Gesture::Resize` and `Gesture::Create`
- Produces: dimensions pass. A rotated item contributes its own side lengths. Spacing still returns None.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn rotated_item_matches_its_own_side() {
    let mut doc = Document::new(&NewDocument::default());
    let layer = doc.default_layer();
    let id = ItemId(doc.alloc());
    let mut item = Item::new(id, layer, Shape::Rectangle, shapes::rectangle(Rect::new(0.0, 0.0, 200.0, 40.0)));
    item.xf = Affine::rotate((-30.0_f64).to_radians());
    doc.insert_item(SpreadRef::Doc(0), item, None).unwrap();
    let cache = Cache::new();
    let layout = CanvasLayout::new(&doc, false);
    let sel = Selection::default();
    let mut cx = ctx_on(&doc, &sel, &cache, &layout);
    cx.snap.snap_to_guides = false;
    cx.snap.snap_to_document_grid = false;
    // Width 193.2 is the axis-aligned box. 200 is the side. Zone 4 reaches 200 from 197, not from 193.
    let hit = snap(&cx, SnapRequest {
        spread: SpreadRef::Doc(0),
        gesture: Gesture::Resize,
        rect: Rect::new(80.0, 200.0, 277.0, 280.0),
        x_edges: [false, false, true],
        y_edges: [false, false, false],
        exclude: &[],
        copying: false,
        lengths: [Some(197.0), None],
        angle: None,
        radius: 0.0,
        pointer: Point::new(277.0, 240.0),
    });
    assert!((hit.delta.x - 3.0).abs() < 1e-6);
    assert!(matches!(hit.guides.last(), Some(Overlay::Measure { text, .. }) if text.contains("200")));
}
```

`designcraft_geom::decompose` reports `Affine::rotate((-30.0_f64).to_radians())` as rotation `30.0`. The own width is 200. The delta is `200 - 197`.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p designcraft-tools rotated_item_matches_its_own_side`

Expected: FAIL. The dimensions pass returns None.

- [ ] **Step 3: Implement dimensions and call it from the three gestures**

Own size: path bounds in item space, inflated the same way `visible_bounds` inflates (half the weight when centered, the whole weight when outside, nothing when inside, and nothing when `stroke` is none), then multiplied by `decompose(item.xf).scale_x.abs()` and `scale_y.abs()`. Both numbers are candidates for whichever length is set.

The pass runs only for `Gesture::Resize` or `Gesture::Create`, when `smart_guides && smart_dimensions`. Closest length within the zone wins. Delta on that axis is the length difference (the rect grows or shrinks from the moving edge). Push `Overlay::Guide` along the matched side and `Overlay::Measure` at `req.pointer` whose text is `format_measure(length, cx.unit)`.

Resize arm in `select.rs`: if `ev.mods.space && selection.len() > 1`, skip `snap`. Otherwise build `to` with `resize_rect`. Edge flags: handle 3 sets right only, handle 7 sets left only, handle 1 sets top only, handle 5 sets bottom only, corners set the two edges they move. Centers stay false. Lengths are `to.width()` and `to.height()` for the axes that move, plus the stroke pad of a single selected item (`visible` width minus path width) so a centered stroke matches the outer size. If Shift is held, snap the free edge, then set the other edge from the original aspect ratio. `Mods.cmd` does not skip snap. Put `hit.guides` on the tool.

Scale tool: treat the drag as a resize of both axes around the selection center. Same passes. Shear stays a direct preview with no `snap` call.

Frame drag: compute the constrained rect only after the snap when Shift is held. Delete the `!ev.mods.shift` guard that skips snapping. Call `snap` with `Gesture::Create` on the rect from the drag. A line snaps the moving endpoint with the position passes and passes its length in `lengths[0]`. Spacing must not run for `Gesture::Create`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p designcraft-tools rotated_item_matches_its_own_side frame_tool_drag_emits_begin_preview_commit`

Run: `cargo test -p designcraft-engine tool_gesture_creates_one_undo_step`

Expected: PASS. The frame test still uses `SnapView::OFF`, so its rect stays `[10, 10, 110, 60]`.

- [ ] **Step 5: Commit**

```bash
git add crates/tools/src/snap.rs crates/tools/src/select.rs crates/tools/src/xform.rs crates/tools/src/frame.rs crates/engine/src/tests.rs
git commit -m "Snap: dimensions on resize and create"
```

### Task 7: Equal spacing on moves

**Files:**
- Modify: `crates/tools/src/snap.rs`
- Test: `crates/tools/src/snap.rs`

**Interfaces:**
- Consumes: `Gesture::Move`, `smart_spacing`, `Overlay::Gap`
- Produces: spacing pass after alignment. Create and resize do not call it.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn move_matches_a_stationary_gap() {
    let mut doc = Document::new(&NewDocument::default());
    let layer = doc.default_layer();
    // Stationary gap of 20 between [0,40] and [60,90]. Moving box at [112,150], 22 away from 90.
    for r in [Rect::new(0.0, 0.0, 40.0, 30.0), Rect::new(60.0, 0.0, 90.0, 30.0), Rect::new(112.0, 0.0, 150.0, 30.0)] {
        let id = ItemId(doc.alloc());
        let item = Item::new(id, layer, Shape::Rectangle, shapes::rectangle(r));
        doc.insert_item(SpreadRef::Doc(0), item, None).unwrap();
    }
    let moving = doc.spreads[0].items[2].id;
    let cache = Cache::new();
    let layout = CanvasLayout::new(&doc, false);
    let sel = Selection::default();
    let mut cx = ctx_on(&doc, &sel, &cache, &layout);
    cx.snap.snap_to_guides = false;
    let hit = snap(&cx, SnapRequest {
        spread: SpreadRef::Doc(0),
        gesture: Gesture::Move,
        rect: Rect::new(112.0, 0.0, 150.0, 30.0),
        x_edges: [true, true, true],
        y_edges: [false, false, false],
        exclude: &[moving],
        copying: false,
        lengths: [None, None],
        angle: None,
        radius: 0.0,
        pointer: Point::new(0.0, 0.0),
    });
    assert!((hit.delta.x - -2.0).abs() < 1e-6);
    assert!(hit.guides.iter().any(|g| matches!(g, Overlay::Gap { .. })));
}

#[test]
fn create_does_not_match_spacing() {
    let mut doc = Document::new(&NewDocument::default());
    let layer = doc.default_layer();
    for r in [Rect::new(0.0, 0.0, 40.0, 30.0), Rect::new(60.0, 0.0, 90.0, 30.0)] {
        let id = ItemId(doc.alloc());
        let item = Item::new(id, layer, Shape::Rectangle, shapes::rectangle(r));
        doc.insert_item(SpreadRef::Doc(0), item, None).unwrap();
    }
    let cache = Cache::new();
    let layout = CanvasLayout::new(&doc, false);
    let sel = Selection::default();
    let mut cx = ctx_on(&doc, &sel, &cache, &layout);
    cx.snap.snap_to_guides = false;
    cx.snap.smart_dimensions = false;
    let hit = snap(&cx, SnapRequest {
        spread: SpreadRef::Doc(0),
        gesture: Gesture::Create,
        rect: Rect::new(112.0, 0.0, 150.0, 30.0),
        x_edges: [true, true, true],
        y_edges: [false, false, false],
        exclude: &[],
        copying: false,
        lengths: [Some(38.0), None],
        angle: None,
        radius: 0.0,
        pointer: Point::new(0.0, 0.0),
    });
    assert!((hit.delta.x).abs() < 1e-6);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p designcraft-tools move_matches_a_stationary_gap`

Expected: FAIL on `move_matches_a_stationary_gap` because the delta is 0. `create_does_not_match_spacing` already passes while the pass is absent. Keep it. It fails later if create starts returning the spacing delta.

- [ ] **Step 3: Implement the spacing pass**

Only `Gesture::Move`, only when `smart_guides && smart_spacing`. On one axis, keep boxes whose ranges overlap on the other axis. Nearest neighbor on each side of the moving box. Stationary nearest-neighbor gaps between the other boxes. A candidate makes one moving gap equal one stationary gap, and the shift must lie inside the zone. Closest shift wins. Overlaps and zero gaps produce no candidate. Push one `Overlay::Gap` per gap that equals the winning distance, with `label` from `format_measure`. Endpoints are the facing edges, in canvas space.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p designcraft-tools move_matches_a_stationary_gap create_does_not_match_spacing`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/tools/src/snap.rs
git commit -m "Snap: equal spacing on moves"
```

### Task 8: Rotation match

**Files:**
- Modify: `crates/tools/src/snap.rs`
- Modify: `crates/tools/src/select.rs` rotate arm
- Modify: `crates/tools/src/xform.rs` rotate arm
- Test: `crates/tools/src/snap.rs`

**Interfaces:**
- Consumes: `SnapRequest::angle`, `SnapRequest::radius`, `decompose(item.xf).rotation`
- Produces: `Snap.angle` as the replacement degrees. The two rotate callers use it instead of the raw angle when it is `Some`. Shift still quantizes to 45 and does not call the pass.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn rotation_matches_another_items_angle() {
    let mut doc = Document::new(&NewDocument::default());
    let layer = doc.default_layer();
    let id = ItemId(doc.alloc());
    let mut item = Item::new(id, layer, Shape::Rectangle, shapes::rectangle(Rect::new(0.0, 0.0, 80.0, 40.0)));
    item.xf = Affine::rotate((-30.0_f64).to_radians());
    doc.insert_item(SpreadRef::Doc(0), item, None).unwrap();
    let cache = Cache::new();
    let layout = CanvasLayout::new(&doc, false);
    let sel = Selection::default();
    let mut cx = ctx_on(&doc, &sel, &cache, &layout);
    cx.snap.snap_to_guides = false;
    let hit = snap(&cx, SnapRequest {
        spread: SpreadRef::Doc(0),
        gesture: Gesture::Rotate,
        rect: Rect::new(100.0, 100.0, 160.0, 140.0),
        x_edges: [false, false, false],
        y_edges: [false, false, false],
        exclude: &[],
        copying: false,
        lengths: [None, None],
        angle: Some(28.0),
        radius: 80.0,
        pointer: Point::new(180.0, 120.0),
    });
    let angle = hit.angle.expect("rotation hit");
    assert!((angle - 30.0).abs() < 1e-6);
    assert!(matches!(hit.guides.first(), Some(Overlay::Measure { .. })));
}
```

`decompose(Affine::rotate((-30.0_f64).to_radians())).rotation` is 30.0. The hit must be 30.0.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p designcraft-tools rotation_matches_another_items_angle`

Expected: FAIL. `hit.angle` is None.

- [ ] **Step 3: Implement the rotation pass and both callers**

The pass runs only for `Gesture::Rotate` when `smart_guides && smart_dimensions` and `angle` is `Some`. If `radius < 1.0`, return None. Angular tolerance is `(zone_length / radius).atan().to_degrees()`, where `zone_length` is `cx.tol(zone_px)`. Candidates are `decompose(item.xf).rotation` for visible non-excluded items, including 0. Closest within the tolerance wins. `Snap.angle` is that angle. The measure text is the angle in degrees, with no decimal when it is a whole number and one decimal otherwise. No guide line.

Selection rotate and the Rotate tool: when Shift is down, keep the 45 degree quantize and do not call `snap`. Otherwise call `snap` and, when `hit.angle` is `Some`, preview that angle with the same sign the tool already sends (`-a` in the selection tool).

- [ ] **Step 4: Run the test**

Run: `cargo test -p designcraft-tools rotation_matches_another_items_angle`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/tools/src/snap.rs crates/tools/src/select.rs crates/tools/src/xform.rs
git commit -m "Snap: rotation matches another angle"
```

### Task 9: Pen and anchor points

**Files:**
- Modify: `crates/tools/src/pen.rs`
- Modify: `crates/tools/src/select.rs` anchor arm
- Test: `crates/tools/src/tests.rs`

**Interfaces:**
- Consumes: `Gesture::Point`, zero-size rect, all six edge flags true, `lengths: [None, None]`
- Produces: snapped anchor position. Direction-handle drags (`handle: Some`) do not call `snap`.

- [ ] **Step 1: Write the failing test**

`commit_anchor` emits `path.create` only once two anchors exist. The param is `anchors[i].p = [x, y]`. The first click is 2 pt from the guide. The second click is far from it, so only the first x changes.

```rust
#[test]
fn pen_click_snaps_to_a_guide() {
    let mut d = Document::new(&NewDocument::default());
    Arc::make_mut(&mut d.spreads[0]).pages[0].guides.push(Guide {
        orientation: Orientation::Vertical, position: 100.0, spread: true,
        locked: false, layer: None, liquid: false,
    });
    let (s, c, l) = (Selection::default(), Cache::new(), CanvasLayout::new(&d, false));
    let mut cx = ctx(&d, &s, &c, &l);
    cx.snap = SnapView::FACTORY;
    let mut t = create("pen");
    let click = |t: &mut Box<dyn Tool>, cx: &ToolContext, x: f64, y: f64| {
        t.pointer(cx, &PointerEvent::new(PointerKind::Down, x, y));
        t.pointer(cx, &PointerEvent::new(PointerKind::Up, x, y))
    };
    assert!(click(&mut t, &cx, 102.0, 80.0).is_empty());
    let up = click(&mut t, &cx, 180.0, 80.0);
    let Action::Exec(cmd, p) = &up[0] else { panic!("expected path.create, got {up:?}"); };
    assert_eq!(cmd, "path.create");
    let x = p["anchors"][0]["p"][0].as_f64().unwrap();
    assert!((x - 100.0).abs() < 1e-6, "anchor x {x}");
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p designcraft-tools pen_click_snaps_to_a_guide`

Expected: FAIL. The anchor stays at 102.

- [ ] **Step 3: Snap the point before it is stored**

In `PenTool::pointer`, after Shift angle constraint, call `snap` with `Gesture::Point` and a zero-size rect at the spread point. Store the snapped spread point. On `PointerKind::Move`, store the guide overlays and return them from `overlays` so the guide is visible before the click. A direct-selection anchor drag with `handle: None` adds the snap delta to `dx` and `dy`. A drag with `handle: Some` is unchanged.

- [ ] **Step 4: Run the test**

Run: `cargo test -p designcraft-tools pen_click_snaps_to_a_guide`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/tools/src/pen.rs crates/tools/src/select.rs crates/tools/src/tests.rs
git commit -m "Snap: pen and anchor points"
```

### Task 10: Commands, preferences, and labels

**Files:**
- Modify: `crates/ui-egui/src/lib.rs`
- Modify: `crates/ui-egui/src/menus.rs`
- Modify: `crates/ui-egui/src/dialogs.rs`
- Modify: `crates/ui-egui/src/panels/properties.rs`
- Modify: `crates/ui-egui/src/i18n.rs`
- Test: `crates/ui-egui/src/lib.rs`

**Interfaces:**
- Consumes: `SnapView::FACTORY`
- Produces: `view.snapToGuides`, `view.snapToDocumentGrid`, `view.smartGuides`. `view_info` copies the live `UiState` into `SnapView`.

- [ ] **Step 1: Write the failing assertion**

In `crates/ui-egui/src/lib.rs` tests, or a new `#[cfg(test)]` module there:

```rust
#[test]
fn snap_view_uses_the_saved_switches() {
    let mut ui = UiState::default();
    assert!(ui.snap_view().snap_to_guides);
    assert!(!ui.snap_view().snap_to_document_grid);
    assert_eq!(ui.snap_view().zone_px, 4.0);
    ui.snap_to_guides = false;
    ui.snap_zone = 0.0;
    assert!(!ui.snap_view().snap_to_guides);
    assert_eq!(ui.snap_view().zone_px, 0.0);
}
```

Add `fn snap_view(&self) -> SnapView` as the thing under test. Bool fields that default on need `#[serde(default = "default_true")]` because a container `#[serde(default)]` fills a missing field from the field type, and `bool::default` is false. Use the same function for `align_edges`, `align_centers`, `smart_dimensions`, `smart_spacing`, and `snap_to_guides`. `snap_zone` uses `#[serde(default = "default_zone")]` returning `4.0`. `snap_to_document_grid` stays a plain bool.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p designcraft-ui-egui snap_view_uses_the_saved_switches`

Expected: FAIL to compile. `snap_view` does not exist.

- [ ] **Step 3: Wire the commands and the preferences page**

`UiState` fields: `snap_to_guides`, `snap_to_document_grid`, `align_edges`, `align_centers`, `smart_dimensions`, `smart_spacing`, `snap_zone`. Keep the existing `smart_guides` and `guides` fields. `Default` matches the factory.

`view_info` builds `SnapView` from those fields and `self.ui.guides`, and passes `self.ui.units`.

Add the three rows to `UI_COMMANDS` next to `view.guides`. Shortcut for Snap to Guides is `Cmd+Shift+;`. The other two have no shortcut. Menu entries go in the Grids & Guides submenu under `ui:view.guides`. `checked` returns the three bools. The command arms toggle them the same way `view.guides` uses `flag`.

Properties smart-guides icon calls `app.run("view.smartGuides", json!({}))` instead of writing `app.ui.smart_guides` directly.

Guides & Pasteboard page, after the pasteboard fields: four `check` rows and a zone number. Seed the keys in `app.preferences` from `UiState` even when no document is open (`snap.alignEdges`, `snap.alignCenters`, `snap.dimensions`, `snap.spacing`, `snap.zone`). On apply, write them back. A zone below 0 is stored as 0. A non-finite zone leaves the stored value unchanged.

Add the eight spec strings to `i18n.rs` `TABLE`. Order is German, French, Spanish, Japanese:

- Snap to Guides: An Hilfslinien ausrichten; Coller aux repères; Ajustar a las guías; ガイドにスナップ.
- Snap to Document Grid: Am Dokumentraster ausrichten; Coller à la grille du document; Ajustar a la cuadrícula del documento; ドキュメントグリッドにスナップ.
- Smart Guides: Smarte Hilfslinien; Repères intelligents; Guías inteligentes; スマートガイド.
- Align to Object Edges: An Objektkanten ausrichten; Aligner sur les bords des objets; Alinear a los bordes del objeto; オブジェクトの端に揃える.
- Align to Object Centers: An Objektmitten ausrichten; Aligner sur les centres des objets; Alinear a los centros del objeto; オブジェクトの中心に揃える.
- Smart Dimensions: Smarte Maße; Mesures intelligentes; Dimensiones inteligentes; スマートサイズ.
- Smart Spacing: Smarte Abstände; Espacements intelligents; Espaciado inteligente; スマート間隔.
- Snap to Zone: Fangzone; Zone de magnétisme; Zona de ajuste; スナップゾーン.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p designcraft-ui-egui snap_view_uses_the_saved_switches`

Run: `cargo test -p designcraft-tools`

Run: `cargo test -p designcraft-engine tool_gesture_creates_one_undo_step shift_move_does_not_take_the_other_axis`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/ui-egui/src/lib.rs crates/ui-egui/src/menus.rs crates/ui-egui/src/dialogs.rs crates/ui-egui/src/panels/properties.rs crates/ui-egui/src/i18n.rs
git commit -m "Snap: view commands and guide preferences"
```

## Spec coverage

- Pass order and first-hit: Tasks 2, 4, 6, 7, 8.
- Gestures, Shift, Command, Live Distribute, Alt-drag: Tasks 3, 6, 8, 9.
- Parent stroke, groups, own size, baseline formula: Tasks 4, 5, 6.
- Switches, zone, labels, overlays: Tasks 1, 2, 6, 7, 10.
- Failure rules: Tasks 1 and 4. Out of scope items are in Global Constraints and have no task.
