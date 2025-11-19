use std::path::Path;
use std::{collections::HashMap, fs::File};
use std::io::prelude::*;

use macroquad::prelude::*;
use macroquad_tiled::TileSet;
use tiled::{LayerType, TileLayer};

#[macroquad::main("Graal Kingdoms")]
async fn main() {
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

pub struct Map {
    tilemap: tiled::Map,
    heightmap: Vec<u32>,
    lightmap: Vec<f32>,
    max_height: u32,
    shader: Option<Material>,
    texture: Option<Texture2D>,
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
        let mut texture = macroquad::texture::load_texture("assets/picso.png").await.ok();
        if let Some(texture) = texture.as_mut() {
            texture.set_filter(FilterMode::Nearest);
        }
        let mut map = Map {
            tilemap,
            heightmap,
            lightmap: vec![],
            max_height,
            shader: None,
            texture,
        };
        if !map.heightmap.is_empty() {
            match load_material(
                ShaderSource::Glsl {
                    vertex: TILE_VERTEX_SHADER,
                    fragment: TILE_FRAGMENT_SHADER,
                },
                MaterialParams {
                    uniforms: vec![
                        UniformDesc::new("tile_size", UniformType::Float2),
                        UniformDesc::new("tile_coord", UniformType::Float2),
                        UniformDesc::new("shadow_alpha", UniformType::Float1),
                    ],
                    ..Default::default()
                },
            ) {
                Ok(shader) => map.shader = Some(shader),
                Err(e) => warn!("Failed to load shader: [{:?}]", e),
            }
        }
        map.calculate_lightmap();
        Some(map)
    }

    pub fn draw(&self, viewport: Rect) {
        let width = self.tilemap.width;
        let height = self.tilemap.height;
        if let Some(shader) = &self.shader {
            gl_use_material(&shader);
        }
        let y_offset = self.max_height as f32 * TILE_HEIGHT_OFFSET;
        for layer in self.tilemap.layers() {
            if let LayerType::Tiles(tiles) = layer.layer_type() {
                for y in 0..height {
                    for x in 0..width {
                        if let Some(tile) = tiles.get_tile(x as i32, y as i32) {
                            let tileset = tile.get_tileset();
                            let tx = x as f32 * tileset.tile_width as f32;
                            let ty = y as f32 * tileset.tile_height as f32 - y_offset;
                            let tw = tileset.tile_width as f32;
                            let th = tileset.tile_height as f32;
                            let height_offset = self.max_height as f32 * self.get_entity_offset_per_height_unit() * th * 2.;
                            if !viewport.overlaps(&Rect::new(tx, ty - height_offset, tw, th + height_offset)) {
                                // TODO: these height offsets arent very precise..
                                continue;
                            }
                            if tileset.image.is_none() {
                                continue;
                            }
                            if let Some(texture) = &self.texture {
                                let tileset_width = texture.width() / tileset.tile_width as f32;
                                let s = (tile.id() % tileset_width as u32) as f32 * tileset.tile_width as f32 / texture.width();
                                let t = (tile.id() / tileset_width as u32) as f32 * tileset.tile_height as f32 / texture.height();
                                let s1 = tileset.tile_width as f32 / texture.width();
                                let t1 = tileset.tile_height as f32 / texture.height();
                                let top_left_offset = *self.get_height(x, y).unwrap_or(&0) as f32 * TILE_HEIGHT_OFFSET;
                                let top_right_offset = *self.get_height(x + 1, y).unwrap_or(&0) as f32 * TILE_HEIGHT_OFFSET;
                                let bottom_left_offset = *self.get_height(x, y + 1).unwrap_or(&0) as f32 * TILE_HEIGHT_OFFSET;
                                let bottom_right_offset = *self.get_height(x + 1, y + 1).unwrap_or(&0) as f32 * TILE_HEIGHT_OFFSET;
                                let mesh = Mesh {
                                    indices: vec![0, 1, 3, 0, 2, 3],
                                    vertices: vec![
                                        Vertex { position: vec3(tx, ty + top_left_offset, 0.), uv: vec2(s, t), color: [255, 255, 255, 255], normal: Vec4::default(), },
                                        Vertex { position: vec3(tx + tw, ty + top_right_offset, 0.), uv: vec2(s + s1, t), color: [255, 255, 255, 255], normal: Vec4::default(), },
                                        Vertex { position: vec3(tx, ty + th + bottom_left_offset, 0.), uv: vec2(s, t + t1), color: [255, 255, 255, 255], normal: Vec4::default(), },
                                        Vertex { position: vec3(tx + tw, ty + th + bottom_right_offset, 0.), uv: vec2(s + s1, t + t1), color: [255, 255, 255, 255], normal: Vec4::default(), },
                                    ],
                                    texture: Some(texture.clone()),
                                };
                                if let Some(shader) = &self.shader {
                                    if let Some(light) = self.get_light(x, y) {
                                        if let Some(height) = self.get_height(x, y) {
                                            if *height > WATER_LEVEL_HEIGHT {
                                                shader.set_uniform("shadow_alpha", *light);
                                            } else {
                                                let value = 255.0 - 10.0 * (WATER_LEVEL_HEIGHT as f32 - *height as f32);
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
        if self.heightmap.len() < total_indices as usize{
            warn!("Heightmap dimensions are not aligned with map dimensions: [{total_indices}] vs [{}]", self.heightmap.len());
            return None;
        }
        let index = (y * self.tilemap.width + x) % total_indices;
        self.heightmap.get(index as usize)
    }

    pub fn get_light(&self, x: u32, y: u32) -> Option<&f32> {
        let total_indices = self.tilemap.width * self.tilemap.height;
        if self.lightmap.len() < total_indices as usize{
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
                    let shadow_slope =  (shadow - *neighbor_height as f32) * 0.8;
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
}

const TILE_FRAGMENT_SHADER: &'static str = r#"#version 330
precision lowp float;
uniform vec2 tile_size;
uniform vec2 tile_coord;
uniform sampler2D Texture;
uniform sampler2D heightmap;

uniform float shadow_alpha;

in vec2 uv;

void main() {
    vec4 color = texture2D(Texture, uv);
    //vec4 shadow = vec4(0.6, 0.6, 0.6, 1.0);
    float value = shadow_alpha / 255;
    vec4 shadow = vec4(value, value, value, value);
    gl_FragColor = color * shadow; //texture2D(Texture, uv);
}
"#;

const TILE_VERTEX_SHADER: &'static str = "#version 330
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
uniform mat4 Model;
uniform mat4 Projection;

out vec2 uv;

void main() {
    uv = texcoord;
    gl_Position = Projection * Model * vec4(position, 1);
}
";
