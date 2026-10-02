//! Soft object effects (Effects panel): drop shadow, outer glow, inner shadow and basic feather,
//! drawn with vello filter layers.
//!
//! vello_cpu only supports filter layers in single-threaded contexts, so on the multithreaded
//! pipeline each effect is rendered into an offscreen single-threaded context cropped to its
//! reach and composited back as an image ([`Renderer::with_filters`]); single-threaded contexts
//! draw directly.
//!
//! - Drop shadow / outer glow: the object's silhouette (everything it paints: fill, image, text,
//!   stroke) in a `DropShadowOnly` layer under the object. `size` sets the blur (σ = size / 2),
//!   `spread`/`choke` (percent of size) grow the silhouette instead of blurring it. The shadow's
//!   opacity rides on the filter colour's alpha.
//! - Inner shadow: the inverse of the offset shape, blurred, clipped to the shape, on top.
//! - Feather: the object is masked by its shape inset by half the width and blurred, so the
//!   edge fades from ~0 at the path to full strength `width` inside.

use designcraft_doc::Item;
use designcraft_geom::{Affine, BezPath, Rect, Shape, Vec2};
use vello_common::filter_effects::{EdgeMode, Filter, FilterPrimitive};
use vello_cpu::peniko::{self, BlendMode, Compose, Mix};
use vello_cpu::{RenderContext, kurbo};

use crate::{Frame, Renderer, color_of};

/// Shadow offset (spread units) for an InDesign angle (degrees, light direction) and distance.
pub(crate) fn offset(angle: f64, distance: f64) -> Vec2 {
    let a = angle.to_radians();
    Vec2::new(-a.cos() * distance, a.sin() * distance)
}

/// How far (points) the effects of `it` reach outside its geometry.
pub(crate) fn outset(it: &Item) -> f64 {
    let e = &it.effects;
    let mut o: f64 = 0.0;
    if e.drop_shadow.on {
        o = o.max(e.drop_shadow.distance.abs() + e.drop_shadow.size.abs() * 1.6 + 1.0);
    }
    if e.outer_glow.on {
        o = o.max(e.outer_glow.size.abs() * 1.6 + 1.0);
    }
    o
}

/// (blur σ, growth) in points for an effect `size` with `pct` percent hardened.
fn split(size: f64, pct: f64) -> (f64, f64) {
    let size = size.max(0.0);
    let p = (pct / 100.0).clamp(0.0, 1.0);
    (size * (1.0 - p) / 2.0, size * p)
}

fn blur(sigma: f64) -> Filter {
    Filter::from_primitive(FilterPrimitive::GaussianBlur { std_deviation: sigma.max(0.0) as f32, edge_mode: EdgeMode::None })
}

fn shadow(sigma: f64, color: peniko::Color) -> Filter {
    Filter::from_primitive(FilterPrimitive::DropShadowOnly {
        dx: 0.0,
        dy: 0.0,
        std_deviation: sigma.max(0.0) as f32,
        color,
        edge_mode: EdgeMode::None,
    })
}

