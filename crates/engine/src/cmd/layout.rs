//! Layout menu and Pages/Layers panels: pages, spreads, parents, margins & columns, layers.

use designcraft_doc::{LAYER_COLORS, Layer, LayerId, Margins, SpreadId};
use serde_json::{Value, json};

use super::{CommandSpec, bad, cmd, has_doc, ok, str_param};
use crate::Result;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "layout.pages.insert",
            "Insert Pages…",
            ["Layout", "Pages"],
            Some("Cmd+Shift+P"),
            "{count?: 1, after?: page index (default: last), parent?: \"A\"|null}",
            has_doc,
            |s, p| {
                let count = p.get("count").and_then(Value::as_u64).unwrap_or(1).clamp(1, 9999) as usize;
                let st = s.doc()?;
                let n = st.doc.page_count();
                let after = p.get("after").and_then(Value::as_u64).map(|v| v as usize).unwrap_or(n - 1);
                let parent = parent_ref(&st.doc, p.get("parent")).unwrap_or_else(|| st.doc.parents.first().map(|x| x.id));
                s.edit(|d, _| {
                    let ids = d.insert_pages(Some(after), count, parent)?;
                    Ok(json!({"pages": ids.len(), "total": d.page_count()}))
                })
            }
        ),
        cmd!("layout.pages.delete", "Delete Pages", ["Layout", "Pages"], None, "{pages: [index]}", has_doc, |s, p| {
            let pages: Vec<usize> = p
                .get("pages")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_u64).map(|v| v as usize).collect())
                .unwrap_or_default();
            s.edit(|d, sel| {
                d.delete_pages(&pages)?;
                sel.items.retain(|i| d.item(*i).is_some());
                sel.text = sel.text.filter(|t| d.story(t.story).is_some());
                ok()
            })
        }),
        cmd!("layout.pages.move", "Move Pages…", ["Layout", "Pages"], None, "{from, to}", has_doc, |s, p| {
            let from = p.get("from").and_then(Value::as_u64).ok_or_else(|| bad("layout.pages.move", "missing from"))? as usize;
            let to = p.get("to").and_then(Value::as_u64).ok_or_else(|| bad("layout.pages.move", "missing to"))? as usize;
            s.edit(|d, _| {
                d.move_page(from, to)?;
                ok()
            })
        }),
        cmd!("layout.pages.duplicateSpread", "Duplicate Spread", ["Layout", "Pages"], None, "{spread}", has_doc, |s, p| {
            let si = p.get("spread").and_then(Value::as_u64).unwrap_or(0) as usize;
            s.edit(|d, _| {
                let sp = d.spreads.get(si).cloned().ok_or_else(|| bad("layout.pages.duplicateSpread", "no such spread"))?;
                let after = d.first_page_of_spread(si) + sp.pages.len() - 1;
                let parent = sp.pages.first().and_then(|p| p.parent);
                d.insert_pages(Some(after), sp.pages.len(), parent)?;
                // Copy items onto the new spread(s) at the same positions.
                let src = d.clone();
                let ids: Vec<_> = sp.items.iter().map(|i| i.id).collect();
                let (nsi, _) = d.page_loc(after + 1).ok_or_else(|| bad("layout.pages.duplicateSpread", "insert failed"))?;
                super::object::duplicate_from(d, &src, &ids, designcraft_doc::SpreadRef::Doc(nsi), designcraft_geom::Vec2::ZERO)?;
                ok()
            })
        }),
        cmd!(
            "layout.pages.applyParent",
            "Apply Parent to Pages…",
            ["Layout", "Pages"],
            None,
            "{pages: [index], parent: \"A\"|null}",
            has_doc,
            |s, p| {
                let pages: Vec<usize> = p
                    .get("pages")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().filter_map(Value::as_u64).map(|v| v as usize).collect())
                    .unwrap_or_default();
                let parent = parent_ref(&s.doc()?.doc, p.get("parent")).unwrap_or(None);
                s.edit(|d, _| {
                    d.apply_parent(&pages, parent)?;
                    ok()
                })
            }
        ),
        cmd!("layout.parents.new", "New Parent…", ["Layout", "Pages"], None, "{prefix?, name?: \"Parent\"}", has_doc, |s, p| {
            let name = str_param(p, "name").unwrap_or("Parent").to_string();
            s.edit(|d, _| {
                let prefix = str_param(p, "prefix").map(str::to_string).unwrap_or_else(|| d.next_parent_prefix());
                let m = d.page(0).map(|p| (p.columns.count, p.columns.gutter, p.margins)).unwrap_or((1, 12.0, Margins::uniform(36.0)));
                let id = d.add_parent(&prefix, &name, m.0, m.1, m.2);
                Ok(json!({"id": id.0, "prefix": prefix}))
            })
        }),
        cmd!(noundo "layout.parents.edit", "Edit Parents", [], None, "{on: bool} — show parent spreads on the canvas", has_doc, |s, p| {
            let st = s.doc_mut()?;
            st.editing_parents = p.get("on").and_then(Value::as_bool).unwrap_or(!st.editing_parents);
            st.selection = Default::default();
            st.revision += 1;
            ok()
        }),
        cmd!(
            "layout.marginsAndColumns",
            "Margins and Columns…",
            ["Layout"],
            None,
            "{pages?: [index] (default all), margins?: number|{top,bottom,inside,outside}, columns?, gutter?}",
            has_doc,
            |s, p| {
                let pages: Option<Vec<usize>> =
                    p.get("pages").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).map(|v| v as usize).collect());
                let p = p.clone();
                s.edit(|d, _| {
                    let all: Vec<usize> = pages.clone().unwrap_or_else(|| (0..d.page_count()).collect());
                    for abs in all {
                        let Some((si, pi)) = d.page_loc(abs) else { continue };
                        let pg = &mut std::sync::Arc::make_mut(&mut d.spreads[si]).pages[pi];
                        match p.get("margins") {
                            Some(Value::Number(n)) => pg.margins = Margins::uniform(n.as_f64().unwrap_or(36.0)),
                            Some(v @ Value::Object(_)) => {
                                if let Ok(m) = serde_json::from_value(v.clone()) {
                                    pg.margins = m;
                                }
                            }
                            _ => {}
                        }
                        if let Some(c) = p.get("columns").and_then(Value::as_u64) {
                            pg.columns.count = (c as u32).clamp(1, 216);
                        }
                        if let Some(g) = p.get("gutter").and_then(Value::as_f64) {
                            pg.columns.gutter = g.max(0.0);
                        }
                    }
                    ok()
                })
            }
        ),
        cmd!(
            "layout.documentSetup",
            "Document Setup…",
            ["File"],
            Some("Cmd+Alt+P"),
            "{width?, height?, pages?: count, startPage?: n, facingPages?, intent?: print|web|mobile, bleed?, slug?: n | [top, bottom, inside, outside]} → the document setup",
            has_doc,
            |s, p| {
                let p = p.clone();
                if p.as_object().is_none_or(|o| o.is_empty()) {
                    return Ok(document_setup(&s.doc()?.doc));
                }
                let edges = |k: &str| -> Option<[f64; 4]> {
                    match p.get(k)? {
                        Value::Array(a) if a.len() == 4 => {
                            let v: Vec<f64> = a.iter().filter_map(Value::as_f64).map(|v| v.max(0.0)).collect();
                            (v.len() == 4).then(|| [v[0], v[1], v[2], v[3]])
                        }
                        v => v.as_f64().map(|b| [b.max(0.0); 4]),
                    }
                };
                let (bleed, slug) = (edges("bleed"), edges("slug"));
                let intent: Option<designcraft_doc::Intent> = p.get("intent").and_then(|v| serde_json::from_value(v.clone()).ok());
                let pages = p.get("pages").and_then(Value::as_u64).map(|n| n.clamp(1, 9999) as usize);
                s.edit(|d, _| {
                    if let Some(f) = p.get("facingPages").and_then(Value::as_bool) {
                        d.settings.facing_pages = f;
                    }
                    if let Some(b) = bleed {
                        d.settings.bleed = b;
                    }
                    if let Some(b) = slug {
                        d.settings.slug = b;
                    }
                    if let Some(i) = intent {
                        d.settings.intent = i;
                    }
                    if let Some(n) = p.get("startPage").and_then(Value::as_u64) {
                        if !d.sections.iter().any(|x| x.start == 0) {
                            d.sections.insert(
                                0,
                                designcraft_doc::Section {
                                    start: 0,
                                    start_number: None,
                                    style: Default::default(),
                                    prefix: String::new(),
                                    marker: String::new(),
                                    include_prefix: false,
                                },
                            );
                        }
                        if let Some(first) = d.sections.iter_mut().find(|x| x.start == 0) {
                            first.start_number = Some(n.max(1) as u32);
                        }
                    }
                    // Number of Pages: add at the end (with the last page's parent) or remove from the end.
                    if let Some(n) = pages {
                        let have = d.page_count();
                        if n > have {
                            let parent = d.page_loc(have - 1).and_then(|(si, pi)| d.spreads[si].pages[pi].parent);
                            d.insert_pages(Some(have - 1), n - have, parent)?;
                        } else if n < have {
                            d.delete_pages(&(n..have).collect::<Vec<_>>())?;
                        }
                    }
                    let w = p.get("width").and_then(Value::as_f64);
                    let h = p.get("height").and_then(Value::as_f64);
                    if w.is_some() || h.is_some() {
                        d.settings.page_width = w.unwrap_or(d.settings.page_width);
                        d.settings.page_height = h.unwrap_or(d.settings.page_height);
                        for sp in d.spreads.iter_mut().chain(d.parents.iter_mut()) {
                            let sp = std::sync::Arc::make_mut(sp);
                            for pg in &mut sp.pages {
                                pg.width = d.settings.page_width;
                                pg.height = d.settings.page_height;
                            }
                            sp.relayout();
                        }
                    }
                    d.repaginate();
                    Ok(document_setup(d))
                })
            }
        ),
        cmd!(
            "layout.section",
            "Numbering & Section Options…",
            ["Layout"],
            None,
            "{page (1-based; starts a section there), startNumber?: n|null (continue), style?: arabic|upperRoman|lowerRoman|upperLetters|lowerLetters, prefix?, includePrefix?, marker?, remove?: bool}",
            has_doc,
            |s, p| {
                let page = p.get("page").and_then(Value::as_u64).ok_or_else(|| bad("layout.section", "missing page"))? as usize;
                let n = s.doc()?.doc.page_count();
                if page == 0 || page > n {
                    return Err(bad("layout.section", format!("no page {page}")));
                }
                let start = page - 1;
                let p = p.clone();
                s.edit(|d, _| {
                    d.sections.retain(|x| x.start != start || start == 0);
                    if p.get("remove").and_then(Value::as_bool).unwrap_or(false) && start != 0 {
                        return ok();
                    }
                    let mut sec = d.sections.iter().find(|x| x.start == start).cloned().unwrap_or(designcraft_doc::Section {
                        start,
                        start_number: None,
                        style: Default::default(),
                        prefix: String::new(),
                        marker: String::new(),
                        include_prefix: false,
                    });
                    match p.get("startNumber") {
                        Some(Value::Null) => sec.start_number = None,
                        Some(v) => sec.start_number = v.as_u64().map(|v| v.max(1) as u32),
                        None => {}
                    }
                    if let Some(v) = p.get("style") {
                        sec.style = serde_json::from_value(v.clone()).map_err(|e| bad("layout.section", e.to_string()))?;
                    }
                    if let Some(v) = p.get("prefix").and_then(Value::as_str) {
                        sec.prefix = v.into();
                    }
                    if let Some(v) = p.get("includePrefix").and_then(Value::as_bool) {
                        sec.include_prefix = v;
                    }
                    if let Some(v) = p.get("marker").and_then(Value::as_str) {
                        sec.marker = v.into();
                    }
                    d.sections.retain(|x| x.start != start);
                    d.sections.push(sec);
                    d.sections.sort_by_key(|x| x.start);
                    Ok(json!({"names": (0..d.page_count()).map(|i| d.page_name(i)).collect::<Vec<_>>()}))
                })
            }
        ),
        cmd!("layer.move", "Move Layer", [], None, "{id, to: index (0 = top/frontmost)}", has_doc, |s, p| {
            let id = LayerId(p.get("id").and_then(Value::as_u64).unwrap_or(0));
            let to = p.get("to").and_then(Value::as_u64).unwrap_or(0) as usize;
            s.edit(|d, _| {
                let i = d.layers.iter().position(|l| l.id == id).ok_or_else(|| bad("layer.move", "no such layer"))?;
                let l = d.layers.remove(i);
                let to = to.min(d.layers.len());
                d.layers.insert(to, l);
                ok()
            })
        }),
        cmd!(noundo "layer.selectItems", "Select All on Layer", [], None, "{id}", has_doc, |s, p| {
            let id = LayerId(p.get("id").and_then(Value::as_u64).unwrap_or(0));
            let st = s.doc_mut()?;
            let ids: Vec<_> = st.doc.spreads.iter().flat_map(|sp| sp.items.iter().filter(|i| i.layer == id && !i.locked).map(|i| i.id)).collect();
            st.selection = designcraft_doc::Selection::items(ids);
            st.revision += 1;
            ok()
        }),
        cmd!("layer.new", "New Layer…", [], None, "{name?}", has_doc, |s, p| {
            let name = str_param(p, "name").map(str::to_string);
            let r = s.edit(|d, _| {
                let id = LayerId(d.alloc());
                let n = d.layers.len();
                let name = name.clone().unwrap_or_else(|| format!("Layer {}", n + 1));
                d.layers.insert(
                    0,
                    Layer {
                        id,
                        name,
                        color: LAYER_COLORS[n % LAYER_COLORS.len()].1,
                        visible: true,
                        locked: false,
                        printable: true,
                        show_guides: true,
                        suppress_wrap_when_hidden: false,
                    },
                );
                Ok(id)
            })?;
            s.doc_mut()?.active_layer = r;
            Ok(json!({"id": r.0}))
        }),
        cmd!("layer.set", "Layer Options…", [], None, "{id, name?, visible?, locked?, printable?, color?: [r,g,b]}", has_doc, |s, p| {
            let id = LayerId(p.get("id").and_then(Value::as_u64).unwrap_or(0));
            let p = p.clone();
            s.edit(|d, _| {
                let l = d.layer_mut(id).ok_or_else(|| bad("layer.set", "no such layer"))?;
                if let Some(v) = p.get("name").and_then(Value::as_str) {
                    l.name = v.into();
                }
                if let Some(v) = p.get("visible").and_then(Value::as_bool) {
                    l.visible = v;
                }
                if let Some(v) = p.get("locked").and_then(Value::as_bool) {
                    l.locked = v;
                }
                if let Some(v) = p.get("printable").and_then(Value::as_bool) {
                    l.printable = v;
                }
                if let Some(c) = p.get("color").and_then(|c| serde_json::from_value(c.clone()).ok()) {
                    l.color = c;
                }
                ok()
            })
        }),
        cmd!("layer.delete", "Delete Layer", [], None, "{id}", has_doc, |s, p| {
            let id = LayerId(p.get("id").and_then(Value::as_u64).unwrap_or(0));
            s.edit(|d, sel| {
                if d.layers.len() < 2 {
                    return Err(bad("layer.delete", "a document needs at least one layer"));
                }
                let doomed: Vec<_> =
                    d.spreads.iter().chain(d.parents.iter()).flat_map(|sp| sp.items.iter().filter(|i| i.layer == id).map(|i| i.id)).collect();
                for i in doomed {
                    let _ = d.remove_item(i);
                }
                d.layers.retain(|l| l.id != id);
                *sel = Default::default();
                ok()
            })
        }),
        cmd!(
            "layer.merge",
            "Merge Layers",
            [],
            None,
            "{ids: [layer ids], into?: layer id (default: the first)} — their objects move to `into`, the other layers are deleted",
            has_doc,
            |s, p| {
                let ids: Vec<LayerId> =
                    p.get("ids").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).map(LayerId).collect()).unwrap_or_default();
                let into = p
                    .get("into")
                    .and_then(Value::as_u64)
                    .map(LayerId)
                    .or_else(|| ids.first().copied())
                    .ok_or_else(|| bad("layer.merge", "no layers"))?;
                s.edit(|d, _| {
                    if d.layer(into).is_none() || ids.iter().any(|l| d.layer(*l).is_none()) {
                        return Err(bad("layer.merge", "no such layer"));
                    }
                    let gone: Vec<LayerId> = ids.iter().copied().filter(|l| *l != into).collect();
                    for sp in d.spreads.iter_mut().chain(d.parents.iter_mut()) {
                        if !sp.items.iter().any(|i| gone.contains(&i.layer)) {
                            continue;
                        }
                        for it in &mut std::sync::Arc::make_mut(sp).items {
                            if gone.contains(&it.layer) {
                                std::sync::Arc::make_mut(it).layer = into;
                            }
                        }
                    }
                    d.layers.retain(|l| !gone.contains(&l.id));
                    Ok(json!({"merged": gone.len()}))
                })
            }
        ),
        cmd!("layer.deleteUnused", "Delete Unused Layers", [], None, "{} — layers without objects (one layer always stays)", has_doc, |s, _| {
            s.edit(|d, _| {
                let used: std::collections::HashSet<LayerId> =
                    d.spreads.iter().chain(d.parents.iter()).flat_map(|sp| sp.items.iter().map(|i| i.layer)).collect();
                let before = d.layers.len();
                if used.is_empty() {
                    // Nothing anywhere: keep the first layer.
                    d.layers.truncate(1);
                } else {
                    d.layers.retain(|l| used.contains(&l.id));
                }
                Ok(json!({"deleted": before - d.layers.len()}))
            })
        }),
        cmd!(
            "layer.others",
            "Hide/Lock Others",
            [],
            None,
            "{id, hide?: bool, lock?: bool, show?: true (Show All Layers), unlock?: true (Unlock All Layers)} — Hide Others / Lock Others act on every layer but `id`",
            has_doc,
            |s, p| {
                let id = LayerId(p.get("id").and_then(Value::as_u64).unwrap_or(0));
                let f = |k: &str| p.get(k).and_then(Value::as_bool).unwrap_or(false);
                let (hide, lock, show, unlock) = (f("hide"), f("lock"), f("show"), f("unlock"));
                s.edit(|d, _| {
                    for l in &mut d.layers {
                        if show {
                            l.visible = true;
                        }
                        if unlock {
                            l.locked = false;
                        }
                        if l.id != id {
                            if hide {
                                l.visible = false;
                            }
                            if lock {
                                l.locked = true;
                            }
                        }
                    }
                    ok()
                })
            }
        ),
        cmd!(noundo "layer.activate", "Set Active Layer", [], None, "{id}", has_doc, |s, p| {
            let id = LayerId(p.get("id").and_then(Value::as_u64).unwrap_or(0));
            let st = s.doc_mut()?;
            if st.doc.layer(id).is_none() {
                return Err(bad("layer.activate", "no such layer"));
            }
            st.active_layer = id;
            st.revision += 1;
            ok()
        }),
    ]
}

