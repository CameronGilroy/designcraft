//! Table menu: create, convert, insert/delete rows and columns, merge, cell/row/column options,
//! table options and cell selection.
//!
//! Commands target the table at the text selection (a caret in a cell or next to a table anchor),
//! the selected cells, or an explicit `{story, table}`; `rows: [a, b]` / `cols: [a, b]` (or `row` /
//! `col`) narrow the cell range (default: the selected cells, else the caret's cell, else the
//! whole table).

use designcraft_compose as compose;
use designcraft_doc::{
    AltFills, CellAddr, CellRange, CellStroke, Document, RowHeightMode, Selection, StoryId, StrokeType, Table, TableSel, TextSel,
    VerticalJustification,
};
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, f64_or, has_doc, has_text, ok, str_param};
use crate::{Result, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "table.insert",
            "Create Table",
            [],
            Some("Cmd+Alt+Shift+T"),
            "{rows?: body rows (4), cols? (4), headerRows? (0), footerRows? (0), width?} — at the text insertion point",
            has_text,
            insert
        ),
        cmd!(
            "table.convertFromText",
            "Convert Text to Table",
            ["Table"],
            None,
            "{columnSeparator?: tab|comma} — selected paragraphs become rows",
            has_text,
            convert_from_text
        ),
        cmd!(
            "table.convertToText",
            "Convert Table to Text",
            ["Table"],
            None,
            "{} — cells separated by tabs, rows by paragraphs",
            in_table,
            convert_to_text
        ),
        cmd!("table.insertRow", "Insert Row", [], None, "{where?: above|below, count? (1)}", in_table, |s, p| insert_row(s, p, None)),
        cmd!("table.insertRowAbove", "Insert Row Above", ["Table", "Insert"], None, "{count?}", in_table, |s, p| insert_row(s, p, Some(true))),
        cmd!("table.insertRowBelow", "Insert Row Below", ["Table", "Insert"], None, "{count?}", in_table, |s, p| insert_row(s, p, Some(false))),
        cmd!("table.insertColumn", "Insert Column", [], None, "{where?: left|right, count? (1), width?}", in_table, |s, p| insert_col(s, p, None)),
        cmd!("table.insertColumnLeft", "Insert Column Left", ["Table", "Insert"], None, "{count?}", in_table, |s, p| insert_col(s, p, Some(true))),
        cmd!("table.insertColumnRight", "Insert Column Right", ["Table", "Insert"], None, "{count?}", in_table, |s, p| insert_col(s, p, Some(false))),
        cmd!("table.deleteRow", "Delete Row", ["Table", "Delete"], None, "{} — the rows of the target cells", in_table, delete_rows),
        cmd!("table.deleteColumn", "Delete Column", ["Table", "Delete"], None, "{} — the columns of the target cells", in_table, delete_cols),
        cmd!("table.delete", "Delete Table", ["Table", "Delete"], None, "{}", in_table, delete_table),
        cmd!("table.merge", "Merge Cells", ["Table"], None, "{} — merge the target cell range", in_table, merge),
        cmd!("table.unmerge", "Unmerge Cells", ["Table"], None, "{}", in_table, unmerge),
        cmd!(
            "table.setCell",
            "Cell Options",
            [],
            None,
            "{fill?: swatch, tint?, insets?: n | [t,l,b,r], vj?: top|center|bottom|justify, text?, stroke?: {weight?, color?, tint?, type?: solid|dashed|dotted, edges?: all|outer|inner|top|left|bottom|right}}",
            in_table,
            set_cell
        ),
        cmd!("table.setRowHeight", "Row Height", [], None, "{height, mode?: atLeast|exactly}", in_table, set_row_height),
        cmd!("table.setColumnWidth", "Column Width", [], None, "{width}", in_table, set_col_width),
        cmd!("table.distributeColumns", "Distribute Columns Evenly", ["Table"], None, "{}", in_table, distribute_cols),
        cmd!(
            "table.options",
            "Table Options",
            [],
            None,
            "{border?: {weight?, color?, tint?, type?}, spaceBefore?, spaceAfter?, headerRows?, footerRows?, repeatHeader?, repeatFooter?, altRows?: {first, firstColor, firstTint, next, nextColor, nextTint, skipFirst, skipLast} | null, altCols?: … | null}",
            in_table,
            options
        ),
        cmd!(noundo "table.select", "Select Cells", [], None, "{story?, table?, rows?: [a,b], cols?: [a,b], what?: cell|row|column|table}", in_table_or_ids, select),
        cmd!(noundo "table.selectTable", "Select Table", ["Table", "Select"], None, "{}", in_table, |s, p| {
            let mut p = p.clone();
            p["what"] = json!("table");
            select(s, &p)
        }),
        cmd!(noundo "table.selectRow", "Select Row", ["Table", "Select"], None, "{}", in_table, |s, p| {
            let mut p = p.clone();
            p["what"] = json!("row");
            select(s, &p)
        }),
        cmd!(noundo "table.selectColumn", "Select Column", ["Table", "Select"], None, "{}", in_table, |s, p| {
            let mut p = p.clone();
            p["what"] = json!("column");
            select(s, &p)
        }),
        cmd!(noundo "table.nextCell", "Next Cell", [], None, "{}", in_cell, |s, _| step_cell(s, true)),
        cmd!(noundo "table.prevCell", "Previous Cell", [], None, "{}", in_cell, |s, _| step_cell(s, false)),
        cmd!(query "table.get", "Get Table", [], None, "{story?, table?} → rows, columns, cells (text, spans, fill), options", in_table_or_ids, get),
    ]
}

