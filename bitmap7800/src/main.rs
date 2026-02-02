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
    background: Option<(u8, u8, u8)>,
    palettes: Option<Vec<Palette>>,
    bitmap_sheets: Vec<BitmapSheet>,
}

#[derive(Debug, Deserialize)]
struct BitmapSheet {
    image: String,
    mode: String,
    dl_height: u8,
    bank: Option<u8>,
    noholeydma: Option<bool>,
    bitmaps: Vec<Bitmap>,
}

#[derive(Debug, Deserialize)]
struct Palette {
    colors: Vec<(u8, u8, u8)>,
}

#[derive(Debug, Deserialize)]
struct Bitmap {
    name: String,
    top: u32,
    left: u32,
    width: u32,
    height: u32,
    xoffset: Option<u32>,
}

// Color tables:
//
// | bitmap_sheet.mode | colors |
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

// Atari 7800 Palette
static PALETTE: [u8; 768] = [
    0x00, 0x00, 0x00, 0x11, 0x11, 0x11, 0x22, 0x22, 0x22, 0x33, 0x33, 0x33, 0x44, 0x44, 0x44, 0x55,
    0x55, 0x55, 0x66, 0x66, 0x66, 0x77, 0x77, 0x77, 0x88, 0x88, 0x88, 0x99, 0x99, 0x99, 0xaa, 0xaa,
    0xaa, 0xbb, 0xbb, 0xbb, 0xcc, 0xcc, 0xcc, 0xdd, 0xdd, 0xdd, 0xee, 0xee, 0xee, 0xff, 0xff, 0xff,
    0x16, 0x0a, 0x00, 0x27, 0x1b, 0x00, 0x38, 0x2c, 0x00, 0x49, 0x3d, 0x00, 0x5a, 0x4e, 0x00, 0x6b,
    0x5f, 0x00, 0x7c, 0x70, 0x00, 0x8d, 0x81, 0x05, 0x9e, 0x92, 0x16, 0xaf, 0xa3, 0x27, 0xc0, 0xb4,
    0x38, 0xd1, 0xc5, 0x49, 0xe2, 0xd6, 0x5a, 0xf3, 0xe7, 0x6b, 0xff, 0xf8, 0x7c, 0xff, 0xff, 0x8d,
    0x2f, 0x00, 0x00, 0x40, 0x08, 0x00, 0x51, 0x19, 0x00, 0x62, 0x2a, 0x00, 0x73, 0x3b, 0x00, 0x84,
    0x4c, 0x00, 0x95, 0x5d, 0x00, 0xa6, 0x6e, 0x11, 0xb7, 0x7f, 0x22, 0xc8, 0x90, 0x33, 0xd9, 0xa1,
    0x44, 0xea, 0xb2, 0x55, 0xfb, 0xc3, 0x66, 0xff, 0xd4, 0x77, 0xff, 0xe5, 0x88, 0xff, 0xf6, 0x99,
    0x3d, 0x00, 0x00, 0x4e, 0x00, 0x00, 0x5f, 0x09, 0x00, 0x70, 0x1a, 0x00, 0x81, 0x2b, 0x00, 0x92,
    0x3c, 0x11, 0xa3, 0x4d, 0x22, 0xb4, 0x5e, 0x33, 0xc5, 0x6f, 0x44, 0xd6, 0x80, 0x55, 0xe7, 0x91,
    0x66, 0xf8, 0xa2, 0x77, 0xff, 0xb3, 0x88, 0xff, 0xc4, 0x99, 0xff, 0xd5, 0xaa, 0xff, 0xe6, 0xbb,
    0x3f, 0x00, 0x00, 0x50, 0x00, 0x00, 0x61, 0x00, 0x0f, 0x72, 0x0f, 0x20, 0x83, 0x20, 0x31, 0x94,
    0x31, 0x42, 0xa5, 0x42, 0x53, 0xb6, 0x53, 0x64, 0xc7, 0x64, 0x75, 0xd8, 0x75, 0x86, 0xe9, 0x86,
    0x97, 0xfa, 0x97, 0xa8, 0xff, 0xa8, 0xb9, 0xff, 0xb9, 0xca, 0xff, 0xca, 0xdb, 0xff, 0xdb, 0xec,
    0x33, 0x00, 0x21, 0x44, 0x00, 0x32, 0x55, 0x00, 0x43, 0x66, 0x0c, 0x54, 0x77, 0x1d, 0x65, 0x88,
    0x2e, 0x76, 0x99, 0x3f, 0x87, 0xaa, 0x50, 0x98, 0xbb, 0x61, 0xa9, 0xcc, 0x72, 0xba, 0xdd, 0x83,
    0xcb, 0xee, 0x94, 0xdc, 0xff, 0xa5, 0xed, 0xff, 0xb6, 0xfe, 0xff, 0xc7, 0xff, 0xff, 0xd8, 0xff,
    0x1c, 0x00, 0x4f, 0x2d, 0x00, 0x60, 0x3e, 0x00, 0x71, 0x4f, 0x11, 0x82, 0x60, 0x22, 0x93, 0x71,
    0x33, 0xa4, 0x82, 0x44, 0xb5, 0x93, 0x55, 0xc6, 0xa4, 0x66, 0xd7, 0xb5, 0x77, 0xe8, 0xc6, 0x88,
    0xf9, 0xd7, 0x99, 0xff, 0xe8, 0xaa, 0xff, 0xf9, 0xbb, 0xff, 0xff, 0xcc, 0xff, 0xff, 0xdd, 0xff,
    0x00, 0x00, 0x6b, 0x11, 0x00, 0x7c, 0x22, 0x0c, 0x8d, 0x33, 0x1d, 0x9e, 0x44, 0x2e, 0xaf, 0x55,
    0x3f, 0xc0, 0x66, 0x50, 0xd1, 0x77, 0x61, 0xe2, 0x88, 0x72, 0xf3, 0x99, 0x83, 0xff, 0xaa, 0x94,
    0xff, 0xbb, 0xa5, 0xff, 0xcc, 0xb6, 0xff, 0xdd, 0xc7, 0xff, 0xee, 0xd8, 0xff, 0xff, 0xe9, 0xff,
    0x00, 0x00, 0x71, 0x00, 0x0c, 0x82, 0x05, 0x1d, 0x93, 0x16, 0x2e, 0xa4, 0x27, 0x3f, 0xb5, 0x38,
    0x50, 0xc6, 0x49, 0x61, 0xd7, 0x5a, 0x72, 0xe8, 0x6b, 0x83, 0xf9, 0x7c, 0x94, 0xff, 0x8d, 0xa5,
    0xff, 0x9e, 0xb6, 0xff, 0xaf, 0xc7, 0xff, 0xc0, 0xd8, 0xff, 0xd1, 0xe9, 0xff, 0xe2, 0xfa, 0xff,
    0x00, 0x0d, 0x5f, 0x00, 0x1e, 0x70, 0x00, 0x2f, 0x81, 0x00, 0x40, 0x92, 0x10, 0x51, 0xa3, 0x21,
    0x62, 0xb4, 0x32, 0x73, 0xc5, 0x43, 0x84, 0xd6, 0x54, 0x95, 0xe7, 0x65, 0xa6, 0xf8, 0x76, 0xb7,
    0xff, 0x87, 0xc8, 0xff, 0x98, 0xd9, 0xff, 0xa9, 0xea, 0xff, 0xba, 0xfb, 0xff, 0xcb, 0xff, 0xff,
    0x00, 0x1d, 0x38, 0x00, 0x2e, 0x49, 0x00, 0x3f, 0x5a, 0x00, 0x50, 0x6b, 0x05, 0x61, 0x7c, 0x16,
    0x72, 0x8d, 0x27, 0x83, 0x9e, 0x38, 0x94, 0xaf, 0x49, 0xa5, 0xc0, 0x5a, 0xb6, 0xd1, 0x6b, 0xc7,
    0xe2, 0x7c, 0xd8, 0xf3, 0x8d, 0xe9, 0xff, 0x9e, 0xfa, 0xff, 0xaf, 0xff, 0xff, 0xc0, 0xff, 0xff,
    0x00, 0x26, 0x05, 0x00, 0x37, 0x16, 0x00, 0x48, 0x27, 0x00, 0x59, 0x38, 0x07, 0x6a, 0x49, 0x18,
    0x7b, 0x5a, 0x29, 0x8c, 0x6b, 0x3a, 0x9d, 0x7c, 0x4b, 0xae, 0x8d, 0x5c, 0xbf, 0x9e, 0x6d, 0xd0,
    0xaf, 0x7e, 0xe1, 0xc0, 0x8f, 0xf2, 0xd1, 0xa0, 0xff, 0xe2, 0xb1, 0xff, 0xf3, 0xc2, 0xff, 0xff,
    0x00, 0x27, 0x00, 0x00, 0x38, 0x00, 0x00, 0x49, 0x00, 0x05, 0x5a, 0x05, 0x16, 0x6b, 0x16, 0x27,
    0x7c, 0x27, 0x38, 0x8d, 0x38, 0x49, 0x9e, 0x49, 0x5a, 0xaf, 0x5a, 0x6b, 0xc0, 0x6b, 0x7c, 0xd1,
    0x7c, 0x8d, 0xe2, 0x8d, 0x9e, 0xf3, 0x9e, 0xaf, 0xff, 0xaf, 0xc0, 0xff, 0xc0, 0xd1, 0xff, 0xd1,
    0x00, 0x20, 0x00, 0x00, 0x31, 0x00, 0x0d, 0x42, 0x00, 0x1e, 0x53, 0x00, 0x2f, 0x64, 0x00, 0x40,
    0x75, 0x00, 0x51, 0x86, 0x0e, 0x62, 0x97, 0x1f, 0x73, 0xa8, 0x30, 0x84, 0xb9, 0x41, 0x95, 0xca,
    0x52, 0xa6, 0xdb, 0x63, 0xb7, 0xec, 0x74, 0xc8, 0xfd, 0x85, 0xd9, 0xff, 0x96, 0xea, 0xff, 0xa7,
    0x08, 0x12, 0x00, 0x19, 0x23, 0x00, 0x2a, 0x34, 0x00, 0x3b, 0x45, 0x00, 0x4c, 0x56, 0x00, 0x5d,
    0x67, 0x00, 0x6e, 0x78, 0x00, 0x7f, 0x89, 0x08, 0x90, 0x9a, 0x19, 0xa1, 0xab, 0x2a, 0xb2, 0xbc,
    0x3b, 0xc3, 0xcd, 0x4c, 0xd4, 0xde, 0x5d, 0xe5, 0xef, 0x6e, 0xf6, 0xff, 0x7f, 0xff, 0xff, 0x90,
    0x24, 0x00, 0x00, 0x35, 0x11, 0x00, 0x46, 0x22, 0x00, 0x57, 0x33, 0x00, 0x68, 0x44, 0x00, 0x79,
    0x55, 0x00, 0x8a, 0x66, 0x00, 0x9b, 0x77, 0x09, 0xac, 0x88, 0x1a, 0xbd, 0x99, 0x2b, 0xce, 0xaa,
    0x3c, 0xdf, 0xbb, 0x4d, 0xf0, 0xcc, 0x5e, 0xff, 0xdd, 0x6f, 0xff, 0xee, 0x80, 0xff, 0xff, 0x91,
];

