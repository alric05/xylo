#[cfg(feature = "std")]
use std::rc::Rc;

#[cfg(feature = "no-std")]
use alloc::{rc::Rc, vec::Vec};

use crate::error::Result;
use crate::shape::{BasicShape, HslaChange, PathSegment, Shape, IDENTITY};

use core::ops::Add;
use palette::{rgb::Rgba, FromColor, Hsla};
use tiny_skia::{
    BlendMode, Color, FillRule, Paint, Path, PathBuilder, Pixmap, Rect, Shader, Transform,
};

#[derive(Debug)]
enum ShapeData<'a> {
    FillPath {
        path: Path,
        transform: Transform,
        zindex: f32,
        paint: Paint<'a>,
    },
    Fill {
        zindex: f32,
        color: Color,
    },
}

fn convert_color(color: Rgba<f32>) -> Color {
    Color::from_rgba(
        color.red.clamp(0.0, 1.0),
        color.green.clamp(0.0, 1.0),
        color.blue.clamp(0.0, 1.0),
        color.alpha.clamp(0.0, 1.0),
    )
    .unwrap()
}

fn solid_color_paint<'a>(color: Rgba<f32>, blend_mode: BlendMode, anti_alias: bool) -> Paint<'a> {
    Paint {
        shader: Shader::SolidColor(convert_color(color)),
        blend_mode,
        anti_alias,
        force_hq_pipeline: false,
    }
}

fn overwrite_zindex(
    zindex: Option<f32>,
    zindex_overwrite: Option<f32>,
    zindex_shift: Option<f32>,
) -> f32 {
    zindex_overwrite.or(zindex).unwrap_or(0.0) + zindex_shift.unwrap_or(0.0)
}

fn overwrite_color(
    color: Hsla<f32>,
    color_overwrite: HslaChange,
    color_shift: HslaChange,
) -> Hsla<f32> {
    let hue = color_overwrite.hue.unwrap_or(color.hue) + color_shift.hue.unwrap_or(0.0.into());
    let saturation = color_overwrite.saturation.unwrap_or(color.saturation)
        + color_shift.saturation.unwrap_or(0.0.into());
    let lightness = color_overwrite.lightness.unwrap_or(color.lightness)
        + color_shift.lightness.unwrap_or(0.0.into());
    let alpha =
        color_overwrite.alpha.unwrap_or(color.alpha) + color_shift.alpha.unwrap_or(0.0.into());
    Hsla::new(hue, saturation, lightness, alpha)
}

fn overwrite_blend_mode(
    blend_mode: BlendMode,
    blend_mode_overwrite: Option<BlendMode>,
) -> BlendMode {
    blend_mode_overwrite.unwrap_or(blend_mode)
}

fn overwrite_anti_alias(anti_alias: bool, anti_alias_overwrite: Option<bool>) -> bool {
    anti_alias_overwrite.unwrap_or(anti_alias)
}

fn combine_shift<T: Add<Output = T> + Copy>(shift: Option<T>, curr: Option<T>) -> Option<T> {
    shift.map(|s| curr.map_or(s, |c| c + s)).or(curr)
}

fn resolve_zindex_overwrites(
    zindex_overwrite: Option<f32>,
    zindex_shift: Option<f32>,
    curr_zindex_overwrite: Option<f32>,
    curr_zindex_shift: Option<f32>,
) -> (Option<f32>, Option<f32>) {
    let zindex_overwrite = zindex_overwrite.or(curr_zindex_overwrite);
    let zindex_shift = combine_shift(zindex_shift, curr_zindex_shift);
    (zindex_overwrite, zindex_shift)
}

fn resolve_color_overwrites(
    color_overwrite: HslaChange,
    color_shift: HslaChange,
    curr_color_overwrite: HslaChange,
    curr_color_shift: HslaChange,
) -> (HslaChange, HslaChange) {
    let color_overwrite = HslaChange {
        hue: color_overwrite.hue.or(curr_color_overwrite.hue),
        saturation: color_overwrite
            .saturation
            .or(curr_color_overwrite.saturation),
        lightness: color_overwrite.lightness.or(curr_color_overwrite.lightness),
        alpha: color_overwrite.alpha.or(curr_color_overwrite.alpha),
    };
    let color_shift = HslaChange {
        hue: combine_shift(color_shift.hue, curr_color_shift.hue),
        saturation: combine_shift(color_shift.saturation, curr_color_shift.saturation),
        lightness: combine_shift(color_shift.lightness, curr_color_shift.lightness),
        alpha: combine_shift(color_shift.alpha, curr_color_shift.alpha),
    };
    (color_overwrite, color_shift)
}

fn resolve_blend_mode_overwrite(
    blend_mode_overwrite: Option<BlendMode>,
    curr_blend_mode_overwrite: Option<BlendMode>,
) -> Option<BlendMode> {
    blend_mode_overwrite.or(curr_blend_mode_overwrite)
}

