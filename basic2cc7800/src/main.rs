use std::fs;
use std::str::FromStr;
use clap::Parser as ClapParser;
use pest::Parser;

extern crate pest;
#[macro_use]
extern crate pest_derive;

#[derive(Parser)]
#[grammar = "basic2cc7800.pest"]
struct BasicParser;

/// Atari 7800 tool that generates C array code from a 7800basic file (data section in .bas file) 
#[derive(ClapParser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// 7800Basic (.bas) input file
    filename: String
}

fn main() -> Result <(), std::io::Error> {
    let args = Args::parse();
    let content = fs::read_to_string(args.filename).expect("Unable to read input file");
    let parsed = BasicParser::parse(Rule::file, &content);
    match parsed {
        Ok(p) => {
            // Parse the file
            let mut arrays = Vec::new();
            for px in p {
                match px.as_rule() {
                    Rule::file => {
                        let p = px.into_inner();
                        for px in p {
                            match px.as_rule() {
                                Rule::data => {
                                    let mut p = px.into_inner();
                                    let varname = p.next().unwrap().as_str();
                                    let mut data = Vec::new();
                                    for i in p {
                                        let pp = i.into_inner();
                                        for j in pp {
                                            let mut ppp = j.into_inner();
                                            let pppx = ppp.next().unwrap();
                                            match pppx.as_rule() {
                                                Rule::int => {
                                                    data.push(u32::from_str(pppx.as_str()).unwrap());
                                                },
                                                Rule::hexa => {
                                                    data.push(u32::from_str_radix(pppx.as_str().split_at(1).1, 16).unwrap());
                                                },
                                                _ => unreachable!()
                                            };
                                        }
                                    }
                                    arrays.push((varname, data));
                                },
                                _ => ()

                            };
                        }
                    },
                    _ => unreachable!()
                };
            }
            // Write the Result
            print!("const char *array_name[{}] = {{\n\t", arrays.len());
            for (i, x) in arrays.iter().enumerate() {
                print!("\"{}\"", x.0);
                if i != arrays.len() - 1 {
                    if ((i + 1) % 8) == 0 {
                        print!(",\n\t");
                    } else {
                        print!(", ");
                    }
                }
            }
            println!("\n}};\n");
            for x in &arrays {
                print!("const char {}[{}] = {{\n\t", x.0, x.1.len());
                for (j, y) in x.1.iter().enumerate() {
                    print!("0x{:02x}", y);
                    if j != x.1.len() - 1 {
                        if ((j + 1) % 16) == 0 {
                            print!(",\n\t");
                        } else {
                            print!(", ");
                        }
                    }
                }
                println!("\n}};\n");
            }
            print!("const char *array_data[{}] = {{\n\t", arrays.len());
            for (i, x) in arrays.iter().enumerate() {
                print!("{}", x.0);
                if i != arrays.len() - 1 {
                    if ((i + 1) % 8) == 0 {
                        print!(",\n\t");
                    } else {
                        print!(", ");
                    }
                }
            }
            println!("\n}};\n");

            Ok(())
        },
        Err(e) => {
            println!("Error: {}", e);
            Err(std::io::Error::from(std::io::ErrorKind::InvalidInput))
        }
    }
}