impl Renderer {
    /// Draw an item with soft effects: below-effects, the (feathered) body, inner shadow.
    pub(crate) fn draw_item_fx(&mut self, ctx: &mut RenderContext, f: &Frame, it: &Item, bp: &BezPath, xf: Affine, page_name: Option<&str>) {
        let doc = f.doc;
        let e = &it.effects;
        let sw = if it.stroke.is_none() { 0.0 } else { it.stroke.extent() };
        // Spread-space reach of what the object paints.
        let reach = xf.transform_rect_bbox(bp.bounding_box()).inflate(sw + 1.0, sw + 1.0);
        // The silhouette is the shape when the object paints its area (fill / image); text frames
        // without a fill cast the shadow of their text only, which can't be grown.
        let solid = !it.fill.is_none() || matches!(it.content, designcraft_doc::Content::Graphic(_));
        let mut below = vec![];
        if e.outer_glow.on {
            let g = &e.outer_glow;
            below.push((Vec2::ZERO, g.size, g.spread, g.color.clone(), g.opacity));
        }
        if e.drop_shadow.on {
            let d = &e.drop_shadow;
            below.push((offset(d.angle, d.distance), d.size, d.spread, d.color.clone(), d.opacity));
        }
        for (off, size, pct, color, opacity) in below {
            let Some(c) = doc.resolve_color(&color, 1.0) else { continue };
            let (sigma, grow) = split(size, pct);
            let filter = shadow(sigma, color_of(&c, opacity.clamp(0.0, 1.0)));
            // The offset moves the geometry, not the filter: vello drops layer content that lies
            // entirely outside the viewport before filtering.
            let r = (reach + off).inflate(grow, grow);
            self.with_filters(ctx, f, r, sigma, |me, c, fr| {
                let moved = Frame { view: fr.view * Affine::translate(off), ..*fr };
                c.set_transform(moved.view);
                c.push_layer(None, None, None, None, Some(filter));
                me.draw_body(c, &moved, it, bp, xf, page_name);
                if grow > 0.0 && solid {
                    c.set_transform(moved.view * xf);
                    c.set_paint(peniko::Color::BLACK);
                    c.set_stroke(kurbo::Stroke::new(grow * 2.0).with_join(kurbo::Join::Round));
                    c.stroke_path(bp);
                }
                c.pop_layer();
            });
        }
        // The object, feathered or not.
        if e.feather > 0.0 && it.path.is_closed() {
            let w = e.feather;
            self.with_filters(ctx, f, reach, w / 4.0, |me, c, fr| {
                c.set_transform(fr.view * xf);
                c.push_clip_layer(bp);
                me.draw_body(c, fr, it, bp, xf, page_name);
                // Keep the body where the blurred inset silhouette is.
                c.set_transform(Affine::IDENTITY);
                c.push_layer(None, Some(BlendMode::new(Mix::Normal, Compose::DestIn)), None, None, None);
                c.set_transform(fr.view);
                c.push_layer(None, None, None, None, Some(blur(w / 4.0)));
                c.set_transform(fr.view * xf);
                c.set_paint(peniko::Color::BLACK);
                c.fill_path(bp);
                c.set_transform(Affine::IDENTITY);
                c.push_layer(None, Some(BlendMode::new(Mix::Normal, Compose::DestOut)), None, None, None);
                c.set_transform(fr.view * xf);
                c.set_stroke(kurbo::Stroke::new(w).with_join(kurbo::Join::Round));
                c.stroke_path(bp);
                c.pop_layer();
                c.pop_layer();
                c.pop_layer();
                c.pop_layer();
            });
        } else {
            self.draw_body(ctx, f, it, bp, xf, page_name);
        }
        // Inner shadow, clipped to the shape.
        if e.inner_shadow.on
            && it.path.is_closed()
            && let Some(c) = doc.resolve_color(&e.inner_shadow.color, 1.0)
        {
            let s = &e.inner_shadow;
            let (sigma, choke) = split(s.size, s.choke);
            let off = offset(s.angle, s.distance);
            let paint = color_of(&c, s.opacity.clamp(0.0, 1.0));
            self.with_filters(ctx, f, reach, sigma, |_, c, fr| {
                c.set_transform(fr.view * xf);
                c.push_clip_layer(bp);
                c.set_transform(fr.view);
                c.push_layer(None, None, None, None, Some(blur(sigma)));
                // Everything outside the offset shape, so the shadow bleeds in from the edges.
                let m = Affine::translate(off) * xf;
                let pad = sigma * 4.0 + choke + off.hypot() + 4.0 * fr.px;
                let mut inv = m.transform_rect_bbox(bp.bounding_box()).inflate(pad, pad).to_path(0.1);
                let mut shape = bp.clone();
                shape.apply_affine(m);
                inv.extend(shape.iter());
                c.set_paint(paint);
                c.set_fill_rule(peniko::Fill::EvenOdd);
                c.fill_path(&inv);
                c.set_fill_rule(peniko::Fill::NonZero);
                if choke > 0.0 {
                    c.set_stroke(kurbo::Stroke::new(choke * 2.0).with_join(kurbo::Join::Round));
                    c.stroke_path(&shape);
                }
                c.pop_layer();
                c.pop_layer();
            });
        }
    }

    /// Run `draw`, which pushes filter layers. Single-threaded contexts draw directly. On a
    /// multithreaded context the layer is drawn into an offscreen single-threaded context
    /// covering `reach` (spread space) plus the blur's spread, then composited back.
    pub(crate) fn with_filters(
        &mut self,
        ctx: &mut RenderContext,
        f: &Frame,
        reach: Rect,
        sigma: f64,
        draw: impl FnOnce(&mut Self, &mut RenderContext, &Frame),
    ) {
        if !f.mt {
            return draw(self, ctx, f);
        }
        // 3σ of the Gaussian in pixels, plus a pixel of antialiasing.
        let spread = sigma.max(0.0) * 3.0 / f.px + 2.0;
        let screen = Rect::new(0.0, 0.0, ctx.width() as f64, ctx.height() as f64).inflate(spread, spread);
        let r = f.view.transform_rect_bbox(reach).inflate(spread, spread).intersect(screen);
        let (x0, y0) = (r.x0.floor(), r.y0.floor());
        let (w, h) = ((r.x1.ceil() - x0).min(u16::MAX as f64), (r.y1.ceil() - y0).min(u16::MAX as f64));
        if w < 1.0 || h < 1.0 {
            return;
        }
        let (w, h) = (w as u16, h as u16);
        let mut off = RenderContext::new_with(w, h, vello_cpu::RenderSettings { num_threads: 0, ..Default::default() });
        let shifted = Frame { mt: false, view: Affine::translate((-x0, -y0)) * f.view, ..*f };
        draw(self, &mut off, &shifted);
        off.flush();
        let mut pm = vello_cpu::Pixmap::new(w, h);
        off.render(&mut pm, &mut self.resources);
        ctx.set_transform(Affine::translate((x0, y0)));
        ctx.set_paint(vello_cpu::Image { image: vello_cpu::ImageSource::Pixmap(std::sync::Arc::new(pm)), sampler: peniko::ImageSampler::default() });
        ctx.fill_rect(&Rect::new(0.0, 0.0, w as f64, h as f64));
        ctx.set_transform(Affine::IDENTITY);
    }
}
