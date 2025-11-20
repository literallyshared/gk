use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

use macroquad::prelude::*;
use tiled::LayerType;

#[macroquad::main("Graal Kingdoms")]
async fn main() {
    macroquad::window::request_new_screen_size(1024., 920.);
    let map = Map::load("assets/map").await;
    loop {
        if let Some(map) = &map {
            clear_background(BLACK);
            let (map_width, map_height) = map.get_pixel_size();
            let viewport_width = screen_width();
            let viewport_height = screen_height();
            let viewport = Rect::new(
                map_width * 0.5 - viewport_width * 0.5,
                map_height * 0.5 - viewport_height * 0.5,
                viewport_width,
                viewport_height,
            );
            let camera = Camera2D {
                target: vec2(map_width * 0.5, map_height * 0.5),
                zoom: vec2(2.0 / viewport_width, 2.0 / viewport_height),
                ..Default::default()
            };
            set_camera(&camera);
            map.draw(viewport);
            set_default_camera();
        }
        next_frame().await;
    }
}

const TILE_HEIGHT_OFFSET: f32 = -4.0;
const WATER_LEVEL_HEIGHT: u32 = 50;
const MAX_WATER_DISTANCE: f32 = 12.0;
const SHALLOW_WATER_COLOR: Color = Color::new(0.28, 0.78, 0.9, 1.0);
const DEEP_WATER_COLOR: Color = Color::new(0.0, 0.16, 0.38, 1.0);
const FOAM_EDGE_COLOR: Color = Color::new(0.98, 0.99, 1.0, 1.0);
const MIN_WATER_ALPHA: f32 = 0.9;
const LAND_BLEND_DISTANCE: f32 = 3.5;
const MIN_LAND_ALPHA: f32 = 0.85;
const COAST_SMOOTHING_RANGE: f32 = 2.4;
const COAST_OFFSET_SCALE: f32 = 0.7;
const COAST_CORNER_PULL: f32 = 0.98;
const COAST_SUBDIV: usize = 3;
const COAST_BLEND_DISTANCE: f32 = 2.5;
const CARDINAL_NEIGHBORS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
const ALL_NEIGHBORS: [(i32, i32); 8] = [
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
];
const DISTANCE_NEIGHBORS: [(i32, i32, f32); 8] = [
    (-1, 0, 1.0),
    (1, 0, 1.0),
    (0, -1, 1.0),
    (0, 1, 1.0),
    (-1, -1, SQRT_2),
    (-1, 1, SQRT_2),
    (1, -1, SQRT_2),
    (1, 1, SQRT_2),
];
const SQRT_2: f32 = 1.41421356237;

pub struct Map {
    tilemap: tiled::Map,
    heightmap: Vec<u32>,
    lightmap: Vec<f32>,
    max_height: u32,
    shader: Option<Material>,
    water_shader: Option<Material>,
    texture: Option<Texture2D>,
    tile_classes: Vec<TileClassification>,
    distance_to_land: Vec<f32>,
    distance_texture: Option<Texture2D>,
    vertex_offsets: Vec<Vec2>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileClassification {
    Land,
    Coast,
    ShallowWater,
    DeepWater,
}

impl Map {
    pub async fn load(map_path: &str) -> Option<Map> {
        let tilemap_path = format!("{}.tmx", map_path);
        if !Path::new(&tilemap_path).exists() {
            error!("Tilemap not found: {}", tilemap_path);
            return None;
        }
        let tilemap = if let Ok(tilemap) = tiled::Loader::new().load_tmx_map(tilemap_path) {
            tilemap
        } else {
            return None;
        };
        let heightmap_path = format!("{}.heightmap", map_path);
        let heightmap = if Path::new(&heightmap_path).exists() {
            Self::load_heightmap(&heightmap_path)
        } else {
            vec![]
        };
        let max_height = heightmap.iter().cloned().fold(0, u32::max);
        let mut texture = macroquad::texture::load_texture("assets/picso.png")
            .await
            .ok();
        if let Some(texture) = texture.as_mut() {
            texture.set_filter(macroquad::texture::FilterMode::Nearest);
        }
        let mut map = Map {
            tilemap,
            heightmap,
            lightmap: vec![],
            max_height,
            shader: None,
            water_shader: None,
            texture,
            tile_classes: vec![],
            distance_to_land: vec![],
            distance_texture: None,
            vertex_offsets: vec![],
        };
        if !map.heightmap.is_empty() {
            match load_material(
                ShaderSource::Glsl {
                    vertex: TILE_VERTEX_SHADER,
                    fragment: TILE_FRAGMENT_SHADER,
                },
                MaterialParams {
                    uniforms: vec![
                        UniformDesc::new("shadow_alpha", UniformType::Float1),
                        UniformDesc::new("min_land_alpha", UniformType::Float1),
                    ],
                    ..Default::default()
                },
            ) {
                Ok(shader) => map.shader = Some(shader),
                Err(e) => warn!("Failed to load shader: [{:?}]", e),
            }

            match load_material(
                ShaderSource::Glsl {
                    vertex: WATER_VERTEX_SHADER,
                    fragment: WATER_FRAGMENT_SHADER,
                },
                MaterialParams {
                    uniforms: vec![
                        UniformDesc::new("shallow_color", UniformType::Float4),
                        UniformDesc::new("deep_color", UniformType::Float4),
                        UniformDesc::new("foam_color", UniformType::Float4),
                        UniformDesc::new("foam_width", UniformType::Float1),
                        UniformDesc::new("shallow_width", UniformType::Float1),
                        UniformDesc::new("min_alpha", UniformType::Float1),
                        UniformDesc::new("max_distance", UniformType::Float1),
                    ],
                    ..Default::default()
                },
            ) {
                Ok(shader) => map.water_shader = Some(shader),
                Err(e) => warn!("Failed to load water shader: [{:?}]", e),
            }
        }
        map.calculate_lightmap();
        map.rebuild_tile_metadata();
        Some(map)
    }

