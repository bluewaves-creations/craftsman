# The mutate-gate e2e target (cli/tests/mutate.rs): a tiny function whose
# boundary mutants (< -> <=, > -> >=) the deliberately weak test cannot
# kill — guaranteed survivors, deterministic red verdict at min-score 100.


def clamp(n, lo, hi):
    if n < lo:
        return lo
    if n > hi:
        return hi
    return n
