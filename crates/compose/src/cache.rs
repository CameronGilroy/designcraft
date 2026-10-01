//! Composition cache shared by the engine, the renderer and exporters.
//!
//! An entry is valid while the inputs it was composed from are the *same allocations*: the story
//! `Arc` (and its revision), the styles `Arc`, the frame items' `Arc`s and the `Arc`s of every
//! wrapping item on the frames' spreads. Entries hold clones of those `Arc`s, so the pointers can't
//! be reused by other allocations while the entry lives.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use designcraft_doc::{Document, Item, Story, StoryId, Styles, WrapMode};

use crate::{ComposeOptions, ComposedStory};

type Key = (StoryId, Option<String>);

struct Entry {
    sig: Vec<usize>,
    _keep: (Arc<Story>, Arc<Styles>, Vec<Arc<Item>>),
    out: Arc<ComposedStory>,
    stamp: u64,
}

#[derive(Default)]
pub struct Cache {
    map: Mutex<(HashMap<Key, Entry>, u64)>,
}

impl Cache {
    pub fn new() -> Self {
        Self::default()
    }

    /// The composed story, from cache or freshly composed.
    pub fn get(&self, doc: &Document, sid: StoryId, page_name: Option<&str>) -> Arc<ComposedStory> {
        let Some(story) = doc.stories.get(&sid) else { return Arc::new(ComposedStory { story: sid, ..Default::default() }) };
        let (sig, keep_items) = signature(doc, story);
        let key = (sid, page_name.map(str::to_string));
        {
            let mut g = self.map.lock().unwrap_or_else(|e| e.into_inner());
            g.1 += 1;
            let stamp = g.1;
            if let Some(e) = g.0.get_mut(&key)
                && e.sig == sig
            {
                e.stamp = stamp;
                return e.out.clone();
            }
        }
        let out = Arc::new(crate::compose_story(doc, sid, &ComposeOptions { page_name: page_name.map(str::to_string) }));
        let mut g = self.map.lock().unwrap_or_else(|e| e.into_inner());
        let stamp = g.1;
        if g.0.len() > 4096 {
            g.0.retain(|_, e| stamp - e.stamp < 64);
        }
        g.0.insert(key, Entry { sig, _keep: (story.clone(), doc.styles.clone(), keep_items), out: out.clone(), stamp });
        out
    }

    pub fn clear(&self) {
        self.map.lock().unwrap_or_else(|e| e.into_inner()).0.clear();
    }
}

fn signature(doc: &Document, story: &Arc<Story>) -> (Vec<usize>, Vec<Arc<Item>>) {
    let mut sig = vec![Arc::as_ptr(story) as usize, story.rev as usize, Arc::as_ptr(&doc.styles) as usize, doc.sections.len()];
    let mut keep = Vec::new();
    let mut spreads = Vec::new();
    for f in &story.frames {
        let Some(loc) = doc.find(*f) else {
            sig.push(0);
            continue;
        };
        if let Some(sp) = doc.spread(loc.spread) {
            let top = &sp.items[loc.top()];
            sig.push(Arc::as_ptr(top) as usize);
            keep.push(top.clone());
            // Page position matters for page-number markers.
            sig.push(loc.top());
            if !spreads.contains(&loc.spread) {
                spreads.push(loc.spread);
                for it in &sp.items {
                    if it.wrap.mode != WrapMode::None {
                        sig.push(Arc::as_ptr(it) as usize);
                        keep.push(it.clone());
                    }
                }
                sig.push(usize::MAX);
            }
        }
    }
    (sig, keep)
}
