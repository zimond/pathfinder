// pathfinder/renderer/src/builder.rs
//
// Copyright © 2019 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Packs data onto the GPU.

use crate::concurrent::executor::Executor;
use crate::gpu_data::{AlphaTile, FillBatchPrimitive, RenderCommand};
use crate::gpu_data::{RenderStage, TileObjectPrimitive};
use crate::options::{PreparedBuildOptions, RenderCommandListener};
use crate::paint::{PaintInfo, PaintMetadata};
use crate::scene::{ClipPathId, DrawPathId, Scene};
use crate::tile_map::DenseTileMap;
use crate::tiles::{self, TILE_HEIGHT, TILE_WIDTH, Tiler, TilingPaintClipInfo, TilingPathInfo};
use crate::z_buffer::ZBuffer;
use pathfinder_geometry::line_segment::{LineSegment2F, LineSegmentU4, LineSegmentU8};
use pathfinder_geometry::vector::{Vector2F, Vector2I};
use pathfinder_geometry::rect::{RectF, RectI};
use pathfinder_geometry::util;
use pathfinder_simd::default::{F32x4, I32x4};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;
use std::u16;

pub(crate) struct SceneBuilder<'a> {
    scene: &'a Scene,
    built_options: &'a PreparedBuildOptions,

    pub(crate) next_stage_0_alpha_tile_index: AtomicUsize,
    pub(crate) next_stage_1_alpha_tile_index: AtomicUsize,

    pub(crate) z_buffer: ZBuffer,
    pub(crate) listener: Box<dyn RenderCommandListener>,
}

