// Reference model ONLY; not yet run through a pinned Dafny verification gate.
// A per-instance agreement lemma is not a proof that schemas are equivalent.
module ZordeParity {
  datatype Comparison = Equivalent | Different | Refused

  function Compare(authoredAccepts: bool, generatedAccepts: bool, supported: bool): Comparison {
    if !supported then Refused
    else if authoredAccepts == generatedAccepts then Equivalent
    else Different
  }

  lemma NeverEquateDisagreement(a: bool, b: bool, supported: bool)
    ensures a != b ==> Compare(a, b, supported) != Equivalent
  {}

  lemma NeverEquateUnsupported(a: bool, b: bool)
    ensures Compare(a, b, false) == Refused
  {}

  lemma EquivalentRequiresSupportedAgreement(a: bool, b: bool, supported: bool)
    ensures Compare(a, b, supported) == Equivalent ==> supported && a == b
  {}
}
