use anyhow::{anyhow, Result};
use clap::Parser;
use image::{GenericImageView, Rgba};
use serde::Deserialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::str::FromStr;
use xml_dom::level2::{Node, NodeType};

//
// DONE: For lonely and consecutive tiles, automatically switch to immediate mode
// DONE: Pregenerate immediate mode sequences (max 15 tiles long -> 30 bytes)
// TODO: immediate option in Sprite, to force immediate mode generation (to go beyond 128 tiles limit)
//
/// Atari 7800 tool that generates C code for tiles map generated using tiled editor (tmx files)
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Generate 0xff boundaries
    #[arg(short, long, default_value = "false")]
    boundaries: bool,
    /// Tiled input file (.TMX file)
    filename: String,
    /// Sparse tiling code generation (provide yaml file)
    #[arg(long = "sparse")]
    yaml: Option<String>,
    /// Generated array name (default: tilemap)
    #[arg(short, long)]
    varname: Option<String>,
    /// Tileset maximum size
    #[arg(short, long)]
    maxsize: Option<usize>,
    /// Generate immediate mode sparse tiling
    #[arg(short, long, default_value = "false")]
    immediate: bool,
    /// Force left to right tileset order generation (for horizontal scrolling games)
    #[arg(short = 'o', long, default_value = "false")]
    force_left_to_right_order: bool,
    /// Forbid immediate mode usage when generating tilesets
    #[arg(short = 'f', long, default_value = "false")]
    forbid_immediate: bool,
    /// Generate direct tilesets (4 or 5 bytes direct use instead of 7 bytes) 
    #[arg(short = 'd', long, default_value = "false")]
    direct: bool,
    /// Generate 4 bytes headers in direct mode 
    #[arg(short = '4', long, default_value = "true")]
    four_bytes: bool,
    /// Adds an offset to directly generated tilesets 
    #[arg(short = 'o', long, default_value = "0")]
    offset: u8,
}

#[derive(Deserialize)]
struct AllSprites {
    #[serde(default)]
    palettes: Option<Vec<Palette>>,
    sprite_sheets: Vec<SpriteSheet>,
}

#[derive(Deserialize)]
struct SpriteSheet {
    image: String,
    #[serde(default = "default_mode")]
    mode: String,
    bank: Option<u8>,
    #[serde(default)]
    mirror: Option<Mirror>,
    sequences: Option<Vec<Sequence>>,
    sprites: Vec<Sprite>,
}

#[derive(Deserialize)]
struct Palette {
    name: String,
    colors: Vec<(u8, u8, u8)>,
}

#[derive(Deserialize)]
struct Sequence {
    sequence: Vec<String>,
    repeat: Option<usize>,
    holeydma: Option<bool>,
    bank: Option<u8>,
    generate: Option<bool>,
    name: Option<String>,
    prefix: Option<String>,
    postfix: Option<String>,
    ignore: Option<Vec<String>>,
}

#[derive(Deserialize)]
enum Mirror {
    Vertical,
    Horizontal,
    Both,
}

#[allow(unused)]
#[derive(Deserialize)]
struct Sprite {
    name: String,
    top: u32,
    left: u32,
    width: u32,
    #[serde(default = "default_holeydma")]
    holeydma: bool,
    #[serde(default = "default_sprite_size")]
    height: u32,
    #[serde(default)]
    palette: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    palette_number: Option<u8>,
    #[serde(default)]
    alias: Option<String>,
    #[serde(default)]
    mirror: Option<Mirror>,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    fake: Option<bool>,
}

#[derive(Debug, Clone)]
struct Tile<'a> {
    index: u32,
    mode: &'a str,
    palette_number: u8,
    background: Option<u32>,
    gfx: Vec<u8>,
    fake: bool,
}

fn default_sprite_size() -> u32 {
    16
}
fn default_holeydma() -> bool {
    true
}
fn default_mode() -> String {
    "160A".to_string()
}

