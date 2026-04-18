# Brand

## Identity

Precise, research-honest, technically dry — named after a thing that exists to be consumed. "Galvanic" refers to galvanic corrosion protection: a sacrificial anode corrodes so that the structure it protects doesn't. The README makes this explicit: "It's a sacrificial anode — it exists to find ambiguities in the spec and to explore what 'dumb but cache-aware' codegen can do. Nobody needs to use this. Value comes from what we learn." (README.md line 15.)

Not self-deprecating as a posture. Self-aware as a fact. The project knows what it is and isn't bothered by what it isn't.

---

## How We Speak

**When we say no:**
Name the thing we don't support, cite the FLS section, don't apologize. "not yet supported: nested tuple pattern not yet supported (FLS §5.10.3, §8.1)" (lower.rs:8322). The project refuses features without drama — each refusal is specific enough to navigate from. A "no" without an FLS citation is off-brand.

**When we fail:**
Emit what succeeded. Never discard partial work silently. From main.rs:88–109, the comment reads: "the goal of partial output: a partial success should not be entirely silent." When some functions lowered and some didn't, the compiler prints all the errors *and* emits assembly for the functions that worked, labeled honestly: "emitted {} (partial — some functions failed)". The partial label is explicit; the work is never quietly thrown away.

**When we explain:**
Tight technical prose that includes *why*, not just *what*. Comments name the ratio, the reasoning, the tradeoff. From codegen.rs:217–219: "each static occupies 8 bytes (.quad). Eight statics fill one 64-byte data cache line. We align each static to 8 bytes (.align 3) to prevent two statics from sharing a single alignment unit." No hand-waving — the number is there, the derivation is there.

**When we onboard a new user:**
Blunt, not cold. README.md line 17: "Do not use this to compile anything you care about." Not softened. But the next line gives them the actual value proposition clearly. The usage line is flat: "usage: galvanic <source.rs> [-o <output>]" (main.rs:38). No cheerful welcome, no feature list — just the shape of the command.

**When something compiles successfully:**
Plain statement of what happened. "galvanic: emitted {path}" (main.rs:174). No congratulations, no color. The assembly file is the output; the message just names where it went. Success is a fact, not a moment.

---

## The Thing We'd Never Do

Emit a vague error. Galvanic never produces a bare "not yet supported" without an FLS section cite — the CI audit enforces this via `lower_source_all_unsupported_strings_cite_fls`. (Established in the test suite and referenced in champion.md.) A "not yet supported" without a section number and affected construct is not an error message — it's a dead end. The project treats dead ends as bugs.

---

## Signals to Preserve

1. **Lowercase imperative commit messages with FLS section citations.** Pattern: `fix: §8.2 named block expression as statement now infers tail type`. The section number is part of the subject, not the body. The message is action-first, no capitalization. (git log, consistent across 30+ cycles.)

2. **"not yet supported (FLS §X.Y, §Z)" as the standard refusal form.** Specific section, specific construct, parenthetical citation. Lower.rs:124 defines the format; individual error strings fill in the detail. When someone adds a new "not yet supported" message, this is the template.

3. **Partial output is always emitted with an honest label.** The label names the limitation — "inspection-only — no fn main; this assembly has no entry point" (main.rs:132), "partial — some functions failed" (main.rs:171). The label is an obligation, not optional copy.
