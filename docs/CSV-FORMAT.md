# CSV format

What `tablekit-csv` writes. By default it is exactly what pandas writes with
`DataFrame.to_csv(path, index=False)` on Windows (pandas 3, NumPy 2), because that is what
the users already open. The expected files in `testdata/expected/csv/` were written by pandas
itself (`make.py` there), and tablekit's tests compare against them byte for byte.

## Contents

- [The file](#the-file)
- [Values](#values)
- [Numbers with a decimal point](#numbers-with-a-decimal-point)
- [Options](#options)
- [Where tablekit differs from pandas](#where-tablekit-differs-from-pandas)

## The file

| | Written as |
|---|---|
| Encoding | UTF-8, no byte-order mark (option `encoding`: `utf-8-sig` adds one) |
| First line | The column names |
| Then | One line per row, the columns in order |
| Separator | `,` |
| Line ending | CRLF, `\r\n`, also after the last line (option `line_ending`) |
| Quoting | Only a field holding `,`, `"`, a carriage return or a line feed is put in double quotes, each `"` inside doubled: `"a, b"`, `"say ""hi"""`. Python's `csv.QUOTE_MINIMAL`, as pandas uses. A line break inside a quoted field is written as it is. |
| A row of one empty field | `""`, so the line isn't mistaken for a blank one (Python's `csv` module does the same) |

## Values

| Column type | Written as | Examples |
|---|---|---|
| Boolean | `True` or `False` | `True` |
| Integers, any width, signed or unsigned | Decimal digits, with `-` if negative | `-128`, `18446744073709551615` |
| `f32`, `f64` | See below | `0.1`, `1e-05`, `123.0` |
| Infinity | `inf`, `-inf` | |
| NaN | Empty, like a missing value (option `na_rep`) | |
| Text | As it is, quoted if needed | `On`, `"a, b"` |
| A missing value, any type | Empty (option `na_rep`) | |

## Numbers with a decimal point

Each value is written with the fewest digits that read back as exactly the same number. When
two equally short choices are equally close, the one ending in an even digit is chosen. This
is how Python's `repr` and NumPy choose digits.

The layout is NumPy's, which pandas uses:

| | Positional when | Otherwise |
|---|---|---|
| `f64` | `0.0001 <= abs(value) < 1e16`, or zero | scientific |
| `f32` | `0.0001 <= abs(value) < 1e6`, or zero | scientific |

- **Positional** always has a decimal point: `123.0`, `0.0001234`, `-0.0`,
  `1500000000000000.0`.
- **Scientific** has a point only with more than one digit, then `e`, a sign and at least two
  exponent digits: `1e-05`, `1e+16`, `1.2345679e+08`, `5e-324`.
- An `f32` is written with its own shortest digits (`0.1`, not `0.10000000149011612`), and
  switches to scientific from 1e6 up (`1e+06`, `1.6777216e+07`). NumPy does this for
  `float32`.

`testdata/floats/` holds about 16,000 values with the text NumPy gives each: random bit
patterns, values across every magnitude, and the values either side of each cutoff. A test
requires tablekit to write every one identically.

## Options

`CsvOptions::default()` is pandas' default. Each option has a pandas equivalent:

| Option | Default | pandas `to_csv` |
|---|---|---|
| `na_rep` | empty | `na_rep` |
| `line_ending` | `CrLf` | `lineterminator`: `"\r\n"` on Windows, `"\n"` elsewhere |
| `encoding` | `utf-8` | `encoding`: `"utf-8"`, or `"utf-8-sig"` for a byte-order mark, which Excel needs to show non-ASCII text |

The default line ending is CRLF on every system, so the same table gives the same bytes
everywhere. pandas uses the system's line ending; on Linux, choose `Lf` to match it.

Encoding names are Python's, matched as Python matches them: case, `-` and `_` aside
(`"UTF-8"`, `"utf_8_sig"`). An encoding tablekit does not write is refused, with the list of
those it does; more can be added without changing the interface.

## Where tablekit differs from pandas

| Case | pandas | tablekit | Why |
|---|---|---|---|
| A whole-number column with missing values | Converts the column to floats: `5.0`, empty | Keeps whole numbers: `5`, empty | The value is a whole number; `5.0` suggests otherwise. |
| An `f32` column handed to pandas as Python floats, as FATMOPs-Binary's `to_pandas()` does when some values are missing | `float64` digits: `0.10000000149011612` | `f32` digits: `0.1` | The value is an `f32`. |
| A boolean column with missing values | `True`, `False`, empty | The same | No difference. |
| Line ending on Linux | `\n` | `\r\n` unless `Lf` is chosen | The same bytes on every system. |

Each case has a test in `crates/tablekit-csv/tests/expected.rs`.
