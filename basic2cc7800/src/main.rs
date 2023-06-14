use std::fs;
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

fn main() -> Result <(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let content = fs::read_to_string(args.filename).expect("Unable to read input file");
    let parsed = BasicParser::parse(Rule::file, &content);
    if let Err(e) = parsed {
        println!("Error: {}", e);
    } else {
        println!("Parsed: {:?}", parsed);
    }
    Ok(())
}