// ---------- enablement and targeting ----------

fn in_table(s: &Session) -> std::result::Result<(), String> {
    has_doc(s)?;
    let st = s.active().expect("doc");
    if st.selection.cells.is_some() || st.selection.text.is_some_and(|t| t.cell.is_some()) {
        return Ok(());
    }
    if let Some(t) = st.selection.text
        && st.doc.story(t.story).is_some_and(|x| x.table_at(t.focus).is_some())
    {
        return Ok(());
    }
    Err("place the insertion point in a table".into())
}

fn in_table_or_ids(s: &Session) -> std::result::Result<(), String> {
    has_doc(s)
}

fn in_cell(s: &Session) -> std::result::Result<(), String> {
    has_doc(s)?;
    if s.active().is_some_and(|d| d.selection.text.is_some_and(|t| t.cell.is_some())) { Ok(()) } else { Err("not in a table cell".into()) }
}

/// The table a command acts on and its target cell range.
struct Tgt {
    story: StoryId,
    table: u64,
    range: CellRange,
}

fn find_table(doc: &Document, id: u64) -> Option<StoryId> {
    doc.stories.iter().find(|(_, s)| s.tables.contains_key(&id)).map(|(k, _)| *k)
}

fn pair(p: &Value, many: &str, one: &str) -> Option<(usize, usize)> {
    if let Some(a) = p.get(many).and_then(Value::as_array) {
        let a0 = a.first()?.as_u64()? as usize;
        let a1 = a.get(1).and_then(Value::as_u64).map_or(a0, |v| v as usize);
        return Some((a0.min(a1), a0.max(a1)));
    }
    p.get(one).and_then(Value::as_u64).map(|v| (v as usize, v as usize))
}

fn target(s: &Session, p: &Value, cmd: &str) -> Result<Tgt> {
    let st = s.doc()?;
    let doc = &st.doc;
    let explicit_table = p.get("table").and_then(Value::as_u64);
    let explicit_story = p.get("story").and_then(Value::as_u64).map(StoryId);
    let (story, table, range) = if let Some(tid) = explicit_table {
        let sid = explicit_story.or_else(|| find_table(doc, tid)).ok_or_else(|| bad(cmd, format!("no table {tid}")))?;
        (sid, tid, None)
    } else if let Some(c) = st.selection.cells {
        (c.story, c.table, Some(c.range))
    } else if let Some(t) = st.selection.text {
        match t.cell {
            Some(c) => (t.story, c.table, Some(CellRange::cell(c.row, c.col))),
            None => {
                let tid = doc.story(t.story).and_then(|x| x.table_at(t.focus)).ok_or_else(|| bad(cmd, "the insertion point is not in a table"))?;
                (t.story, tid, None)
            }
        }
    } else {
        return Err(bad(cmd, "place the insertion point in a table"));
    };
    let t = doc.story(story).and_then(|x| x.tables.get(&table)).ok_or_else(|| bad(cmd, format!("no table {table}")))?;
    let (nr, nc) = (t.nrows(), t.ncols());
    let mut range = range.unwrap_or(CellRange { r0: 0, c0: 0, r1: nr - 1, c1: nc - 1 });
    if let Some((a, b)) = pair(p, "rows", "row") {
        range.r0 = a;
        range.r1 = b;
    }
    if let Some((a, b)) = pair(p, "cols", "col") {
        range.c0 = a;
        range.c1 = b;
    }
    if range.r1 >= nr || range.c1 >= nc {
        return Err(bad(cmd, format!("cell range out of bounds ({nr}×{nc} table)")));
    }
    Ok(Tgt { story, table, range })
}

