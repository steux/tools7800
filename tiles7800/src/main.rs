use std::fs;
use std::error;
use clap::Parser;
use std::str::FromStr;

use xml_dom::level2::{Node, NodeType};

/// Atari 7800 tool that generates C code for tiles map generated using tiled editor (tmx files) 
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// YAML input file
    filename: String
}

fn main() -> Result <(), Box<dyn error::Error>>
{
    let mut width = 0;
    let mut height = 0;
    let args = Args::parse();
    let xml = fs::read_to_string(args.filename).expect("Unable to read input file");
    let dom = xml_dom::parser::read_xml(&xml)?;
    let root = dom.first_child().unwrap();
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
                                print!("const char tilemap[{}] = {{", (width + 1) * height + 1);
                                for i in 0..height {
                                    print!("\n\t0xff,");
                                    for j in 0..width {
                                        print!(" {},", (array[i * height + j] - 1) * 2);
                                    }
                                }
                                println!("\n\t0xff}};");
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
