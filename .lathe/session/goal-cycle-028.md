# Goal — Cycle 028

**Stakeholder:** Compiler Contributor

**What to change:** In the `ExprKind::Match` handler in `src/lower.rs` (the scalar
path in `lower_expr`, around line 12495), add an early-detection check for tuple
scrutinees **before** the call to `lower_expr(scrutinee, &scrut_ty)` on line 12497.
If `scrutinee.kind` is `ExprKind::Tuple`, return a `LowerError::Unsupported` message
that names FLS §6.18 (match expressions) and §6.10 (tuple expressions), and states
that the fix belongs in the `ExprKind::Match` handler — specifically before the
`lower_expr(scrutinee, ...)` call, following the existing `enum_base_slot` /
`struct_base_slot` detection patterns above it.

Also update the comment on the generic `ExprKind::Tuple` fallback (lines 18640–18647)
to clarify that the tuple-as-scrutinee case is now caught earlier (in the match handler),
and that the fallback handles only the remaining contexts: tuple as tail expression or
as a standalone value context.

**Why this stakeholder:** Cycles 027 = Cache-Line, 026 = Lead, 025 = Spec, 024 = Compiler
Contributor. All four were served in the last four cycles. Compiler Contributor is next in
rotation. The journey immediately surfaced a concrete misdirection.

**Why now:** At step 4 of the Compiler Contributor's journey — "trace the error back
through the source" — running `galvanic tests/fixtures/fls_6_18_match_expressions.rs`
produces:

```
error: lower failed in 'match_tuple': not yet supported: tuple expression must be bound
to a `let` variable at this milestone
```

Grepping for this message sends the contributor to `lower.rs:18645` — inside the generic
`ExprKind::Tuple` fallback of `lower_expr`. The comment at lines 18640–18644 describes
the tuple fallback's purpose as: "when a tuple literal appears as a tail expression or
in a context where it's used as a value directly." No mention of match scrutinees. The
contributor is now in the wrong place.

The actual fix for tuple-scrutinee match support belongs in the `ExprKind::Match`
handler — specifically before the `lower_expr(scrutinee, &scrut_ty)` call on line 12497.
That handler already has two detection patterns for compound scrutinees:
`enum_base_slot` (lines 12469–12480) and `struct_base_slot` (lines 12484–12495). A
tuple scrutinee detection pattern would follow the same structure. But the contributor
sent to `ExprKind::Tuple` at line 18645 has no way to find this without reading the
call chain backwards — or re-reading the entire match handler.

The lib.rs invariant says: "When it fails, the error must name the function, FLS
section, and specific construct." (lib.rs:55–56) The current error names the function
(`match_tuple`) but not the FLS section (§6.18, §6.10) and not the specific construct
(tuple expression as match scrutinee).

**The class of fix:** When a generic expression fallback fires because an expression
appears in a context that the calling handler doesn't yet support, the error should be
produced by the calling handler — not the generic fallback. The generic `ExprKind::Tuple`
fallback fires in a match scrutinee context and produces an error that points to the
tuple handler instead of the match handler. Adding early detection to the match handler
moves the error to the right location, where:
- The FLS sections (§6.18 and §6.10) are both visible in the surrounding comments
- The existing `enum_base_slot` / `struct_base_slot` patterns are directly above it
  as models for adding the new case
- The contributor is exactly where the fix must go

This eliminates the entire category of "generic fallback misdirects contributor to the
wrong module or function" for the tuple-as-scrutinee case, and models the pattern for
future compound-scrutinee additions.

**What success looks like:**

Running `galvanic` on a `match (x, y)` program produces:

```
error: lower failed in 'match_tuple': not yet supported: tuple scrutinee in match
expression (FLS §6.18, §6.10) — add Pat::Tuple arm handling in ExprKind::Match,
before the lower_expr(scrutinee, ...) call
```

The contributor searches for this error → finds it in the `ExprKind::Match` handler,
directly above the `lower_expr(scrutinee, &scrut_ty)` call, beside the `enum_base_slot`
and `struct_base_slot` patterns they need to follow. They know exactly where to add
the new case without reading any call chains. That is the Compiler Contributor's
emotional signal: **confidence**.

**Constraint:** No functional change — the tuple scrutinee match is still unsupported.
This is error placement and message quality only. Do not add tuple scrutinee
support in this cycle; the goal is to make the existing "not yet supported" error
appear at the right location with the right information.

**Lived experience note:** I became the Compiler Contributor. I ran `cargo build`
(clean) and `cargo test` (2115 pass). I found the failing feature by running all §6
expression fixtures — `fls_6_18_match_expressions.rs` was the only one with a partial
failure. The error: "lower failed in 'match_tuple': not yet supported: tuple expression
must be bound to a `let` variable at this milestone."

I opened `lib.rs` — excellent. The pipeline diagram is clear, the Adding a new language
feature checklist is useful, and step 4 explicitly says the error must name the function,
FLS section, and specific construct. My error had the function but nothing else.

I grepped for the error string → `lower.rs:18645`. I was inside `ExprKind::Tuple` in
`lower_expr`. The comment above said: "This path is reached when a tuple literal appears
as a tail expression or in a context where it's used as a value directly (rare; most tuple
usage goes through the `let` path above)." I stared at this. My program had `match (x, y)`,
not a tuple tail expression. I was in the wrong place.

I searched for `ExprKind::Match` to find the match handler. Found it at line 12436. I
saw `enum_base_slot` (lines 12469–12480) and `struct_base_slot` (12484–12495) — two
compound scrutinee detection blocks already in place. Then line 12497: `lower_expr(scrutinee, &scrut_ty)` — called unconditionally. There's no tuple scrutinee detection. That's
where the error should have been caught. Instead it fell through to the generic fallback
at line 18645, 6,000 lines later, with a message that described a different context.

The hollowest moment: I found the right place myself only because I read the entire match
handler. A contributor who grepped for the error and stopped at line 18645 — exactly as
the contributor's journey describes — would have added code to `ExprKind::Tuple` instead
of `ExprKind::Match`. The architecture is right; the error's provenance is wrong.
