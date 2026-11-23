use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};

fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let input = File::open(&args[1])?;
    let dim: usize = args[2].parse().unwrap();
    let mut output = BufWriter::new(File::create(&args[3])?);
    for line in BufReader::new(input).lines() {
        let line = line?;
        let mut fields = line.split_whitespace();
        fields.next();
        for _ in 0..dim {
            let value: f32 = fields.next().expect("short line").parse().expect("bad float");
            output.write_all(&value.to_le_bytes())?;
        }
    }
    Ok(())
}
