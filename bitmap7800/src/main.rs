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
    palettes: Vec<Palette>,
    bitmap_sheets: Vec<BitmapSheet>,
}

#[derive(Debug, Deserialize)]
struct BitmapSheet {
    image: String,
    mode: String,
    dl_height: u8,
    bank: Option<u8>,
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
    0x00, 0x00, 0x00, 0x25, 0x25, 0x25, 0x34, 0x34, 0x34, 0x4F, 0x4F, 0x4F, 0x5B, 0x5B, 0x5B, 0x69,
    0x69, 0x69, 0x7B, 0x7B, 0x7B, 0x8A, 0x8A, 0x8A, 0xA7, 0xA7, 0xA7, 0xB9, 0xB9, 0xB9, 0xC5, 0xC5,
    0xC5, 0xD0, 0xD0, 0xD0, 0xD7, 0xD7, 0xD7, 0xE1, 0xE1, 0xE1, 0xF4, 0xF4, 0xF4, 0xFF, 0xFF, 0xFF,
    0x4C, 0x32, 0x00, 0x62, 0x3A, 0x00, 0x7B, 0x4A, 0x00, 0x9A, 0x60, 0x00, 0xB5, 0x74, 0x00, 0xCC,
    0x85, 0x00, 0xE7, 0x9E, 0x08, 0xF7, 0xAF, 0x10, 0xFF, 0xC3, 0x18, 0xFF, 0xD0, 0x20, 0xFF, 0xD8,
    0x28, 0xFF, 0xDF, 0x30, 0xFF, 0xE6, 0x3B, 0xFF, 0xF4, 0x40, 0xFF, 0xFA, 0x4B, 0xFF, 0xFF, 0x50,
    0x99, 0x25, 0x00, 0xAA, 0x25, 0x00, 0xB4, 0x25, 0x00, 0xD3, 0x30, 0x00, 0xDD, 0x48, 0x02, 0xE2,
    0x50, 0x09, 0xF4, 0x67, 0x00, 0xF4, 0x75, 0x10, 0xFF, 0x9E, 0x10, 0xFF, 0xAC, 0x20, 0xFF, 0xBA,
    0x3A, 0xFF, 0xBF, 0x50, 0xFF, 0xC6, 0x6D, 0xFF, 0xD5, 0x80, 0xFF, 0xE4, 0x90, 0xFF, 0xE6, 0x99,
    0x98, 0x0C, 0x0C, 0x99, 0x0C, 0x0C, 0xC2, 0x13, 0x00, 0xD3, 0x13, 0x00, 0xE2, 0x35, 0x00, 0xE3,
    0x40, 0x00, 0xE4, 0x40, 0x20, 0xE5, 0x52, 0x30, 0xFD, 0x78, 0x54, 0xFF, 0x8A, 0x6A, 0xFF, 0x98,
    0x7C, 0xFF, 0xA4, 0x8B, 0xFF, 0xB3, 0x9E, 0xFF, 0xC2, 0xB2, 0xFF, 0xD0, 0xBA, 0xFF, 0xD7, 0xC0,
    0x99, 0x00, 0x00, 0xA9, 0x00, 0x00, 0xC2, 0x04, 0x00, 0xD3, 0x04, 0x00, 0xDA, 0x04, 0x00, 0xDB,
    0x08, 0x00, 0xE4, 0x20, 0x20, 0xF6, 0x40, 0x40, 0xFB, 0x70, 0x70, 0xFB, 0x7E, 0x7E, 0xFB, 0x8F,
    0x8F, 0xFF, 0x9F, 0x9F, 0xFF, 0xAB, 0xAB, 0xFF, 0xB9, 0xB9, 0xFF, 0xC9, 0xC9, 0xFF, 0xCF, 0xCF,
    0x7E, 0x00, 0x50, 0x80, 0x00, 0x50, 0x80, 0x00, 0x5F, 0x95, 0x0B, 0x74, 0xAA, 0x22, 0x88, 0xBB,
    0x2F, 0x9A, 0xCE, 0x3F, 0xAD, 0xD7, 0x5A, 0xB6, 0xE4, 0x67, 0xC3, 0xEF, 0x72, 0xCE, 0xFB, 0x7E,
    0xDA, 0xFF, 0x8D, 0xE1, 0xFF, 0x9D, 0xE5, 0xFF, 0xA5, 0xE7, 0xFF, 0xAF, 0xEA, 0xFF, 0xB8, 0xEC,
    0x48, 0x00, 0x6C, 0x5C, 0x04, 0x88, 0x65, 0x0D, 0x90, 0x7B, 0x23, 0xA7, 0x93, 0x3B, 0xBF, 0x9D,
    0x45, 0xC9, 0xA7, 0x4F, 0xD3, 0xB2, 0x5A, 0xDE, 0xBD, 0x65, 0xE9, 0xC5, 0x6D, 0xF1, 0xCE, 0x76,
    0xFA, 0xD5, 0x83, 0xFF, 0xDA, 0x90, 0xFF, 0xDE, 0x9C, 0xFF, 0xE2, 0xA9, 0xFF, 0xE6, 0xB6, 0xFF,
    0x1B, 0x00, 0x70, 0x22, 0x1B, 0x8D, 0x37, 0x30, 0xA2, 0x48, 0x41, 0xB3, 0x59, 0x52, 0xC4, 0x63,
    0x5C, 0xCE, 0x6F, 0x68, 0xDA, 0x7D, 0x76, 0xE8, 0x87, 0x80, 0xF8, 0x93, 0x8C, 0xFF, 0x9D, 0x97,
    0xFF, 0xA8, 0xA3, 0xFF, 0xB3, 0xAF, 0xFF, 0xBC, 0xB8, 0xFF, 0xC4, 0xC1, 0xFF, 0xDA, 0xD1, 0xFF,
    0x00, 0x0D, 0x7F, 0x00, 0x12, 0xA7, 0x00, 0x18, 0xC0, 0x0A, 0x2B, 0xD1, 0x1B, 0x4A, 0xE3, 0x2F,
    0x58, 0xF0, 0x37, 0x68, 0xFF, 0x49, 0x79, 0xFF, 0x5B, 0x85, 0xFF, 0x6D, 0x96, 0xFF, 0x7F, 0xA3,
    0xFF, 0x8C, 0xAD, 0xFF, 0x96, 0xB4, 0xFF, 0xA8, 0xC0, 0xFF, 0xB7, 0xCB, 0xFF, 0xC6, 0xD6, 0xFF,
    0x00, 0x29, 0x5A, 0x00, 0x38, 0x76, 0x00, 0x48, 0x92, 0x00, 0x5C, 0xAC, 0x00, 0x71, 0xC6, 0x00,
    0x86, 0xD0, 0x0A, 0x9B, 0xDF, 0x1A, 0xA8, 0xEC, 0x2B, 0xB6, 0xFF, 0x3F, 0xC2, 0xFF, 0x45, 0xCB,
    0xFF, 0x59, 0xD3, 0xFF, 0x7F, 0xDA, 0xFF, 0x8F, 0xDE, 0xFF, 0xA0, 0xE2, 0xFF, 0xB0, 0xEB, 0xFF,
    0x00, 0x4A, 0x00, 0x00, 0x4C, 0x00, 0x00, 0x6A, 0x20, 0x50, 0x8E, 0x79, 0x40, 0x99, 0x99, 0x00,
    0x9C, 0xAA, 0x00, 0xA1, 0xBB, 0x01, 0xA4, 0xCC, 0x03, 0xA5, 0xD7, 0x05, 0xDA, 0xE2, 0x18, 0xE5,
    0xFF, 0x34, 0xEA, 0xFF, 0x49, 0xEF, 0xFF, 0x66, 0xF2, 0xFF, 0x84, 0xF4, 0xFF, 0x9E, 0xF9, 0xFF,
    0x00, 0x4A, 0x00, 0x00, 0x5D, 0x00, 0x00, 0x70, 0x00, 0x00, 0x83, 0x00, 0x00, 0x95, 0x00, 0x00,
    0xAB, 0x00, 0x07, 0xBD, 0x07, 0x0A, 0xD0, 0x0A, 0x1A, 0xD5, 0x40, 0x5A, 0xF1, 0x77, 0x82, 0xEF,
    0xA7, 0x84, 0xED, 0xD1, 0x89, 0xFF, 0xED, 0x7D, 0xFF, 0xFF, 0x93, 0xFF, 0xFF, 0x9B, 0xFF, 0xFF,
    0x22, 0x4A, 0x03, 0x27, 0x53, 0x04, 0x30, 0x64, 0x05, 0x3C, 0x77, 0x0C, 0x45, 0x8C, 0x11, 0x5A,
    0xA5, 0x13, 0x1B, 0xD2, 0x09, 0x1F, 0xDD, 0x00, 0x3D, 0xCD, 0x2D, 0x3D, 0xCD, 0x30, 0x58, 0xCC,
    0x40, 0x60, 0xD3, 0x50, 0xA2, 0xEC, 0x55, 0xB3, 0xF2, 0x4A, 0xBB, 0xF6, 0x5D, 0xC4, 0xF8, 0x70,
    0x2E, 0x3F, 0x0C, 0x36, 0x4A, 0x0F, 0x40, 0x56, 0x15, 0x46, 0x5F, 0x17, 0x57, 0x77, 0x1A, 0x65,
    0x85, 0x1C, 0x74, 0x93, 0x1D, 0x8F, 0xA5, 0x25, 0xAD, 0xB7, 0x2C, 0xBC, 0xC7, 0x30, 0xC9, 0xD5,
    0x33, 0xD4, 0xE0, 0x3B, 0xE0, 0xEC, 0x42, 0xEA, 0xF6, 0x45, 0xF0, 0xFD, 0x47, 0xF4, 0xFF, 0x6F,
    0x55, 0x24, 0x00, 0x5A, 0x2C, 0x00, 0x6C, 0x3B, 0x00, 0x79, 0x4B, 0x00, 0xB9, 0x75, 0x00, 0xBB,
    0x85, 0x00, 0xC1, 0xA1, 0x20, 0xD0, 0xB0, 0x2F, 0xDE, 0xBE, 0x3F, 0xE6, 0xC6, 0x45, 0xED, 0xCD,
    0x57, 0xF5, 0xDB, 0x62, 0xFB, 0xE5, 0x69, 0xFC, 0xEE, 0x6F, 0xFD, 0xF3, 0x77, 0xFD, 0xF3, 0x7F,
    0x5C, 0x27, 0x00, 0x5C, 0x2F, 0x00, 0x71, 0x3B, 0x00, 0x7B, 0x48, 0x00, 0xB9, 0x68, 0x20, 0xBB,
    0x72, 0x20, 0xC5, 0x86, 0x29, 0xD7, 0x96, 0x33, 0xE6, 0xA4, 0x40, 0xF4, 0xB1, 0x4B, 0xFD, 0xC1,
    0x58, 0xFF, 0xCC, 0x55, 0xFF, 0xD4, 0x61, 0xFF, 0xDD, 0x69, 0xFF, 0xE6, 0x79, 0xFF, 0xEA, 0x98,
];

