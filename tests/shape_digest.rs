use atto_actor_contract_conformance::shape_digest::{canonical_shape,shape_digest};
use serde_json::Value;

#[test]
fn all_shared_shape_vectors() {
    let cases:Vec<Value>=serde_json::from_str(include_str!("../tools/actor_data_shape_digest_vectors_v1.json")).unwrap();
    let expected=[
        ("cbor-positional","610eef10e566bf78d2741deea68e26171a2c03f901ccb2a7c2f032090afa969f"),
        ("cbor-shuffled","610eef10e566bf78d2741deea68e26171a2c03f901ccb2a7c2f032090afa969f"),
        ("protobuf-recursive","4a236e0ad94e1abe167060795850a16747ae5fd1d7c929151bc4ad4eed8bfebf"),
        ("protobuf-version","b84fdc8c82d948ea6d883e886a66d1b04fdcd6e73e102f27263ebebcfcbe7d61"),
        ("field-rename","0ef108ab24bd9c5cf2dc0e6954f22a69a7ff09ab15587227831152c6b8c4dfbd"),
        ("u32-2147483647","7b273a9643527c59ddb46035bcc7ab90c8ec7b47c7c2523d471012cdc4be13ce"),
        ("u32-2147483648","18f5e90a38bf71aaf8bd0e6fc4b8f921eb5e15c14b93345e4cb999de93b80860"),
        ("u32-4294967295","4af948fe9640c4e49b0aad4eb5aea47cab31c227013edf9ead9b30d6276a43aa"),
    ];
    let mut valid=0;
    for case in &cases {
        let name=case["name"].as_str().unwrap();
        let raw=serde_json::to_vec(&case["descriptor"]).unwrap();
        if case["valid"]==true {
            valid+=1;
            let golden=expected.iter().find(|(n,_)|*n==name).unwrap().1;
            assert_eq!(shape_digest(&raw).unwrap(),golden,"{name}");
        } else {
            assert!(canonical_shape(&raw).is_err(),"{name}");
        }
    }
    assert_eq!(valid,expected.len());
    assert_eq!(cases.len(),21);
}

#[test]
fn duplicate_unknown_and_invalid_numeric_fields_fail_closed() {
    for raw in [
        r#"{"codec":"cbor","codec":"cbor","version":1,"root":"A","nodes":[{"id":"A","kind":"bool","fields":[]}]}"#,
        r#"{"codec":"cbor","version":1,"root":"A","nodes":[{"id":"A","kind":"bool","kind":"bool","fields":[]}]}"#,
        r#"{"codec":"cbor","version":true,"root":"A","nodes":[{"id":"A","kind":"bool","fields":[]}]}"#,
        r#"{"codec":"cbor","version":1.5,"root":"A","nodes":[{"id":"A","kind":"bool","fields":[]}]}"#,
        r#"{"codec":"cbor","version":1,"root":"A","nodes":[{"id":"A","kind":"bool","fields":[],"extra":0}]}"#,
    ] {
        assert!(canonical_shape(raw.as_bytes()).is_err());
    }
    let decimal=br#"{"codec":"cbor","version":1.0,"root":"A","nodes":[{"id":"A","kind":"bool","fields":[]}]}"#;
    assert_eq!(canonical_shape(decimal).unwrap(),b"ATTO-SHAPE-1\ncbor\n1\nA\n1\nA\nbool\n0\n");
    assert!(canonical_shape(&vec![b' ';4*1024*1024+1]).is_err());
}
