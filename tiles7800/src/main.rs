use std::fs;
use std::error;
use clap::Parser;
use std::str::FromStr;
use serde::Deserialize;
use xml_dom::level2::{Node, NodeType};
use std::collections::HashMap;

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
    yaml: Option<String>
}

#[allow(unused)]
#[derive(Deserialize)]
struct AllSprites {
    #[serde(default)]
    palettes: Option<Vec<Palette>>,
    sprite_sheets: Vec<SpriteSheet>
}

#[derive(Deserialize)]
struct SpriteSheet {
    image: String,
    #[serde(default = "default_mode")]
    mode: String,
    sprites: Vec<Sprite>
}

#[allow(unused)]
#[derive(Deserialize)]
struct Palette {
    name: String,
    colors: Vec<(u8, u8, u8)>
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
    alias: Option<String>
}

#[derive(Debug, Copy, Clone)]
struct Tile<'a> {
    index: u32,
    mode: &'a str,
    palette_number: u8,
}

fn default_sprite_size() -> u32 { 16 }
fn default_holeydma() -> bool { true }
fn default_mode() -> String { "160A".to_string() }

fn main() -> Result <(), Box<dyn error::Error>>
{
    let mut width = 0;
    let mut height = 0;
    let mut tilewidth: u32 = 8;
    let mut tileheight: u32 = 8;
    let args = Args::parse();
    let xml = fs::read_to_string(args.filename).expect("Unable to read input file");
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
        if n.node_type() == NodeType::Element {
            if n.local_name() == "layer" {
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
                            let array = csv.split(',').map(|x| { u32::from_str(x).unwrap() } ).collect::<Vec::<_>>();
                            if array.len() == width * height {
                                if let Some(yaml_file) = args.yaml {
                                    let contents = fs::read_to_string(yaml_file).expect("Unable to read input file");
                                    let t: AllSprites = serde_yaml::from_str(&contents)?;
                                    // OK, we have the array, we have the tiles specs. Let's match them
                                    // Let's scan all the tiles to make sure all this makes sense
                                    if t.sprite_sheets.len() != 1 {
                                        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Only one sprite sheet (tiles) should be provided")));
                                    }
                                    let tiles_sheet = &t.sprite_sheets[0];
                                    let img = image::open(&tiles_sheet.image).expect(&format!("Can't open image {}", tiles_sheet.image));
                                    let image_width = img.width();
                                    let mut index = 0;
                                    let defmode = tiles_sheet.mode.as_str();
                                    let mut tiles = HashMap::<u32, Tile>::new();
                                    let mut aliases = HashMap::<&str, u32>::new();
                                    for tile in &tiles_sheet.sprites {
                                        // Do not consider sprites
                                        let mode = if let Some(m) = &tile.mode { m.as_str() } else { defmode };
                                        let tile_bytes = match mode {
                                            "160A" => tilewidth / 8,
                                            "160B" => tilewidth / 4,
                                            "320A" => tilewidth / 8,
                                            "320B" => tilewidth / 4,
                                            "320C" => tilewidth / 4,
                                            _ => unreachable!()
                                        };
                                        if !tile.holeydma {
                                            aliases.insert(&tile.name.as_str(), index);
                                            let y = tile.top / tileheight;
                                            let x = tile.left / tilewidth;
                                            let ix = 1 + x + y * image_width / tilewidth;
                                            let nbtiles = tile.width / tilewidth;
                                            let palette_number = if let Some(p) = tile.palette_number { p } else { 0 }; 
                                            let mut idx = if let Some(alias) = &tile.alias {
                                                if let Some(i) = aliases.get(alias.as_str()) {
                                                    *i
                                                } else {
                                                    return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Bad alias")));
                                                }
                                            } else { index };
                                            for i in 0..nbtiles {
                                                tiles.insert(ix + i, Tile {
                                                    index: idx, mode, palette_number
                                                });
                                                index += tile_bytes;
                                                idx += tile_bytes;
                                            }
                                        }
                                    }
                                    //println!("Tiles : {:?}", tiles);

                                    // Generate the C code for the the sparse tiles
                                    // to be used with sparse_tiling.h header
                                    for y in 0..height {
                                        // For each line, find the tilesets 
                                        let mut tilesets = Vec::<(u32, Vec::<Tile>)>::new();
                                        let mut tileset = Vec::<Tile>::new();
                                        let mut startx = 0;
                                        for x in 0..width {
                                            let cell = array[y * height + x];
                                            if cell == 0 {
                                                if !tileset.is_empty() {
                                                    tilesets.push((startx, tileset));
                                                    tileset = Vec::<Tile>::new();
                                                }
                                            } else {
                                                if let Some(tx) = tileset.last() {
                                                    // Is the cell compatible with the tileset in construction ?
                                                    if let Some(t) = tiles.get(&cell) {
                                                        if t.mode == tx.mode && t.palette_number == tx.palette_number {
                                                            // Yes, let's add it the current tileset
                                                            if tileset.len() >= (256 - 160)/ 8 {
                                                                tilesets.push((startx, tileset));
                                                                tileset = Vec::<Tile>::new();
                                                                startx = x as u32;
                                                            }
                                                            tileset.push(t.clone());
                                                        } else {
                                                            tilesets.push((startx, tileset));
                                                            tileset = Vec::<Tile>::new();
                                                            tileset.push(t.clone());
                                                            startx = x as u32;
                                                        }
                                                    } else {
                                                        //return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Wrong tilesheet. Index unknown")));
                                                        // It's not in the tilesheet. Consider it as 0 (empty)
                                                        if !tileset.is_empty() {
                                                            tilesets.push((startx, tileset));
                                                            tileset = Vec::<Tile>::new();
                                                        }
                                                    }
                                                } else {
                                                    if let Some(t) = tiles.get(&cell) {
                                                        tileset.push(t.clone());
                                                        startx = x as u32;
                                                    } else {
                                                        //return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Wrong tilesheet. Index unknown")));
                                                        // It's not in the tilesheet. Consider it as 0 (empty)
                                                        if !tileset.is_empty() {
                                                            tilesets.push((startx, tileset));
                                                            tileset = Vec::<Tile>::new();
                                                        }
                                                    }
                                                }
                                            }           
                                        }
                                        // Write the last tileset
                                        if !tileset.is_empty() {
                                            tilesets.push((startx, tileset));
                                        }
                                        // Write this line data
                                        {
                                            let mut c = 0;
                                            let mut w = Vec::new();
                                            for s in &tilesets {
                                                let mut tn = Vec::new();
                                                for t in &s.1 {
                                                    // We assume we are in 2 bytes mode
                                                    let nb = match t.mode {
                                                        "160A" | "320A" => tilewidth / 16,
                                                        _ => tilewidth / 8,
                                                    };
                                                    for i in 0..nb {
                                                        tn.push(t.index + i * 2);
                                                    }
                                                }
                                                w.push(tn.len());
                                                print!("const char tilemap_{}_{}[{}] = {{", y, c, tn.len());
                                                for i in 0..tn.len() - 1 {
                                                    print!("{}, ", tn[i]);
                                                }
                                                println!("{}}};", tn[tn.len() - 1]);
                                                c += 1;
                                            }
                                            c = 0;
                                            print!("const char tilemap_{}_data[] = {{", y);
                                            for s in &tilesets {
                                                let ttype = s.1.first().unwrap();
                                                let write_mode = match ttype.mode {
                                                    "160A" | "320A" | "320D" => 0x60,
                                                    _ => 0xE0,
                                                };
                                                print!("{}, {}, tilemap_{y}_{c}, 0x{:02x}, tilemap_{y}_{c} >> 8, ({} << 5) | ((-{}) & 0x1f), {}, ", 
                                                    s.0 + s.1.len() as u32 - 1, s.0, write_mode, ttype.palette_number, w[c], (10 + 3 + 9 * w[c]) / 2);
                                                c += 1;
                                            }
                                            println!("96, 0xff}};");
                                        }
                                    }
                                    print!("const char tilemap_data[{}] = {{", height * 2);
                                    for y in 0..height - 1 {
                                        print!("tilemap_{}_data & 0xff, tilemap_{}_data >> 8, ", y, y);
                                    }
                                    println!("tilemap_{}_data & 0xff, tilemap_{}_data >> 8}};\n", height - 1, height - 1);
                                    println!("#define TILING_HEIGHT {}", height);
                                    println!("#define TILING_WIDTH {}", width);
                                    println!("#include \"sparse_tiling.h\"\n");

                                } else {
                                    print!("const char tilemap[{}] = {{", 
                                        if args.boundaries {
                                            (width + 1) * height + 1
                                        } else {
                                            width * height
                                        });
                                    for i in 0..height {
                                        if args.boundaries { 
                                            print!("\n\t0xff, "); 
                                        } else {
                                            print!("\n\t");
                                        }
                                        for j in 0..width {
                                            print!("{}{} ", (array[i * height + j] - 1) * 2,
                                            if args.boundaries || i != height - 1 || j != width - 1 {","} else {""}
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
                                return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Bad data format. Unexpected table size.")));
                            }
                        }
                    }
                }
            }
        }
    }
    Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Unexpected data provided.")))
}
