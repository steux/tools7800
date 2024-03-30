use anyhow::{anyhow, Result};
use clap::Parser;
use image::GenericImageView;
use serde::Deserialize;
use std::fs;

/// Atari 7800 tool that generates C code for bitmaps described in a YAML file
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// YAML input file
    filename: String,
}

#[derive(Debug, Deserialize)]
struct AllBitmaps {
    #[serde(default)]
    palettes: Option<Vec<Palette>>,
    bitmap_sheets: Vec<BitmapSheet>,
}

#[derive(Debug, Deserialize)]
struct BitmapSheet {
    image: String,
    #[serde(default = "default_mode")]
    mode: String,
    default_height: Option<u8>,
    holeydma: Option<u8>,
    bank: Option<u8>,
    bitmaps: Vec<Bitmap>,
}

#[derive(Debug, Deserialize)]
struct Palette {
    name: String,
    colors: Vec<(u8, u8, u8)>,
}

#[derive(Debug, Deserialize)]
struct Bitmap {
    name: String,
    top: u32,
    left: u32,
    width: u32,
    #[serde(default = "default_bitmap_size")]
    height: u32,
    #[serde(default = "default_holeydma")]
    holeydma: bool,
    #[serde(default)]
    palette: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    alias: Option<String>,
    #[serde(default)]
    background: Option<String>,
}

fn default_bitmap_size() -> u32 {
    16
}
fn default_holeydma() -> bool {
    true
}
fn default_mode() -> String {
    "160A".to_string()
}

// Color tables:
//
// | mode | colors |
// | 160A | PXC1, PXC2, PXC3
// | 160B | P2 = 0 => P0C1, P0C2, P0C3, P1C1, P1C2, P1C3, P2C1, P2C2, P2C3, P3C1, P3C2, P3C3
// |      | P2 = 1 => P4C1, P4C2, P4C3, P5C1, P5C2, P5C3, P6C1, P6C2, P6C3, P7C1, P7C2, P7C3
// | 320A | PXC2
// | 320B | P2 = 0 => P0C1, P0C2, P0C3
// |      | P2 = 1 => P4C1, P4C2, P4C3
// | 320C | P2 = 0 => P0C2, P1C2, P2C2, P3C2
// |      | P2 = 1 => P4C2, P5C2, P6C2, P7C2
// | 320D | P2 = X, P1 = 0, P0 = 0 => PXC2
// |      | P2 = X, P1 = 0, P0 = 1 => PXC1, PXC2, PXC3 with BG on the left
// |      | P2 = X, P1 = 1, P0 = 0 => PXC1, PXC2, PXC3 with BG on the right
// |      | P2 = X, P1 = 1, P0 = 1 => PXC1, PXC3

