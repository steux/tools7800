use binrw::{BinRead, BinReaderExt};
use clap::Parser;
use std::fs;
use std::io::Read;

#[derive(BinRead, Debug)]
struct RmtVectors {
    _vect1: u16,
    vect2_start: u16,
    _vect3: u16,
}

#[derive(BinRead, Debug)]
struct RmtHeader {
    _magic: [u8; 4], // RMT4 or RMT8
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
    _pointer: u16,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// RMT or SAP file
    input: String,
    song_name: Option<String>,
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();
    let buffer = fs::read(args.input)?;

    // Load the RMT4 header
    let mut found = None;
    let magic = &vec!['R' as u8, 'M' as u8, 'T' as u8];
    for (i, w) in buffer.windows(3).enumerate() {
        if w == magic {
            found = Some(i);
        }
    }
    let rmtstart = found.expect("Missing RMT header");
    let mut cursor = std::io::Cursor::new(buffer);
    cursor.set_position(rmtstart as u64);
    let header: RmtHeader = cursor.read_le().unwrap();

    // If the RMT4 file doesn't have the load vector, than calculate the RMT load location.
    // This isn't normally a problem, but rmt files prepped for 7800 may have the vectors
    // stripped, since it doesn't use them.

    // Let's check if we have the vectors or not
    let memstart = if rmtstart < 6 {
        // We don't have the load vectors.
        header.pointer_to_instrument_pointers - 0x10
    } else {
        cursor.set_position((rmtstart - 6) as u64);
        let vectors: RmtVectors = cursor.read_le().unwrap();
        vectors.vect2_start
    };

    let song = args.song_name.unwrap_or("RMTSTART".into());
    print!(
        "const char {song}[] = {{'{}', '{}', '{}', '{}', 
    {},  // Tracklen
    {}, // Song speed
    {}, // Player freq
    {}, // Format version number
    {song} + 0x{:04x}, {song} + 0x{:04x} >> 8, // Pointer to instrument pointers
    {song} + 0x{:04x}, {song} + 0x{:04x} >> 8, // Pointer to track pointers, lo 
    {song} + 0x{:04x}, {song} + 0x{:04x} >> 8, // Pointer to track pointers, hi
    {song} + 0x{:04x}, {song} + 0x{:04x} >> 8, // Pointer to song",
        header._magic[0] as char,
        header._magic[1] as char,
        header._magic[2] as char,
        header._magic[3] as char,
        header.track_len,
        header.song_speed,
        header.player_freq,
        header.format_version_number,
        header.pointer_to_instrument_pointers - memstart,
        header.pointer_to_instrument_pointers - memstart,
        header.pointer_to_track_pointers_lo - memstart,
        header.pointer_to_track_pointers_lo - memstart,
        header.pointer_to_track_pointers_hi - memstart,
        header.pointer_to_track_pointers_hi - memstart,
        header.pointer_to_song - memstart,
        header.pointer_to_song - memstart
    );

    // Output the instrument pointers
    {
        print!(
            "
    // Instrument pointer table, hi"
        );
        let startrange = header.pointer_to_instrument_pointers - memstart;
        let endrange = header.pointer_to_track_pointers_lo - memstart;
        cursor.set_position((rmtstart + (startrange as usize)) as u64);
        for _ in 0..(endrange - startrange) / 2 {
            let pointer: u16 = cursor.read_le().unwrap();
            if pointer != 0 {
                print!(
                    "
    {song} + 0x{:04x}, {song} + 0x{:04x} >> 8,",
                    pointer - memstart,
                    pointer - memstart
                )
            } else {
                print!(
                    "
    0, 0, "
                );
            }
        }
    }

    // Output the track pointers, which are split into 2 separate LO and HI byte tables
    let startrange = header.pointer_to_track_pointers_lo - memstart;
    let endrange = header.pointer_to_track_pointers_hi - memstart;
    {
        print!(
            "
    // Track pointer table, lo"
        );
        for c in 0..endrange - startrange {
            let mut lo = [0u8; 1];
            let mut hi = [0u8; 1];
            cursor.set_position((rmtstart + (startrange as usize) + (c as usize)) as u64);
            cursor.read_exact(&mut lo).unwrap();
            cursor.set_position(
                (rmtstart
                    + (startrange as usize)
                    + ((endrange - startrange) as usize)
                    + (c as usize)) as u64,
            );
            cursor.read_exact(&mut hi).unwrap();
            if lo[0] == 0 && hi[0] == 0 {
                print!(
                    "
    0, "
                );
            } else {
                let pointer = (lo[0] as u16) + ((hi[0] as u16) << 8);
                print!(
                    "
    {song} + 0x{:04x},",
                    pointer - memstart
                );
            }
        }
        print!(
            "
    // Track pointer table, hi"
        );
        for c in 0..endrange - startrange {
            let mut lo = [0u8; 1];
            let mut hi = [0u8; 1];
            cursor.set_position((rmtstart + (startrange as usize) + (c as usize)) as u64);
            cursor.read_exact(&mut lo).unwrap();
            cursor.set_position(
                (rmtstart
                    + (startrange as usize)
                    + ((endrange - startrange) as usize)
                    + (c as usize)) as u64,
            );
            cursor.read_exact(&mut hi).unwrap();
            if lo[0] == 0 && hi[0] == 0 {
                print!(
                    "
    0, "
                );
            } else {
                let pointer = (lo[0] as u16) + ((hi[0] as u16) << 8);
                print!(
                    "
    {song} + 0x{:04x} >> 8,",
                    pointer - memstart
                );
            }
        }
    }

    // Track+instruments data
    {
        print!(
            "
    // Track+Instrument data"
        );
        let startrange = endrange + (endrange - startrange);
        let endrange = header.pointer_to_song - memstart;
        cursor.set_position((rmtstart + (startrange as usize)) as u64);
        for c in 0..endrange - startrange {
            if c % 16 == 0 {
                print!(
                    "
    "
                );
            }
            let mut byte = [0u8; 1];
            cursor.read_exact(&mut byte).unwrap();
            print!("0x{:02x}, ", byte[0]);
        }
    }

    // Song data
    {
        print!(
            "
    // Song data"
        );
        let startrange = header.pointer_to_song - memstart;
        cursor.set_position((rmtstart + (startrange as usize)) as u64);
        let mut i = 0;
        let mut c = 0;
        loop {
            if i % 16 == 0 {
                print!(
                    "
    "
                );
            }
            let mut byte = [0u8; 1];
            match cursor.read_exact(&mut byte) {
                Ok(()) => {
                    if byte[0] == 0xfe && (c & 3) == 0 {
                        cursor.read_exact(&mut byte).unwrap();
                        let pointer: u16 = cursor.read_le().unwrap();
                        if i % 16 != 0 {
                            print!(
                                "
    0xfe, 0x00, {song} + 0x{:04x}, {song} + 0x{:04x} >> 8,",
                                pointer - memstart,
                                pointer - memstart
                            );
                        } else {
                            print!(
                                "0xfe, 0x00, {song} + 0x{:04x}, {song} + 0x{:04x} >> 8,",
                                pointer - memstart,
                                pointer - memstart
                            );
                        }
                        i = 0;
                        c += 4;
                    } else {
                        print!("0x{:02x}, ", byte[0]);
                        i += 1;
                        c += 1;
                    }
                }
                _ => break,
            }
        }
    }

    println!("0}};");
    Ok(())
}
