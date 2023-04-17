use std::fs;
use serde::Deserialize;
use clap::Parser;
use image::GenericImageView;

/// Atari 7800 tool that generates C code for sprites described in a YAML file
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// YAML input file
    filename: String
}

#[derive(Debug, Deserialize)]
struct AllSprites {
    sprite_sheets: Vec<SpriteSheet>
}

#[derive(Debug, Deserialize)]
struct SpriteSheet {
    image: String,
    #[serde(default = "default_resolution")]
    resolution: String,
    sprites: Vec<Sprite>
}

#[derive(Debug, Deserialize)]
struct Sprite {
    name: String,
    top: u32,
    left: u32,
    width: u32,
    #[serde(default = "default_holeydma")]
    holeydma: bool,
    #[serde(default = "default_sprite_size")]
    height: u32,
    #[serde(default = "default_color")]
    color1: (u8, u8, u8),
    #[serde(default = "default_color")]
    color2: (u8, u8, u8),
    #[serde(default = "default_color")]
    color3: (u8, u8, u8),
}

fn default_sprite_size() -> u32 { 16 }
fn default_holeydma() -> bool { true }
fn default_resolution() -> String { "160A".to_string() }
fn default_color() -> (u8, u8, u8) { (0, 0, 0) }

fn main() -> Result <(), serde_yaml::Error> {
    let args = Args::parse();
    let contents = fs::read_to_string(args.filename).expect("Unable to read input file");
    let all_sprites: AllSprites = serde_yaml::from_str(&contents)?;
    for sprite_sheet in all_sprites.sprite_sheets {
        let img = image::open(&sprite_sheet.image).expect(&format!("Can't open image {}", sprite_sheet.image));
        for sprite in sprite_sheet.sprites {
            let mut colors = [sprite.color1, sprite.color2, sprite.color3];
            let mut bytes = Vec::<u8>::new();
            let pixel_width = match sprite_sheet.resolution.as_str() {
                "320A" | "320B" => 1,
                _ => 2,
            };
            let pixel_bits = match sprite_sheet.resolution.as_str() {
                "320A" => 1,
                "160B" => 4,
                _ => 2,
            };
            for y in 0..sprite.height {
                let mut current_byte: u8 = 0;
                let mut current_bits: u8 = 0;
                for x in 0..sprite.width / pixel_width {
                    let color = img.get_pixel(sprite.left + x * pixel_width, sprite.top + y);
                    let mut cx: Option<u8> = None;
                    if color[3] == 0 || (color[0] == 0 && color[1] == 0 && color[2] == 0) {
                        cx = Some(0); // Background color (either black or transparent)
                    } else {
                        for c in 0..3 {
                            if color[0] == colors[c].0 && color[1] == colors[c].1 && color[2] == colors[c].2 {
                                // Ok. this is a pixel of color c
                                cx = Some((c + 1) as u8);
                                break;
                            }
                        }
                        if cx.is_none() {
                            // Let's find a unaffected color
                            for c in 0..3 {
                                if colors[c].0 == 0 && colors[c].1 == 0 && colors[c].2 == 0 {
                                    colors[c].0 = color[0];
                                    colors[c].1 = color[1];
                                    colors[c].2 = color[2];
                                    cx = Some((c + 1) as u8);
                                    break;
                                }
                            }
                            if cx.is_none() {
                                panic!("Sprite {} has more than 3 colors", sprite.name);
                            }
                        }
                    }
                    current_byte |= cx.unwrap();
                    current_bits += pixel_bits;
                    if current_bits == 8 {
                        bytes.push(current_byte);
                        current_byte = 0;
                        current_bits = 0;
                    } else {
                        current_byte <<= pixel_bits;
                    }
                }
            }
            // Whoaw. We do have our pixels vector. Let's output it
            if sprite.holeydma && (sprite.height == 8 || sprite.height == 16) {
                print!("holeydma ");
            }
            print!("reversed scattered({},{}) char {}[{}] = {{\n\t", sprite.height, sprite.width / pixel_width * (pixel_bits as u32) / 8, sprite.name, bytes.len());
            for i in 0..bytes.len() - 1 {
                print!("0x{:02x}", bytes[i]);
                if (i + 1) % 16 != 0 {
                    print!(", ");
                } else {
                    print!(",\n\t");
                }
            } 
            println!("0x{:02x}\n}};", bytes[bytes.len() - 1]);
        }
    } 

    Ok(())
}