/// Edit the target table; afterwards selections pointing into it are clamped to its new shape.
fn edit_table<T>(s: &mut Session, g: &Tgt, cmd: &str, f: impl FnOnce(&mut Table) -> Result<T>) -> Result<T> {
    let cmd = cmd.to_string();
    s.edit(|d, sel| {
        let st = d.story_mut(g.story).ok_or_else(|| bad(&cmd, "no story"))?;
        let t = st.table_mut(g.table).ok_or_else(|| bad(&cmd, "no table"))?;
        let r = f(t)?;
        clamp_selection(d, sel);
        Ok(r)
    })
}

/// Keep text/cell selections valid after the table changed shape (or disappeared).
fn clamp_selection(d: &Document, sel: &mut Selection) {
    if let Some(ts) = sel.cells {
        match d.story(ts.story).and_then(|x| x.tables.get(&ts.table)) {
            Some(t) => {
                let (mr, mc) = (t.nrows() - 1, t.ncols() - 1);
                let r = ts.range;
                sel.cells = Some(TableSel { range: CellRange::new(r.r0.min(mr), r.c0.min(mc), r.r1.min(mr), r.c1.min(mc)), ..ts });
            }
            None => sel.cells = None,
        }
    }
    if let Some(t) = sel.text
        && let Some(c) = t.cell
    {
        let story = d.story(t.story);
        match story.and_then(|x| x.tables.get(&c.table)) {
            Some(tb) => {
                let (r, col) = tb.owner(c.row.min(tb.nrows() - 1), c.col.min(tb.ncols() - 1));
                let len = tb.cell(r, col).map_or(0, |x| x.text.len());
                sel.text =
                    Some(TextSel { anchor: t.anchor.min(len), focus: t.focus.min(len), cell: Some(CellAddr { table: c.table, row: r, col }), ..t });
            }
            None => {
                let len = story.map_or(0, |x| x.len());
                sel.text = Some(TextSel { anchor: t.anchor.min(len), focus: t.anchor.min(len), cell: None, ..t });
            }
        }
    }
}

fn count(p: &Value) -> usize {
    p.get("count").and_then(Value::as_u64).unwrap_or(1).clamp(1, 1000) as usize
}

fn color_param(p: &Value, key: &str) -> Option<String> {
    str_param(p, key).map(str::to_string)
}

// ---------- create / convert ----------

/// Width of the text column at the caret (the new table spans it).
fn column_width(s: &Session, t: &TextSel) -> f64 {
    let Some(st) = s.active() else { return 300.0 };
    let specs = compose::frame_specs(&st.doc, t.story);
    let spec = t.frame.and_then(|f| specs.iter().find(|x| x.id == f)).or(specs.first());
    spec.and_then(|f| f.columns().first().map(|c| c.width())).unwrap_or(300.0)
}

fn insert(s: &mut Session, p: &Value) -> Result<Value> {
    let t = s.doc()?.selection.text.ok_or_else(|| bad("table.insert", "no insertion point"))?;
    if t.cell.is_some() {
        return Err(bad("table.insert", "tables can't be nested in cells"));
    }
    let rows = p.get("rows").and_then(Value::as_u64).unwrap_or(4).clamp(1, 500) as usize;
    let cols = p.get("cols").or_else(|| p.get("columns")).and_then(Value::as_u64).unwrap_or(4).clamp(1, 200) as usize;
    let header = p.get("headerRows").and_then(Value::as_u64).unwrap_or(0).min(100) as usize;
    let footer = p.get("footerRows").and_then(Value::as_u64).unwrap_or(0).min(100) as usize;
    let width = p.get("width").and_then(Value::as_f64).unwrap_or_else(|| column_width(s, &t));
    s.edit(|d, sel| {
        let id = d.alloc();
        let mut table = Table::new(id, rows, cols, header, footer, width);
        let st = d.story_mut(t.story).ok_or_else(|| bad("table.insert", "no story"))?;
        // Cells start with the paragraph format at the caret.
        let pf = st.paras[st.para_at(t.range().start)].clone();
        let cf = st.char_format_at(t.range().start).clone();
        for c in &mut table.cells {
            c.text.paras[0] = designcraft_doc::ParaFormat { table: None, ..pf.clone() };
            c.text.chars[0].format = cf.clone();
        }
        let r = t.range();
        st.delete(r.clone());
        st.insert_table(r.start, table);
        *sel = Selection::text(TextSel { story: t.story, anchor: 0, focus: 0, frame: t.frame, cell: Some(CellAddr { table: id, row: 0, col: 0 }) });
        Ok(json!({"story": t.story.0, "table": id}))
    })
}

