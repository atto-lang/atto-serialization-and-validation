//! Host-only reference policy, not a guest codec or authority boundary.
//! Run: rustc --edition=2021 --test tools/actor_data_codec_policy_v1.rs -o /tmp/atto-codec-policy && /tmp/atto-codec-policy
//! Labels are configuration metadata, not on-wire IDs. No payload parsing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec { MessagePack, Protobuf, Cbor, FlatBuffers }
impl Codec {
    fn parse(label: &str) -> Option<Self> {
        match label { "messagepack" => Some(Self::MessagePack), "protobuf" => Some(Self::Protobuf),
            "cbor" => Some(Self::Cbor), "flatbuffers" => Some(Self::FlatBuffers), _ => None }
    }
    fn bit(self) -> u8 {
        match self { Self::MessagePack => 1, Self::Protobuf => 2, Self::Cbor => 4, Self::FlatBuffers => 8 }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reject { UnknownCodec, InvalidMask, Denied, Unsupported, InvalidBinding, Mismatch }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding<'a> { codec: Codec, version: u32, schema: &'a str, shape: [u8; 32] }
fn valid_schema(schema: &str) -> bool {
    !schema.is_empty() && schema.len() <= 256 && schema.bytes().all(|b| (33..=126).contains(&b))
}
impl<'a> Binding<'a> {
    /// Only trusted registration may call this; verified support is not inferred here.
    pub fn select(requested: Option<&str>, allowed: u8, supported: u8,
                  version: u32, schema: &'a str, shape: [u8;32]) -> Result<Self, Reject> {
        if (allowed | supported) & !15 != 0 { return Err(Reject::InvalidMask); }
        let codec = Codec::parse(requested.unwrap_or("messagepack")).ok_or(Reject::UnknownCodec)?;
        if allowed & codec.bit() == 0 { return Err(Reject::Denied); }
        if supported & codec.bit() == 0 { return Err(Reject::Unsupported); }
        if version == 0 || !valid_schema(schema) { return Err(Reject::InvalidBinding); }
        Ok(Self { codec, version, schema, shape })
    }
    /// Call after authenticated metadata admission; this is not an authentication check.
    /// Compare incoming metadata to the fixed binding; never use it to select a decoder.
    pub fn matches(&self, label: &str, version: u32, schema: &str, shape: &[u8;32]) -> Result<(), Reject> {
        if label.len() > 11 || !valid_schema(schema) || Codec::parse(label) != Some(self.codec)
            || version != self.version || schema != self.schema || shape != &self.shape {
            return Err(Reject::Mismatch);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const H: [u8;32] = [7;32];
    #[test] fn default_and_explicit() {
        let b = Binding::select(None, 15, 1, 2, "order.v1", H).unwrap();
        assert_eq!(b.matches("messagepack",2,"order.v1",&H), Ok(()));
        for (label, mask) in [("protobuf",2),("cbor",4),("flatbuffers",8)] {
            let b = Binding::select(Some(label),15,mask,1,"order.v1",H).unwrap();
            assert_eq!(b.matches(label,1,"order.v1",&H),Ok(()));
        }
    }
    #[test] fn no_fallback() {
        assert_eq!(Binding::select(None,2,2,1,"s",H),Err(Reject::Denied));
        assert_eq!(Binding::select(Some("protobuf"),15,1,1,"s",H),Err(Reject::Unsupported));
        assert_eq!(Binding::select(Some("CBOR"),15,15,1,"s",H),Err(Reject::UnknownCodec));
        assert_eq!(Binding::select(Some(""),15,15,1,"s",H),Err(Reject::UnknownCodec));
        assert_eq!(Binding::select(None,16,1,1,"s",H),Err(Reject::InvalidMask));
    }
    #[test] fn metadata_mismatch() {
        let b=Binding::select(None,1,1,2,"s",H).unwrap();
        for (label,v,s,h) in [("protobuf",2,"s",H),("messagepack",1,"s",H),
            ("messagepack",2,"t",H),("messagepack",2,"s",[8;32]),("MESSAGEPACK",2,"s",H)] {
            assert_eq!(b.matches(label,v,s,&h),Err(Reject::Mismatch));
        }
    }
    #[test] fn bounded_schema() {
        for s in ["", "a b", "é", "\n"] {
            assert_eq!(Binding::select(None,1,1,2,s,H),Err(Reject::InvalidBinding));
        }
        assert!(Binding::select(None,1,1,2,&"x".repeat(256),H).is_ok());
        assert_eq!(Binding::select(None,1,1,2,&"x".repeat(257),H),Err(Reject::InvalidBinding));
        assert_eq!(Binding::select(None,1,1,0,"s",H),Err(Reject::InvalidBinding));
    }
}
