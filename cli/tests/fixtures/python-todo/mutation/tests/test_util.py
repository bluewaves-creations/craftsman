# Deliberately WEAK test (mutate e2e): exercises only the pass-through
# path, so boundary mutants in clamp survive while return-value mutants die.
from todo_util import clamp


def test_clamp_passes_a_mid_value_through():
    assert clamp(5, 0, 10) == 5
