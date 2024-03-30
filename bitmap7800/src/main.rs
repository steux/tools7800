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

fn main() -> Result<()> {
    let args = Args::parse();
    let contents = fs::read_to_string(args.filename).expect("Unable to read input file");
    let all_bitmaps: AllBitmaps = serde_yaml::from_str(&contents)?;
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

                        // OK. Now we have our series of bytes. Let's output them
                        if let Some(b) = bitmap_sheet.bank {
                            print!("bank{} ", b);
                        }
                        print!(
                            "reversed scattered({},{}) char {}_{}_{}[{}] = {{\n\t",
                            bitmap_sheet.dl_height,
                            last - first,
                            bitmap.name,
                            yy,
                            range_counter,
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
                            dl.push_str(format!("{}_{}_0 & 0xff, 0x{:02x}, {}_{}_0 >> 8, (-{} & 0x1f) | ({} << 5), {}, ", bitmap.name, yy, mode_byte, bitmap.name, yy, last - first, palette, x).as_str());
                            nb_bytes += 5;
                        } else {
                            dl.push_str(
                                format!(
                                    "{}_{}_{} & 0xff, (-{} & 0x1f) | ({} << 5), {}_{}_{} >> 8, {}, ",
                                    bitmap.name,
                                    yy,
                                    last - first,
                                    palette,
                                    range_counter,
                                    bitmap.name,
                                    yy,
                                    range_counter,
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
        }
    }

    Ok(())
}