fn resolve_anti_alias_overwrite(
    anti_alias_overwrite: Option<bool>,
    curr_anti_alias_overwrite: Option<bool>,
) -> Option<bool> {
    anti_alias_overwrite.or(curr_anti_alias_overwrite)
}

fn convert_shape(
    data: &mut Vec<ShapeData>,
    shape: Shape,
    parent_transform: Transform,
    zindex_overwrite: Option<f32>,
    zindex_shift: Option<f32>,
    color_overwrite: HslaChange,
    color_shift: HslaChange,
    blend_mode_overwrite: Option<BlendMode>,
    anti_alias_overwrite: Option<bool>,
) -> Result<()> {
    match shape {
        Shape::Basic(BasicShape::Square {
            x,
            y,
            width,
            height,
            transform,
            zindex,
            color,
            blend_mode,
            anti_alias,
        }) => {
            let path = PathBuilder::from_rect(Rect::from_xywh(x, y, width, height).unwrap());
            let transform = transform.post_concat(parent_transform);
            let zindex = overwrite_zindex(zindex, zindex_overwrite, zindex_shift);
            let color = overwrite_color(color, color_overwrite, color_shift);
            let blend_mode = overwrite_blend_mode(blend_mode, blend_mode_overwrite);
            let anti_alias = overwrite_anti_alias(anti_alias, anti_alias_overwrite);
            let paint = solid_color_paint(Rgba::from_color(*color), blend_mode, anti_alias);
            data.push(ShapeData::FillPath {
                path,
                transform,
                zindex,
                paint,
            });
        }
        Shape::Basic(BasicShape::Circle {
            x,
            y,
            radius,
            transform,
            zindex,
            color,
            blend_mode,
            anti_alias,
        }) => {
            let path = PathBuilder::from_circle(x, y, radius).unwrap();
            let transform = transform.post_concat(parent_transform);
            let zindex = overwrite_zindex(zindex, zindex_overwrite, zindex_shift);
            let color = overwrite_color(color, color_overwrite, color_shift);
            let blend_mode = overwrite_blend_mode(blend_mode, blend_mode_overwrite);
            let anti_alias = overwrite_anti_alias(anti_alias, anti_alias_overwrite);
            let paint = solid_color_paint(Rgba::from_color(*color), blend_mode, anti_alias);
            data.push(ShapeData::FillPath {
                path,
                transform,
                zindex,
                paint,
            });
        }
        Shape::Basic(BasicShape::Triangle {
            points,
            transform,
            zindex,
            color,
            blend_mode,
            anti_alias,
        }) => {
            let mut pb = PathBuilder::new();
            pb.move_to(points[0], points[1]);
            pb.line_to(points[2], points[3]);
            pb.line_to(points[4], points[5]);
            pb.close();
            let path = pb.finish().unwrap();

            let transform = transform.post_concat(parent_transform);
            let zindex = overwrite_zindex(zindex, zindex_overwrite, zindex_shift);
            let color = overwrite_color(color, color_overwrite, color_shift);
            let blend_mode = overwrite_blend_mode(blend_mode, blend_mode_overwrite);
            let anti_alias = overwrite_anti_alias(anti_alias, anti_alias_overwrite);
            let paint = solid_color_paint(Rgba::from_color(*color), blend_mode, anti_alias);
            data.push(ShapeData::FillPath {
                path,
                transform,
                zindex,
                paint,
            });
        }
        Shape::Basic(BasicShape::Fill { zindex, color }) => {
            let zindex = zindex.unwrap_or(0.0);
            let color = convert_color(Rgba::from_color(*color));
            data.push(ShapeData::Fill { zindex, color });
        }
        Shape::Basic(BasicShape::Empty) => (),
        Shape::Path {
            segments,
            transform,
            zindex,
            color,
            blend_mode,
            anti_alias,
        } => {
            let mut pb = PathBuilder::new();
            for segment in segments {
                match segment {
                    PathSegment::MoveTo(x, y) => pb.move_to(x, y),
                    PathSegment::LineTo(x, y) => pb.line_to(x, y),
                    PathSegment::QuadTo(x1, y1, x, y) => pb.quad_to(x1, y1, x, y),
                    PathSegment::CubicTo(x1, y1, x2, y2, x, y) => pb.cubic_to(x1, y1, x2, y2, x, y),
                    PathSegment::Close => pb.close(),
                }
            }
            let path = pb.finish();

            if let Some(path) = path {
                let transform = transform.post_concat(parent_transform);
                let zindex = overwrite_zindex(zindex, zindex_overwrite, zindex_shift);
                let color = overwrite_color(color, color_overwrite, color_shift);
                let blend_mode = overwrite_blend_mode(blend_mode, blend_mode_overwrite);
                let anti_alias = overwrite_anti_alias(anti_alias, anti_alias_overwrite);
                let paint = solid_color_paint(Rgba::from_color(*color), blend_mode, anti_alias);

                data.push(ShapeData::FillPath {
                    path,
                    transform,
                    zindex,
                    paint,
                });
            }
        }
        Shape::Composite {
            a,
            b,
            transform,
            zindex_overwrite: curr_zindex_overwrite,
            zindex_shift: curr_zindex_shift,
            color_overwrite: curr_color_overwrite,
            color_shift: curr_color_shift,
            blend_mode_overwrite: curr_blend_mode_overwrite,
            anti_alias_overwrite: curr_anti_alias_overwrite,
        } => {
            let transform = transform.post_concat(parent_transform);
            let (zindex_overwrite, zindex_shift) = resolve_zindex_overwrites(
                zindex_overwrite,
                zindex_shift,
                curr_zindex_overwrite,
                curr_zindex_shift,
            );
            let (color_overwrite, color_shift) = resolve_color_overwrites(
                color_overwrite,
                color_shift,
                curr_color_overwrite,
                curr_color_shift,
            );
            let blend_mode_overwrite =
                resolve_blend_mode_overwrite(blend_mode_overwrite, curr_blend_mode_overwrite);
            let anti_alias_overwrite =
                resolve_anti_alias_overwrite(anti_alias_overwrite, curr_anti_alias_overwrite);

            let a = Rc::try_unwrap(a).unwrap().into_inner();
            convert_shape(
                data,
                a,
                transform,
                zindex_overwrite,
                zindex_shift,
                color_overwrite,
                color_shift,
                blend_mode_overwrite,
                anti_alias_overwrite,
            )?;
            let b = Rc::try_unwrap(b).unwrap().into_inner();
            convert_shape(
                data,
                b,
                transform,
                zindex_overwrite,
                zindex_shift,
                color_overwrite,
                color_shift,
                blend_mode_overwrite,
                anti_alias_overwrite,
            )?;
        }
        Shape::Collection {
            shapes,
            transform,
            zindex_overwrite: curr_zindex_overwrite,
            zindex_shift: curr_zindex_shift,
            color_overwrite: curr_color_overwrite,
            color_shift: curr_color_shift,
            blend_mode_overwrite: curr_blend_mode_overwrite,
            anti_alias_overwrite: curr_anti_alias_overwrite,
        } => {
            let transform = transform.post_concat(parent_transform);
            let (zindex_overwrite, zindex_shift) = resolve_zindex_overwrites(
                zindex_overwrite,
                zindex_shift,
                curr_zindex_overwrite,
                curr_zindex_shift,
            );
            let (color_overwrite, color_shift) = resolve_color_overwrites(
                color_overwrite,
                color_shift,
                curr_color_overwrite,
                curr_color_shift,
            );
            let blend_mode_overwrite =
                resolve_blend_mode_overwrite(blend_mode_overwrite, curr_blend_mode_overwrite);
            let anti_alias_overwrite =
                resolve_anti_alias_overwrite(anti_alias_overwrite, curr_anti_alias_overwrite);

            for shape in shapes {
                let shape = Rc::try_unwrap(shape).unwrap().into_inner();
                convert_shape(
                    data,
                    shape,
                    transform,
                    zindex_overwrite,
                    zindex_shift,
                    color_overwrite,
                    color_shift,
                    blend_mode_overwrite,
                    anti_alias_overwrite,
                )?;
            }
        }
    }
    Ok(())
}

