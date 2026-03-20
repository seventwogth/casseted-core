# 0010 Final Reconstruction Stage Cleanup

Date: 2026-03-20

Stage:
cleanup and refinement of the final reconstruction/output stage inside the existing limited multi-pass still-image architecture

Status:
implemented in the repository working tree

Agent commit status:
no git commit was created by the agent during this stage. Future entries in this directory should list any agent-created commits explicitly when they exist.

## Why this stage was needed

After the luma and chroma branches were deepened, the final pass had become the least clear part of the chain:

- the architecture itself was still good and did not need another pass
- but the final shader was starting to read like one fused bucket for dropout, contamination, Y/C leakage, and decode
- that made the reconstruction semantics harder to reason about than the now more intentional luma/chroma branches

The useful move at this point was therefore cleanup inside the existing pass boundary, not a new subsystem.

## Problems found

1. Reconstruction semantics were implicit:
   the shader sampled luma/chroma, applied dropout, injected contamination, and decoded to RGB, but the code did not clearly expose those as separate internal responsibilities.
2. Final-stage naming still leaned too hard on `noise`:
   pipeline-side resolved terms were named as if the whole stage were just noise/crosstalk, even though the pass now owns a broader reconstruction/output contract.
3. Dropout and leakage interacted a bit too opaquely:
   stronger dropout concealment could still look like the final stage was immediately reintroducing color/leakage semantics instead of keeping the span washed and structurally repaired.

## What changed

- `crates/casseted-pipeline/src/stages.rs` now resolves the final-stage terms as:
  `luma_contamination_amount`,
  `chroma_contamination_amount`,
  `y_c_leakage`,
  plus the existing dropout controls
- `shaders/passes/still_reconstruction_output.wgsl` now makes the internal sequence explicit:
  1. sample the branch-resolved reconstruction signal
  2. apply restrained dropout conditioning in `Y/C` space
  3. add reconstruction contamination
  4. compose display `YUV`
  5. decode to RGB
- stronger dropout concealment now reduces neighboring-line chroma support slightly instead of treating concealed chroma as equally trustworthy at all dropout strengths
- the Y/C leakage term now also backs off slightly inside stronger dropout concealment, so washed dropout spans stay cleaner and the final stage reads less like a catch-all
- the fixed four-pass runtime, uniform packing footprint, compiled runtime layer, preview guardrails, and crate boundaries stayed intact

## Why this integration path was chosen

- the current architecture already gives luma and chroma their own refined branches, so the next cleanup point naturally belongs at reconstruction
- the pass was overloaded more by semantics than by raw size, so the right fix was internal staging clarity rather than another physical pass
- the preview/runtime contract was already compact enough; this stage could become clearer by naming and sequencing its responsibilities better instead of widening the API

## Still-image v1 approximations used

- dropout remains a still-image local concealment approximation, not a temporal compensator
- contamination is still deterministic hash-based contamination shaped to read like analog dirt rather than a uniform overlay
- Y/C leakage is still a compact decode-side approximation, not a fuller decoder/crosstalk model

## Remaining debts

- the final pass still intentionally keeps dropout conditioning, contamination, and decode in one physical pass; that remains acceptable for the current phase, but it is still the place to revisit first if deeper future work proves a harder split is necessary
- contamination still comes from preview controls named `noise`, because the public preview API remains intentionally compact for still-image v1
- this stage is still not a temporal dropout recovery model, a carrier-accurate chroma decoder, or a broader output-device simulation

## Verification completed for this stage

- `cargo check --locked`
- `cargo test --workspace --locked`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Related touched areas

- `crates/casseted-pipeline`
- `crates/casseted-shaderlib`
- `shaders/passes/still_reconstruction_output.wgsl`
- `docs/architecture/`
- `docs/math/`
- `docs/agent-log/`
