"""Writes f64.txt and f32.txt: values and the text NumPy gives them, the text pandas' to_csv
writes. Each line is the value's bits in hex, a space, and NumPy's `astype(str)` of it.

    python make.py      (NumPy 2; run from this directory)

The values are random bit patterns, random values across every magnitude, and the values on
each side of the cutoffs between positional and scientific notation.
"""

import numpy as np

rng = np.random.default_rng(20261018)


def cases(dtype, uint, cutoffs, count):
    bits = rng.integers(0, np.iinfo(uint).max, size=count, dtype=uint, endpoint=True)
    values = bits.view(dtype)
    values = values[np.isfinite(values)]
    spread = (rng.uniform(1, 10, count) * 10.0 ** rng.integers(-40, 40, count)).astype(dtype)
    spread = spread[np.isfinite(spread)]
    signs = np.where(rng.integers(0, 2, len(spread)) == 1, -1, 1).astype(dtype)
    edges = []
    for c in cutoffs:
        v = dtype(c)
        for x in (v, np.nextafter(v, dtype(0)), np.nextafter(v, dtype(np.inf))):
            edges += [x, -x]
    edges += [dtype(0), -dtype(0), dtype(1), dtype(0.1), dtype(1) / dtype(3)]
    edges += [np.finfo(dtype).max, np.finfo(dtype).tiny, np.finfo(dtype).smallest_subnormal]
    return np.concatenate([values, spread * signs, np.array(edges, dtype=dtype)])


for name, dtype, uint, cutoffs in [
    ("f64", np.float64, np.uint64, [1e-4, 1e16]),
    ("f32", np.float32, np.uint32, [1e-4, 1e6]),
]:
    values = cases(dtype, uint, cutoffs, 4000)
    width = np.dtype(uint).itemsize * 2
    with open(f"{name}.txt", "w", encoding="ascii", newline="\n") as f:
        for v, s in zip(values.view(uint), values.astype(str)):
            f.write(f"{int(v):0{width}x} {s}\n")