/// `"A"` → that parent's id; `null` → Some(None) (= [None]); missing → None (use default).
fn parent_ref(d: &designcraft_doc::Document, v: Option<&Value>) -> Option<Option<SpreadId>> {
    match v? {
        Value::Null => Some(None),
        Value::String(s) if s == "[None]" => Some(None),
        Value::String(s) => Some(d.parents.iter().find(|p| p.parent.as_ref().is_some_and(|i| i.prefix == *s || i.label() == *s)).map(|p| p.id)),
        _ => None,
    }
}

#[allow(dead_code)]
fn _r(_: Result<()>) {}

/// File › Document Setup values.
fn document_setup(d: &designcraft_doc::Document) -> Value {
    let st = &d.settings;
    let start = d.sections.iter().find(|x| x.start == 0).and_then(|x| x.start_number).unwrap_or(1);
    json!({"width": st.page_width, "height": st.page_height, "pages": d.page_count(), "startPage": start, "facingPages": st.facing_pages,
        "intent": st.intent, "bleed": st.bleed, "slug": st.slug})
}

#[cfg(test)]
mod setup_tests {
    use serde_json::json;

    #[test]
    fn document_setup_pages_start_bleed_slug() {
        let mut s = crate::Session::new();
        s.execute("file.new", &json!({"pages": 2})).unwrap();
        let r = s.execute("layout.documentSetup", &json!({})).unwrap();
        assert_eq!(r["pages"], 2);
        assert_eq!(r["startPage"], 1);
        let r = s.execute("layout.documentSetup", &json!({"pages": 5, "startPage": 2, "bleed": 9, "slug": [0, 18, 0, 0], "intent": "web"})).unwrap();
        assert_eq!(r["pages"], 5);
        assert_eq!(r["startPage"], 2);
        assert_eq!(r["bleed"], json!([9.0, 9.0, 9.0, 9.0]));
        assert_eq!(r["slug"], json!([0.0, 18.0, 0.0, 0.0]));
        assert_eq!(r["intent"], "web");
        let d = &s.doc().unwrap().doc;
        assert_eq!(d.page_name(0), "2");
        assert_eq!(d.page(0).unwrap().side, designcraft_doc::PageSide::Left);
        assert_eq!(d.spreads[0].pages.len(), 2, "pages 2–3 form the first spread");
        let r = s.execute("layout.documentSetup", &json!({"pages": 3})).unwrap();
        assert_eq!(r["pages"], 3);
        s.doc().unwrap().doc.check().unwrap();
    }
}

