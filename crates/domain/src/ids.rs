/// Identifies a player (and therefore their submarine).
///
/// Rust note: this is the *newtype* pattern, a struct wrapping one value.
/// It costs nothing at runtime but stops us accidentally passing a torpedo id
/// where a player id was expected: the compiler treats them as different types.
///
/// `#[derive(...)]` asks the compiler to write common trait implementations:
/// `Copy` (cheap to duplicate, like an integer), `Eq`/`Hash`/`Ord` (usable as a
/// map key), `Debug` (printable with `{:?}`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlayerId(pub u32);

/// Identifies a torpedo in flight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TorpedoId(pub u32);

impl std::fmt::Display for PlayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sub {}", self.0)
    }
}
