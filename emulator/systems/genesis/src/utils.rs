use std::fs;

use moa_core::Error;

const SMD_HEADER: usize = 512;
const SMD_BLOCK_SIZE: usize = 16384;
const SMD_MAGIC: &[u8] = &[0xAA, 0xBB];

pub fn smd_to_bin(input: Vec<u8>) -> Result<Vec<u8>, Error> {
    let mut output = vec![0; input.len() - SMD_HEADER];

    if &input[8..10] != SMD_MAGIC {
        return Err(Error::new(format!("smd: magic not found: {:?}", &input[8..10])));
    }

    let calculated_blocks = (input.len() - SMD_HEADER) / SMD_BLOCK_SIZE;

    for block in 0..calculated_blocks {
        let offset = block * SMD_BLOCK_SIZE;
        let odds = &input[SMD_HEADER + offset..];
        let evens = &input[SMD_HEADER + offset + (SMD_BLOCK_SIZE / 2)..];

        for i in 0..(SMD_BLOCK_SIZE / 2) {
            output[offset + i * 2] = evens[i];
            output[offset + i * 2 + 1] = odds[i];
        }
    }

    Ok(output)
}

pub fn load_rom_file(filename: &str) -> Result<Vec<u8>, Error> {
    let mut contents = fs::read(filename).map_err(|_| Error::new(format!("Error reading contents of {}", filename)))?;

    if filename.ends_with(".smd") {
        contents = smd_to_bin(contents)?;
    }

    Ok(contents)
}
