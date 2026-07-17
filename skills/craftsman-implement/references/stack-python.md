# Stack: Python

Loaded when implementing Python code. The conventions file still binds.

Assumed floor: uv (packaging, venv, lock), ruff (lint + format), pyright strict — all configured in `pyproject.toml`, src-layout. Nothing below restates what they enforce. The packaging wars are over: never emit `requirements.txt`, `setup.py`, or poetry configuration in new code; a dependency change is `uv add`, nothing else.

## Pydantic at the boundary, dataclasses inside

Pydantic models exist to parse untrusted input — HTTP payloads, config files, LLM output, anything crossing a process edge. Parse once at the boundary, convert immediately to plain `@dataclass(frozen=True)` (or plain classes) for domain logic. Never thread `BaseModel` through the core: it couples every function to a serialization framework and makes construction cost invisible.

```python
# Bad: domain logic imports the validation framework
def apply_discount(order: OrderModel) -> OrderModel: ...   # BaseModel everywhere

# Good: pydantic parses at the edge, the domain sees a frozen dataclass
@dataclass(frozen=True)
class Order:
    lines: tuple[Line, ...]
    total: Decimal

def parse_order(raw: bytes) -> Order:          # boundary — the only pydantic site
    return OrderPayload.model_validate_json(raw).to_domain()
```

## Protocol-typed ports

Every real boundary — I/O, external service, clock, randomness — is a `typing.Protocol` the domain depends on. Concrete adapters implement it implicitly; scenarios run against hand-written fakes that satisfy the same Protocol. This is what makes the spec executable without `unittest.mock` — reserve mock/patch for legacy edges you cannot yet seam.

```python
class PaymentGateway(Protocol):
    def charge(self, amount: Decimal, token: str) -> ChargeResult: ...

class FakeGateway:                              # lives next to the specs
    def __init__(self) -> None: self.charges: list[Decimal] = []
    def charge(self, amount: Decimal, token: str) -> ChargeResult:
        self.charges.append(amount); return ChargeResult(ok=True)
```

Keep ports at real boundaries only — a Protocol wrapping a pure in-process function is ceremony, not a seam.

## Exceptions — narrow and typed

Define one exception hierarchy per package, rooted in a single base, with leaf classes carrying structured context as attributes (not formatted into the message). Catch the narrowest class that lets you act; a bare `except Exception` is legal only at a top-level boundary that logs and translates. Never signal domain outcomes with exceptions the caller must string-match.

```python
# Bad: stringly-typed, caller must parse the message
raise RuntimeError(f"charge failed for {order_id}: card declined")

# Good: typed, contextual, catchable precisely
class PaymentError(AppError): ...
class CardDeclined(PaymentError):
    def __init__(self, order_id: OrderId, code: str) -> None:
        super().__init__(f"card declined for {order_id}")
        self.order_id, self.code = order_id, code
```

Expected, recoverable outcomes (lookup miss, validation verdict) return values — `X | None` or a small result dataclass — not exceptions.

## Module shape

One module, one reason to change. No God-modules (`utils.py`, `helpers.py`, `common.py`) — the place agent-generated duplication goes to die; name modules for the capability they serve and keep shared code where its single consumer cluster lives. Public surface is explicit: `__all__` in package `__init__.py`, underscore-prefix everything else. Prefer module-level functions over classes with no state; a class with one method and no fields is a function wearing a costume.

Modern syntax throughout: builtin generics, `X | None`, `Self`, `type` statements — pyright strict already assumes 3.13+; write to it, no `from __future__` imports.