fn convert_from_text(s: &mut Session, p: &Value) -> Result<Value> {
    let t = s.doc()?.selection.text.ok_or_else(|| bad("table.convertFromText", "no text selected"))?;
    if t.cell.is_some() {
        return Err(bad("table.convertFromText", "select text outside tables"));
    }
    let sep = if str_param(p, "columnSeparator") == Some("comma") { ',' } else { '\t' };
    let width = p.get("width").and_then(Value::as_f64).unwrap_or_else(|| column_width(s, &t));
    s.edit(|d, sel| {
        let id = d.alloc();
        let st = d.story_mut(t.story).ok_or_else(|| bad("table.convertFromText", "no story"))?;
        let ranges = st.para_ranges();
        let (pa, pb) = (st.para_at(t.range().start), st.para_at(t.range().end));
        if (pa..=pb).any(|i| st.paras[i].table.is_some()) {
            return Err(bad("table.convertFromText", "the selection contains a table"));
        }
        let (a, b) = (ranges[pa].start, ranges[pb].end);
        let data: Vec<Vec<String>> = st.text[a..b].split('\n').map(|line| line.split(sep).map(|c| c.trim().to_string()).collect()).collect();
        let pf = st.paras[pa].clone();
        let cf = st.format_after(a).clone();
        let table = Table::from_strings(id, &data, width, &pf, &cf);
        st.delete(a..b);
        let anchor = st.insert_table(a, table);
        *sel = Selection::text(TextSel::caret(t.story, anchor));
        Ok(json!({"table": id, "rows": data.len()}))
    })
}

fn convert_to_text(s: &mut Session, p: &Value) -> Result<Value> {
    let g = target(s, p, "table.convertToText")?;
    s.edit(|d, sel| {
        let st = d.story_mut(g.story).ok_or_else(|| bad("table.convertToText", "no story"))?;
        let text = st.tables.get(&g.table).map(|t| t.plain_text()).unwrap_or_default();
        let a = st.table_anchor(g.table).ok_or_else(|| bad("table.convertToText", "no anchor"))?;
        st.replace(a..a + designcraft_doc::TABLE_ANCHOR.len_utf8(), &text);
        *sel = Selection::text(TextSel::caret(g.story, a + text.len()));
        ok()
    })
}

fn delete_table(s: &mut Session, p: &Value) -> Result<Value> {
    let g = target(s, p, "table.delete")?;
    s.edit(|d, sel| {
        let st = d.story_mut(g.story).ok_or_else(|| bad("table.delete", "no story"))?;
        let a = st.table_anchor(g.table).ok_or_else(|| bad("table.delete", "no anchor"))?;
        // Remove the anchor paragraph (with one separator) so no empty paragraph is left behind.
        let end = a + designcraft_doc::TABLE_ANCHOR.len_utf8();
        let r = if &st.text[end..] == "\n" && a > 0 && st.text[..a].ends_with('\n') {
            // Inserted at the end of the story: remove the empty paragraph after it too.
            a - 1..end + 1
        } else if st.text[end..].starts_with('\n') {
            a..end + 1
        } else if a > 0 && st.text[..a].ends_with('\n') {
            a - 1..end
        } else {
            a..end
        };
        st.delete(r.clone());
        *sel = Selection::text(TextSel::caret(g.story, r.start.min(st.len())));
        ok()
    })
}

// ---------- rows and columns ----------

fn insert_row(s: &mut Session, p: &Value, above: Option<bool>) -> Result<Value> {
    let g = target(s, p, "table.insertRow")?;
    let above = above.unwrap_or(str_param(p, "where") == Some("above"));
    let n = count(p);
    let at = if above { g.range.r0 } else { g.range.r1 + 1 };
    let r = edit_table(s, &g, "table.insertRow", |t| {
        t.insert_rows(at, n);
        Ok(json!({"rows": t.nrows()}))
    })?;
    shift_selection(s, g.table, at, n, true);
    Ok(r)
}

