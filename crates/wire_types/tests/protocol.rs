use wire_types::*;
#[test]
fn frames_handle_short_reads_and_eof() {
    let v = serde_json::json!({"kind":"inspect"});
    let mut bytes = vec![];
    write_frame(&mut bytes, &v).unwrap();
    assert_eq!(
        read_frame::<_, serde_json::Value>(&mut &bytes[..]).unwrap(),
        v
    );
    assert!(read_frame::<_, serde_json::Value>(&mut &bytes[..5]).is_err());
    assert!(read_frame::<_, serde_json::Value>(&mut &[0, 32, 0, 0][..]).is_err());
}
fn request() -> Step {
    Step::new(
        "0".repeat(32),
        0,
        0,
        0,
        "a".repeat(64),
        &vec![0; 128 * 96 * 3],
    )
}
#[test]
fn replies_match_every_identity_field() {
    let r = request();
    let mut a = Reply::for_request(&r, 0.5);
    assert!(a.validate(&r).is_ok());
    a.epoch = "1".repeat(32);
    assert!(a.validate(&r).is_err());
    a = Reply::for_request(&r, 0.);
    a.world_revision = 1;
    assert!(a.validate(&r).is_err());
    a = Reply::for_request(&r, 0.);
    a.neural_steps_done = 4;
    assert!(a.validate(&r).is_err());
}
#[test]
fn malformed_actions_are_rejected() {
    let r = request();
    assert!(Reply::for_request(&r, f64::NAN).validate(&r).is_err());
    let mut v = serde_json::to_value(Reply::for_request(&r, 0.)).unwrap();
    v["target_x"] = 100.into();
    assert!(serde_json::from_value::<Reply>(v).is_err());
    let s = serde_json::to_string(&Reply::for_request(&r, 0.))
        .unwrap()
        .replace("\"step_id\":0", "\"step_id\":0,\"step_id\":0");
    assert!(serde_json::from_str::<Reply>(&s).is_err());
}
#[test]
fn twenty_five_hz_requests_use_two_neural_steps() {
    let r = request();
    assert_eq!(r.neural_steps, 2);
    assert_eq!(r.version, 2);
    assert_eq!(Reply::for_request(&r, 0.).neural_steps_done, 2);
}
#[test]
fn request_validation_rejects_legacy_timing_and_modified_pixels() {
    let r = request();
    assert_eq!(r.validate().unwrap().len(), 128 * 96 * 3);
    let mut bad = r.clone();
    bad.version = 1;
    assert!(bad.validate().is_err());
    let mut bad = r.clone();
    bad.neural_steps = 5;
    assert!(bad.validate().is_err());
    let mut bad = r.clone();
    bad.step_id = 1;
    assert!(bad.validate().is_err());
    let mut bad = r;
    bad.rgb_sha256 = "f".repeat(64);
    assert!(bad.validate().is_err());
}
