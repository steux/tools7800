use clap::Parser;

#[repr(C, packed)]
struct Rmtheader {
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

#[repr(C, packed)]
struct Upoint {
    pointer: u16
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// RMT or SAP file 
    #[arg(short, long)]
    input: String,

    #[arg(short, long, default_value = "rmt.c")]
    output: String,
}

fn main() {
    let args = Args::parse();
}