/// After inserting rows/columns at `at`, selections at or after it follow their cells.
fn shift_selection(s: &mut Session, table: u64, at: usize, n: usize, rows: bool) {
    let Ok(st) = s.doc_mut() else { return };
    if let Some(t) = st.selection.text.as_mut()
        && let Some(c) = t.cell.as_mut()
        && c.table == table
    {
        let v = if rows { &mut c.row } else { &mut c.col };
        if *v >= at {
            *v += n;
        }
    }
    if let Some(ts) = st.selection.cells.as_mut()
        && ts.table == table
    {
        let (a, b) = if rows { (&mut ts.range.r0, &mut ts.range.r1) } else { (&mut ts.range.c0, &mut ts.range.c1) };
        if *a >= at {
            *a += n;
        }
        if *b >= at {
            *b += n;
        }
    }
}

fn insert_col(s: &mut Session, p: &Value, left: Option<bool>) -> Result<Value> {
    let g = target(s, p, "table.insertColumn")?;
    let left = left.unwrap_or(str_param(p, "where") == Some("left"));
    let n = count(p);
    let at = if left { g.range.c0 } else { g.range.c1 + 1 };
    let width = p.get("width").and_then(Value::as_f64);
    let r = edit_table(s, &g, "table.insertColumn", |t| {
        let w = width.unwrap_or_else(|| t.columns[g.range.c0.min(t.ncols() - 1)].width);
        t.insert_cols(at, n, w);
        Ok(json!({"columns": t.ncols()}))
    })?;
    shift_selection(s, g.table, at, n, false);
    Ok(r)
}

fn delete_rows(s: &mut Session, p: &Value) -> Result<Value> {
    let g = target(s, p, "table.deleteRow")?;
    let all = s.doc()?.doc.story(g.story).and_then(|x| x.tables.get(&g.table)).is_some_and(|t| g.range.r0 == 0 && g.range.r1 + 1 >= t.nrows());
    if all {
        return delete_table(s, p);
    }
    edit_table(s, &g, "table.deleteRow", |t| {
        t.delete_rows(g.range.r0, g.range.r1 - g.range.r0 + 1);
        Ok(json!({"rows": t.nrows()}))
    })
}

fn delete_cols(s: &mut Session, p: &Value) -> Result<Value> {
    let g = target(s, p, "table.deleteColumn")?;
    let all = s.doc()?.doc.story(g.story).and_then(|x| x.tables.get(&g.table)).is_some_and(|t| g.range.c0 == 0 && g.range.c1 + 1 >= t.ncols());
    if all {
        return delete_table(s, p);
    }
    edit_table(s, &g, "table.deleteColumn", |t| {
        t.delete_cols(g.range.c0, g.range.c1 - g.range.c0 + 1);
        Ok(json!({"columns": t.ncols()}))
    })
}

fn merge(s: &mut Session, p: &Value) -> Result<Value> {
    let g = target(s, p, "table.merge")?;
    let r = edit_table(s, &g, "table.merge", |t| {
        t.merge(g.range).map_err(|e| bad("table.merge", e))?;
        Ok(t.expand_range(g.range))
    })?;
    // The caret goes to the merged cell.
    let st = s.doc_mut()?;
    st.selection = Selection::text(TextSel {
        story: g.story,
        anchor: 0,
        focus: 0,
        frame: st.selection.text.and_then(|t| t.frame),
        cell: Some(CellAddr { table: g.table, row: r.r0, col: r.c0 }),
    });
    ok()
}

fn unmerge(s: &mut Session, p: &Value) -> Result<Value> {
    let g = target(s, p, "table.unmerge")?;
    edit_table(s, &g, "table.unmerge", |t| {
        for r in g.range.r0..=g.range.r1 {
            for c in g.range.c0..=g.range.c1 {
                t.unmerge(r, c);
            }
        }
        ok()
    })
}