fn find_color_in_palette(c: &(u8, u8, u8)) -> u8 {
    let mut maxdist = 256 * 256 * 256;
    let mut bestcolor = 0;
    for color in 0..255 {
        let dist = (PALETTE[color * 3] as i32 - c.0 as i32).abs()
            + (PALETTE[color * 3 + 1] as i32 - c.1 as i32).abs()
            + (PALETTE[color * 3 + 2] as i32 - c.2 as i32).abs();
        if dist < maxdist {
            maxdist = dist;
            bestcolor = color as u8;
        }
    }
    bestcolor
}

fn main() -> Result<()> {
    let args = Args::parse();
    let contents = fs::read_to_string(args.filename).expect("Unable to read input file");
    let all_bitmaps: AllBitmaps = serde_yaml::from_str(&contents)?;

    let mut store = Vec::<(String, Vec<Vec<u8>>)>::new();

    for bitmap_sheet in all_bitmaps.bitmap_sheets {
        let byte_width = match bitmap_sheet.mode.as_str() {
            "160A" | "320A" | "320D" => 8,
            _ => 4,
        };
        let maxmaxcolors = match bitmap_sheet.mode.as_str() {
            "160A" | "160B" => 24,
            "320B" => 6,
            "320A" | "320C" => 8,
            _ => unimplemented!(),
        };

        let pixel_width = match bitmap_sheet.mode.as_str() {
            "320A" | "320B" | "320C" | "320D" => 1,
            _ => 2,
        };
        let pixel_bits = match bitmap_sheet.mode.as_str() {
            "320A" | "320D" => 1,
            "160B" => 4,
            _ => 2,
        };

        let img = image::open(&bitmap_sheet.image)
            .expect(&format!("Can't open image {}", bitmap_sheet.image));

        if let Some(b) = bitmap_sheet.bank {
            println!("#ifndef BITMAP_TABLE_BANK\n#define BITMAP_TABLE_BANK bank{b}\n#endif");
        }

        // Generate bitmaps data
        for bitmap in &bitmap_sheet.bitmaps {
            let mut colors = [(0u8, 0u8, 0u8); 24];
            let mut maxcolors = 0;
            if let Some(palettes) = &all_bitmaps.palettes {
                for p in palettes {
                    for c in &p.colors {
                        colors[maxcolors] = *c;
                        maxcolors += 1;
                    }
                }
            }
            let background = all_bitmaps.background.unwrap_or((0, 0, 0));

            for yy in 0..bitmap.height / bitmap_sheet.dl_height as u32 {
                let mut fullbytes = Vec::<Vec<u8>>::new();
                let mut palettes = vec![0u8; (bitmap.width / byte_width) as usize];
                for y in 0..bitmap_sheet.dl_height as u32 {
                    let mut bytes = Vec::<u8>::new();
                    let mut current_byte: u8 = 0;
                    let mut current_bits: u8 = 0;
                    let mut palette: Option<u8> = None;
                    for x in 0..bitmap.width / pixel_width {
                        let xp = bitmap.left + x * pixel_width;
                        let yp = bitmap.top + yy * bitmap_sheet.dl_height as u32 + y;
                        let color = img.get_pixel(xp, yp);
                        let mut cx = 0u8;

                        if color[3] != 0
                            && (color[0] != background.0
                                || color[1] != background.1
                                || color[2] != background.2)
                        {
                            let mut found: Option<u8> = None;
                            for c in 0..maxcolors {
                                if color[0] == colors[c].0
                                    && color[1] == colors[c].1
                                    && color[2] == colors[c].2
                                {
                                    found = Some(c as u8);
                                    break;
                                }
                            }

                            let c = if let Some(c) = found {
                                c
                            } else {
                                // Add a new color to the color table
                                if maxcolors < maxmaxcolors {
                                    colors[maxcolors].0 = color[0];
                                    colors[maxcolors].1 = color[1];
                                    colors[maxcolors].2 = color[2];
                                    maxcolors += 1;
                                    println!("// Added new color {:?} to the palette at {x},{y}", color);
                                    (maxcolors - 1) as u8
                                } else {
                                    return Err(anyhow!(
                                        "Bitmap {}: Too many colors at {xp}, {yp}",
                                        bitmap.name
                                    ));
                                }
                            };

                            match bitmap_sheet.mode.as_str() {
                                "320A" => {
                                    cx = 1;
                                    if let Some(p) = palette {
                                        if c as u8 != p {
                                            return Err(anyhow!("Bitmap {}: Two pixels use a different palette in the same byte (x = {}, y = {}, color1 = {:?}, color2 = {:?})", bitmap.name, xp, yp, c, p));
                                        }
                                    } else {
                                        palette = Some(c as u8);
                                    }
                                }
                                "160B" => {
                                    cx = (c % 12) + 1; // 0 is background
                                    let px = c / 12;
                                    if let Some(p) = palette {
                                        if px != p {
                                            return Err(anyhow!("Bitmap {}: Two pixels use a different palette in the same byte (x = {}, y = {})", bitmap.name, xp, yp));
                                        }
                                    } else {
                                        palette = Some(px as u8);
                                    }
                                }
                                "320C" => {
                                    cx = (c % 4) + 1; // 0 is background
                                    let px = (c / 4) * 4;
                                    if let Some(p) = palette {
                                        if px != p {
                                            return Err(anyhow!("Bitmap {}: Two pixels use a different palette in the same byte (x = {}, y = {}, color = {:?}, palette = {:?})", bitmap.name, xp, yp, c, p));
                                        }
                                    } else {
                                        palette = Some(px as u8);
                                    }
                                }
                                _ => {
                                    return Err(anyhow!(
                                        "Unimplemented for gfx {} mode",
                                        bitmap_sheet.mode
                                    ))
                                }
                            }
                            // TODO: Identify used palette, and check that it is consistent
                            // with previous pixels, and pixels on the previous line

                            // 320C bitmap_sheet.mode contraint check
                            if bitmap_sheet.mode == "320C" {
                                // Check next pixel, should be background or same color
                                if x & 1 == 0 {
                                    let colorr = img.get_pixel(xp + 1, yp);
                                    if !(colorr[3] == 0
                                        || (colorr[0] == background.0
                                            && colorr[1] == background.1
                                            && colorr[2] == background.2))
                                    {
                                        // This is not background
                                        if colorr != color {
                                            println!("// Bitmap {}: Two consecutive pixels have a different color in 320C mode (x = {}, y = {}, color1 = {:?}, color2 = {:?})", bitmap.name, x, y, color, colorr);
                                            //return Err(anyhow!("Bitmap {}: Two consecutive pixels have a different color in 320C mode (x = {}, y = {}, color1 = {:?}, color2 = {:?})", bitmap.name, x, y, color, colorr));
                                        }
                                    }
                                }
                            }
                        }

                        match bitmap_sheet.mode.as_str() {
                            "160A" | "320A" | "320D" => {
                                current_byte |= cx;
                                current_bits += pixel_bits;
                                if current_bits == 8 {
                                    if let Some(p) = palette {
                                        palettes[((x * pixel_width) / byte_width) as usize] = p;
                                        palette = None;
                                    }
                                    bytes.push(current_byte);
                                    current_byte = 0;
                                    current_bits = 0;
                                } else {
                                    current_byte <<= pixel_bits;
                                };
                            }
                            "160B" => {
                                let c = match cx {
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
                                    if let Some(p) = palette {
                                        palettes[((x * pixel_width) / byte_width) as usize] = p;
                                        palette = None;
                                    }
                                    bytes.push(current_byte);
                                    current_byte = 0;
                                    current_bits = 0;
                                } else {
                                    current_byte <<= 2;
                                };
                            }
                            "320B" => {
                                let c = cx;
                                current_byte |= (if c & 1 != 0 { 1 } else { 0 })
                                    | (if c & 2 != 0 { 16 } else { 0 });
                                current_bits += 1;
                                if current_bits == 4 {
                                    if let Some(p) = palette {
                                        palettes[(x / 4) as usize] = p;
                                        palette = None;
                                    }
                                    bytes.push(current_byte);
                                    current_byte = 0;
                                    current_bits = 0;
                                } else {
                                    current_byte <<= 1;
                                };
                            }
                            "320C" => {
                                let c = cx;
                                //println!("Color: {}", c);
                                if c != 0 {
                                    current_byte |= 1 << (7 - current_bits);
                                    if current_bits < 2 {
                                        current_byte &= 0xf3;
                                        current_byte |= (c - 1) << 2;
                                    } else {
                                        current_byte &= 0xfc;
                                        current_byte |= c - 1;
                                    }
                                }
                                current_bits += 1;
                                if current_bits == 4 {
                                    if let Some(p) = palette {
                                        palettes[(x / 4) as usize] = p;
                                        palette = None;
                                    }
                                    bytes.push(current_byte);
                                    current_byte = 0;
                                    current_bits = 0;
                                }
                            }
                            _ => unreachable!(),
                        };
                    }

                    fullbytes.push(bytes)
                }

                // Whoaw. We do have our pixels vector. Let's output it

                // Let's find ranges of bytes that are not all 0s on all lines (for memory
                // compression)
                let mut first = 0;
                let end = fullbytes[0].len();
                let mut range_counter = 0;
                let mut dl = String::new();
                let mut nb_bytes = 0;
                let mut palette;
                loop {
                    if first == end {
                        break;
                    }
                    let mut empty = true;
                    for v in &fullbytes {
                        if v[first] != 0 {
                            empty = false;
                            break;
                        }
                    }
                    if empty {
                        first += 1;
                        if first == end {
                            break;
                        }
                    } else {
                        // Ok, we have found a first char that is not empty
                        // Let's find an end (or a char that has different palette)
                        palette = palettes[first];
                        let mut last = first + 1;
                        if last != end {
                            loop {
                                let mut empty = true;
                                for v in &fullbytes {
                                    if v[last] != 0 {
                                        empty = false;
                                        break;
                                    }
                                }
                                if !empty {
                                    // Is it the same palette ?
                                    if palettes[last] != palette {
                                        break;
                                    }
                                    // Is it bigger than 31 bytes
                                    if range_counter != 0 {
                                        if last - first == 31 {
                                            break;
                                        }
                                    } else if last - first == 32 {
                                        break;
                                    }
                                    last += 1;
                                    if last == end {
                                        break;
                                    }
                                } else {
                                    break;
                                }
                            }
                        }

                        // OK. Now we have our series of bytes.
                        // Reconstruct this array of bytes to be ready to compare with store
                        let mut bytespart = Vec::<Vec<u8>>::new();
                        for i in &fullbytes {
                            bytespart.push(i[first..last].to_vec());
                        }

                        // Let's look for them in the store
                        let mut found = None;
                        let mut name = String::new();
                        for r in &store {
                            if r.1[0].len() >= bytespart[0].len() {
                                let f = r.1[0]
                                    .windows(bytespart[0].len())
                                    .position(|w| w == bytespart[0]);
                                if let Some(offset) = f {
                                    // Check each line
                                    let mut ok = true;
                                    for j in 1..bitmap_sheet.dl_height as usize {
                                        if r.1[j][offset..offset + bytespart[j].len()]
                                            != bytespart[j]
                                        {
                                            ok = false;
                                            break;
                                        }
                                    }
                                    if ok {
                                        found = Some(offset);
                                        name = r.0.clone();
                                        break;
                                    }
                                }
                            }
                        }
                        if let Some(offset) = found {
                            name = format!("{name} + {offset}");
                        } else {
                            // We haven't found it in the store, so Let's output them
                            name = format!("{}_{}_{}", bitmap.name, yy, range_counter);
                            if let Some(b) = bitmap_sheet.bank {
                                print!("bank{} ", b);
                            }
                            if let Some(no) = bitmap_sheet.noholeydma {
                                if no {
                                    print!("noholeydma ");
                                }
                            }
                            print!(
                                "reversed scattered({},{}) char {}[{}] = {{\n\t",
                                bitmap_sheet.dl_height,
                                last - first,
                                name,
                                (last - first) * bitmap_sheet.dl_height as usize
                            );
                            let mut c = 0;
                            for bytes in &fullbytes {
                                for i in first..last {
                                    print!("0x{:02x}", bytes[i]);
                                    if c == (last - first) * bitmap_sheet.dl_height as usize - 1 {
                                        println!("}};");
                                    } else if (c + 1) % 16 != 0 {
                                        print!(", ");
                                    } else {
                                        print!(",\n\t");
                                    }
                                    c += 1;
                                }
                            }
                            // Put them in store
                            store.push((name.clone(), bytespart));
                        }

                        let byte_width = match bitmap_sheet.mode.as_str() {
                            "160A" | "320A" | "320D" => 4,
                            _ => 2,
                        };
                        let x = bitmap.xoffset.unwrap_or(0) + first as u32 * byte_width;
                        if range_counter == 0 {
                            let mode_byte = match bitmap_sheet.mode.as_str() {
                                "320A" | "160A" => 0x40,
                                _ => 0xc0,
                            };
                            dl.push_str(
                                format!(
                                    "{} & 0xff, 0x{:02x}, {} >> 8, (-{} & 0x1f) | ({} << 5), {}, ",
                                    name,
                                    mode_byte,
                                    name,
                                    last - first,
                                    palette,
                                    x
                                )
                                .as_str(),
                            );
                            nb_bytes += 5;
                        } else {
                            dl.push_str(
                                format!(
                                    "{} & 0xff, (-{} & 0x1f) | ({} << 5), {} >> 8, {}, ",
                                    name,
                                    last - first,
                                    palette,
                                    name,
                                    x
                                )
                                .as_str(),
                            );
                            nb_bytes += 4;
                        }

                        range_counter += 1;
                        first = last;
                    }
                }
                if bitmap_sheet.bank.is_some() {
                    print!("BITMAP_TABLE_BANK ");
                }
                println!(
                    "const unsigned char {}_{}_dl[{}] = {{{}0, 0}};",
                    bitmap.name,
                    yy,
                    nb_bytes + 2,
                    dl
                );
            }
            println!();
            let nb_dls = bitmap.height / bitmap_sheet.dl_height as u32;
            let bitmapname = &bitmap.name;
            if bitmap_sheet.bank.is_some() {
                print!("BITMAP_TABLE_BANK ");
            }
            print!("const char {bitmapname}_data_ptrs_high[{}] = {{", nb_dls);
            for y in 0..nb_dls - 1 {
                print!("{bitmapname}_{y}_dl >> 8, ");
            }
            println!("{bitmapname}_{}_dl >> 8}};", nb_dls - 1);
            if bitmap_sheet.bank.is_some() {
                print!("BITMAP_TABLE_BANK ");
            }
            print!("const char {bitmapname}_data_ptrs_low[{}] = {{", nb_dls);
            for y in 0..nb_dls - 1 {
                print!("{bitmapname}_{y}_dl & 0xff, ");
            }
            println!("{bitmapname}_{}_dl & 0xff}};", nb_dls - 1);
            if bitmap_sheet.bank.is_some() {
                print!("BITMAP_TABLE_BANK ");
            }
            println!("const char *{bitmapname}_data_ptrs[2] = {{{bitmapname}_data_ptrs_high, {bitmapname}_data_ptrs_low}};\n");

            // Output palettes
            println!("inline void {bitmapname}_set_palette() {{");
            let color = find_color_in_palette(&background);
            println!("\t*BACKGRND = multisprite_color(0x{:02x});", color);
            for i in 0..maxcolors {
                let color = find_color_in_palette(&colors[i]);
                let palette;
                let index_in_palette;
                match bitmap_sheet.mode.as_str() {
                    "320A" | "320C" => {
                        palette = i;
                        index_in_palette = 2;
                    }
                    "160B" => {
                        palette = i / 3;
                        index_in_palette = 1 + i % 3;
                    }
                    _ => unimplemented!(),
                }
                println!(
                    "\t*P{palette}C{index_in_palette} = multisprite_color(0x{:02x});",
                    color
                );
            }
            println!("}}");
        }
    }

    Ok(())
}
