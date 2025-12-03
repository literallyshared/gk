use client::map::Map;
use macroquad::prelude::*;

struct CommandLineArgs {
    pub offline_mode: bool,
}

impl CommandLineArgs {
    pub fn parse(args: Vec<String>) -> Self {
        let offline_mode = args.contains(&"--offline".to_string());
        Self { offline_mode }
    }
}

#[macroquad::main("Graal Kingdoms")]
async fn main() {
    info!("Starting Graal Kingdoms client");
    let args: Vec<String> = std::env::args().collect();
    let args = CommandLineArgs::parse(args);

    macroquad::window::request_new_screen_size(1024., 768.);
    // TODO: pass asset path
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