fn parse_stroke(v: &Value, mut base: CellStroke) -> CellStroke {
    if let Some(w) = v.get("weight").and_then(Value::as_f64) {
        base.weight = w.max(0.0);
    }
    if let Some(c) = color_param(v, "color") {
        base.color = c;
    }
    if let Some(t) = v.get("tint").and_then(Value::as_f64) {
        base.tint = t.clamp(0.0, 1.0) as f32;
    }
    match str_param(v, "type") {
        Some("dashed") => base.kind = StrokeType::Dashed { pattern: vec![base.weight.max(1.0) * 3.0, base.weight.max(1.0) * 2.0] },
        Some("dotted") => base.kind = StrokeType::Dotted,
        Some("solid") => base.kind = StrokeType::Solid,
        _ => {}
    }
    base
}

fn set_cell(s: &mut Session, p: &Value) -> Result<Value> {
    let g = target(s, p, "table.setCell")?;
    let fill = color_param(p, "fill");
    let tint = p.get("tint").and_then(Value::as_f64).map(|t| t.clamp(0.0, 1.0) as f32);
    let insets: Option<[f64; 4]> = match p.get("insets") {
        Some(Value::Number(n)) => n.as_f64().map(|v| [v.max(0.0); 4]),
        Some(Value::Array(a)) if a.len() == 4 => {
            let v: Vec<f64> = a.iter().map(|x| x.as_f64().unwrap_or(0.0).max(0.0)).collect();
            Some([v[0], v[1], v[2], v[3]])
        }
        _ => None,
    };
    let vj = match str_param(p, "vj").or_else(|| str_param(p, "verticalJustification")) {
        Some("top") => Some(VerticalJustification::Top),
        Some("center") => Some(VerticalJustification::Center),
        Some("bottom") => Some(VerticalJustification::Bottom),
        Some("justify") => Some(VerticalJustification::Justify),
        Some(o) => return Err(bad("table.setCell", format!("unknown vj `{o}`"))),
        None => None,
    };
    let text = str_param(p, "text").map(str::to_string);
    let stroke = p.get("stroke").cloned();
    edit_table(s, &g, "table.setCell", |t| {
        let owners = t.owners();
        let nc = t.ncols();
        let rg = g.range;
        for r in rg.r0..=rg.r1 {
            for c in rg.c0..=rg.c1 {
                if owners[r * nc + c] != (r, c) {
                    continue;
                }
                let cell = t.cell_mut(r, c).expect("in range");
                if let Some(f) = &fill {
                    cell.fill = f.clone();
                }
                if let Some(ti) = tint {
                    cell.fill_tint = ti;
                }
                if let Some(i) = insets {
                    cell.insets = i;
                }
                if let Some(v) = vj {
                    cell.vj = v;
                }
                if let Some(tx) = &text {
                    let len = cell.text.len();
                    cell.text.replace(0..len, tx);
                }
                if let Some(sv) = &stroke {
                    let edges = str_param(sv, "edges").unwrap_or("all");
                    for (i, on) in [
                        ("top", r == rg.r0),
                        ("left", c == rg.c0),
                        ("bottom", r + cell.row_span as usize - 1 == rg.r1),
                        ("right", c + cell.col_span as usize - 1 == rg.c1),
                    ]
                    .iter()
                    .enumerate()
                    .map(|(i, (name, outer))| (i, edges == "all" || edges == *name || (edges == "outer" && *outer) || (edges == "inner" && !*outer)))
                    {
                        if on {
                            cell.strokes[i] = parse_stroke(sv, cell.strokes[i].clone());
                        }
                    }
                }
            }
        }
        // Neighbouring cells share edges: mirror the edges along the range boundary.
        if let Some(sv) = &stroke {
            let edges = str_param(sv, "edges").unwrap_or("all");
            if matches!(edges, "all" | "outer" | "bottom") && rg.r1 + 1 < t.nrows() {
                for c in rg.c0..=rg.c1 {
                    let s0 = t.cell(rg.r1, c).map(|x| x.strokes[2].clone());
                    if let (Some(s0), Some(n)) = (s0, t.cell_mut(rg.r1 + 1, c)) {
                        n.strokes[0] = s0;
                    }
                }
            }
            if matches!(edges, "all" | "outer" | "right") && rg.c1 + 1 < t.ncols() {
                for r in rg.r0..=rg.r1 {
                    let s0 = t.cell(r, rg.c1).map(|x| x.strokes[3].clone());
                    if let (Some(s0), Some(n)) = (s0, t.cell_mut(r, rg.c1 + 1)) {
                        n.strokes[1] = s0;
                    }
                }
            }
        }
        ok()
    })
}

