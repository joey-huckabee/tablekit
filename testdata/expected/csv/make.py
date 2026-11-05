"""Writes the expected CSV files with pandas itself: what tablekit-csv must write byte for byte.

    python make.py      (pandas 3 and NumPy 2; run from this directory)

`tests/expected.rs` in tablekit-csv builds the same tables and compares. Integer columns have
no missing values here: pandas turns such a column into floats (`5.0`), and tablekit keeps
integers; docs/CSV-FORMAT.md lists that difference.
"""

import numpy as np
import pandas as pd

table = pd.DataFrame(
    {
        "time": np.array([0.5, 1e-05, 1e16, 123.0]),
        "flag": np.array([True, False, True, False]),
        "small": np.array([-128, 0, 127, 5], dtype=np.int8),
        "count": np.array([18446744073709551615, 0, 1, 2], dtype=np.uint64),
        "offset": np.array([-9223372036854775808, 9223372036854775807, -1, 0], dtype=np.int64),
        "level": np.array([0.1, 1e-4, 3.4e38, np.nan], dtype=np.float32),
        "edge": np.array([np.inf, -np.inf, 0.0, -0.0]),
        "note": ["plain", "a, b", 'say "hi"', "two\nlines"],
        "Mode": ["On", None, "Off", ""],
        "Pos.X": np.array([1, -2, 3, -4], dtype=np.int16),
    }
)

table.to_csv("table.csv", index=False, lineterminator="\r\n")
table.to_csv("table-utf-8-sig.csv", index=False, lineterminator="\r\n", encoding="utf-8-sig")
table.to_csv("table-na.csv", index=False, lineterminator="\r\n", na_rep="NA")
table.to_csv("table-lf.csv", index=False, lineterminator="\n")

# One column: an empty or missing value is written "" so the line is not blank.
pd.DataFrame({"x": ["a", "", None]}).to_csv("one-column.csv", index=False, lineterminator="\r\n")

# Cell formats (CsvOptions.cell_formats): pandas writes the columns after Python's own
# formatting has turned them into text -- format(v, "04X"), f"{v:.6f}" -- which is the
# referee for CellFormat::HexUpper, HexLower and Fixed. None stays missing (written empty).
# NaN is missing too, and infinity is written as for the default format.
def cells(values, spec):
    return [None if v is None else format(v, spec) for v in values]


def fixed(values, decimals):
    out = []
    for v in values:
        if v is None or v != v:
            out.append(None)
        elif v in (float("inf"), float("-inf")):
            out.append("inf" if v > 0 else "-inf")
        else:
            out.append(f"{v:.{decimals}f}")
    return out


level = [float(np.float32(v)) for v in (0.1, 1e-4, 3.4e38, -2.5, 0.0, 1.5, 7.0, 1e-3, 2.0, 0.5)]
pd.DataFrame(
    {
        "word": cells([0x0400, 0, 0xFFFF, 0x2F, None, 1, 0xABCD, 0x10, 0x8000, 0x7FFF], "04X"),
        "byte": cells([255, 0, 10, 1, 128, None, 15, 16, 254, 3], "02X"),
        "addr": cells([0xDEADBEEF, 1, 0, 0xABC, 0xFFFFFFFF, 0x10000, 7, None, 0x80000000, 2], "08X"),
        "big": cells(
            [2**64 - 1, 0, 0x1234, 1 << 40, 0xFFFF, 0x10000, None, 5, 0xABCDEF, 1], "04x"
        ),
        "delta": fixed(
            [0.0, 1.5e-05, 123.456789, -0.0, 1.0000005, 5e-07, -1e-09, 1e20, float("nan"),
             float("inf")],
            6,
        ),
        "level": fixed(level, 3),
        "n": np.array([1, -2, 3, -4, 5, -6, 7, -8, 9, -10], dtype=np.int32),
    }
).to_csv("table-formats.csv", index=False, lineterminator="\r\n")