fn main() -> Result<()> {
    let args = Args::parse();
    let contents = fs::read_to_string(args.filename).expect("Unable to read input file");
    let all_bitmaps: AllBitmaps = serde_yaml::from_str(&contents)?;
    for bitmap_sheet in all_bitmaps.bitmap_sheets {
        let img = image::open(&bitmap_sheet.image)
            .expect(&format!("Can't open image {}", bitmap_sheet.image));

        // Generate bitmaps data
        for bitmap in &bitmap_sheet.bitmaps {
            if bitmap.alias.is_none() {
                let mode = if let Some(s) = &bitmap.mode {
                    s.as_str()
                } else {
                    bitmap_sheet.mode.as_str()
                };

                let pixel_width = match mode {
                    "320A" | "320B" | "320C" | "320D" => 1,
                    _ => 2,
                };
                let pixel_bits = match mode {
                    "320A" | "320D" => 1,
                    "160B" => 4,
                    _ => 2,
                };
                let maxcolors = match mode {
                    "160A" => 3,
                    "160B" => 12,
                    "320A" => 1,
                    "320B" => 3,
                    "320C" => 4,
                    "320D" => 1,
                    _ => return Err(anyhow!("Unknown gfx {} mode", mode)),
                };

                let mut colors = [(0u8, 0u8, 0u8); 12];
                if maxcolors != 1 {
                    if let Some(palettes) = &all_bitmaps.palettes {
                        if let Some(pname) = &bitmap.palette {
                            let px = palettes.into_iter().find(|x| &x.name == pname);
                            if let Some(p) = px {
                                let mut i = 0;
                                for c in &p.colors {
                                    colors[i] = *c;
                                    i += 1;
                                }
                            }
                        }
                    }
                }

                let mut bytes = Vec::<u8>::new();
                for y in 0..bitmap.height {
                    let mut current_byte: u8 = 0;
                    let mut current_bits: u8 = 0;
                    for x in 0..bitmap.width / pixel_width {
                        let xp = bitmap.left + x * pixel_width;
                        let yp = bitmap.top + y;
                        let color = img.get_pixel(xp, yp);
                        let mut cx: Option<u8> = None;
                        // In case of defined palette, priority is to find the color in the palette, so that black is not considered as a background color
                        if (color[3] != 0 && bitmap.palette.is_some())
                            || (bitmap.palette.is_none()
                                && (color[0] != 0 || color[1] != 0 || color[2] != 0))
                        {
                            // Not transparent
                            for c in 0..maxcolors {
                                if color[0] == colors[c].0
                                    && color[1] == colors[c].1
                                    && color[2] == colors[c].2
                                {
                                    // Ok. this is a pixel of color c
                                    cx = Some((c + 1) as u8);
                                    // 320C mode contraint check
                                    if mode == "320C" {
                                        // Check next pixel, should be background or same color
                                        if x & 1 == 0 {
                                            let colorr = img.get_pixel(
                                                bitmap.left + x * pixel_width + 1,
                                                bitmap.top + y,
                                            );
                                            if !(colorr[3] == 0
                                                || (colorr[0] == 0
                                                    && colorr[1] == 0
                                                    && colorr[2] == 0))
                                            {
                                                // This is not background
                                                if colorr != color {
                                                    return Err(anyhow!("Bitmap {}: Two consecutive pixels have a different color in 320C mode (x = {}, y = {}, color1 = {:?}, color2 = {:?})", bitmap.name, x, y, color, colorr));
                                                }
                                            }
                                        }
                                    }
                                    break;
                                }
                            }
                        }
                        if cx.is_none() {
                            if color[3] == 0 || (color[0] == 0 && color[1] == 0 && color[2] == 0) {
                                cx = Some(0); // Background color (either black or transparent)
                            } else {
                                // Let's find a unaffected color
                                for c in 0..maxcolors {
                                    if colors[c].0 == 0 && colors[c].1 == 0 && colors[c].2 == 0 {
                                        colors[c].0 = color[0];
                                        colors[c].1 = color[1];
                                        colors[c].2 = color[2];
                                        cx = Some((c + 1) as u8);
                                        //println!("color {c} affected to {:?}", color);
                                        if mode == "320C" {
                                            // Check next pixel, should be background or same color
                                            if x & 1 == 0 {
                                                let colorr = img.get_pixel(
                                                    bitmap.left + x * pixel_width + 1,
                                                    bitmap.top + y,
                                                );
                                                if !(colorr[3] == 0
                                                    || (colorr[0] == 0
                                                        && colorr[1] == 0
                                                        && colorr[2] == 0))
                                                {
                                                    // This is not background
                                                    if colorr != color {
                                                        return Err(anyhow!("Bitmap {}: Two consecutive pixels have a different color in 320C mode (x = {}, y = {}, color1 = {:?}, color2 = {:?})", bitmap.name, x, y, color, colorr));
                                                    }
                                                }
                                            }
                                        }
                                        break;
                                    }
                                }
                                if cx.is_none() {
                                    if bitmap.background.is_some() {
                                        // If a background is specified
                                        cx = Some(0); // This unknown color is affected to background
                                    } else {
                                        println!(
                                            "Unexpected color {:?} found at {},{}",
                                            color,
                                            bitmap.left + x * pixel_width,
                                            bitmap.top + y
                                        );
                                        return Err(anyhow!(
                                            "Bitmap {} has more than {} colors",
                                            bitmap.name,
                                            maxcolors
                                        ));
                                    }
                                }
                            }
                        }
                        match mode {
                            "160A" | "320A" | "320D" => {
                                current_byte |= cx.unwrap();
                                current_bits += pixel_bits;
                                if current_bits == 8 {
                                    bytes.push(current_byte);
                                    current_byte = 0;
                                    current_bits = 0;
                                } else {
                                    current_byte <<= pixel_bits;
                                };
                            }
                            "160B" => {
                                let c = match cx.unwrap() {
                                    0 => 0,
                                    1 => 1,
                                    2 => 2,
                                    3 => 3,
                                    4 => 5,
                                    5 => 6,
                                    6 => 7,
                                    7 => 9,
                                    8 => 10,
                                    9 => 11,
                                    10 => 13,
                                    11 => 14,
                                    12 => 15,
                                    _ => 0,
                                };
                                current_byte |= (if c & 1 != 0 { 16 } else { 0 })
                                    | (if c & 2 != 0 { 32 } else { 0 })
                                    | (if c & 4 != 0 { 1 } else { 0 })
                                    | (if c & 8 != 0 { 2 } else { 0 });
                                current_bits += 1;
                                if current_bits == 2 {
                                    bytes.push(current_byte);
                                    current_byte = 0;
                                    current_bits = 0;
                                } else {
                                    current_byte <<= 2;
                                };
                            }
                            "320B" => {
                                let c = cx.unwrap();
                                current_byte |= (if c & 1 != 0 { 1 } else { 0 })
                                    | (if c & 2 != 0 { 16 } else { 0 });
                                current_bits += 1;
                                if current_bits == 4 {
                                    bytes.push(current_byte);
                                    current_byte = 0;
                                    current_bits = 0;
                                } else {
                                    current_byte <<= 1;
                                };
                            }
                            "320C" => {
                                let c = cx.unwrap();
                                //println!("Color: {}", c);
                                if c != 0 {
                                    current_byte |= 1 << (7 - current_bits);
                                    if current_bits < 2 {
                                        current_byte |= (c - 1) << 2;
                                    } else {
                                        current_byte |= c - 1;
                                    }
                                }
                                current_bits += 1;
                                if current_bits == 4 {
                                    bytes.push(current_byte);
                                    current_byte = 0;
                                    current_bits = 0;
                                }
                            }
                            _ => unreachable!(),
                        };
                    }
                }
                // Whoaw. We do have our pixels vector. Let's output it
                if let Some(b) = bitmap_sheet.bank {
                    print!("bank{} ", b);
                }
                let default_height = if let Some(h) = bitmap_sheet.holeydma {
                    h
                } else if let Some(h) = bitmap_sheet.default_height {
                    h
                } else if bitmap.height == 8 {
                    8
                } else {
                    16
                };
                if bitmap.holeydma && (default_height == 8 || default_height == 16) {
                    print!("holeydma ");
                }
                if default_height == 16 && bitmap.height == 8 {
                    // This is a special case: small bitmap for 16 holey DMA (a bullet for instance)
                    print!(
                        "reversed scattered(16,{}) char {}[{}] = {{\n\t",
                        bytes.len() / 8,
                        bitmap.name,
                        bytes.len() * 2
                    );
                    let mut c = 1;
                    for i in 0..bytes.len() {
                        print!("0x{:02x}", bytes[i]);
                        if c % 16 != 0 {
                            print!(", ");
                        } else {
                            print!(",\n\t");
                        }
                        c += 1;
                    }
                    for _ in 0..bytes.len() - 1 {
                        print!("0x00");
                        if c % 16 != 0 {
                            print!(", ");
                        } else {
                            print!(",\n\t");
                        }
                        c += 1;
                    }
                    println!("0x00\n}};");
                } else {
                    let nb_bitmaps = bitmap.height / default_height as u32;
                    if nb_bitmaps * default_height as u32 != bitmap.height {
                        return Err(anyhow!(
                            "Bitmap {}: height {} not proportional to default height {}",
                            bitmap.name,
                            bitmap.height,
                            default_height
                        ));
                    }
                    let mut c = 0;
                    let l = bytes.len() / nb_bitmaps as usize;
                    print!(
                        "reversed scattered({},{}) char {}[{}] = {{\n\t",
                        default_height,
                        l / default_height as usize,
                        bitmap.name,
                        l
                    );
                    for _ in 0..l - 1 {
                        print!("0x{:02x}", bytes[c]);
                        if (c + 1) % 16 != 0 {
                            print!(", ");
                        } else {
                            print!(",\n\t");
                        }
                        c += 1;
                    }
                    println!("0x{:02x}\n}};", bytes[c]);
                    c += 1;
                    for i in 1..nb_bitmaps {
                        if bitmap.holeydma && (default_height == 8 || default_height == 16) {
                            print!("holeydma ");
                        }
                        if let Some(b) = bitmap_sheet.bank {
                            print!("bank{} ", b);
                        }
                        print!(
                            "reversed scattered({},{}) char {}_{}[{}] = {{\n\t",
                            default_height,
                            l / default_height as usize,
                            bitmap.name,
                            i,
                            l
                        );
                        for _ in 0..l - 1 {
                            print!("0x{:02x}", bytes[c]);
                            if (c + 1) % 16 != 0 {
                                print!(", ");
                            } else {
                                print!(",\n\t");
                            }
                            c += 1;
                        }
                        println!("0x{:02x}\n}};", bytes[c]);
                        c += 1;
                    }
                }
            }
        }
    }

    Ok(())
}