fn sprite_gfx(
    img: &dyn GenericImageView<Pixel = Rgba<u8>>,
    all_sprites: &AllSprites,
    sprite_sheet: &SpriteSheet,
    sprite: &Sprite,
) -> Result<Vec<u8>> {
    let mode = if let Some(s) = &sprite.mode {
        s.as_str()
    } else {
        sprite_sheet.mode.as_str()
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
        if let Some(palettes) = &all_sprites.palettes {
            if let Some(pname) = &sprite.palette {
                let px = palettes.iter().find(|x| &x.name == pname);
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
    for y in 0..sprite.height {
        let mut current_byte: u8 = 0;
        let mut current_bits: u8 = 0;
        for x in 0..sprite.width / pixel_width {
            let xp = sprite.left + x * pixel_width;
            let yp = sprite.top + y;
            let color = img.get_pixel(xp, yp);
            let mut cx: Option<u8> = None;
            // In case of defined palette, priority is to find the color in the palette, so that black is not considered as a background color
            if (color[3] != 0 && sprite.palette.is_some())
                || (sprite.palette.is_none() && (color[0] != 0 || color[1] != 0 || color[2] != 0))
            {
                // Not transparent
                for c in 0..maxcolors {
                    if color[0] == colors[c].0 && color[1] == colors[c].1 && color[2] == colors[c].2
                    {
                        // Ok. this is a pixel of color c
                        cx = Some((c + 1) as u8);
                        // 320C mode contraint check
                        if mode == "320C" {
                            // Check next pixel, should be background or same color
                            if x & 1 == 0 {
                                let colorr = img
                                    .get_pixel(sprite.left + x * pixel_width + 1, sprite.top + y);
                                if !(colorr[3] == 0
                                    || (colorr[0] == 0 && colorr[1] == 0 && colorr[2] == 0))
                                {
                                    // This is not background
                                    if colorr != color {
                                        return Err(anyhow!("Sprite {}: Two consecutive pixels have a different color in 320C mode (x = {}, y = {}, color1 = {:?}, color2 = {:?})", sprite.name, x, y, color, colorr));
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
                                        sprite.left + x * pixel_width + 1,
                                        sprite.top + y,
                                    );
                                    if !(colorr[3] == 0
                                        || (colorr[0] == 0 && colorr[1] == 0 && colorr[2] == 0))
                                    {
                                        // This is not background
                                        if colorr != color {
                                            return Err(anyhow!("Sprite {}: Two consecutive pixels have a different color in 320C mode (x = {}, y = {}, color1 = {:?}, color2 = {:?})", sprite.name, x, y, color, colorr));
                                        }
                                    }
                                }
                            }
                            break;
                        }
                    }
                    if cx.is_none() {
                        if sprite.background.is_some() {
                            // If a background is specified
                            cx = Some(0); // This unknown color is affected to background
                        } else {
                            println!(
                                "Unexpected color {:?} found at {},{}",
                                color,
                                sprite.left + x * pixel_width,
                                sprite.top + y
                            );
                            return Err(anyhow!(
                                "Sprite {} has more than {} colors",
                                sprite.name,
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
                    current_byte |=
                        (if c & 1 != 0 { 1 } else { 0 }) | (if c & 2 != 0 { 16 } else { 0 });
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
    Ok(bytes)
}

fn main() -> Result<()> {
    let mut width = 0;
    let mut height = 0;
    let mut tilewidth: u32 = 8;
    let mut tileheight: u32 = 8;
    let args = Args::parse();
    let xml = fs::read_to_string(args.filename).expect("Unable to read input file");
    let varname = args.varname.unwrap_or("tilemap".into());

    let dom = xml_dom::parser::read_xml(&xml)?;
    let root = dom.first_child().unwrap();
    if root.local_name() == "map" {
        for a in &root.attributes() {
            if a.0.local_name() == "tileheight" {
                let h = a.1.first_child().unwrap().node_value();
                if let Some(s) = h {
                    tileheight = s.parse::<u32>()?;
                }
            }
            if a.0.local_name() == "tilewidth" {
                let h = a.1.first_child().unwrap().node_value();
                if let Some(s) = h {
                    tilewidth = s.parse::<u32>()?;
                }
            }
        }
    }
    let mut imagewidth = None;
    for n in &root.child_nodes() {
        if n.node_type() == NodeType::Element && n.local_name() == "tileset" {
            for nx in &n.child_nodes() {
                if nx.node_type() == NodeType::Element && nx.local_name() == "image" {
                    for a in &nx.attributes() {
                        if a.0.local_name() == "width" {
                            let h = a.1.first_child().unwrap().node_value();
                            if let Some(s) = h {
                                imagewidth = s.parse::<u32>().ok();
                            }
                        }
                    }
                }
            }
        } else if n.node_type() == NodeType::Element && n.local_name() == "layer" {
            for a in &n.attributes() {
                if a.0.local_name() == "width" {
                    let w = a.1.first_child().unwrap().node_value();
                    if let Some(s) = w {
                        width = s.parse::<usize>()?;
                        //println!("Tilemap width = {}", width);
                    }
                }
                if a.0.local_name() == "height" {
                    let h = a.1.first_child().unwrap().node_value();
                    if let Some(s) = h {
                        height = s.parse::<usize>()?;
                        //println!("Tilemap height = {}", height);
                    }
                }
            }
            for nx in &n.child_nodes() {
                if nx.node_type() == NodeType::Element && nx.local_name() == "data" {
                    let t = nx.first_child().unwrap();
                    if t.node_type() == NodeType::Text {
                        let csv = t.node_value().unwrap();
                        let csv: String = csv.split_whitespace().collect();
                        //println!("Tiles: {}", csv);
                        let array = csv
                            .split(',')
                            .map(|x| u32::from_str(x).unwrap())
                            .collect::<Vec<_>>();
                        if array.len() == width * height {
                            if let Some(yaml_file) = args.yaml {
                                let tileset_maxsize =
                                    args.maxsize
                                        .unwrap_or(if tilewidth == 8 && !args.immediate {
                                            31
                                        } else {
                                            15
                                        });
                                let contents = fs::read_to_string(yaml_file)
                                    .expect("Unable to read input file");
                                let t: AllSprites = serde_yaml::from_str(&contents)?;
                                // OK, we have the array, we have the tiles specs. Let's match them
                                // Let's scan all the tiles to make sure all this makes sense
                                if t.sprite_sheets.len() != 1 {
                                    eprintln!("Only the first sprite sheet (tiles) will be used");
                                }
                                let tiles_sheet = &t.sprite_sheets[0];
                                let forbid_immediate =
                                    args.forbid_immediate || tiles_sheet.mirror.is_some(); // Forbid imediate mode if there is any mirroring implied

                                let img = image::open(&tiles_sheet.image)
                                    .expect(&format!("Can't open image {}", tiles_sheet.image));
                                let image_width = if let Some(iw) = imagewidth {
                                    iw
                                } else {
                                    img.width()
                                };
                                let mut index = 0;
                                let defmode = tiles_sheet.mode.as_str();
                                let mut tiles = HashMap::<u32, Tile>::new();
                                let mut tile_names_ex = HashMap::<u32, String>::new();
                                let mut aliases = HashMap::<String, u32>::new();
                                let mut refs = HashMap::<String, u32>::new(); // Mapping from tile name in the Atari YAML file to tile number in tiled array
                                let bytes_per_tile: usize = if tilewidth == 8 { 1 } else { 2 };
                                for tile in &tiles_sheet.sprites {
                                    let gfx = sprite_gfx(&img, &t, tiles_sheet, tile)?;
                                    let mode = if let Some(m) = &tile.mode {
                                        m.as_str()
                                    } else {
                                        defmode
                                    };
                                    let tile_bytes = match mode {
                                        "160A" => tilewidth / 8,
                                        "160B" => tilewidth / 4,
                                        "320A" => tilewidth / 8,
                                        "320B" => tilewidth / 4,
                                        "320C" => tilewidth / 4,
                                        "320D" => tilewidth / 8,
                                        _ => unreachable!(),
                                    };
                                    if tile.alias.is_none() {
                                        aliases.insert(tile.name.clone(), index);
                                    }
                                    let y = tile.top / tileheight;
                                    let x = tile.left / tilewidth;
                                    let ix = 1 + x + y * image_width / tilewidth;
                                    let ixx = 1
                                        + x
                                        + (img.height() / tileheight - 1 - y) * image_width
                                            / tilewidth;
                                    // ixx is the tile number in tiled
                                    // (reversed). index + 1 is an odd tile number that can be used
                                    // by C code for vertical reflection
                                    refs.insert(tile.name.clone(), ix); // index is the tile number in
                                                                        // generated atari 7800 tiles (in the order of yaml file), ix is the tile number in tiled
                                    let nbtilesx = tile.width / tilewidth;
                                    let nbtilesy = tile.height / tileheight;
                                    let palette_number = tile.palette_number.unwrap_or_default();
                                    let background = if let Some(b) = &tile.background {
                                        refs.get(b).copied()
                                    } else {
                                        None
                                    };
                                    let mut idx = if let Some(alias) = &tile.alias {
                                        if let Some(i) = aliases.get(alias.as_str()) {
                                            if let Some(Mirror::Vertical) = tile.mirror {
                                                *i + 1 // Add 1 for vertical mirroring
                                            } else {
                                                *i
                                            }
                                        } else {
                                            return Err(anyhow!("Bad alias {}", alias));
                                        }
                                    } else {
                                        index
                                    };
                                    let mut offset = 0;
                                    for j in 0..nbtilesy {
                                        for i in 0..nbtilesx {
                                            let tgfx = {
                                                let w = bytes_per_tile
                                                    * match mode {
                                                        "160A" | "320A" | "320D" => 1,
                                                        _ => 2,
                                                    };
                                                let mut t = Vec::<u8>::new();
                                                for y in 0..tileheight {
                                                    for c in 0..w {
                                                        t.push(
                                                            gfx[((j * tileheight + y) as usize
                                                                * w
                                                                * nbtilesx as usize)
                                                                + i as usize * w
                                                                + c],
                                                        )
                                                    }
                                                }
                                                t
                                            };
                                            tiles.insert(
                                                ix + i + j * image_width / tilewidth,
                                                Tile {
                                                    index: idx,
                                                    mode,
                                                    palette_number,
                                                    background,
                                                    gfx: tgfx.clone(),
                                                    fake: tile.fake.unwrap_or(false),
                                                },
                                            );
                                            tile_names_ex.insert(
                                                index,
                                                format!("{} + {}", tile.name, offset),
                                            );
                                            if tile.alias.is_none() {
                                                aliases.insert(
                                                    format!("{} + {}", tile.name, offset),
                                                    index,
                                                );
                                                refs.insert(
                                                    format!("{} + {}", tile.name, offset),
                                                    ix + i + j * image_width / tilewidth,
                                                );
                                            }
                                            if let Some(Mirror::Vertical) = tiles_sheet.mirror {
                                                let bg = if let Some(b) = background {
                                                    let yy = (b - 1) / (image_width / tilewidth);
                                                    let xx =
                                                        (b - 1) - yy * (image_width / tilewidth);
                                                    Some(
                                                        1 + xx
                                                            + (img.height() / tileheight - 1 - yy)
                                                                * image_width
                                                                / tilewidth,
                                                    )
                                                } else {
                                                    None
                                                };
                                                tiles.insert(
                                                    ixx + i - j * image_width / tilewidth,
                                                    Tile {
                                                        index: idx + 1,
                                                        mode,
                                                        palette_number,
                                                        background: bg,
                                                        gfx: tgfx,
                                                        fake: tile.fake.unwrap_or(false),
                                                    },
                                                );
                                            }
                                            if tile.alias.is_none() {
                                                index += tile_bytes;
                                            }
                                            idx += tile_bytes;
                                            offset += tile_bytes;
                                        }
                                    }
                                }
                                //println!("Tiles : {:?}", tiles);

                                // Generate the C code for the the sparse tiles
                                // to be used with multisprite.h or sparse_tiling.h header
                                let mut tiles_store = Vec::<(String, Vec<u32>, bool)>::new();
                                let mut sequences_code = HashMap::<String, String>::new();
                                let mut sequences_used = HashSet::<String>::new();

                                // Process sequences & pregenerate immediate data
                                if let Some(sequences) = &tiles_sheet.sequences {
                                    for (i, sequence) in sequences.iter().enumerate() {
                                        let ignore = if let Some(names) = &sequence.ignore {
                                            names.contains(&varname)
                                        } else {
                                            false
                                        };
                                        if !ignore {
                                            let name = if let Some(n) = &sequence.name {
                                                format!("{}_{}", varname, n.clone())
                                            } else {
                                                format!("{}_sequence_{}", varname, i)
                                            };
                                            let mut tn = Vec::new();
                                            let mut tileset = Vec::new();
                                            for s in &sequence.sequence {
                                                let ix;
                                                let idx = s.parse::<u32>();
                                                if let Ok(index) = idx {
                                                    let tile_name = tile_names_ex.get(&index);
                                                    if tile_name.is_none() {
                                                        return Err(anyhow!(
                                                            "Unknown tile number {}",
                                                            index
                                                        ));
                                                    }
                                                    ix = refs.get(tile_name.unwrap());
                                                } else {
                                                    ix = refs.get(s);
                                                }
                                                if ix.is_none() {
                                                    return Err(anyhow!("Unknown tile name {}", s));
                                                }
                                                let tile = tiles.get(ix.unwrap()).unwrap();
                                                let nb = match tile.mode {
                                                    "160A" | "320A" | "320D" => 1,
                                                    _ => 2,
                                                };
                                                for i in 0..nb {
                                                    tn.push(
                                                        tile.index + (i * bytes_per_tile) as u32,
                                                    );
                                                }
                                                tileset.push(tile);
                                            }

                                            let mut seq = Vec::<&Tile>::new();
                                            let mut tnx = Vec::new();
                                            if let Some(prefix) = &sequence.prefix {
                                                let ix;
                                                let idx = prefix.parse::<u32>();
                                                if let Ok(index) = idx {
                                                    let tile_name = tile_names_ex.get(&index);
                                                    if tile_name.is_none() {
                                                        return Err(anyhow!(
                                                            "Unknown tile number {}",
                                                            index
                                                        ));
                                                    }
                                                    ix = refs.get(tile_name.unwrap());
                                                } else {
                                                    ix = refs.get(prefix);
                                                }
                                                if ix.is_none() {
                                                    return Err(anyhow!(
                                                        "Unknown tile name {}",
                                                        prefix
                                                    ));
                                                }
                                                let tile = tiles.get(ix.unwrap()).unwrap();
                                                let nb = match tile.mode {
                                                    "160A" | "320A" | "320D" => 1,
                                                    _ => 2,
                                                };
                                                for i in 0..nb {
                                                    tnx.push(
                                                        tile.index + (i * bytes_per_tile) as u32,
                                                    );
                                                }
                                                seq.push(tile);
                                            }
                                            for _ in 0..sequence.repeat.unwrap_or(1) {
                                                seq.extend(tileset.iter());
                                                tnx.extend(tn.iter());
                                            }
                                            if let Some(postfix) = &sequence.postfix {
                                                let ix;
                                                let idx = postfix.parse::<u32>();
                                                if let Ok(index) = idx {
                                                    let tile_name = tile_names_ex.get(&index);
                                                    if tile_name.is_none() {
                                                        return Err(anyhow!(
                                                            "Unknown tile number {}",
                                                            index
                                                        ));
                                                    }
                                                    ix = refs.get(tile_name.unwrap());
                                                } else {
                                                    ix = refs.get(postfix);
                                                }
                                                if ix.is_none() {
                                                    return Err(anyhow!(
                                                        "Unknown tile name {}",
                                                        postfix
                                                    ));
                                                }
                                                let tile = tiles.get(ix.unwrap()).unwrap();
                                                let nb = match tile.mode {
                                                    "160A" | "320A" | "320D" => 1,
                                                    _ => 2,
                                                };
                                                for i in 0..nb {
                                                    tnx.push(
                                                        tile.index + (i * bytes_per_tile) as u32,
                                                    );
                                                }
                                                seq.push(tile);
                                            }
                                            let mut generate = true;
                                            if let Some(g) = sequence.generate {
                                                if !g {
                                                    generate = false;
                                                }
                                            }
                                            if generate {
                                                let mut s = String::new();

                                                let l = tnx.len() * bytes_per_tile;
                                                if let Some(b) = sequence.bank {
                                                    s.push_str(&format!("bank{b} "));
                                                } else if let Some(b) = tiles_sheet.bank {
                                                    s.push_str(&format!("bank{b} "));
                                                }
                                                if let Some(h) = sequence.holeydma {
                                                    if h {
                                                        s.push_str("holeydma ");
                                                    }
                                                }
                                                s.push_str(&format!(
                                                "reversed scattered({},{}) char {}[{}] = {{\n\t",
                                                tileheight,
                                                l,
                                                &name,
                                                l * tileheight as usize
                                            ));
                                                let mut i = 0;
                                                for y in 0..tileheight as usize {
                                                    for t in &seq {
                                                        let nb = match t.mode {
                                                            "160A" | "320A" | "320D" => 1,
                                                            _ => 2,
                                                        };
                                                        for b in 0..(nb * bytes_per_tile) {
                                                            s.push_str(&format!(
                                                                "0x{:02x}",
                                                                t.gfx
                                                                    [y * (nb * bytes_per_tile) + b]
                                                            ));
                                                            if i != l * tileheight as usize - 1 {
                                                                if (i + 1) % 16 != 0 {
                                                                    s.push_str(", ");
                                                                } else {
                                                                    s.push_str(",\n\t");
                                                                }
                                                            }
                                                            i += 1;
                                                        }
                                                    }
                                                }
                                                s.push_str("};\n");
                                                sequences_code.insert(name.clone(), s);
                                            }
                                            tiles_store.push((name, tnx, true));
                                        }
                                    }
                                }

                                let mut tilesmap_store = Vec::<(String, String)>::new();
                                let mut tilesmap = Vec::<String>::new();
                                let mut output = String::new();
                                let mut tilesets_set = Vec::<Vec::<(u32, Vec<Tile>)>>::new();

                                for y in 0..height {
                                    // For each line, find the tilesets
                                    let mut tilesets =
                                        VecDeque::<(u32, Vec<Tile>)>::with_capacity(10);
                                    let mut background_tileset = Vec::<Tile>::new();
                                    let mut foreground_tileset = Vec::<Tile>::new();
                                    let mut deferred_tileset = Vec::<Vec<Tile>>::new();
                                    let mut background_startx = 0;
                                    let mut foreground_startx = 0;
                                    let mut deferred_startx = Vec::<u32>::new();
                                    for x in 0..width {
                                        let cell = array[y * width + x];
                                        if cell == 0 {
                                            // Empty cell
                                            if !background_tileset.is_empty() {
                                                if args.force_left_to_right_order {
                                                    tilesets.push_back((
                                                        background_startx,
                                                        background_tileset,
                                                    ));
                                                } else {
                                                    tilesets.push_front((
                                                        background_startx,
                                                        background_tileset,
                                                    ));
                                                }
                                                background_tileset = Vec::<Tile>::new();
                                            }
                                            if !foreground_tileset.is_empty() {
                                                tilesets.push_back((
                                                    foreground_startx,
                                                    foreground_tileset,
                                                ));
                                                foreground_tileset = Vec::<Tile>::new();
                                            }
                                            for i in 0..deferred_tileset.len() {
                                                tilesets.push_back((
                                                    deferred_startx[i],
                                                    deferred_tileset[i].clone(),
                                                ))
                                            }
                                            deferred_tileset = Vec::<Vec<Tile>>::new();
                                            deferred_startx = Vec::<u32>::new();
                                        } else if let Some(t) = tiles.get(&cell) {
                                            if let Some(r) = t.background {
                                                // It's a tile with background info
                                                if let Some(bt) = tiles.get(&r) {
                                                    // Let's check the background tile
                                                    if let Some(tx) = background_tileset.last() {
                                                        // Is the cell compatible with the background tileset in construction ?
                                                        if bt.mode == tx.mode
                                                            && bt.palette_number
                                                                == tx.palette_number
                                                            && bt.fake == tx.fake
                                                        {
                                                            // Yes, let's add it to the current background tileset
                                                            if background_tileset.len()
                                                                >= tileset_maxsize
                                                            {
                                                                if args.force_left_to_right_order {
                                                                    tilesets.push_back((
                                                                        background_startx,
                                                                        background_tileset,
                                                                    ));
                                                                } else {
                                                                    tilesets.push_front((
                                                                        background_startx,
                                                                        background_tileset,
                                                                    ));
                                                                }
                                                                background_tileset =
                                                                    Vec::<Tile>::new();
                                                                background_startx = x as u32;
                                                            }
                                                            background_tileset.push(bt.clone());
                                                        } else {
                                                            // No. Let's write this background tileset
                                                            if args.force_left_to_right_order {
                                                                tilesets.push_back((
                                                                    background_startx,
                                                                    background_tileset,
                                                                ));
                                                            } else {
                                                                tilesets.push_front((
                                                                    background_startx,
                                                                    background_tileset,
                                                                ));
                                                            }
                                                            // And let's start a new background tileset
                                                            background_tileset = vec![bt.clone()];
                                                            background_startx = x as u32;
                                                        }
                                                    } else {
                                                        // Let's look at the current foreground
                                                        // tileset to see if it would fit as a
                                                        // background tileset
                                                        if let Some(tx) = foreground_tileset.last()
                                                        {
                                                            // Is the cell compatible with the foreground tileset in construction ?
                                                            if bt.mode == tx.mode
                                                                && bt.palette_number
                                                                    == tx.palette_number
                                                                && bt.fake == tx.fake
                                                            {
                                                                // Yes, it's compatible. Let's
                                                                // convert this foreground tileset
                                                                // into a background tileset
                                                                background_tileset =
                                                                    foreground_tileset.clone();
                                                                background_startx =
                                                                    foreground_startx;
                                                                foreground_tileset =
                                                                    Vec::<Tile>::new();
                                                            } else {
                                                                background_startx = x as u32;
                                                            }
                                                        } else {
                                                            background_startx = x as u32;
                                                        }
                                                        // No, so start a new background tileset
                                                        background_tileset.push(bt.clone());
                                                        // And send the current foreground
                                                        if !foreground_tileset.is_empty() {
                                                            tilesets.push_back((
                                                                foreground_startx,
                                                                foreground_tileset,
                                                            ));
                                                            foreground_tileset = Vec::<Tile>::new();
                                                        }
                                                    }
                                                    // Let's check the foreground tile
                                                    if let Some(tx) = foreground_tileset.last() {
                                                        // Is the cell compatible with the foreground tileset in construction ?
                                                        if t.mode == tx.mode
                                                            && t.palette_number == tx.palette_number
                                                            && t.fake == tx.fake
                                                        {
                                                            // Yes, let's add it to the current foreground tileset
                                                            if foreground_tileset.len()
                                                                >= tileset_maxsize
                                                            {
                                                                tilesets.push_back((
                                                                    foreground_startx,
                                                                    foreground_tileset,
                                                                ));
                                                                foreground_tileset =
                                                                    Vec::<Tile>::new();
                                                                foreground_startx = x as u32;
                                                            }
                                                            foreground_tileset.push(t.clone());
                                                            //println!("foreground_tileset = {:?}", foreground_tileset);
                                                        } else {
                                                            // No. Let's write this foreground tileset
                                                            tilesets.push_back((
                                                                foreground_startx,
                                                                foreground_tileset,
                                                            ));
                                                            foreground_tileset = Vec::<Tile>::new();
                                                            // And let's start a new foreground tileset
                                                            foreground_tileset.push(t.clone());
                                                            foreground_startx = x as u32;
                                                        }
                                                    } else {
                                                        // No, so start a new foreground tileset
                                                        foreground_tileset.push(t.clone());
                                                        foreground_startx = x as u32;
                                                    }
                                                } else {
                                                    // Empty cell
                                                    if !background_tileset.is_empty() {
                                                        if args.force_left_to_right_order {
                                                            tilesets.push_back((
                                                                background_startx,
                                                                background_tileset,
                                                            ));
                                                        } else {
                                                            tilesets.push_front((
                                                                background_startx,
                                                                background_tileset,
                                                            ));
                                                        }
                                                        background_tileset = Vec::<Tile>::new();
                                                    }
                                                    if !foreground_tileset.is_empty() {
                                                        tilesets.push_back((
                                                            foreground_startx,
                                                            foreground_tileset,
                                                        ));
                                                        foreground_tileset = Vec::<Tile>::new();
                                                    }
                                                }
                                            } else {
                                                // It's a normal tile
                                                if let Some(tx) = background_tileset.last() {
                                                    // Is the cell compatible with the background tileset in construction ?
                                                    if t.mode == tx.mode
                                                        && t.palette_number == tx.palette_number
                                                        && t.fake == tx.fake
                                                    {
                                                        // Yes, let's add it the current background tileset
                                                        if background_tileset.len()
                                                            >= tileset_maxsize
                                                        {
                                                            if args.force_left_to_right_order {
                                                                tilesets.push_back((
                                                                    background_startx,
                                                                    background_tileset,
                                                                ));
                                                            } else {
                                                                tilesets.push_front((
                                                                    background_startx,
                                                                    background_tileset,
                                                                ));
                                                            }
                                                            background_tileset = Vec::<Tile>::new();
                                                            background_startx = x as u32;
                                                        }
                                                        background_tileset.push(t.clone());
                                                        // Is there a foreground tileset ?
                                                        if !foreground_tileset.is_empty() {
                                                            // Yes. Let's write this foreground tileset
                                                            deferred_tileset
                                                                .push(foreground_tileset.clone());
                                                            deferred_startx.push(foreground_startx);
                                                            //println!("deferred_tileset = {:?}", deferred_tileset);
                                                            foreground_tileset = Vec::<Tile>::new();
                                                        }
                                                    } else {
                                                        // No. Let's write this background tileset
                                                        if args.force_left_to_right_order {
                                                            tilesets.push_back((
                                                                background_startx,
                                                                background_tileset,
                                                            ));
                                                        } else {
                                                            tilesets.push_front((
                                                                background_startx,
                                                                background_tileset,
                                                            ));
                                                        }
                                                        background_tileset = Vec::<Tile>::new();
                                                        // Is there a foreground tileset ?
                                                        if let Some(tx) = foreground_tileset.last()
                                                        {
                                                            // Yes. Is it compatible ?
                                                            if t.mode == tx.mode
                                                                && t.palette_number
                                                                    == tx.palette_number
                                                                && t.fake == tx.fake
                                                            {
                                                                // Yes, let's add it the current foreground tileset
                                                                if foreground_tileset.len()
                                                                    >= tileset_maxsize
                                                                {
                                                                    tilesets.push_back((
                                                                        foreground_startx,
                                                                        foreground_tileset,
                                                                    ));
                                                                    foreground_tileset =
                                                                        Vec::<Tile>::new();
                                                                    foreground_startx = x as u32;
                                                                }
                                                                foreground_tileset.push(t.clone());
                                                            } else {
                                                                // No. It's not compatible. Let's write this foreground tileset
                                                                tilesets.push_back((
                                                                    foreground_startx,
                                                                    foreground_tileset,
                                                                ));
                                                                foreground_tileset =
                                                                    Vec::<Tile>::new();
                                                                // And let's start a new background tileset
                                                                background_tileset.push(t.clone());
                                                                background_startx = x as u32;
                                                            }
                                                        } else {
                                                            // No, so start a new background tileset
                                                            background_tileset.push(t.clone());
                                                            background_startx = x as u32;
                                                        }
                                                    }
                                                } else {
                                                    // There is no background tileset. But maybe is there a foregound tileset ?
                                                    if let Some(tx) = foreground_tileset.last() {
                                                        if t.mode == tx.mode
                                                            && t.palette_number == tx.palette_number
                                                            && t.fake == tx.fake
                                                        {
                                                            // Yes, let's add it the current foreground tileset
                                                            if foreground_tileset.len()
                                                                >= tileset_maxsize
                                                            {
                                                                tilesets.push_back((
                                                                    foreground_startx,
                                                                    foreground_tileset,
                                                                ));
                                                                foreground_tileset =
                                                                    Vec::<Tile>::new();
                                                                foreground_startx = x as u32;
                                                            }
                                                            foreground_tileset.push(t.clone());
                                                        } else {
                                                            // No, it's not compatible. Let's write the foreground tileset as it is
                                                            tilesets.push_back((
                                                                foreground_startx,
                                                                foreground_tileset,
                                                            ));
                                                            foreground_tileset = Vec::<Tile>::new();
                                                            // And let's start a foreground tileset
                                                            foreground_tileset.push(t.clone());
                                                            foreground_startx = x as u32;
                                                        }
                                                    } else {
                                                        // No there is nothing. So let's start a foreground tileset
                                                        foreground_tileset.push(t.clone());
                                                        foreground_startx = x as u32;
                                                    }
                                                }
                                            }
                                        } else {
                                            //return Err(anyhow!("Wrong tilesheet. Index unknown"));
                                            // It's not in the tilesheet. Consider it as 0 (empty)
                                            if !background_tileset.is_empty() {
                                                if args.force_left_to_right_order {
                                                    tilesets.push_back((
                                                        background_startx,
                                                        background_tileset,
                                                    ));
                                                } else {
                                                    tilesets.push_front((
                                                        background_startx,
                                                        background_tileset,
                                                    ));
                                                }
                                                //    .push((background_startx, background_tileset));
                                                background_tileset = Vec::<Tile>::new();
                                            }
                                            if !foreground_tileset.is_empty() {
                                                tilesets.push_back((
                                                    foreground_startx,
                                                    foreground_tileset,
                                                ));
                                                foreground_tileset = Vec::<Tile>::new();
                                            }
                                            for i in 0..deferred_tileset.len() {
                                                tilesets.push_back((
                                                    deferred_startx[i],
                                                    deferred_tileset[i].clone(),
                                                ))
                                            }
                                            deferred_tileset = Vec::<Vec<Tile>>::new();
                                            deferred_startx = Vec::<u32>::new();
                                        }
                                    }
                                    // Write the last tilesets
                                    if !background_tileset.is_empty() {
                                        if args.force_left_to_right_order {
                                            tilesets
                                                .push_back((background_startx, background_tileset));
                                        } else {
                                            tilesets.push_front((
                                                background_startx,
                                                background_tileset,
                                            ));
                                        }
                                    }
                                    if !foreground_tileset.is_empty() {
                                        tilesets.push_back((foreground_startx, foreground_tileset));
                                    }
                                    for i in 0..deferred_tileset.len() {
                                        tilesets.push_back((
                                            deferred_startx[i],
                                            deferred_tileset[i].clone(),
                                        ));
                                    }

                                    // OK. Now we have the tilesets. Let's try to see if we can
                                    // break these into a new tilesets for better optimization
                                    let mut tilesets_ex = Vec::<(u32, Vec<Tile>)>::new();
                                    for s in tilesets {
                                        if s.1.len() >= 5 {
                                            // The tilesets must be at least 5 tiles long
                                            let mut tn = Vec::new(); // The vector of tile numbers (in Atari 7800 format)
                                            let nb = match s.1[0].mode {
                                                "160A" | "320A" | "320D" => 1,
                                                _ => 2,
                                            };
                                            for t in &s.1 {
                                                for i in 0..nb {
                                                    tn.push(t.index + (i * bytes_per_tile) as u32);
                                                }
                                            }
                                            // Let's look at the previous sequences
                                            let mut found = false;
                                            for c in &tiles_store {
                                                if c.2 {
                                                    //println!("Compare {:?} with {}", tn, c.0);
                                                    // Look only at immediate sequences
                                                    // Look for tn in c.1
                                                    if c.1.windows(tn.len()).any(|w| tn == w) {
                                                        found = true;
                                                        break;
                                                    }
                                                }
                                            }
                                            if found {
                                                // Keep it. It's a part of sequence
                                                tilesets_ex.push(s);
                                            } else {
                                                // OK. This is not a sequence. Let's try to cut it.
                                                // Let's look at the sequence but the first tile
                                                // And then at the sequence but the last tile
                                                let mut tnx = VecDeque::from(tn.clone());
                                                for _ in 0..nb {
                                                    tnx.pop_front();
                                                }
                                                for c in &tiles_store {
                                                    if c.2 {
                                                        //println!("Compare {:?} with {}", tnx, c.0);
                                                        // Look only at immediate sequences
                                                        // Look for tnx in c.1
                                                        if c.1.windows(tnx.len()).any(|w| tnx == w)
                                                        {
                                                            found = true;
                                                            break;
                                                        }
                                                    }
                                                }
                                                if found {
                                                    //println!("I was here");
                                                    // Let's split it into two tilesets
                                                    let tileset1 = vec![s.1[0].clone()];
                                                    tilesets_ex.push((s.0, tileset1));
                                                    let mut tileset2 = s.1.clone();
                                                    tileset2.remove(0);
                                                    tilesets_ex.push((s.0 + 1, tileset2));
                                                } else {
                                                    let mut tnx = tn.clone();
                                                    for _ in 0..nb {
                                                        tnx.pop();
                                                    }
                                                    for c in &tiles_store {
                                                        if c.2 {
                                                            //println!("Compare {:?} with {}", tnx, c.0);
                                                            // Look only at immediate sequences
                                                            // Look for tnx in c.1
                                                            if c.1
                                                                .windows(tnx.len())
                                                                .any(|w| tnx == w)
                                                            {
                                                                found = true;
                                                                break;
                                                            }
                                                        }
                                                    }
                                                    if found {
                                                        //println!("I was here");
                                                        // Let's split it into two tilesets
                                                        let mut tileset2 = s.1.clone();
                                                        let tileset1 =
                                                            vec![tileset2.pop().unwrap()];
                                                        tilesets_ex.push((
                                                            s.0 + tileset2.len() as u32,
                                                            tileset1,
                                                        ));
                                                        tilesets_ex.push((s.0, tileset2));
                                                    } else {
                                                        tilesets_ex.push(s);
                                                    }
                                                }
                                            }
                                        } else {
                                            tilesets_ex.push(s);
                                        }
                                    }
                                    // OK. Now we have the tilesets_ex vector of (pos, tiles)
                                    tilesets_set.push(tilesets_ex);
                                }
                                
                                let mut y = 0;
                                for tilesets_ex in tilesets_set {
                                    // Write this line of data
                                    {
                                        let mut c = 0;
                                        let mut w = Vec::new();
                                        let mut tile_names = Vec::new();
                                        let mut imm = Vec::new();
                                        for s in &tilesets_ex {
                                            let mut immediate = args.immediate;
                                            let mut tn = Vec::new(); // The vector of tile numbers (in Atari 7800 format)
                                            let mut continuous_tileset = true;
                                            let mut previous_index = None;
                                            for t in &s.1 {
                                                let tile_bytes = match t.mode {
                                                    "160A" => tilewidth / 8,
                                                    "160B" => tilewidth / 4,
                                                    "320A" => tilewidth / 8,
                                                    "320B" => tilewidth / 4,
                                                    "320C" => tilewidth / 4,
                                                    "320D" => tilewidth / 8,
                                                    _ => unreachable!(),
                                                };
                                                let nb = match t.mode {
                                                    "160A" | "320A" | "320D" => 1,
                                                    _ => 2,
                                                };
                                                for i in 0..nb {
                                                    tn.push(t.index + (i * bytes_per_tile) as u32);
                                                }
                                                if let Some(pi) = previous_index {
                                                    if pi + tile_bytes != t.index {
                                                        continuous_tileset = false;
                                                    }
                                                }
                                                previous_index = Some(t.index);
                                                if t.fake {
                                                    continuous_tileset = false; // Avoid direct use
                                                                                // of immediate tile data, since it's fake
                                                }
                                            }
                                            if continuous_tileset
                                                && !forbid_immediate
                                                && !args.immediate
                                            {
                                                w.push(tn.len() * bytes_per_tile);
                                                imm.push(true);
                                                tile_names.push(
                                                    tile_names_ex
                                                        .get(&s.1[0].index)
                                                        .unwrap()
                                                        .clone(),
                                                );
                                            } else {
                                                // 1st optimization : look in the tiles_store if it's already there
                                                let mut found = None;
                                                for c in &tiles_store {
                                                    // Look for tn in c.1
                                                    if let Some(p) =
                                                        c.1.windows(tn.len()).position(|w| tn == w)
                                                    {
                                                        sequences_used.insert(c.0.clone());
                                                        immediate = c.2;
                                                        found = if p == 0 {
                                                            Some(c.0.clone())
                                                        } else {
                                                            let offset = if immediate {
                                                                p * bytes_per_tile
                                                            } else {
                                                                p
                                                            };
                                                            Some(format!(
                                                                "{} + {}",
                                                                c.0.clone(),
                                                                offset
                                                            ))
                                                        };
                                                        break;
                                                    } /*
                                                      if c.1.starts_with(&tn) {
                                                          found = Some(c.0.clone());
                                                          immediate = c.2;
                                                          break;
                                                      }
                                                      */
                                                }

                                                // l is the number of bytes in the current tileset
                                                let l = if immediate {
                                                    tn.len() * bytes_per_tile
                                                } else {
                                                    tn.len()
                                                };
                                                w.push(l);
                                                imm.push(immediate);

                                                if let Some(name) = found {
                                                    tile_names.push(name);
                                                } else {
                                                    let name = format!("{}_{}_{}", varname, y, c);
                                                    if let Some(b) = tiles_sheet.bank {
                                                        output.push_str(&format!("bank{b} "));
                                                    }
                                                    if immediate {
                                                        output.push_str(&format!(
                                                        "reversed scattered({},{}) char {}[{}] = {{\n\t",
                                                        tileheight,
                                                        l,
                                                        &name,
                                                        l * tileheight as usize
                                                    ));
                                                        let mut i = 0;
                                                        for y in 0..tileheight as usize {
                                                            for t in &s.1 {
                                                                let nb = match t.mode {
                                                                    "160A" | "320A" | "320D" => 1,
                                                                    _ => 2,
                                                                };
                                                                for b in 0..(nb * bytes_per_tile) {
                                                                    output.push_str(&format!(
                                                                        "0x{:02x}",
                                                                        t.gfx[y
                                                                            * (nb
                                                                                * bytes_per_tile)
                                                                            + b]
                                                                    ));
                                                                    if i != l * tileheight as usize
                                                                        - 1
                                                                    {
                                                                        if (i + 1) % 16 != 0 {
                                                                            output.push_str(", ");
                                                                        } else {
                                                                            output
                                                                                .push_str(",\n\t");
                                                                        }
                                                                    }
                                                                    i += 1;
                                                                }
                                                            }
                                                        }
                                                        output.push_str("};\n");
                                                    } else {
                                                        output.push_str(&format!(
                                                            "const char {}[{}] = {{",
                                                            &name,
                                                            tn.len()
                                                        ));
                                                        for i in 0..tn.len() - 1 {
                                                            output
                                                                .push_str(&format!("{}, ", tn[i]));
                                                        }
                                                        output.push_str(&format!(
                                                            "{}}};\n",
                                                            tn[tn.len() - 1]
                                                        ));
                                                    }
                                                    tiles_store.push((name.clone(), tn, immediate));
                                                    tile_names.push(name);
                                                }
                                            }
                                            c += 1;
                                        }
                                        c = 0;
                                        let mut tilemap_str = String::new();
                                        for s in &tilesets_ex {
                                            let ttype = s.1.first().unwrap();
                                            let write_mode = match ttype.mode {
                                                "160A" | "320A" | "320D" => 0x40,
                                                _ => 0xc0,
                                            } | if imm[c] { 0 } else { 0x20 };
                                            let dma = if imm[c] {
                                                (10 + 3 * w[c]) / 2
                                            } else {
                                                (10 + 3 + 9 * w[c]) / 2
                                            };
                                            let tn = &tile_names[c];
                                            if args.direct {
                                                if imm[c] && args.four_bytes && c != 0 {
                                                    tilemap_str.push_str(&format!("{}, ({} << 5) | ((-{}) & 0x1f), {} >> 8, {}, ", 
                                                        tn, ttype.palette_number, w[c], tn, s.0 * 8 + args.offset as u32));
                                                } else {
                                                    tilemap_str.push_str(&format!("{}, 0x{:02x}, {} >> 8, ({} << 5) | ((-{}) & 0x1f), {}, ", 
                                                        tn, write_mode, tn, ttype.palette_number, w[c], s.0 * 8 + args.offset as u32));
                                                }
                                            } else {
                                                tilemap_str.push_str(&format!("{}, {}, {}, 0x{:02x}, {} >> 8, ({} << 5) | ((-{}) & 0x1f), {dma}, ", 
                                                    s.0 + s.1.len() as u32 - 1, s.0, tn, write_mode, tn, ttype.palette_number, w[c]));
                                            }
                                            c += 1;
                                        }
                                        let mut found = None;
                                        for c in &tilesmap_store {
                                            if c.1 == tilemap_str {
                                                found = Some(c.0.clone());
                                            }
                                        }
                                        if let Some(name) = found {
                                            tilesmap.push(name);
                                        } else {
                                            let tilemap_name = format!("{}_{}_data", varname, y);
                                            if let Some(b) = tiles_sheet.bank {
                                                output.push_str(&format!("bank{} ", b));
                                            }
                                            if args.direct {
                                                output.push_str(&format!(
                                                    "const char {}[] = {{{}0, 0}};\n",
                                                    &tilemap_name, tilemap_str
                                                ));
                                            } else {
                                                output.push_str(&format!(
                                                    "const char {}[] = {{{}96, 0xff}};\n",
                                                    &tilemap_name, tilemap_str
                                                ));
                                            }
                                            tilesmap_store
                                                .push((tilemap_name.clone(), tilemap_str.clone()));
                                            tilesmap.push(tilemap_name);
                                        }
                                    }
                                    y += 1;
                                }

                                // Output sequences
                                if let Some(sequences) = &tiles_sheet.sequences {
                                    for (i, sequence) in sequences.iter().enumerate() {
                                        let name = if let Some(n) = &sequence.name {
                                            format!("{}_{}", varname, n.clone())
                                        } else {
                                            format!("{}_sequence_{}", varname, i)
                                        };
                                        if sequences_used.contains(&name) {
                                            print!("{}", sequences_code.get(&name).unwrap());
                                        }
                                    }
                                }
                                // Output tilemap
                                //
                                print!("{output}");

                                println!();
                                if let Some(b) = tiles_sheet.bank {
                                    print!("bank{b} ");
                                }
                                print!("const char {varname}_data_ptrs_high[{}] = {{", height);
                                for y in 0..height - 1 {
                                    print!("{} >> 8, ", &tilesmap[y]);
                                }
                                println!("{} >> 8}};\n", &tilesmap[height - 1]);
                                if let Some(b) = tiles_sheet.bank {
                                    print!("bank{b} ");
                                }
                                print!("const char {varname}_data_ptrs_low[{}] = {{", height);
                                for y in 0..height - 1 {
                                    print!("{} & 0xff, ", &tilesmap[y]);
                                }
                                println!("{} & 0xff}};\n", &tilesmap[height - 1]);
                                if let Some(b) = tiles_sheet.bank {
                                    print!("bank{b} ");
                                }
                                println!("const char *{varname}_data_ptrs[2] = {{{varname}_data_ptrs_high, {varname}_data_ptrs_low}};\n");
                                println!("/*\n#define TILING_HEIGHT {}", height);
                                println!("#define TILING_WIDTH {}", width);
                                println!("#include \"sparse_tiling.h\"\n*/\n");
                            } else {
                                print!(
                                    "const char {varname}[{}] = {{",
                                    if args.boundaries {
                                        (width + 1) * height + 1
                                    } else {
                                        width * height
                                    }
                                );
                                for i in 0..height {
                                    if args.boundaries {
                                        print!("\n\t0xff, ");
                                    } else {
                                        print!("\n\t");
                                    }
                                    for j in 0..width {
                                        let v = array[i * width + j];
                                        let w = if v == 0 { 0 } else { (v - 1) * 2 };
                                        print!(
                                            "{}{} ",
                                            w,
                                            if args.boundaries || i != height - 1 || j != width - 1
                                            {
                                                ","
                                            } else {
                                                ""
                                            }
                                        );
                                    }
                                }
                                if args.boundaries {
                                    println!("\n\t0xff}};");
                                } else {
                                    println!("\n\t}};");
                                }
                            }
                            return Ok(());
                        }
                        return Err(anyhow!("Bad data format. Unexpected table size."));
                    }
                }
            }
        }
    }
    Err(anyhow!("Unexpected data provided."))
}