    pub fn draw(&self, viewport: Rect) {
        self.draw_water_tiles(viewport);
        if let Some(shader) = &self.shader {
            shader.set_uniform("min_land_alpha", MIN_LAND_ALPHA);
            gl_use_material(shader);
        }
        self.draw_land_tiles(viewport);
        gl_use_default_material();
    }

    fn draw_land_tiles(&self, viewport: Rect) {
        let Some(texture) = &self.texture else {
            return;
        };
        let width = self.tilemap.width;
        let height = self.tilemap.height;
        let y_offset = self.max_height as f32 * TILE_HEIGHT_OFFSET;
        for layer in self.tilemap.layers() {
            if let LayerType::Tiles(tiles) = layer.layer_type() {
                for y in 0..height {
                    for x in 0..width {
                        if let Some(tile) = tiles.get_tile(x as i32, y as i32) {
                            let classification = self.get_tile_classification(x, y);
                            if !matches!(
                                classification,
                                Some(TileClassification::Land) | Some(TileClassification::Coast)
                            ) {
                                continue;
                            }
                            let tileset = tile.get_tileset();
                            if tileset.image.is_none() {
                                continue;
                            }
                            let tx = x as f32 * tileset.tile_width as f32;
                            let ty = y as f32 * tileset.tile_height as f32 - y_offset;
                            let tw = tileset.tile_width as f32;
                            let th = tileset.tile_height as f32;
                            let height_offset = self.max_height as f32
                                * self.get_entity_offset_per_height_unit()
                                * th
                                * 2.;
                            if !viewport.overlaps(&Rect::new(
                                tx,
                                ty - height_offset,
                                tw,
                                th + height_offset,
                            )) {
                                continue;
                            }
                            let tileset_width = texture.width() / tileset.tile_width as f32;
                            let s = (tile.id() % tileset_width as u32) as f32
                                * tileset.tile_width as f32
                                / texture.width();
                            let t = (tile.id() / tileset_width as u32) as f32
                                * tileset.tile_height as f32
                                / texture.height();
                            let s1 = tileset.tile_width as f32 / texture.width();
                            let t1 = tileset.tile_height as f32 / texture.height();
                            let top_left_offset =
                                *self.get_height(x, y).unwrap_or(&0) as f32 * TILE_HEIGHT_OFFSET;
                            let top_right_offset = *self.get_height(x + 1, y).unwrap_or(&0) as f32
                                * TILE_HEIGHT_OFFSET;
                            let bottom_left_offset = *self.get_height(x, y + 1).unwrap_or(&0)
                                as f32
                                * TILE_HEIGHT_OFFSET;
                            let bottom_right_offset = *self.get_height(x + 1, y + 1).unwrap_or(&0)
                                as f32
                                * TILE_HEIGHT_OFFSET;
                            let offsets = self.quad_vertex_offsets(x, y);
                            let mesh = Mesh {
                                indices: vec![0, 1, 3, 0, 2, 3],
                                vertices: vec![
                                    Vertex {
                                        position: vec3(
                                            tx + offsets[0].x,
                                            ty + top_left_offset + offsets[0].y,
                                            0.,
                                        ),
                                        uv: vec2(s, t),
                                        color: self.land_vertex_color(x as i32, y as i32),
                                        normal: Vec4::default(),
                                    },
                                    Vertex {
                                        position: vec3(
                                            tx + tw + offsets[1].x,
                                            ty + top_right_offset + offsets[1].y,
                                            0.,
                                        ),
                                        uv: vec2(s + s1, t),
                                        color: self.land_vertex_color(x as i32 + 1, y as i32),
                                        normal: Vec4::default(),
                                    },
                                    Vertex {
                                        position: vec3(
                                            tx + offsets[2].x,
                                            ty + th + bottom_left_offset + offsets[2].y,
                                            0.,
                                        ),
                                        uv: vec2(s, t + t1),
                                        color: self.land_vertex_color(x as i32, y as i32 + 1),
                                        normal: Vec4::default(),
                                    },
                                    Vertex {
                                        position: vec3(
                                            tx + tw + offsets[3].x,
                                            ty + th + bottom_right_offset + offsets[3].y,
                                            0.,
                                        ),
                                        uv: vec2(s + s1, t + t1),
                                        color: self.land_vertex_color(x as i32 + 1, y as i32 + 1),
                                        normal: Vec4::default(),
                                    },
                                ],
                                texture: Some(texture.clone()),
                            };
                            if let Some(shader) = &self.shader {
                                if let Some(light) = self.get_light(x, y) {
                                    if let Some(height) = self.get_height(x, y) {
                                        if *height > WATER_LEVEL_HEIGHT {
                                            shader.set_uniform("shadow_alpha", *light);
                                        } else {
                                            let value = 255.0
                                                - 10.0
                                                    * (WATER_LEVEL_HEIGHT as f32 - *height as f32);
                                            shader.set_uniform("shadow_alpha", value);
                                        }
                                    }
                                }
                            }
                            draw_mesh(&mesh);
                        }
                    }
                }
            }
        }
    }

