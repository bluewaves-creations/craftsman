# Stack: Rust

Loaded when implementing Rust code. The conventions file still binds.

Assumed floor: clippy `pedantic` + `nursery` warn at the workspace root, `-D warnings` in CI, rustfmt, cargo-mutants on changed code. Nothing below restates what they enforce. The failure mode to guard against is "C++ in disguise": clone-everything ownership, `unwrap` chains, stringly-typed errors.

## Errors — thiserror for libraries, anyhow for binaries

Library crates define one error enum per crate with `thiserror`, `#[from]` for wrapped sources, and messages that state what failed without capitalization or trailing punctuation (they compose into chains). Binary crates use `anyhow::Result` and attach `.with_context()` at every I/O boundary so the chain reads as a narrative. Never let `anyhow` leak into a library's public API, and never make callers string-match an error.

`unwrap`/`expect` are for tests and provably-impossible states only — and an `expect` on an invariant states the invariant, not the failure ("mutex poisoned only if a panic already occurred").

```rust
// Library crate
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("key `{0}` not found")]
    NotFound(String),
    #[error("storage io")]
    Io(#[from] std::io::Error),
}

// Binary crate
let cfg = std::fs::read_to_string(&path)
    .with_context(|| format!("reading config {}", path.display()))?;
```

## Public API shape — caller control, ownership at the edges

Follow the API Guidelines checklist; the two rules agents drift on:

- **C-CALLER-CONTROL**: take the least-demanding type that works — `&str` not `String`, `&[T]` not `Vec<T>`, `impl Into<…>`/`impl AsRef<Path>` where genericity pays. Take owned values only when you will store them; then take them by value so the caller decides whether to clone.
- **Return owned** (`String`, `Vec<T>`, or an iterator) from constructors and transformations; return borrows only when the lifetime relationship is the point of the API.

```rust
// Bad: forces an allocation on every caller, then clones anyway
pub fn normalize(name: String) -> String { name.trim().to_lowercase() }

// Good: borrow in, owned out — the caller keeps its String
pub fn normalize(name: &str) -> String { name.trim().to_lowercase() }
```

Derive the standard traits eagerly on public types (`Debug`, `Clone`, `PartialEq`, and `Default` where a zero value is meaningful); a public type missing `Debug` is a bug report waiting.

## Workspace lint inheritance

Lint policy lives once, in the root `Cargo.toml` `[workspace.lints]` table; every member declares `lints.workspace = true`. Never configure lints per-crate — policy that can drift per-member will. Local deviations are narrow, scoped `#[allow(...)]` with a reason comment, never a crate-wide attribute.

## Unsafe policy

`unsafe_code = "deny"` at the workspace root. A crate that genuinely needs it (FFI, proven hot path) overrides locally and pays the full tax: `#![deny(unsafe_op_in_unsafe_fn)]`, a `// SAFETY:` comment on every unsafe block stating which invariants hold and why, and a `# Safety` doc section on every unsafe fn stating what the caller must uphold. An unsafe block whose safety argument you cannot write down is not sound — redesign.

```rust
// SAFETY: `idx` was bounds-checked against `self.len` on the line above,
// and `self.ptr` is valid for `self.len` elements by the struct invariant.
let item = unsafe { &*self.ptr.add(idx) };
```

## Async discipline — no lock across await

Never hold a `std::sync` guard across an `.await` (deadlock risk: the task parks while owning the lock). Structure instead: take the lock, extract or mutate, drop the guard, then await. Reach for `tokio::sync::Mutex` only when the critical section itself must await — it is the exception, not the default.

```rust
// Bad: guard alive across await — can deadlock the runtime
let mut cache = self.cache.lock().unwrap();
let value = fetch_remote(key).await?;
cache.insert(key, value);

// Good: await outside the critical section
let value = fetch_remote(key).await?;
self.cache.lock().expect("cache lock").insert(key, value);
```

Prefer channels and message-passing ownership over shared-state locking when a component's state has one logical owner.
