use harmonia_store_aterm::{parse_derivation_aterm, print_derivation_aterm};
use harmonia_store_path::StoreDir;

const NIX_WRITTEN_DERIVATION: &str = r#"Derive([("out","/nix/store/00000000000000000000000000000000-structured-attrs-floats","","")],[],[],"x86_64-linux","/bin/sh",["-c","echo > $out"],[("__json","{\"builder\":\"/bin/sh\",\"large\":1e+16,\"name\":\"structured-attrs-floats\",\"precise\":1.2345678901234568e+17,\"scale\":1.0,\"small\":1e-05,\"system\":\"x86_64-linux\",\"track_width_correction\":1e-06,\"x_scale_factor\":0.01}"),("out","/nix/store/00000000000000000000000000000000-structured-attrs-floats")])"#;

#[test]
fn parsing_and_printing_nix_json_floats_gives_byte_identical_output() {
    let store_dir = StoreDir::default();
    let derivation = parse_derivation_aterm(
        &store_dir,
        NIX_WRITTEN_DERIVATION.as_bytes(),
        "structured-attrs-floats".parse().unwrap(),
    )
    .unwrap();

    assert_eq!(
        String::from_utf8(print_derivation_aterm(&store_dir, &derivation.into_full())).unwrap(),
        NIX_WRITTEN_DERIVATION
    );
}
