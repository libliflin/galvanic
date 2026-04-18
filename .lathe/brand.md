# Brand

## Identity

Precise, researcher-voiced, disarmingly honest about its own limits. Galvanic speaks the way a careful scientist writes lab notes — it names the section number, states the constraint, and tells you what it chose and why. It does not perform confidence it doesn't have. The README calls the project "a sacrificial anode" and tells you not to use it on anything you care about (README lines 14–15: "Nobody needs to use this. Value comes from what we learn."). That's the register: exact, unpretentious, oriented toward what can be learned rather than what can be shipped.

## How we speak

**When we say no:** We name the boundary exactly — which construct, which context, which spec section defines the limit. From `lower.rs:79`, the error pattern is `"not yet supported: {msg}"`. The message names what's missing, not what broke. Example at `lower.rs:2677–2681`: `"only identifier, wildcard, and nested tuple patterns are supported inside tuple parameter patterns"` — that's a refusal that doubles as a map: this is what *is* supported.

**When we fail:** We show the full error landscape in one pass, not just the first failure. From `main.rs:88–89`: "Print every per-function error so the researcher sees the full error landscape in a single run." Partial success is never silent — `main.rs:105` states this as a rule: "a partial success should not be entirely silent." The error prefix is flat: `error: {msg}`. No stack trace unless the caller needs it.

**When we explain:** We cite the spec. Comments in `lower.rs` and `parser.rs` look like `// FLS §6.1.2:37–45: Non-const code emits runtime instructions` and `// Cache-line note: N × 4-byte str per self spill`. Explanation is tethered to the source of truth (the FLS) or the constraint that shaped the decision (the cache line). We don't explain what the code does; we explain why the spec requires it or why the hardware constrains it.

**When we onboard a new user:** We tell them what this is not before we tell them what it is. README structure: "What this is" / "What this is not." The second section (lines 13–15) comes immediately after the first and is unhedged: "This is not a production compiler." The last line: "Do not use this to compile anything you care about." A new user who reads past that has been told the truth.

**When we succeed:** Quiet and terse. `main.rs:54`: `"galvanic: compiling {filename}"`. `main.rs:139`: `"galvanic: emitted {out_path}"`. No exclamation. Partial success gets one parenthetical: `"(partial — some functions failed)"`. The bar for celebration is the research question advancing, not the command completing.

## The thing we'd never do

We'd never hide a missing feature behind a vague error. When the catch-all in `lower_expr` fires, it includes the variant name and a comment directing contributors to grep for it (`lower.rs:18817–18822`): `"{} expression in non-const context (runtime codegen not yet implemented)"`. The error message is also a contributor affordance. Vague errors ("something went wrong," "unsupported operation") would corrupt the research value — a researcher can't cite "it didn't work."

## Signals to preserve

- **FLS section numbers in everything** — commits, code comments, error messages, `fls-ambiguities.md` entries. The spec citation is the citation of authority; drop it and the research claim loses its grounding.
- **`not yet supported: {specific thing}`** — the error pattern that distinguishes "this is a hard limit" from "we haven't gotten here yet." The word "yet" matters; it keeps the map honest.
- **Structured commit messages: `type: what for {stakeholder} {signal} (cycle N)`** — the commit history is a research log. Each commit names who it serves and what it moves. This pattern keeps the log legible across hundreds of commits.
