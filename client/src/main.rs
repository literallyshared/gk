use client::map::Map;
use macroquad::prelude::*;

#[macroquad::main("Graal Kingdoms")]
async fn main() {
    macroquad::window::request_new_screen_size(1024., 920.);
    let map = Map::load("client/assets/map").await;
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
