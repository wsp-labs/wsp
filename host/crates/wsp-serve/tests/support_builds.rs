mod support;

#[test]
fn the_fakes_build() {
    let _ = support::FakeRuntime::new();
}