fn set_row_height(s: &mut Session, p: &Value) -> Result<Value> {
    let g = target(s, p, "table.setRowHeight")?;
    let h = p.get("height").and_then(Value::as_f64).map(|h| h.clamp(0.0, 10000.0));
    let mode = match str_param(p, "mode") {
        Some("exactly") => Some(RowHeightMode::Exactly),
        Some("atLeast") => Some(RowHeightMode::AtLeast),
        Some(o) => return Err(bad("table.setRowHeight", format!("unknown mode `{o}`"))),
        None => None,
    };
    if h.is_none() && mode.is_none() {
        return Err(bad("table.setRowHeight", "missing height"));
    }
    edit_table(s, &g, "table.setRowHeight", |t| {
        for row in &mut t.rows[g.range.r0..=g.range.r1] {
            if let Some(h) = h {
                row.height = h;
            }
            if let Some(m) = mode {
                row.mode = m;
            }
        }
        ok()
    })
}

fn set_col_width(s: &mut Session, p: &Value) -> Result<Value> {
    let g = target(s, p, "table.setColumnWidth")?;
    let w = p.get("width").and_then(Value::as_f64).ok_or_else(|| bad("table.setColumnWidth", "missing width"))?.clamp(3.0, 10000.0);
    edit_table(s, &g, "table.setColumnWidth", |t| {
        for c in &mut t.columns[g.range.c0..=g.range.c1] {
            c.width = w;
        }
        ok()
    })
}

fn distribute_cols(s: &mut Session, p: &Value) -> Result<Value> {
    let g = target(s, p, "table.distributeColumns")?;
    edit_table(s, &g, "table.distributeColumns", |t| {
        let cols = &mut t.columns[g.range.c0..=g.range.c1];
        let avg = cols.iter().map(|c| c.width).sum::<f64>() / cols.len() as f64;
        cols.iter_mut().for_each(|c| c.width = avg);
        ok()
    })
}

fn parse_alt(v: &Value) -> Option<AltFills> {
    if v.is_null() {
        return None;
    }
    let d = AltFills::default();
    Some(AltFills {
        first: v.get("first").and_then(Value::as_u64).map_or(d.first, |x| x as u32),
        first_color: color_param(v, "firstColor").unwrap_or(d.first_color),
        first_tint: v.get("firstTint").and_then(Value::as_f64).map_or(d.first_tint, |x| x as f32),
        next: v.get("next").and_then(Value::as_u64).map_or(d.next, |x| x as u32),
        next_color: color_param(v, "nextColor").unwrap_or(d.next_color),
        next_tint: v.get("nextTint").and_then(Value::as_f64).map_or(d.next_tint, |x| x as f32),
        skip_first: v.get("skipFirst").and_then(Value::as_u64).map_or(0, |x| x as u32),
        skip_last: v.get("skipLast").and_then(Value::as_u64).map_or(0, |x| x as u32),
    })
}

fn options(s: &mut Session, p: &Value) -> Result<Value> {
    let g = target(s, p, "table.options")?;
    let p = p.clone();
    edit_table(s, &g, "table.options", |t| {
        let o = &mut t.options;
        if let Some(b) = p.get("border") {
            o.border = parse_stroke(b, o.border.clone());
        }
        o.space_before = f64_or(&p, "spaceBefore", o.space_before);
        o.space_after = f64_or(&p, "spaceAfter", o.space_after);
        if let Some(b) = p.get("repeatHeader").and_then(Value::as_bool) {
            o.repeat_header = b;
        }
        if let Some(b) = p.get("repeatFooter").and_then(Value::as_bool) {
            o.repeat_footer = b;
        }
        if let Some(v) = p.get("altRows") {
            o.alt_rows = parse_alt(v);
        }
        if let Some(v) = p.get("altCols") {
            o.alt_cols = parse_alt(v);
        }
        let h = p.get("headerRows").and_then(Value::as_u64).map(|v| v as usize);
        let f = p.get("footerRows").and_then(Value::as_u64).map(|v| v as usize);
        if h.is_some() || f.is_some() {
            let (h, f) = (h.unwrap_or(t.header_rows()), f.unwrap_or(t.footer_rows()));
            // Add rows when there aren't enough for the requested header/footer plus one body row.
            let need = h + f + 1;
            if t.nrows() < need {
                let at = t.nrows();
                t.insert_rows(at, need - at);
            }
            t.set_header_footer(h, f);
        }
        Ok(json!({"options": t.options}))
    })
}

