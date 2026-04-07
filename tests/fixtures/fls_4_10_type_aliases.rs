// FLS §4.10: Type aliases.
//
// "A type alias defines a new name for an existing type."
//
// A type alias does not introduce a new type — it is purely a compile-time
// name substitution. Every use of the alias in a type position is equivalent
// to writing the aliased type directly.
//
// FLS §4.10 AMBIGUOUS: The spec does not specify whether a type alias can
// appear as a cast target (e.g., `x as MyInt`). Galvanic treats the aliased
// type name as transparent in annotations but not in cast targets (deferred).
//
// FLS §4.10 NOTE: The spec provides no worked code examples for type aliases;
// the programs below are derived from the section's semantic description.

/// FLS §4.10: Simple scalar alias — `type MyInt = i32`.
///
/// A function return type and a let-binding annotation can both use the alias.
/// The alias resolves to `i32` during lowering; no extra code is emitted.
type MyInt = i32;

/// FLS §4.10: Alias for a boolean type.
type Flag = bool;

/// FLS §4.10: Alias for an unsigned integer type.
type Count = u32;

/// FLS §4.10: Functions using type aliases in parameter and return positions.
///
/// `double(n: MyInt) -> MyInt` is equivalent to `double(n: i32) -> i32`.
fn double(n: MyInt) -> MyInt {
    n * 2
}

/// FLS §4.10: Conditional returning an aliased type.
fn clamp(x: MyInt) -> MyInt {
    if x < 0 { 0 } else if x > 100 { 100 } else { x }
}

/// FLS §4.10: Entry point using the alias in a let binding.
fn main() -> i32 {
    let result: MyInt = double(21);
    result - 42
}
