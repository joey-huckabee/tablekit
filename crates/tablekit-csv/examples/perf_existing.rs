//! Reproducible synthetic A/B for existing CSV APIs. Copy this same file into a main
//! worktree before comparing: `perf_existing MODE ROWS ROUNDS [OUTPUT]`.
//! Modes: batch, writer, row. ROUNDS=0 profiles one warm-up render only.
use std::{hint::black_box, time::Instant};
use tablekit::{Batch, Column, Value, Values};
use tablekit_csv::{CsvFormat, CsvOptions, CsvWriter};

type Row = [Value<'static>; 7];

fn input(n: usize) -> Result<(Batch, Vec<Row>), Box<dyn std::error::Error>> {
    let mut rows = Vec::with_capacity(n);
    let (mut flags, mut signed, mut unsigned, mut doubles, mut singles, mut words, mut texts) = (
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    for i in 0..n {
        let number = u32::try_from(i)?;
        let word = u16::try_from(i % 65_536)?;
        let flag = i % 2 == 0;
        let signed_value = -i64::from(number);
        let unsigned_value = u64::from(number) * 2_654_435_761;
        let double = f64::from(number) / 1_000_000.0;
        let single = f32::from(word) / 8.0;
        let text = ["plain", "a,b", "say \"hi\"", "é\r\n"][i % 4];
        rows.push([
            Value::Bool(flag),
            Value::I64(signed_value),
            Value::U64(unsigned_value),
            Value::F64(double),
            Value::F32(single),
            Value::U16(word),
            Value::Text(text),
        ]);
        flags.push(flag);
        signed.push(signed_value);
        unsigned.push(unsigned_value);
        doubles.push(double);
        singles.push(single);
        words.push(word);
        texts.push(text);
    }
    let batch = Batch::new(vec![
        Column::new("flag", Values::Bool(flags)),
        Column::new("signed", Values::I64(signed)),
        Column::new("unsigned", Values::U64(unsigned)),
        Column::new("double", Values::F64(doubles)),
        Column::new("single", Values::F32(singles)),
        Column::new("word", Values::U16(words)),
        Column::new("text", Values::Text(texts.into())),
    ])?;
    Ok((batch, rows))
}

// Keep a visible function boundary for Callgrind's --toggle-collect='*perf_existing*render*'.
#[inline(never)]
fn render(
    mode: &str,
    batch: &Batch,
    rows: &[Row],
    out: &mut Vec<u8>,
) -> Result<(), tablekit::TableError> {
    out.clear();
    match mode {
        "batch" => {
            tablekit_csv::write_to(&mut *out, batch, &CsvOptions::default())?;
        }
        "writer" => {
            let mut writer = CsvWriter::new(&mut *out, batch.schema(), CsvOptions::default())?;
            writer.write(batch)?;
            writer.finish()?;
        }
        "row" => {
            let mut format = CsvFormat::new(batch.schema(), CsvOptions::default())?;
            format.start(out);
            // HOT PATH: begin.
            for row in rows {
                format.row(out, row)?;
            }
            // HOT PATH: end.
        }
        _ => {
            return Err(tablekit::TableError::Format(
                "expected batch, writer or row".into(),
            ));
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let mode = args.get(1).map_or("row", String::as_str);
    let n: usize = args.get(2).map_or(Ok(200_000), |s| s.parse())?;
    let rounds: usize = args.get(3).map_or(Ok(1), |s| s.parse())?;
    let (batch, rows) = input(n)?;
    let mut out = Vec::with_capacity(n * 128 + 128);
    render(mode, &batch, &rows, &mut out)?;
    for _ in 0..rounds {
        let start = Instant::now();
        render(
            black_box(mode),
            black_box(&batch),
            black_box(&rows),
            &mut out,
        )?;
        println!("ns={}", start.elapsed().as_nanos());
    }
    let checksum = out.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    });
    println!(
        "mode={mode} rows={n} bytes={} fnv1a={checksum:016x}",
        out.len()
    );
    if let Some(path) = args.get(4) {
        std::fs::write(path, &out)?;
    }
    Ok(())
}
