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
