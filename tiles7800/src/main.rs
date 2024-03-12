use anyhow::{anyhow, Result};
use clap::Parser;
use image::{GenericImageView, Rgba};
use serde::Deserialize;
use std::collections::HashMap;
use std::error;
use std::fs;
use std::str::FromStr;
use xml_dom::level2::{Node, NodeType};

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
}

#[allow(unused)]
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
    sprites: Vec<Sprite>,
}

#[allow(unused)]
#[derive(Deserialize)]
struct Palette {
    name: String,
    colors: Vec<(u8, u8, u8)>,
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
}

#[derive(Debug, Clone)]
struct Tile<'a> {
    index: u32,
    mode: &'a str,
    palette_number: u8,
    background: Option<u32>,
    gfx: Vec<u8>,
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

fn main() -> Result<(), Box<dyn error::Error>> {
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
    for n in &root.child_nodes() {
        if n.node_type() == NodeType::Element && n.local_name() == "layer" {
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
                                    args.maxsize.unwrap_or(if tilewidth == 8 { 16 } else { 8 });
                                let contents = fs::read_to_string(yaml_file)
                                    .expect("Unable to read input file");
                                let t: AllSprites = serde_yaml::from_str(&contents)?;
                                // OK, we have the array, we have the tiles specs. Let's match them
                                // Let's scan all the tiles to make sure all this makes sense
                                if t.sprite_sheets.len() != 1 {
                                    eprintln!("Only the first sprite sheet (tiles) will be used");
                                }
                                let tiles_sheet = &t.sprite_sheets[0];
                                let img = image::open(&tiles_sheet.image)
                                    .expect(&format!("Can't open image {}", tiles_sheet.image));
                                let image_width = img.width();
                                let mut index = 0;
                                let defmode = tiles_sheet.mode.as_str();
                                let mut tiles = HashMap::<u32, Tile>::new();
                                let mut aliases = HashMap::<&str, u32>::new();
                                let mut refs = HashMap::<u32, u32>::new(); // Mapping from tile number in the Atari tiles array to tile number in tiled array
                                let bytes_per_tile: usize = if tilewidth == 8 { 1 } else { 2 };
                                for tile in &tiles_sheet.sprites {
                                    let gfx = if args.immediate {
                                        sprite_gfx(&img, &t, tiles_sheet, tile)?
                                    } else {
                                        Vec::<u8>::new()
                                    };
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
                                    aliases.insert(&tile.name.as_str(), index);
                                    let y = tile.top / tileheight;
                                    let x = tile.left / tilewidth;
                                    let ix = 1 + x + y * image_width / tilewidth;
                                    let ixx = 1
                                        + x
                                        + (img.height() / tileheight - 1 - y) * image_width
                                            / tilewidth;
                                    refs.insert(index, ix);
                                    refs.insert(index + 1, ixx);
                                    let nbtilesx = tile.width / tilewidth;
                                    let nbtilesy = tile.height / tileheight;
                                    let palette_number = if let Some(p) = tile.palette_number {
                                        p
                                    } else {
                                        0
                                    };
                                    let mut background = if let Some(b) = &tile.background {
                                        aliases.get(b.as_str()).copied()
                                    } else {
                                        None
                                    };
                                    let mut idx = if let Some(alias) = &tile.alias {
                                        if let Some(i) = aliases.get(alias.as_str()) {
                                            if let Some(Mirror::Vertical) = tile.mirror {
                                                if let Some(b) = background {
                                                    // Background is also reversed
                                                    background = Some(b + 1);
                                                }
                                                *i + 1 // Add 1 for vertical mirroring
                                            } else {
                                                *i
                                            }
                                        } else {
                                            return Err(Box::new(std::io::Error::new(
                                                std::io::ErrorKind::Other,
                                                "Bad alias",
                                            )));
                                        }
                                    } else {
                                        index
                                    };
                                    for j in 0..nbtilesy {
                                        for i in 0..nbtilesx {
                                            let tgfx = if args.immediate {
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
                                            } else {
                                                Vec::<u8>::new()
                                            };
                                            tiles.insert(
                                                ix + i + j * image_width / tilewidth,
                                                Tile {
                                                    index: idx,
                                                    mode,
                                                    palette_number,
                                                    background,
                                                    gfx: tgfx.clone(),
                                                },
                                            );
                                            if let Some(Mirror::Vertical) = tiles_sheet.mirror {
                                                let bg = background.map(|b| b + 1);
                                                tiles.insert(
                                                    ixx + i - j * image_width / tilewidth,
                                                    Tile {
                                                        index: idx + 1,
                                                        mode,
                                                        palette_number,
                                                        background: bg,
                                                        gfx: tgfx,
                                                    },
                                                );
                                            }
                                            index += tile_bytes;
                                            idx += tile_bytes;
                                        }
                                    }
                                }
                                //println!("Tiles : {:?}", tiles);

                                // Generate the C code for the the sparse tiles
                                // to be used with multisprite.h or sparse_tiling.h header
                                let mut tiles_store = Vec::<(String, Vec<u32>)>::new();
                                let mut tilesmap_store = Vec::<(String, String)>::new();
                                let mut tilesmap = Vec::<String>::new();

                                for y in 0..height {
                                    // For each line, find the tilesets
                                    let mut tilesets = Vec::<(u32, Vec<Tile>)>::new();
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
                                                tilesets
                                                    .push((background_startx, background_tileset));
                                                background_tileset = Vec::<Tile>::new();
                                            }
                                            if !foreground_tileset.is_empty() {
                                                tilesets
                                                    .push((foreground_startx, foreground_tileset));
                                                foreground_tileset = Vec::<Tile>::new();
                                            }
                                            for i in 0..deferred_tileset.len() {
                                                tilesets.push((
                                                    deferred_startx[i],
                                                    deferred_tileset[i].clone(),
                                                ))
                                            }
                                            deferred_tileset = Vec::<Vec<Tile>>::new();
                                            deferred_startx = Vec::<u32>::new();
                                        } else if let Some(t) = tiles.get(&cell) {
                                            if let Some(btx) = t.background {
                                                let r = refs.get(&btx).unwrap();
                                                // It's a tile with background info
                                                if let Some(bt) = tiles.get(r) {
                                                    // Let's check the background tile
                                                    if let Some(tx) = background_tileset.last() {
                                                        // Is the cell compatible with the background tileset in construction ?
                                                        if bt.mode == tx.mode
                                                            && bt.palette_number
                                                                == tx.palette_number
                                                        {
                                                            // Yes, let's add it to the current background tileset
                                                            if background_tileset.len()
                                                                >= tileset_maxsize
                                                            {
                                                                tilesets.push((
                                                                    background_startx,
                                                                    background_tileset,
                                                                ));
                                                                background_tileset =
                                                                    Vec::<Tile>::new();
                                                                background_startx = x as u32;
                                                            }
                                                            background_tileset.push(bt.clone());
                                                        } else {
                                                            // No. Let's write this background tileset
                                                            tilesets.push((
                                                                background_startx,
                                                                background_tileset,
                                                            ));
                                                            background_tileset = Vec::<Tile>::new();
                                                            // And let's start a new background tileset
                                                            background_tileset.push(bt.clone());
                                                            background_startx = x as u32;
                                                        }
                                                    } else {
                                                        // No, so start a new background tileset
                                                        background_tileset.push(bt.clone());
                                                        background_startx = x as u32;
                                                    }
                                                    // Let's check the foreground tile
                                                    if let Some(tx) = foreground_tileset.last() {
                                                        // Is the cell compatible with the foreground tileset in construction ?
                                                        if t.mode == tx.mode
                                                            && t.palette_number == tx.palette_number
                                                        {
                                                            // Yes, let's add it to the current foreground tileset
                                                            if foreground_tileset.len()
                                                                >= tileset_maxsize
                                                            {
                                                                tilesets.push((
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
                                                            tilesets.push((
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
                                                        tilesets.push((
                                                            background_startx,
                                                            background_tileset,
                                                        ));
                                                        background_tileset = Vec::<Tile>::new();
                                                    }
                                                    if !foreground_tileset.is_empty() {
                                                        tilesets.push((
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
                                                    {
                                                        // Yes, let's add it the current background tileset
                                                        if background_tileset.len()
                                                            >= tileset_maxsize
                                                        {
                                                            tilesets.push((
                                                                background_startx,
                                                                background_tileset,
                                                            ));
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
                                                        tilesets.push((
                                                            background_startx,
                                                            background_tileset,
                                                        ));
                                                        background_tileset = Vec::<Tile>::new();
                                                        // Is there a foreground tileset ?
                                                        if let Some(tx) = foreground_tileset.last()
                                                        {
                                                            // Yes. Is it compatible ?
                                                            if t.mode == tx.mode
                                                                && t.palette_number
                                                                    == tx.palette_number
                                                            {
                                                                // Yes, let's add it the current foreground tileset
                                                                if foreground_tileset.len()
                                                                    >= tileset_maxsize
                                                                {
                                                                    tilesets.push((
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
                                                                tilesets.push((
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
                                                        {
                                                            // Yes, let's add it the current foreground tileset
                                                            if foreground_tileset.len()
                                                                >= tileset_maxsize
                                                            {
                                                                tilesets.push((
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
                                                            tilesets.push((
                                                                foreground_startx,
                                                                foreground_tileset,
                                                            ));
                                                            foreground_tileset = Vec::<Tile>::new();
                                                            // And let's start a background tileset
                                                            background_tileset.push(t.clone());
                                                            background_startx = x as u32;
                                                        }
                                                    } else {
                                                        // No there is nothing. So let's start a background tileset
                                                        background_tileset.push(t.clone());
                                                        background_startx = x as u32;
                                                    }
                                                }
                                            }
                                        } else {
                                            //return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Wrong tilesheet. Index unknown")));
                                            // It's not in the tilesheet. Consider it as 0 (empty)
                                            if !background_tileset.is_empty() {
                                                tilesets
                                                    .push((background_startx, background_tileset));
                                                background_tileset = Vec::<Tile>::new();
                                            }
                                            if !foreground_tileset.is_empty() {
                                                tilesets
                                                    .push((foreground_startx, foreground_tileset));
                                                foreground_tileset = Vec::<Tile>::new();
                                            }
                                            for i in 0..deferred_tileset.len() {
                                                tilesets.push((
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
                                        tilesets.push((background_startx, background_tileset));
                                    }
                                    if !foreground_tileset.is_empty() {
                                        tilesets.push((foreground_startx, foreground_tileset));
                                    }
                                    for i in 0..deferred_tileset.len() {
                                        tilesets.push((
                                            deferred_startx[i],
                                            deferred_tileset[i].clone(),
                                        ));
                                    }
                                    // Write this line of data
                                    {
                                        let mut c = 0;
                                        let mut w = Vec::new();
                                        let mut tile_names = Vec::new();
                                        if args.immediate {
                                            for s in &tilesets {
                                                let mut tn = Vec::new();
                                                let mut l = 0;
                                                for t in &s.1 {
                                                    let nb = match t.mode {
                                                        "160A" | "320A" | "320D" => 1,
                                                        _ => 2,
                                                    };
                                                    for i in 0..nb {
                                                        tn.push(
                                                            t.index + (i * bytes_per_tile) as u32,
                                                        );
                                                    }
                                                    l += nb * bytes_per_tile;
                                                }
                                                w.push(l);

                                                // 1st optimization : look in the tiles_store if it's already there
                                                let mut found = None;
                                                for c in &tiles_store {
                                                    if c.1 == tn {
                                                        found = Some(c.0.clone());
                                                    }
                                                }
                                                if let Some(name) = found {
                                                    tile_names.push(name);
                                                } else if l != 0 {
                                                    let name = format!("{}_{}_{}", varname, y, c);
                                                    if let Some(b) = tiles_sheet.bank {
                                                        print!("bank{b} ");
                                                    }
                                                    print!(
                                                            "reversed scattered({},{}) char {}[{}] = {{\n\t",
                                                            tileheight,
                                                            l,
                                                            &name,
                                                            l * tileheight as usize
                                                        );
                                                    let mut i = 0;
                                                    for y in 0..tileheight as usize {
                                                        for t in &s.1 {
                                                            let nb = match t.mode {
                                                                "160A" | "320A" | "320D" => 1,
                                                                _ => 2,
                                                            };
                                                            for b in 0..(nb * bytes_per_tile) {
                                                                print!(
                                                                    "0x{:02x}",
                                                                    t.gfx[y
                                                                        * (nb * bytes_per_tile)
                                                                        + b]
                                                                );
                                                                if i != l * tileheight as usize - 1
                                                                {
                                                                    if (i + 1) % 16 != 0 {
                                                                        print!(", ");
                                                                    } else {
                                                                        print!(",\n\t");
                                                                    }
                                                                }
                                                                i += 1;
                                                            }
                                                        }
                                                    }
                                                    println!("}};");
                                                    c += 1;
                                                    tiles_store.push((name.clone(), tn));
                                                    tile_names.push(name);
                                                }
                                            }
                                            c = 0;
                                            let mut tilemap_str = String::new();
                                            for s in &tilesets {
                                                let ttype = s.1.first().unwrap();
                                                let write_mode = match ttype.mode {
                                                    "160A" | "320A" | "320D" => 0x40,
                                                    _ => 0xc0,
                                                };
                                                tilemap_str.push_str(&format!("{}, {}, {}, 0x{:02x}, {} >> 8, ({} << 5) | ((-{}) & 0x1f), {}, ", 
                                                    s.0 + s.1.len() as u32 - 1, s.0, tile_names[c], write_mode, tile_names[c], ttype.palette_number, w[c], (10 + 3 * w[c]) / 2));
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
                                                let tilemap_name =
                                                    format!("{}_{}_data", varname, y);
                                                if let Some(b) = tiles_sheet.bank {
                                                    print!("bank{b} ");
                                                }
                                                println!(
                                                    "const char {}[] = {{{}96, 0xff}};",
                                                    &tilemap_name, tilemap_str
                                                );
                                                tilesmap_store.push((
                                                    tilemap_name.clone(),
                                                    tilemap_str.clone(),
                                                ));
                                                tilesmap.push(tilemap_name);
                                            }
                                        } else {
                                            for s in &tilesets {
                                                let mut tn = Vec::new();
                                                for t in &s.1 {
                                                    let nb = match t.mode {
                                                        "160A" | "320A" | "320D" => 1,
                                                        _ => 2,
                                                    };
                                                    for i in 0..nb {
                                                        tn.push(
                                                            t.index + (i * bytes_per_tile) as u32,
                                                        );
                                                    }
                                                }
                                                w.push(tn.len());

                                                // 1st optimization : look in the tiles_store if it's already there
                                                let mut found = None;
                                                for c in &tiles_store {
                                                    if c.1 == tn {
                                                        found = Some(c.0.clone());
                                                    }
                                                }
                                                if let Some(name) = found {
                                                    tile_names.push(name);
                                                } else if !tn.is_empty() {
                                                    let name = format!("{}_{}_{}", varname, y, c);
                                                    if let Some(b) = tiles_sheet.bank {
                                                        print!("bank{b} ");
                                                    }
                                                    print!(
                                                        "const char {}[{}] = {{",
                                                        &name,
                                                        tn.len()
                                                    );
                                                    for i in 0..tn.len() - 1 {
                                                        print!("{}, ", tn[i]);
                                                    }
                                                    println!("{}}};", tn[tn.len() - 1]);
                                                    c += 1;
                                                    tiles_store.push((name.clone(), tn));
                                                    tile_names.push(name);
                                                }
                                            }
                                            c = 0;
                                            let mut tilemap_str = String::new();
                                            for s in &tilesets {
                                                let ttype = s.1.first().unwrap();
                                                let write_mode = match ttype.mode {
                                                    "160A" | "320A" | "320D" => 0x60,
                                                    _ => 0xe0,
                                                };
                                                tilemap_str.push_str(&format!("{}, {}, {}, 0x{:02x}, {} >> 8, ({} << 5) | ((-{}) & 0x1f), {}, ", 
                                                    s.0 + s.1.len() as u32 - 1, s.0, tile_names[c], write_mode, tile_names[c], ttype.palette_number, w[c], (10 + 3 + 9 * w[c]) / 2));
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
                                                let tilemap_name =
                                                    format!("{}_{}_data", varname, y);
                                                if let Some(b) = tiles_sheet.bank {
                                                    print!("bank{b} ");
                                                }
                                                println!(
                                                    "const char {}[] = {{{}96, 0xff}};",
                                                    &tilemap_name, tilemap_str
                                                );
                                                tilesmap_store.push((
                                                    tilemap_name.clone(),
                                                    tilemap_str.clone(),
                                                ));
                                                tilesmap.push(tilemap_name);
                                            }
                                        }
                                    }
                                }
                                println!("");
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
                        } else {
                            return Err(Box::new(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                "Bad data format. Unexpected table size.",
                            )));
                        }
                    }
                }
            }
        }
    }
    Err(Box::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        "Unexpected data provided.",
    )))
}
