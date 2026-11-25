// Trusted-loader reference state machine ONLY: approved evidence is modeled
// as a constructor supplied by an external verified gate. This file does not
// prove that a caller cannot forge that constructor in the actual runtime,
// that two arbitrary JSON Schemas are equivalent, or that compiled Oreslang
// implements these transitions.
module ZordeRegistry {
  datatype Registry = Empty | Published(
    generation: nat, authoredDigest: string, generatedDigest: string,
    contractIrDigest: string
  )
  datatype Evidence = Denied | Reviewed(
    authoredDigest: string, generatedDigest: string, contractIrDigest: string
  )

  function Generation(state: Registry): nat {
    match state
      case Empty => 0
      case Published(g, _, _, _) => g
  }

  function WellFormed(evidence: Evidence): bool {
    match evidence
      case Denied => false
      case Reviewed(a, g, ir) => a != "" && g != "" && ir != ""
  }

  // A generation replacement is indivisible at the *reference model* level.
  // Real actor isolation, immutable snapshot publication and memory barriers
  // require separate implementation/refinement proofs.
  function Activate(state: Registry, evidence: Evidence, next: nat): Registry {
    if next <= Generation(state) || !WellFormed(evidence) then state
    else
      match evidence
        case Denied => state
        case Reviewed(a, g, ir) => Published(next, a, g, ir)
  }

  lemma DenialPreservesRegistry(state: Registry, next: nat)
    ensures Activate(state, Denied, next) == state
  {}

  lemma NoGenerationRollback(state: Registry, evidence: Evidence, next: nat)
    ensures Generation(Activate(state, evidence, next)) >= Generation(state)
  {}

  lemma RepeatedGenerationCannotReplace(state: Registry, evidence: Evidence)
    ensures Activate(state, evidence, Generation(state)) == state
  {}

  lemma SuccessfulReplacementBindsReviewedDigests(
    state: Registry, a: string, g: string, ir: string, next: nat
  )
    requires a != "" && g != "" && ir != ""
    requires next > Generation(state)
    ensures Activate(state, Reviewed(a, g, ir), next)
      == Published(next, a, g, ir)
  {}
}
