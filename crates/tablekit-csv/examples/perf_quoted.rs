//! Warmed quoted-text allocation/performance probe, using invented text only.
//! `perf_quoted ROWS`: compare Valgrind total allocations for ROWS=0 and ROWS=10000.
use std::time::Instant;
use tablekit::{DataType, Field};
use tablekit_csv::{CsvFormat, CsvOptions};

#[inline(never)]
fn quoted_rows(
    format: &CsvFormat,
    out: &mut Vec<u8>,
    n: usize,
) -> Result<(), tablekit::TableError> {
    // HOT PATH: begin.
    for _ in 0..n {
        out.clear();
        let mut row = format.begin_row(out);
        row.text_with(|text| {
            text.push_str("é,\"invented\"\r\n");
            text.digits(u64::MAX, 21);
        });
        row.finish()?;
        std::hint::black_box(&out);
    }
    // HOT PATH: end.
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let n: usize = std::env::args().nth(1).map_or(Ok(10_000), |s| s.parse())?;
    let format = CsvFormat::new(
        vec![Field {
            name: "text".into(),
            data_type: DataType::Text,
        }],
        CsvOptions::default(),
    )?;
    let mut out = Vec::with_capacity(4096);
    quoted_rows(&format, &mut out, 1)?;
    let pointer = out.as_ptr();
    let capacity = out.capacity();
    let start = Instant::now();
    quoted_rows(&format, &mut out, n)?;
    let elapsed = start.elapsed();
    if pointer != out.as_ptr() || capacity != out.capacity() {
        return Err("warmed output grew".into());
    }
    println!(
        "rows={n} ns={} capacity={capacity} output={:?}",
        elapsed.as_nanos(),
        String::from_utf8(out)?
    );
    Ok(())
}