    fn draw_water_tiles(&self, _viewport: Rect) {
        let (Some(shader), Some(distance_texture)) = (&self.water_shader, &self.distance_texture)
        else {
            return;
        };
        shader.set_uniform("shallow_color", color_to_vec4(SHALLOW_WATER_COLOR));
        shader.set_uniform("deep_color", color_to_vec4(DEEP_WATER_COLOR));
        shader.set_uniform("foam_color", color_to_vec4(FOAM_EDGE_COLOR));
        shader.set_uniform("foam_width", 0.8f32);
        shader.set_uniform("shallow_width", 2.5f32);
        shader.set_uniform("min_alpha", MIN_WATER_ALPHA);
        shader.set_uniform("max_distance", MAX_WATER_DISTANCE);
        gl_use_material(shader);

        let map_width = self.tilemap.width as f32 * self.tilemap.tile_width as f32;
        let map_height = self.tilemap.height as f32 * self.tilemap.tile_height as f32;
        let y_offset = self.max_height as f32 * TILE_HEIGHT_OFFSET;
        let sea_offset = WATER_LEVEL_HEIGHT as f32 * TILE_HEIGHT_OFFSET;

        let vertices = vec![
            Vertex {
                position: vec3(0.0, -y_offset + sea_offset, 0.0),
                uv: vec2(0.0, 0.0),
                color: [255, 255, 255, 255],
                normal: Vec4::default(),
            },
            Vertex {
                position: vec3(map_width, -y_offset + sea_offset, 0.0),
                uv: vec2(1.0, 0.0),
                color: [255, 255, 255, 255],
                normal: Vec4::default(),
            },
            Vertex {
                position: vec3(0.0, map_height - y_offset + sea_offset, 0.0),
                uv: vec2(0.0, 1.0),
                color: [255, 255, 255, 255],
                normal: Vec4::default(),
            },
            Vertex {
                position: vec3(map_width, map_height - y_offset + sea_offset, 0.0),
                uv: vec2(1.0, 1.0),
                color: [255, 255, 255, 255],
                normal: Vec4::default(),
            },
        ];

        let mesh = Mesh {
            indices: vec![0, 1, 3, 0, 2, 3],
            vertices,
            texture: Some(distance_texture.clone()),
        };
        draw_mesh(&mesh);
        gl_use_default_material();
    }