fn main() -> Result<()> {
    let args = Args::parse();
    let contents = fs::read_to_string(args.filename).expect("Unable to read input file");
    let all_bitmaps: AllBitmaps = serde_yaml::from_str(&contents)?;

    let mut store = Vec::<(String, Vec<Vec<u8>>)>::new();

    for bitmap_sheet in all_bitmaps.bitmap_sheets {
        let img = image::open(&bitmap_sheet.image)
            .expect(&format!("Can't open image {}", bitmap_sheet.image));

        // Generate bitmaps data
        for bitmap in &bitmap_sheet.bitmaps {
            let pixel_width = match bitmap_sheet.mode.as_str() {
                "320A" | "320B" | "320C" | "320D" => 1,
                _ => 2,
            };
            let pixel_bits = match bitmap_sheet.mode.as_str() {
                "320A" | "320D" => 1,
                "160B" => 4,
                _ => 2,
            };

            let mut colors = [(0u8, 0u8, 0u8); 24];
            let mut maxcolors = 0;
            for p in &all_bitmaps.palettes {
                for c in &p.colors {
                    colors[maxcolors] = *c;
                    maxcolors += 1;
                }
            }
            let background = all_bitmaps.background.unwrap_or((0, 0, 0));

            for yy in 0..bitmap.height / bitmap_sheet.dl_height as u32 {
                let mut fullbytes = Vec::<Vec<u8>>::new();
                let byte_width = match bitmap_sheet.mode.as_str() {
                    "160A" | "320A" | "320D" => 8,
                    _ => 4,
                };
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
                            for c in 0..maxcolors {
                                if color[0] == colors[c].0
                                    && color[1] == colors[c].1
                                    && color[2] == colors[c].2
                                {
                                    match bitmap_sheet.mode.as_str() {
                                        "320A" => {
                                            cx = 1;
                                            if let Some(p) = palette {
                                                if c as u8 != p {
                                                    return Err(anyhow!("Bitmap {}: Two pixels use a different palette in the same byte (x = {}, y = {}, color1 = {:?}, color2 = {:?})", bitmap.name, xp, yp, c, p - 1));
                                                }
                                            } else {
                                                palette = Some(c as u8);
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
                                                    return Err(anyhow!("Bitmap {}: Two consecutive pixels have a different color in 320C bitmap_sheet.mode (x = {}, y = {}, color1 = {:?}, color2 = {:?})", bitmap.name, x, y, color, colorr));
                                                }
                                            }
                                        }
                                    }
                                    break;
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
                if let Some(b) = bitmap_sheet.bank {
                    print!("bank{} ", b);
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
            if let Some(b) = bitmap_sheet.bank {
                print!("bank{b} ");
            }
            print!("const char {bitmapname}_data_ptrs_high[{}] = {{", nb_dls);
            for y in 0..nb_dls - 1 {
                print!("{bitmapname}_{y}_dl >> 8, ");
            }
            println!("{bitmapname}_{}_dl >> 8}};", nb_dls - 1);
            if let Some(b) = bitmap_sheet.bank {
                print!("bank{b} ");
            }
            print!("const char {bitmapname}_data_ptrs_low[{}] = {{", nb_dls);
            for y in 0..nb_dls - 1 {
                print!("{bitmapname}_{y}_dl & 0xff, ");
            }
            println!("{bitmapname}_{}_dl & 0xff}};", nb_dls - 1);
            if let Some(b) = bitmap_sheet.bank {
                print!("bank{b} ");
            }
            println!("const char *{bitmapname}_data_ptrs[2] = {{{bitmapname}_data_ptrs_high, {bitmapname}_data_ptrs_low}};");
        }
    }

    Ok(())
}
