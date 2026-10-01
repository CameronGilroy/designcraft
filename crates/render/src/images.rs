//! Decoded placed images, shared by every [`crate::Renderer`] in the process (canvas worker,
//! thumbnails, exports), so each asset is decoded once and each mip level built once.
//!
//! Entries are keyed by the address of the asset's encoded bytes (`Arc<Vec<u8>>`) and hold a
//! clone of that `Arc`, so the address can't be reused while cached. Least recently used levels
//! are evicted beyond a memory budget.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use vello_cpu::Pixmap;

use crate::{decode_pixmap, halve};

/// Decoded pixels kept (bytes).
const BUDGET: usize = 768 << 20;

struct Entry {
    _bytes: Arc<Vec<u8>>,
    pm: Arc<Pixmap>,
    stamp: u64,
}

#[derive(Default)]
struct Cache {
    map: HashMap<(usize, u8), Entry>,
    bytes: usize,
    clock: u64,
    /// Assets that failed to decode (don't retry every frame).
    bad: HashMap<usize, Arc<Vec<u8>>>,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);

fn size_of(pm: &Pixmap) -> usize {
    pm.width() as usize * pm.height() as usize * 4
}

fn with<R>(f: impl FnOnce(&mut Cache) -> R) -> R {
    let mut g = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(Cache::default))
}

fn lookup(key: (usize, u8)) -> Option<Arc<Pixmap>> {
    with(|c| {
        c.clock += 1;
        let clock = c.clock;
        c.map.get_mut(&key).map(|e| {
            e.stamp = clock;
            e.pm.clone()
        })
    })
}

fn store(bytes: &Arc<Vec<u8>>, level: u8, pm: Arc<Pixmap>) {
    with(|c| {
        c.clock += 1;
        let key = (Arc::as_ptr(bytes) as usize, level);
        let n = size_of(&pm);
        if let Some(old) = c.map.insert(key, Entry { _bytes: bytes.clone(), pm, stamp: c.clock }) {
            c.bytes -= size_of(&old.pm);
        }
        c.bytes += n;
        if c.bytes > BUDGET {
            let mut v: Vec<(u64, (usize, u8))> = c.map.iter().map(|(k, e)| (e.stamp, *k)).collect();
            v.sort_unstable();
            for (_, k) in v {
                if c.bytes <= BUDGET * 3 / 4 {
                    break;
                }
                if k == key {
                    continue;
                }
                if let Some(e) = c.map.remove(&k) {
                    c.bytes -= size_of(&e.pm);
                }
            }
        }
    });
}

/// Full-resolution decode of `bytes` (cached).
pub fn decoded(bytes: &Arc<Vec<u8>>) -> Option<Arc<Pixmap>> {
    let id = Arc::as_ptr(bytes) as usize;
    if let Some(pm) = lookup((id, 0)) {
        return Some(pm);
    }
    if with(|c| c.bad.contains_key(&id)) {
        return None;
    }
    // Decode outside the lock: other threads keep drawing cached images meanwhile.
    match decode_pixmap(bytes) {
        Some(pm) => {
            let pm = Arc::new(pm);
            store(bytes, 0, pm.clone());
            Some(pm)
        }
        None => {
            with(|c| c.bad.insert(id, bytes.clone()));
            None
        }
    }
}

/// The mip level of `bytes` closest above `on_screen` device pixels per point for a graphic
/// `width_pt` points wide.
pub fn mip(bytes: &Arc<Vec<u8>>, width_pt: f64, on_screen: f64) -> Option<Arc<Pixmap>> {
    let full = decoded(bytes)?;
    let ppt = full.width() as f64 / width_pt.max(1e-6);
    let id = Arc::as_ptr(bytes) as usize;
    let mut pm = full;
    let mut level = 0u8;
    while ppt / (1u64 << (level + 1)) as f64 >= on_screen * 1.2 && pm.width() > 64 && pm.height() > 64 && level < 8 {
        level += 1;
        pm = match lookup((id, level)) {
            Some(p) => p,
            None => {
                let half = Arc::new(halve(&pm));
                store(bytes, level, half.clone());
                half
            }
        };
    }
    Some(pm)
}

/// Drop every decoded image.
pub fn clear() {
    with(|c| {
        c.map.clear();
        c.bad.clear();
        c.bytes = 0;
    });
}