pub fn render(shape: Shape, width: u32, height: u32) -> Result<Pixmap> {
    let mut data = Vec::new();
    convert_shape(
        &mut data,
        shape,
        IDENTITY,
        None,
        None,
        HslaChange::default(),
        HslaChange::default(),
        None,
        None,
    )?;
    data.sort_by(|a, b| match (a, b) {
        (ShapeData::FillPath { zindex: a, .. }, ShapeData::FillPath { zindex: b, .. })
        | (ShapeData::FillPath { zindex: a, .. }, ShapeData::Fill { zindex: b, .. })
        | (ShapeData::Fill { zindex: a, .. }, ShapeData::FillPath { zindex: b, .. })
        | (ShapeData::Fill { zindex: a, .. }, ShapeData::Fill { zindex: b, .. }) => {
            a.partial_cmp(b).unwrap()
        }
    });

    let mut pixmap = Pixmap::new(width, height).unwrap();
    for shape_data in data {
        match shape_data {
            ShapeData::FillPath {
                path,
                transform,
                paint,
                ..
            } => {
                let transform = transform
                    .post_scale(1.0, -1.0)
                    .post_translate(width as f32 / 2.0, height as f32 / 2.0);
                pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
            }
            ShapeData::Fill { color, .. } => {
                pixmap.fill(color);
            }
        }
    }
    Ok(pixmap)
}
