// FLS §10.3: Associated Constants.
//
// An associated constant is a named compile-time value that belongs to a type
// or trait. Inherent impls may declare associated constants; trait impls may
// provide concrete values for required associated constants declared in the
// trait body.
//
// FLS §10.3 AMBIGUOUS: The spec does not provide standalone code examples for
// associated constants in §10.3; this fixture is derived from the semantic
// descriptions in §10.3 and from cross-references to §7.1 (Constant Items)
// and §11 (Implementations).
//
// Access syntax: `TypeName::CONST_NAME` (a two-segment path expression, §6.3).
// FLS §10.3: "Every use of an associated constant is replaced with its value."
// This is identical in semantics to top-level `const` substitution (§7.1:10).

// FLS §11.2: An inherent impl may declare associated constants.
// The constant value is substituted at every use site.
struct Config;
impl Config {
    // FLS §10.3: Associated constant declaration with an initializer.
    const MAX_SIZE: i32 = 256;
    const MIN_SIZE: i32 = 1;
}

// FLS §13, §10.3: A trait may declare required associated constants
// (no default value). Implementors must provide a value.
trait HasId {
    const ID: i32;
}

struct Alpha;
impl HasId for Alpha {
    const ID: i32 = 1;
}

struct Beta;
impl HasId for Beta {
    const ID: i32 = 2;
}

// FLS §10.3: Demonstrating access via `TypeName::CONST_NAME`.
// The two-segment path resolves to the constant's value at compile time.
fn use_assoc_consts() -> i32 {
    let max = Config::MAX_SIZE;
    let min = Config::MIN_SIZE;
    let a_id = Alpha::ID;
    let b_id = Beta::ID;
    max - min + a_id + b_id
}

fn main() -> i32 {
    use_assoc_consts()
}