// ---------- selection ----------

fn select(s: &mut Session, p: &Value) -> Result<Value> {
    let g = target(s, p, "table.select")?;
    let (nr, nc) = {
        let st = s.doc()?;
        let t = st.doc.story(g.story).and_then(|x| x.tables.get(&g.table)).ok_or_else(|| bad("table.select", "no table"))?;
        (t.nrows(), t.ncols())
    };
    let mut r = g.range;
    match str_param(p, "what") {
        Some("table") => r = CellRange { r0: 0, c0: 0, r1: nr - 1, c1: nc - 1 },
        Some("row") => {
            r.c0 = 0;
            r.c1 = nc - 1;
        }
        Some("column") => {
            r.r0 = 0;
            r.r1 = nr - 1;
        }
        _ => {}
    }
    let st = s.doc_mut()?;
    let frame = st.selection.text.and_then(|t| t.frame);
    st.selection = Selection {
        cells: Some(TableSel { story: g.story, table: g.table, range: r }),
        text: Some(TextSel { story: g.story, anchor: 0, focus: 0, frame, cell: Some(CellAddr { table: g.table, row: r.r0, col: r.c0 }) }),
        ..Default::default()
    };
    st.revision += 1;
    Ok(json!({"story": g.story.0, "table": g.table, "range": r}))
}

/// Move the caret to the next/previous cell (Tab / Shift-Tab), selecting its text. Tab in the last
/// cell adds a row.
pub(crate) fn step_cell(s: &mut Session, forward: bool) -> Result<Value> {
    let t = s.doc()?.selection.text.ok_or_else(|| bad("table.nextCell", "no caret"))?;
    let c = t.cell.ok_or_else(|| bad("table.nextCell", "not in a cell"))?;
    let (owners, nr, nc) = {
        let st = s.doc()?;
        let tb = st.doc.story(t.story).and_then(|x| x.tables.get(&c.table)).ok_or_else(|| bad("table.nextCell", "no table"))?;
        (tb.owners(), tb.nrows(), tb.ncols())
    };
    let cur = c.row * nc + c.col;
    let next =
        if forward { (cur + 1..nr * nc).find(|&i| owners[i] == (i / nc, i % nc)) } else { (0..cur).rev().find(|&i| owners[i] == (i / nc, i % nc)) };
    let (row, col) = match next {
        Some(i) => (i / nc, i % nc),
        None if forward => {
            let g = Tgt { story: t.story, table: c.table, range: CellRange::cell(nr - 1, 0) };
            edit_table(s, &g, "table.nextCell", |tb| {
                tb.insert_rows(nr, 1);
                ok()
            })?;
            (nr, 0)
        }
        None => return ok(),
    };
    let st = s.doc_mut()?;
    let len = st.doc.text_story(t.story, Some(CellAddr { table: c.table, row, col })).map_or(0, |x| x.len());
    st.selection = Selection::text(TextSel { anchor: 0, focus: len, cell: Some(CellAddr { table: c.table, row, col }), ..t });
    st.revision += 1;
    Ok(json!({"row": row, "col": col}))
}

fn get(s: &mut Session, p: &Value) -> Result<Value> {
    let g = target(s, p, "table.get")?;
    let st = s.doc()?;
    let t = st.doc.story(g.story).and_then(|x| x.tables.get(&g.table)).ok_or_else(|| bad("table.get", "no table"))?;
    let owners = t.owners();
    let cells: Vec<Value> = (0..t.nrows())
        .flat_map(|r| (0..t.ncols()).map(move |c| (r, c)))
        .filter(|&(r, c)| owners[r * t.ncols() + c] == (r, c))
        .map(|(r, c)| {
            let cell = t.cell(r, c).expect("in range");
            json!({"row": r, "col": c, "rowSpan": cell.row_span, "colSpan": cell.col_span, "text": cell.text.text, "fill": cell.fill,
                "insets": cell.insets, "vj": cell.vj})
        })
        .collect();
    Ok(json!({
        "story": g.story.0, "table": g.table, "anchor": st.doc.story(g.story).and_then(|x| x.table_anchor(g.table)),
        "rows": t.rows, "columns": t.columns, "headerRows": t.header_rows(), "footerRows": t.footer_rows(),
        "cells": cells, "options": t.options, "range": g.range,
    }))
}
