namespace Atto.Contracts.V1;

model EnvelopeBase {
  version: 1;
  @minLength(1)
  @maxLength(128)
  message_id: string;
  @minLength(1)
  @maxLength(128)
  source_actor: string;
}

model MessageEvent extends EnvelopeBase {
  event: "message";
  @minLength(1)
  @maxLength(128)
  target_actor: string;
  @maxLength(65536)
  payload_utf8: string;
}

model FailureEvent extends EnvelopeBase {
  event: "failure";
  @pattern("^[A-Z][A-Z0-9_]{0,63}$")
  error_code: string;
}

model ShutdownEvent extends EnvelopeBase {
  event: "shutdown";
}

union ActorEnvelope {
  message: MessageEvent,
  failure: FailureEvent,
  shutdown: ShutdownEvent,
}