    pub fn get_size(&self) -> usize {
        self.tilemap.width as usize * self.tilemap.height as usize
    }

    pub fn get_tile_dimensions(&self) -> (u32, u32) {
        (self.tilemap.tile_width, self.tilemap.tile_height)
    }

    pub fn get_pixel_size(&self) -> (f32, f32) {
        (
            self.tilemap.width as f32 * self.tilemap.tile_width as f32,
            self.tilemap.height as f32 * self.tilemap.tile_height as f32,
        )
    }

    pub fn get_tile_classification(&self, x: u32, y: u32) -> Option<TileClassification> {
        if self.tile_classes.is_empty() {
            return Some(TileClassification::Land);
        }
        let index = self.try_index(x as i32, y as i32)?;
        self.tile_classes.get(index).copied()
    }

    pub fn get_water_distance(&self, x: u32, y: u32) -> Option<f32> {
        let index = self.try_index(x as i32, y as i32)?;
        self.distance_to_land.get(index).copied()
    }

    pub fn position_to_tile_coordinates(&self, x: f32, y: f32) -> (u32, u32) {
        let tile_x = x as u32 / self.tilemap.tile_width % self.tilemap.width;
        let mut calculated_y = 0.;
        for i in 0..self.tilemap.height {
            if let Some(height) = self.get_height(tile_x, i) {
                let top = *height as f32 * -TILE_HEIGHT_OFFSET;
                if let Some(next_height) = self.get_height(tile_x, i + 1) {
                    let bottom = *next_height as f32 * -TILE_HEIGHT_OFFSET;
                    calculated_y += self.tilemap.tile_height as f32 + bottom - top;
                    if calculated_y > y {
                        return (tile_x, i);
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        (0, 0)
    }

    pub fn is_position_submerged(&self, x: u32, y: u32) -> bool {
        let mut tiles_submerged = 0;
        if *self.get_height(x, y).unwrap_or(&u32::MAX) < WATER_LEVEL_HEIGHT {
            tiles_submerged += 1;
        }
        if *self.get_height(x - 1, y).unwrap_or(&u32::MAX) < WATER_LEVEL_HEIGHT {
            tiles_submerged += 1;
        }
        if *self.get_height(x + 1, y).unwrap_or(&u32::MAX) < WATER_LEVEL_HEIGHT {
            tiles_submerged += 1;
        }
        if *self.get_height(x, y - 1).unwrap_or(&u32::MAX) < WATER_LEVEL_HEIGHT {
            tiles_submerged += 1;
        }
        if *self.get_height(x, y + 1).unwrap_or(&u32::MAX) < WATER_LEVEL_HEIGHT {
            tiles_submerged += 1;
        }
        if *self.get_height(x + 1, y + 1).unwrap_or(&u32::MAX) < WATER_LEVEL_HEIGHT {
            tiles_submerged += 1;
        }
        if *self.get_height(x - 1, y - 1).unwrap_or(&u32::MAX) < WATER_LEVEL_HEIGHT {
            tiles_submerged += 1;
        }
        if *self.get_height(x + 1, y - 1).unwrap_or(&u32::MAX) < WATER_LEVEL_HEIGHT {
            tiles_submerged += 1;
        }
        if *self.get_height(x - 1, y + 1).unwrap_or(&u32::MAX) < WATER_LEVEL_HEIGHT {
            tiles_submerged += 1;
        }
        tiles_submerged > 6
    }

    pub fn get_height(&self, x: u32, y: u32) -> Option<&u32> {
        let total_indices = self.tilemap.width * self.tilemap.height;
        if self.heightmap.len() < total_indices as usize {
            warn!(
                "Heightmap dimensions are not aligned with map dimensions: [{total_indices}] vs [{}]",
                self.heightmap.len()
            );
            return None;
        }
        let index = (y * self.tilemap.width + x) % total_indices;
        self.heightmap.get(index as usize)
    }

    pub fn get_light(&self, x: u32, y: u32) -> Option<&f32> {
        let total_indices = self.tilemap.width * self.tilemap.height;
        if self.lightmap.len() < total_indices as usize {
            warn!("Heightmap dimensions are not aligned with map dimensions.");
            return None;
        }
        let index = (y * self.tilemap.width + x) % total_indices;
        self.lightmap.get(index as usize)
    }

    pub fn adjust_position_for_height(&self, x: &mut f32, y: &mut f32) {
        let (tile_x, tile_y) = self.position_to_tile_coordinates(*x, *y);
        if let Some(height) = self.get_height(tile_x, tile_y) {
            let offset = *height as f32 * TILE_HEIGHT_OFFSET;
            *y += offset;
        }
    }

    pub fn get_entity_offset_per_height_unit(&self) -> f32 {
        2.0 / self.get_tile_dimensions().1 as f32
    }

    fn index_to_x_y(&self, index: usize) -> (usize, usize) {
        let x = index % self.tilemap.width as usize;
        let y = (index / self.tilemap.width as usize) % self.tilemap.height as usize;
        (x, y)
    }

    fn x_y_to_index(&self, x: usize, y: usize) -> usize {
        (y * self.tilemap.width as usize + x) % self.get_size()
    }

    fn load_heightmap(path: &str) -> Vec<u32> {
        let mut heightmap = vec![];
        let mut contents = String::new();
        if let Ok(mut file) = File::open(path) {
            let _ = file.read_to_string(&mut contents);
        }
        for line in contents.lines() {
            for value in line.split(',') {
                if let Ok(value) = value.parse::<u32>() {
                    heightmap.push(value);
                }
            }
        }
        heightmap
    }

    fn calculate_lightmap(&mut self) {
        if self.heightmap.is_empty() {
            return;
        }
        let shadow_weight = 15.0;
        let mut lightmap = vec![0.0; self.get_size()];
        for i in 0..self.get_size() {
            if self.heightmap[i] > WATER_LEVEL_HEIGHT - 2 {
                let (x, y) = self.index_to_x_y(i);
                if let Some(neighbor_height) = self.get_height(x as u32 - 1, y as u32 - 1) {
                    let shadow = lightmap[self.x_y_to_index(x - 1, y - 1)];
                    let shadow_slope = (shadow - *neighbor_height as f32) * 0.8;
                    let value = (*neighbor_height as f32).max(shadow) - shadow_slope;
                    lightmap[i] = value;
                } else {
                    warn!("Lightmap calculation failed.");
                    return;
                }
            }
        }
        for i in 0..self.get_size() {
            if lightmap[i] > self.heightmap[i] as f32 {
                lightmap[i] = 255.0 - shadow_weight * (lightmap[i] - self.heightmap[i] as f32);
            } else {
                lightmap[i] = 255.0;
            }
        }
        self.lightmap = lightmap;
    }

    fn rebuild_tile_metadata(&mut self) {
        if self.heightmap.len() != self.get_size() {
            self.tile_classes.clear();
            self.distance_to_land.clear();
            self.distance_texture = None;
            self.vertex_offsets.clear();
            return;
        }
        self.distance_to_land = self.calculate_distance_to_land();
        self.upload_distance_texture();
        self.vertex_offsets = self.calculate_vertex_offsets();
        self.tile_classes = self.classify_tiles();
    }

    fn calculate_distance_to_land(&self) -> Vec<f32> {
        self.calculate_distance_field(|height| height >= WATER_LEVEL_HEIGHT)
    }

    fn calculate_distance_field<F>(&self, mut source_predicate: F) -> Vec<f32>
    where
        F: FnMut(u32) -> bool,
    {
        #[derive(Copy, Clone)]
        struct State {
            cost: f32,
            index: usize,
        }
        impl PartialEq for State {
            fn eq(&self, other: &Self) -> bool {
                self.cost.eq(&other.cost)
            }
        }
        impl Eq for State {}
        impl PartialOrd for State {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                other.cost.partial_cmp(&self.cost)
            }
        }
        impl Ord for State {
            fn cmp(&self, other: &Self) -> Ordering {
                self.partial_cmp(other).unwrap_or(Ordering::Equal)
            }
        }

        let mut distance = vec![f32::INFINITY; self.get_size()];
        let mut heap = BinaryHeap::new();
        for index in 0..self.get_size() {
            if source_predicate(self.heightmap[index]) {
                distance[index] = 0.0;
                heap.push(State { cost: 0.0, index });
            }
        }
        while let Some(State { cost, index }) = heap.pop() {
            if cost > distance[index] {
                continue;
            }
            let (x, y) = self.index_to_x_y(index);
            for (dx, dy, weight) in DISTANCE_NEIGHBORS {
                if let Some(neighbor_index) = self.try_index(x as i32 + dx, y as i32 + dy) {
                    let next = cost + weight;
                    if next < distance[neighbor_index] {
                        distance[neighbor_index] = next;
                        heap.push(State {
                            cost: next,
                            index: neighbor_index,
                        });
                    }
                }
            }
        }
        distance
    }

    fn upload_distance_texture(&mut self) {
        if self.distance_to_land.is_empty() {
            self.distance_texture = None;
            return;
        }
        let width = self.tilemap.width as u16;
        let height = self.tilemap.height as u16;
        let mut image = Image::gen_image_color(width, height, Color::new(0.0, 0.0, 0.0, 1.0));
        let scale = if MAX_WATER_DISTANCE > 0.0 {
            1.0 / MAX_WATER_DISTANCE
        } else {
            1.0
        };
        for y in 0..height {
            for x in 0..width {
                let index = self.x_y_to_index(x as usize, y as usize);
                let distance = self
                    .distance_to_land
                    .get(index)
                    .copied()
                    .unwrap_or(f32::INFINITY);
                let normalized = if distance.is_finite() {
                    (distance * scale).min(1.0)
                } else {
                    1.0
                };
                image.set_pixel(
                    x as u32,
                    y as u32,
                    Color::new(normalized, normalized, normalized, 1.0),
                );
            }
        }
        let texture = Texture2D::from_image(&image);
        texture.set_filter(FilterMode::Linear);
        self.distance_texture = Some(texture);
    }

    fn classify_tiles(&self) -> Vec<TileClassification> {
        let mut classes = vec![TileClassification::Land; self.get_size()];
        for index in 0..self.get_size() {
            let (x, y) = self.index_to_x_y(index);
            let height = self.heightmap[index];
            if height < WATER_LEVEL_HEIGHT {
                let distance = self
                    .distance_to_land
                    .get(index)
                    .copied()
                    .unwrap_or(f32::INFINITY);
                if distance <= 2.5 {
                    classes[index] = TileClassification::ShallowWater;
                } else {
                    classes[index] = TileClassification::DeepWater;
                }
            } else if self.has_water_neighbor(x as u32, y as u32) {
                classes[index] = TileClassification::Coast;
            } else {
                classes[index] = TileClassification::Land;
            }
        }
        classes
    }

    fn has_water_neighbor(&self, x: u32, y: u32) -> bool {
        for (dx, dy) in ALL_NEIGHBORS {
            if self.is_water_tile(x as i32 + dx, y as i32 + dy) {
                return true;
            }
        }
        false
    }

    fn is_water_tile(&self, x: i32, y: i32) -> bool {
        if let Some(height) = self.get_height_at(x, y) {
            height < WATER_LEVEL_HEIGHT
        } else {
            false
        }
    }

    fn get_height_at(&self, x: i32, y: i32) -> Option<u32> {
        let index = self.try_index(x, y)?;
        self.heightmap.get(index).copied()
    }

    fn try_index(&self, x: i32, y: i32) -> Option<usize> {
        if self.in_bounds(x, y) {
            Some(y as usize * self.tilemap.width as usize + x as usize)
        } else {
            None
        }
    }

    fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.tilemap.width as i32 && y < self.tilemap.height as i32
    }

    fn land_vertex_color(&self, x: i32, y: i32) -> [u8; 4] {
        if self.distance_to_land.is_empty() {
            return [255, 255, 255, 255];
        }
        let distance = self.sample_distance_at(x, y);
        let normalized = (distance / LAND_BLEND_DISTANCE).clamp(0.0, 1.0);
        let value = (normalized * 255.0) as u8;
        [value, 0, 0, 255]
    }

    fn quad_vertex_offsets(&self, x: u32, y: u32) -> [Vec2; 4] {
        if self.vertex_offsets.is_empty() {
            return [Vec2::ZERO; 4];
        }
        let w = self.tilemap.width as usize + 1;
        let idx = |vx: usize, vy: usize| vy * w + vx;
        [
            self.vertex_offsets[idx(x as usize, y as usize)],
            self.vertex_offsets[idx(x as usize + 1, y as usize)],
            self.vertex_offsets[idx(x as usize, y as usize + 1)],
            self.vertex_offsets[idx(x as usize + 1, y as usize + 1)],
        ]
    }

    fn calculate_vertex_offsets(&self) -> Vec<Vec2> {
        if self.distance_to_land.is_empty() {
            return vec![];
        }
        let width = self.tilemap.width as usize;
        let height = self.tilemap.height as usize;
        let mut offsets = vec![Vec2::ZERO; (width + 1) * (height + 1)];
        for y in 0..=height {
            for x in 0..=width {
                let is_border = x == 0 || y == 0 || x == width || y == height;
                offsets[y * (width + 1) + x] = if is_border {
                    Vec2::ZERO
                } else {
                    self.corner_pull_offset(
                        x,
                        y,
                        self.tilemap.tile_width as f32,
                        self.tilemap.tile_height as f32,
                    )
                };
            }
        }
        offsets
    }

    fn corner_pull_offset(&self, vx: usize, vy: usize, tile_width: f32, tile_height: f32) -> Vec2 {
        let vx_i = vx as i32;
        let vy_i = vy as i32;
        let tiles = [
            self.is_water_tile(vx_i - 1, vy_i - 1), // nw
            self.is_water_tile(vx_i, vy_i - 1),     // ne
            self.is_water_tile(vx_i - 1, vy_i),     // sw
            self.is_water_tile(vx_i, vy_i),         // se
        ];
        let water_count = tiles.iter().filter(|t| **t).count();
        if water_count == 0 || water_count == 4 {
            return Vec2::ZERO;
        }

        let mut push = Vec2::ZERO;
        if tiles[0] {
            push += vec2(1.0, 1.0);
        }
        if tiles[1] {
            push += vec2(-1.0, 1.0);
        }
        if tiles[2] {
            push += vec2(1.0, -1.0);
        }
        if tiles[3] {
            push += vec2(-1.0, -1.0);
        }
        if push.length_squared() < 0.0001 {
            return Vec2::ZERO;
        }
        let dir = push.normalize();
        let strength = (water_count as f32 / 3.0).min(1.0) * COAST_CORNER_PULL;
        Vec2::new(
            dir.x * tile_width * strength,
            dir.y * tile_height * strength,
        )
    }

    fn sample_distance_at(&self, mut x: i32, mut y: i32) -> f32 {
        if self.distance_to_land.is_empty() {
            return MAX_WATER_DISTANCE;
        }
        let max_x = self.tilemap.width as i32 - 1;
        let max_y = self.tilemap.height as i32 - 1;
        x = x.clamp(0, max_x);
        y = y.clamp(0, max_y);
        let index = (y as usize * self.tilemap.width as usize + x as usize)
            .min(self.distance_to_land.len().saturating_sub(1));
        if let Some(distance) = self.distance_to_land.get(index) {
            if distance.is_finite() {
                return distance.min(MAX_WATER_DISTANCE);
            }
        }
        MAX_WATER_DISTANCE
    }

    fn sample_distance_continuous(&self, x: f32, y: f32) -> f32 {
        let map_width = self.tilemap.width as f32;
        let map_height = self.tilemap.height as f32;
        let clamped_x = x.clamp(0.0, map_width - 1.0);
        let clamped_y = y.clamp(0.0, map_height - 1.0);
        let x0 = clamped_x.floor() as i32;
        let y0 = clamped_y.floor() as i32;
        let x1 = (x0 + 1).min(self.tilemap.width as i32 - 1);
        let y1 = (y0 + 1).min(self.tilemap.height as i32 - 1);
        let sx = clamped_x - x0 as f32;
        let sy = clamped_y - y0 as f32;
        let d00 = self.sample_distance_at(x0, y0);
        let d10 = self.sample_distance_at(x1, y0);
        let d01 = self.sample_distance_at(x0, y1);
        let d11 = self.sample_distance_at(x1, y1);
        let dx0 = d00 + (d10 - d00) * sx;
        let dx1 = d01 + (d11 - d01) * sx;
        dx0 + (dx1 - dx0) * sy
    }

    fn distance_gradient(&self, x: f32, y: f32) -> Vec2 {
        let s = 0.35;
        let d_right = self.sample_distance_continuous(x + s, y);
        let d_left = self.sample_distance_continuous(x - s, y);
        let d_up = self.sample_distance_continuous(x, y + s);
        let d_down = self.sample_distance_continuous(x, y - s);
        let d_up_right = self.sample_distance_continuous(x + s, y + s);
        let d_up_left = self.sample_distance_continuous(x - s, y + s);
        let d_down_right = self.sample_distance_continuous(x + s, y - s);
        let d_down_left = self.sample_distance_continuous(x - s, y - s);
        let gx = (d_right - d_left) * 0.5
            + (d_up_right - d_up_left) * 0.25
            + (d_down_right - d_down_left) * 0.25;
        let gy = (d_up - d_down) * 0.5
            + (d_up_right - d_down_right) * 0.25
            + (d_up_left - d_down_left) * 0.25;
        vec2(gx, gy)
    }
}

const TILE_VERTEX_SHADER: &'static str = "#version 330
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
uniform mat4 Model;
uniform mat4 Projection;

out vec2 uv;
out float dist_factor;

void main() {
    uv = texcoord;
    dist_factor = color0.r / 255.0;
    gl_Position = Projection * Model * vec4(position, 1);
}
";

const TILE_FRAGMENT_SHADER: &'static str = r#"#version 330
precision lowp float;
uniform sampler2D Texture;
uniform float min_land_alpha;
uniform float shadow_alpha;

in vec2 uv;
in float dist_factor;

void main() {
    vec4 tex_color = texture2D(Texture, uv);
    float alpha = mix(min_land_alpha, 1.0, clamp(dist_factor, 0.0, 1.0));
    float shadow_value = shadow_alpha / 255.0;
    vec4 shadow = vec4(shadow_value, shadow_value, shadow_value, shadow_value);
    gl_FragColor = vec4(tex_color.rgb, alpha) * shadow;
}
"#;

const WATER_VERTEX_SHADER: &'static str = "#version 330
attribute vec3 position;
attribute vec2 texcoord;
uniform mat4 Model;
uniform mat4 Projection;

out vec2 uv;

void main() {
    uv = texcoord;
    gl_Position = Projection * Model * vec4(position, 1);
}
";

const WATER_FRAGMENT_SHADER: &'static str = r#"#version 330
precision mediump float;
uniform vec4 shallow_color;
uniform vec4 deep_color;
uniform vec4 foam_color;
uniform float foam_width;
uniform float shallow_width;
uniform float min_alpha;
uniform float max_distance;
uniform sampler2D Texture;

in vec2 uv;

void main() {
    float encoded = texture2D(Texture, uv).r;
    float distance = encoded * max_distance;
    float shallow_mix = smoothstep(0.0, shallow_width, distance);
    vec3 water_color = mix(shallow_color.rgb, deep_color.rgb, shallow_mix);
    float foam_factor = 1.0 - smoothstep(0.0, foam_width, distance);
    vec3 final_color = mix(water_color, foam_color.rgb, foam_factor);
    float depth_alpha = mix(1.0, min_alpha, clamp(distance / max_distance, 0.0, 1.0));
    gl_FragColor = vec4(final_color, depth_alpha);
}
"#;

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

fn color_to_bytes(color: Color) -> [u8; 4] {
    [
        (color.r.clamp(0.0, 1.0) * 255.0) as u8,
        (color.g.clamp(0.0, 1.0) * 255.0) as u8,
        (color.b.clamp(0.0, 1.0) * 255.0) as u8,
        (color.a.clamp(0.0, 1.0) * 255.0) as u8,
    ]
}

fn color_to_vec4(color: Color) -> (f32, f32, f32, f32) {
    (color.r, color.g, color.b, color.a)
}

fn marching_squares_edges(mask: u8) -> Vec<[[usize; 2]; 2]> {
    match mask {
        0 | 15 => vec![],
        1 => vec![[[0, 1], [0, 3]]],
        2 => vec![[[0, 1], [1, 2]]],
        3 => vec![[[0, 3], [1, 2]]],
        4 => vec![[[1, 2], [2, 3]]],
        5 => vec![[[0, 1], [1, 2]], [[0, 3], [2, 3]]],
        6 => vec![[[0, 1], [2, 3]]],
        7 => vec![[[0, 3], [2, 3]]],
        8 => vec![[[0, 3], [2, 3]]],
        9 => vec![[[0, 1], [2, 3]]],
        10 => vec![[[0, 1], [0, 3]], [[1, 2], [2, 3]]],
        11 => vec![[[1, 2], [2, 3]]],
        12 => vec![[[0, 3], [1, 2]]],
        13 => vec![[[0, 1], [1, 2]]],
        14 => vec![[[0, 1], [0, 3]]],
        _ => vec![],
    }
}

fn interpolate_edge(edge: [usize; 2], d0: f32, d1: f32, iso: f32) -> Vec2 {
    let t = if (d1 - d0).abs() > 0.0001 {
        (iso - d0) / (d1 - d0)
    } else {
        0.5
    };
    match edge[0] {
        0 => vec2(t, 0.0),
        1 => vec2(1.0, t),
        2 => vec2(1.0 - t, 1.0),
        3 => vec2(0.0, 1.0 - t),
        _ => Vec2::ZERO,
    }
}