impl<'a> SceneBuilder<'a> {
    pub(crate) fn new(
        scene: &'a Scene,
        built_options: &'a PreparedBuildOptions,
        listener: Box<dyn RenderCommandListener>,
    ) -> SceneBuilder<'a> {
        let effective_view_box = scene.effective_view_box(built_options);
        SceneBuilder {
            scene,
            built_options,

            next_stage_0_alpha_tile_index: AtomicUsize::new(0),
            next_stage_1_alpha_tile_index: AtomicUsize::new(0),

            z_buffer: ZBuffer::new(effective_view_box),
            listener,
        }
    }

    pub fn build<E>(&mut self, executor: &E) where E: Executor {
        let start_time = Instant::now();

        let draw_path_count = self.scene.draw_paths.len();
        let clip_path_count = self.scene.clip_paths.len();
        let total_path_count = draw_path_count + clip_path_count;

        let bounding_quad = self.built_options.bounding_quad();
        self.listener.send(RenderCommand::Start { bounding_quad, path_count: total_path_count });

        let PaintInfo {
            data: paint_data,
            metadata: paint_metadata,
        } = self.scene.build_paint_info();
        self.listener.send(RenderCommand::AddPaintData(paint_data));

        let effective_view_box = self.scene.effective_view_box(self.built_options);

        // Build clip paths.
        let clip_paths = executor.build_vector(clip_path_count, |path_index| {
            self.build_clip_path(ClipPathId(path_index as u32),
                                 effective_view_box,
                                 &self.built_options,
                                 &self.scene)
        });

        // Build draw paths.
        let draw_paths = executor.build_vector(draw_path_count, |path_index| {
            self.build_draw_path(DrawPathId(path_index as u32),
                                 effective_view_box,
                                 &self.built_options,
                                 &self.scene,
                                 &paint_metadata,
                                 &clip_paths)
        });

        self.finish_building(&paint_metadata, clip_paths, draw_paths);

        let build_time = Instant::now() - start_time;
        self.listener.send(RenderCommand::Finish { build_time });
    }

    fn build_clip_path(
        &self,
        clip_path_id: ClipPathId,
        view_box: RectF,
        built_options: &PreparedBuildOptions,
        scene: &Scene,
    ) -> ClipBuildResult {
        let outline =   
            scene.apply_render_options(scene.clip_paths[clip_path_id.0 as usize].outline(),
                                       built_options);

        let tiling_path_info = TilingPathInfo {
            outline: &outline,
            paint_clip_info: TilingPaintClipInfo::Clip { path_id: clip_path_id },
        };
        let render_stage = tiling_path_info.render_stage();

        let mut tiler = Tiler::new(self, view_box, tiling_path_info);
        tiler.generate_tiles();

        self.listener.send(RenderCommand::AddFills {
            fills: tiler.built_object.fills,
            stage: render_stage,
        });

        ClipBuildResult {
            alpha_tiles: tiler.built_object.alpha_tiles,
            tiles: tiler.built_object.tiles,
            path_id: clip_path_id,
        }
    }

    fn build_draw_path(
        &self,
        draw_path_id: DrawPathId,
        view_box: RectF,
        built_options: &PreparedBuildOptions,
        scene: &Scene,
        paint_metadata: &[PaintMetadata],
        clip_build_results: &[ClipBuildResult],
    ) -> DrawBuildResult {
        let outline = scene.draw_paths[draw_path_id.0 as usize].outline();
        let outline = scene.apply_render_options(&outline, built_options);

        let paint_id = scene.draw_paths[draw_path_id.0 as usize].paint();
        let paint_metadata = &paint_metadata[paint_id.0 as usize];

        let tiling_path_info = TilingPathInfo {
            outline: &outline,
            paint_clip_info: match self.scene.draw_paths[draw_path_id.0 as usize].clip_path() {
                None => {
                    TilingPaintClipInfo::Draw {
                        path_id: draw_path_id,
                        paint_metadata,
                    }
                }
                Some(clip_path_id) => {
                    let clip_build_result = clip_build_results.iter().filter(|clip_build_result| {
                        clip_build_result.path_id == clip_path_id
                    }).next().expect("Where's the clip build result?");

                    TilingPaintClipInfo::DrawClipped {
                        path_id: draw_path_id,
                        clip_path_id,
                        paint_metadata,
                        clip_build_result,
                    }
                }
            }
        };

        let stage = tiling_path_info.render_stage();

        let mut tiler = Tiler::new(self, view_box, tiling_path_info);
        tiler.generate_tiles();

        self.listener.send(RenderCommand::AddFills { fills: tiler.built_object.fills, stage });

        DrawBuildResult { alpha_tiles: tiler.built_object.alpha_tiles, path_id: draw_path_id }
    }

    fn cull_tiles(&self,
                  clip_build_results: Vec<ClipBuildResult>,
                  draw_build_results: Vec<DrawBuildResult>)
                  -> CulledTiles {
        let mut result = CulledTiles::new();

        for clip_build_result in clip_build_results {
            for clip_alpha_tile in clip_build_result.alpha_tiles {
                // TODO(pcwalton): Cull clip tiles if at least one of the following is true:
                // 1. All paths with that clip applied are occluded by a solid tile above them.
                // 2. No path is clipped by this tile.
                //
                // NB: This is `result.draw`, not `result.clip`, because we actually draw 
                result.draw.push(clip_alpha_tile);
            }
        }

        for draw_build_result in draw_build_results {
            for draw_alpha_tile in draw_build_result.alpha_tiles {
                let alpha_tile_coords = draw_alpha_tile.upper_left.tile_position();
                if self.z_buffer.test(alpha_tile_coords, draw_build_result.path_id.0) {
                    result.draw.push(draw_alpha_tile);
                }
            }
        }

        result
    }

    fn pack_tiles(&mut self, paint_metadata: &[PaintMetadata], culled_tiles: CulledTiles) {
        let draw_path_count = self.scene.draw_paths.len() as u32;
        let solid_tiles = self.z_buffer.build_solid_tiles(&self.scene.draw_paths,
                                                          paint_metadata,
                                                          0..draw_path_count);
        if !solid_tiles.is_empty() {
            self.listener.send(RenderCommand::DrawSolidTiles(solid_tiles));
        }
        if !culled_tiles.draw.is_empty() {
            self.listener.send(RenderCommand::DrawAlphaTiles(culled_tiles.draw));
        }
        if !culled_tiles.clip.is_empty() {
            self.listener.send(RenderCommand::DrawClipTiles(culled_tiles.clip));
        }
    }

    fn finish_building(&mut self,
                       paint_metadata: &[PaintMetadata],
                       clip_build_results: Vec<ClipBuildResult>,
                       draw_build_results: Vec<DrawBuildResult>) {
        self.listener.send(RenderCommand::FlushFills);
        let culled_tiles = self.cull_tiles(clip_build_results, draw_build_results);
        self.pack_tiles(paint_metadata, culled_tiles);
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TileStats {
    pub solid_tile_count: u32,
    pub alpha_tile_count: u32,
}

// Utilities for built objects

impl BuiltObject {
    pub(crate) fn new(bounds: RectF) -> BuiltObject {
        let tile_rect = tiles::round_rect_out_to_tile_bounds(bounds);
        let tiles = DenseTileMap::new(tile_rect);
        BuiltObject {
            bounds,
            fills: vec![],
            alpha_tiles: vec![],
            tiles,
        }
    }

    #[inline]
    pub(crate) fn tile_rect(&self) -> RectI {
        self.tiles.rect
    }

    fn add_fill(
        &mut self,
        builder: &SceneBuilder,
        render_stage: RenderStage,
        segment: LineSegment2F,
        tile_coords: Vector2I,
    ) {
        debug!("add_fill({:?} ({:?}))", segment, tile_coords);

        // Ensure this fill is in bounds. If not, cull it.
        if self.tile_coords_to_local_index(tile_coords).is_none() {
            return;
        };

        debug_assert_eq!(TILE_WIDTH, TILE_HEIGHT);

        // Compute the upper left corner of the tile.
        let tile_size = F32x4::splat(TILE_WIDTH as f32);
        let tile_upper_left = tile_coords.to_f32().0.to_f32x4().xyxy() * tile_size;

        // Convert to 4.8 fixed point.
        let segment = (segment.0 - tile_upper_left) * F32x4::splat(256.0);
        let (min, max) = (F32x4::default(), F32x4::splat((TILE_WIDTH * 256 - 1) as f32));
        let segment = segment.clamp(min, max).to_i32x4();
        let (from_x, from_y, to_x, to_y) = (segment[0], segment[1], segment[2], segment[3]);

        // Cull degenerate fills.
        if from_x == to_x {
            debug!("... culling!");
            return;
        }

        // Allocate global tile if necessary.
        let alpha_tile_index = self.get_or_allocate_alpha_tile_index(builder,   
                                                                     render_stage,
                                                                     tile_coords);

        // Pack whole pixels.
        let px = (segment & I32x4::splat(0xf00)).to_u32x4();
        let px = (px >> 8).to_i32x4() | (px >> 4).to_i32x4().yxwz();

        // Pack instance data.
        debug!("... OK, pushing");
        self.fills.push(FillBatchPrimitive {
            px: LineSegmentU4 { from: px[0] as u8, to: px[2] as u8 },
            subpx: LineSegmentU8 {
                from_x: from_x as u8,
                from_y: from_y as u8,
                to_x:   to_x   as u8,
                to_y:   to_y   as u8,
            },
            alpha_tile_index,
        });
    }

    fn get_or_allocate_alpha_tile_index(
        &mut self,
        builder: &SceneBuilder,
        render_stage: RenderStage,
        tile_coords: Vector2I,
    ) -> u16 {
        let local_tile_index = self.tiles.coords_to_index_unchecked(tile_coords);
        let alpha_tile_index = self.tiles.data[local_tile_index].alpha_tile_index;
        if alpha_tile_index != !0 {
            return alpha_tile_index;
        }

        // FIXME(pcwalton): Handle overflow!
        let alpha_tile_index = match render_stage {
            RenderStage::Stage0 => {
                builder.next_stage_0_alpha_tile_index.fetch_add(1, Ordering::Relaxed) as u16
            }
            RenderStage::Stage1 => {
                builder.next_stage_1_alpha_tile_index.fetch_add(1, Ordering::Relaxed) as u16
            }
        };

        self.tiles.data[local_tile_index].alpha_tile_index = alpha_tile_index;
        alpha_tile_index
    }

    pub(crate) fn add_active_fill(
        &mut self,
        builder: &SceneBuilder,
        render_stage: RenderStage,
        left: f32,
        right: f32,
        mut winding: i32,
        tile_coords: Vector2I,
    ) {
        let tile_origin_y = (tile_coords.y() * TILE_HEIGHT as i32) as f32;
        let left = Vector2F::new(left, tile_origin_y);
        let right = Vector2F::new(right, tile_origin_y);

        let segment = if winding < 0 {
            LineSegment2F::new(left, right)
        } else {
            LineSegment2F::new(right, left)
        };

        debug!(
            "... emitting active fill {} -> {} winding {} @ tile {:?}",
            left.x(),
            right.x(),
            winding,
            tile_coords
        );

        while winding != 0 {
            self.add_fill(builder, render_stage, segment, tile_coords);
            if winding < 0 {
                winding += 1
            } else {
                winding -= 1
            }
        }
    }

    pub(crate) fn generate_fill_primitives_for_line(
        &mut self,
        builder: &SceneBuilder,
        render_stage: RenderStage,
        mut segment: LineSegment2F,
        tile_y: i32,
    ) {
        debug!(
            "... generate_fill_primitives_for_line(): segment={:?} tile_y={} ({}-{})",
            segment,
            tile_y,
            tile_y as f32 * TILE_HEIGHT as f32,
            (tile_y + 1) as f32 * TILE_HEIGHT as f32
        );

        let winding = segment.from_x() > segment.to_x();
        let (segment_left, segment_right) = if !winding {
            (segment.from_x(), segment.to_x())
        } else {
            (segment.to_x(), segment.from_x())
        };

        // FIXME(pcwalton): Optimize this.
        let segment_tile_left = f32::floor(segment_left) as i32 / TILE_WIDTH as i32;
        let segment_tile_right =
            util::alignup_i32(f32::ceil(segment_right) as i32, TILE_WIDTH as i32);
        debug!(
            "segment_tile_left={} segment_tile_right={} tile_rect={:?}",
            segment_tile_left,
            segment_tile_right,
            self.tile_rect()
        );

        for subsegment_tile_x in segment_tile_left..segment_tile_right {
            let (mut fill_from, mut fill_to) = (segment.from(), segment.to());
            let subsegment_tile_right =
                ((i32::from(subsegment_tile_x) + 1) * TILE_HEIGHT as i32) as f32;
            if subsegment_tile_right < segment_right {
                let x = subsegment_tile_right;
                let point = Vector2F::new(x, segment.solve_y_for_x(x));
                if !winding {
                    fill_to = point;
                    segment = LineSegment2F::new(point, segment.to());
                } else {
                    fill_from = point;
                    segment = LineSegment2F::new(segment.from(), point);
                }
            }

            let fill_segment = LineSegment2F::new(fill_from, fill_to);
            let fill_tile_coords = Vector2I::new(subsegment_tile_x, tile_y);
            self.add_fill(builder, render_stage, fill_segment, fill_tile_coords);
        }
    }

    #[inline]
    pub(crate) fn tile_coords_to_local_index(&self, coords: Vector2I) -> Option<u32> {
        self.tiles.coords_to_index(coords).map(|index| index as u32)
    }

    #[inline]
    pub(crate) fn local_tile_index_to_coords(&self, tile_index: u32) -> Vector2I {
        self.tiles.index_to_coords(tile_index as usize)
    }
}

struct CulledTiles {
    draw: Vec<AlphaTile>,
    clip: Vec<AlphaTile>,
}

impl CulledTiles {
    fn new() -> CulledTiles {
        CulledTiles { draw: vec![], clip: vec![] }
    }
}

#[derive(Debug)]
pub(crate) struct DrawBuildResult {
    pub alpha_tiles: Vec<AlphaTile>,
    pub path_id: DrawPathId,
}

#[derive(Debug)]
pub(crate) struct ClipBuildResult {
    pub alpha_tiles: Vec<AlphaTile>,
    pub tiles: DenseTileMap<TileObjectPrimitive>,
    pub path_id: ClipPathId,
}

#[derive(Debug)]
pub(crate) struct BuiltObject {
    pub bounds: RectF,
    pub fills: Vec<FillBatchPrimitive>,
    pub tiles: DenseTileMap<TileObjectPrimitive>,
    pub alpha_tiles: Vec<AlphaTile>,
    pub clip_tiles: Vec<AlphaTile>,
}
