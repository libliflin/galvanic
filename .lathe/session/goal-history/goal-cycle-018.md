# Goal — Cycle 018

**Stakeholder:** Lead Researcher

**What to change:** Add FLS provenance to the control-flow IR instructions (`Label`, `Branch`, `CondBranch`) so that the assembly comments they emit cite the correct FLS section for the language construct that generated them — §6.15.2 for `loop {}`, §6.15.3 for `while`, §6.15.6 for `break`, §6.15.7 for `continue`, §6.17 for `if`/`if let` — not uniformly §6.17 for all control flow.

**Why this stakeholder:** The Lead Researcher was last served at cycle 014 — four cycles ago. The Spec Researcher was served last cycle (017). The Lead Researcher is the most under-served.

**Why now:** At step 3 of the Lead Researcher's journey — opening the `.s` file to verify the codegen looks right — the assembly for loop constructs is annotated with the wrong FLS section. Running `galvanic tests/fixtures/fls_6_15_loop_expressions.rs` and inspecting the emitted `.s` file reveals:

```
.L0:                              // FLS §6.17: branch target
    cbz     x3, .L1               // FLS §6.17: branch if false
    b       .L0                   // FLS §6.17: branch to end
```

Every label, conditional branch, and unconditional branch in the while loop cites §6.17 (if/if let expressions). The while loop belongs to §6.15.3. The infinite loop back-edge belongs to §6.15.2. Break expressions belong to §6.15.6.

The IR docstrings confirm the root cause: `Label`, `Branch`, and `CondBranch` in `ir.rs` were introduced for if-expression control flow (milestone 13) and their FLS citations were never updated when loop lowering reused the same IR nodes. The `codegen.rs` match arms emit the hardcoded `§6.17` string unconditionally for all three instruction kinds.

The assembly is structurally correct — the loops execute correctly. The FLS traceability is wrong. For a research compiler where assembly comments ARE the research artifact, this is a serious gap: a researcher verifying §6.15 implementation reads §6.17 annotations throughout.

**The specific moment:** I ran `cargo run -- tests/fixtures/fls_6_15_loop_expressions.rs`, opened the emitted `.s` file, and searched for §6.15 in the comments. Zero matches. The fixture is named for §6.15 and contains only loop expressions, but the control-flow instructions are uniformly annotated §6.17. The IR docstrings reinforce the mislabeling: `Branch` is documented "FLS §6.17: After the then-branch of an if expression…" — a description that is only accurate when the branch comes from an if expression.

**The class of fix:** Control-flow IR instructions need FLS provenance that travels from the lowering site through to the assembly comment. The builder should determine the mechanism — whether that means adding an `origin` or `fls_section` field to the IR branch instructions, introducing separate IR node variants for loop-vs-if control flow, or another approach. The invariant to achieve: the assembly comment for a `Branch`/`CondBranch`/`Label` must cite the section of the construct in `lower.rs` that emitted it.

**Constraint:** The IR docstrings for `Label`, `Branch`, and `CondBranch` in `ir.rs` should also be updated to note that these instructions are used for multiple FLS sections (§6.15.x and §6.17), not only §6.17 — fixing the documentation at the point of definition.

**Lived experience note:** I became the Lead Researcher. I ran `cargo test` — clean (2084 pass). I picked the loop expressions fixture (the most structurally interesting in the set, since loops were one of the more recent milestones). I ran `cargo run -- tests/fixtures/fls_6_15_loop_expressions.rs`. Clean compile. I opened the `.s` file and grepped for §6.15: no results. I grepped for all FLS citations. The control-flow lines — branch targets, conditional branches, back-edges — all said §6.17. I checked the fixture file: it's titled `FLS §6.15 — Loop expressions` and covers §6.15.2, §6.15.3, §6.15.6, §6.15.7. Zero overlap with §6.17. The hollowest moment: realizing the assembly compiles and runs correctly, but as a *research artifact* it actively claims to implement the wrong spec section for every piece of loop control flow. A researcher tracing §6.15 implementation through the assembly would find no evidence of it.