#[cfg(test)]
mod layer_tests {
    use serde_json::json;

    #[test]
    fn merge_delete_unused_hide_and_lock_others() {
        let mut s = crate::Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let l1 = s.doc().unwrap().doc.layers[0].id.0;
        let l2 = s.execute("layer.new", &json!({"name": "Art"})).unwrap()["id"].as_u64().unwrap();
        let l3 = s.execute("layer.new", &json!({"name": "Empty"})).unwrap()["id"].as_u64().unwrap();
        s.execute("layer.activate", &json!({"id": l2})).unwrap();
        let f = s.execute("frame.create", &json!({"rect": [0, 0, 10, 10]})).unwrap()["id"].as_u64().unwrap();
        s.execute("layer.others", &json!({"id": l2, "hide": true, "lock": true})).unwrap();
        let d = s.doc().unwrap().doc.clone();
        assert!(d.layers.iter().all(|l| (l.id.0 == l2) == l.visible && (l.id.0 != l2) == l.locked));
        s.execute("layer.others", &json!({"id": l2, "show": true, "unlock": true})).unwrap();
        assert!(s.doc().unwrap().doc.layers.iter().all(|l| l.visible && !l.locked));
        let r = s.execute("layer.deleteUnused", &json!({})).unwrap();
        assert_eq!(r["deleted"], 2, "the empty default layer and Empty");
        let _ = (l1, l3);
        let l4 = s.execute("layer.new", &json!({"name": "Into"})).unwrap()["id"].as_u64().unwrap();
        s.execute("layer.merge", &json!({"ids": [l4, l2]})).unwrap();
        let d = &s.doc().unwrap().doc;
        assert_eq!(d.layers.len(), 1);
        assert_eq!(d.item(designcraft_doc::ItemId(f)).unwrap().layer.0, l4);
    }
}
