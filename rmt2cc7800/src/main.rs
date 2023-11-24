use std::fs;
use clap::Parser;
use binrw::{BinRead, BinReaderExt};

#[derive(BinRead, Debug)]
struct RmtHeader {
    vect1: u16,
    vect2_start: u16,
    vect3: u16,
    
    magic: [u8;4], // RMT4
    track_len: u8,
    song_speed: u8,
    player_freq: u8,
    format_version_number: u8,
    pointer_to_instrument_pointers: u16,
    pointer_to_track_pointers_lo: u16,
    pointer_to_track_pointers_hi: u16,
    pointer_to_song: u16,
}

#[derive(BinRead)]
struct Upoint {
    pointer: u16
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// RMT or SAP file 
    input: String,
}

fn main() -> std::io::Result<()>
{
    let args = Args::parse();
    let buffer = fs::read(args.input)?;
   
    let mut found = None;
    let magic = &vec!['R' as u8, 'M' as u8, 'T' as u8, '4' as u8];
    for (i, w) in buffer.windows(4).enumerate() {
        if w == magic {
            found = Some(i);
        }
    }
    let mut cursor = std::io::Cursor::new(buffer);
    cursor.set_position((found.unwrap() - 6) as u64); 
    let header: RmtHeader = cursor.read_le().unwrap();
    println!("Header: {:?}", header);
    Ok(())
}
