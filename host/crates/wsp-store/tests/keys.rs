// SPDX-License-Identifier: AGPL-3.0-only
// The keys unit against the TS's own vectors: the two daemon fixtures node regenerates, and values node printed for
// the edge cases (spec/keys.md lists which rule each test pins).
use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use wsp_store::keys::*;

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../daemon/fixtures").join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap()).collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SealVectors {
    place_id: String,
    challenger_private: Jwk,
    answerer_private: Jwk,
    challenger_public: String,
    answerer_public: String,
    secret: String,
    host_to_place_key: String,
    place_to_host_key: String,
    frame: String,
    host_to_place_sealed: String,
    place_to_host_sealed: String,
}

#[derive(Deserialize)]
struct Jwk {
    d: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SigningVectors {
    private_key_pem: String,
    public_key: String,
    host_transcript: String,
    place_transcript: String,
    host_signature: String,
    place_signature: String,
    refusal_transcript: String,
    refusal_signature: String,
}

fn seal_vectors() -> SealVectors {
    serde_json::from_str(&fixture("place-link-seal.json")).unwrap()
}

fn signing_vectors() -> SigningVectors {
    serde_json::from_str(&fixture("place-link-signing.json")).unwrap()
}

const BLOB: &str = "AAAAC3NzaC1lZDI1NTE5AAAAIC6sV8jQCzynkpUOM40rRIjA7tPstUSjC+A/LVMnAZhP";

mod fingerprint {
    use super::*;

    #[test]
    fn is_the_word_every_ssh_tool_prints_for_the_same_key() {
        assert_eq!(key_fingerprint(BLOB), "SHA256:Ge9MQ9S/Faik2WzsxidRXnoEOJRIHkJKTBkOJ903vq4");
    }

    #[test]
    fn wears_that_one_form_for_a_place_link_key_too_which_travels_as_spki_der() {
        let a = new_place_key_pair();
        let b = new_place_key_pair();
        let fa = key_fingerprint(&a.public_key);
        let fb = key_fingerprint(&b.public_key);
        assert!(fa.starts_with("SHA256:"), "{fa}");
        let body = &fa["SHA256:".len()..];
        assert_eq!(body.len(), 43, "{fa}");
        assert!(body.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/'), "{fa}");
        assert_ne!(fa, fb);
    }

    #[test]
    fn reads_the_key_as_node_reads_base64_url_safe_spaced_or_with_junk() {
        let want = "SHA256:Ge9MQ9S/Faik2WzsxidRXnoEOJRIHkJKTBkOJ903vq4";
        assert_eq!(key_fingerprint(&BLOB.replace('+', "-").replace('/', "_")), want);
        assert_eq!(key_fingerprint(&format!("{} \n{}", &BLOB[..10], &BLOB[10..])), want);
        assert_eq!(key_fingerprint(&format!("{}!*{}", &BLOB[..10], &BLOB[10..])), want);
    }

    #[test]
    fn of_nothing_is_the_sha256_of_no_bytes() {
        assert_eq!(key_fingerprint(""), "SHA256:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU");
    }
}

mod base64_as_node_reads_it {
    use super::*;

    #[test]
    fn stops_at_the_first_padding_and_takes_a_missing_one() {
        assert_eq!(node_base64("QUJD=RA=="), b"ABC");
        assert_eq!(node_base64("QUI"), b"AB");
        assert_eq!(node_base64(""), Vec::<u8>::new());
    }

    #[test]
    fn takes_both_alphabets_and_skips_what_is_in_neither() {
        assert_eq!(node_base64("-_8"), node_base64("+/8"));
        assert_eq!(node_base64("QU\nJD"), b"ABC");
        assert_eq!(node_base64("Q!U*JD"), b"ABC");
    }

    #[test]
    fn reads_a_character_past_latin_1_by_its_low_byte() {
        assert_eq!(node_base64("QUJ\u{144}"), b"ABC");
        assert_eq!(node_base64("QU\u{1F600}JD"), b"A");
    }
}

mod signing {
    use super::*;

    #[test]
    fn signs_both_transcripts_and_the_refusal_to_the_fixture_bytes() {
        let v = signing_vectors();
        let host = node_base64(&v.host_transcript);
        let place = node_base64(&v.place_transcript);
        let refusal = node_base64(&v.refusal_transcript);
        assert_eq!(sign_place_bytes(&v.private_key_pem, &host).unwrap(), v.host_signature);
        assert_eq!(sign_place_bytes(&v.private_key_pem, &place).unwrap(), v.place_signature);
        assert_eq!(sign_place_bytes(&v.private_key_pem, &refusal).unwrap(), v.refusal_signature);
    }

    #[test]
    fn verifies_a_signature_only_over_the_bytes_it_was_made_on() {
        let v = signing_vectors();
        let host = node_base64(&v.host_transcript);
        let place = node_base64(&v.place_transcript);
        assert!(verify_place_bytes(&v.public_key, &place, &v.place_signature));
        assert!(!verify_place_bytes(&v.public_key, &host, &v.place_signature));
        assert!(verify_place_bytes(&v.public_key, &host, &v.host_signature));
    }

    #[test]
    fn a_key_or_signature_that_will_not_parse_is_false_never_an_error() {
        let v = signing_vectors();
        let place = node_base64(&v.place_transcript);
        assert!(!verify_place_bytes("AAAA", &place, &v.place_signature));
        assert!(!verify_place_bytes(&v.public_key, &place, &v.place_signature[..20]));
        assert!(!verify_place_bytes(&v.public_key, &place, ""));
    }

    #[test]
    fn a_pem_that_does_not_parse_is_an_error() {
        assert!(sign_place_bytes("nope", b"x").is_err());
    }

    #[test]
    fn a_fresh_pair_is_node_spki_der_and_pkcs8_pem_and_signs_for_itself() {
        let pair = new_place_key_pair();
        let public = node_base64(&pair.public_key);
        assert_eq!(public.len(), 44);
        assert_eq!(hex(&public[..12]), "302a300506032b6570032100");
        assert!(pair.private_key_pem.starts_with("-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEI"), "{}", pair.private_key_pem);
        assert!(pair.private_key_pem.ends_with("\n-----END PRIVATE KEY-----\n"), "{}", pair.private_key_pem);
        let sig = sign_place_bytes(&pair.private_key_pem, b"hello").unwrap();
        assert!(verify_place_bytes(&pair.public_key, b"hello", &sig));
        assert!(!verify_place_bytes(&new_place_key_pair().public_key, b"hello", &sig));
    }

    #[test]
    fn a_pair_printed_for_debugging_shows_no_private_key() {
        let pair = new_place_key_pair();
        let body = pair.private_key_pem.lines().nth(1).unwrap();
        assert!(!format!("{pair:?}").contains(body), "{pair:?}");
    }

    #[test]
    fn signs_with_a_pem_node_reads_with_text_after_it_or_lines_wrapped_short() {
        let v = signing_vectors();
        let want = sign_place_bytes(&v.private_key_pem, b"x").unwrap();
        assert_eq!(sign_place_bytes(&format!("{}trailing\n", v.private_key_pem), b"x").unwrap(), want);
        let (head, body) = v.private_key_pem.split_at("-----BEGIN PRIVATE KEY-----\n".len() + 20);
        assert_eq!(sign_place_bytes(&format!("{head}\n{body}"), b"x").unwrap(), want);
    }

    #[test]
    fn signs_with_a_pem_node_wrote() {
        let v = signing_vectors();
        let sig = sign_place_bytes(&v.private_key_pem, b"hello").unwrap();
        assert!(verify_place_bytes(&v.public_key, b"hello", &sig));
    }
}

mod agreement {
    use super::*;

    #[test]
    fn regenerates_every_derived_field_of_the_seal_fixture() {
        let v = seal_vectors();
        let challenger = EphemeralKey::from_jwk_d(&v.challenger_private.d).unwrap();
        let answerer = EphemeralKey::from_jwk_d(&v.answerer_private.d).unwrap();
        assert_eq!(raw_public_key(&challenger), v.challenger_public);
        assert_eq!(raw_public_key(&answerer), v.answerer_public);
        let secret = shared_secret(&challenger, &v.answerer_public).unwrap();
        assert_eq!(secret.to_vec(), node_base64(&v.secret));
        assert_eq!(shared_secret(&answerer, &v.challenger_public).unwrap(), secret);
        let keys = seal_keys(&secret, &v.place_id);
        assert_eq!(keys.host_to_place.to_vec(), node_base64(&v.host_to_place_key));
        assert_eq!(keys.place_to_host.to_vec(), node_base64(&v.place_to_host_key));
        assert_eq!(seal_with(&keys.host_to_place, 0, &v.frame), node_base64(&v.host_to_place_sealed));
        assert_eq!(seal_with(&keys.place_to_host, 0, &v.frame), node_base64(&v.place_to_host_sealed));
    }

    #[test]
    fn two_fresh_ephemerals_agree_on_one_secret() {
        let (a, a_pub) = fresh_ephemeral();
        let (b, b_pub) = fresh_ephemeral();
        assert_eq!(node_base64(&a_pub).len(), 32);
        assert_eq!(raw_public_key(&a), a_pub);
        assert_ne!(a_pub, b_pub);
        assert_eq!(shared_secret(&a, &b_pub).unwrap(), shared_secret(&b, &a_pub).unwrap());
    }

    #[test]
    fn a_peer_value_that_is_not_32_bytes_is_an_error() {
        let short = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==";
        assert_eq!(node_base64(short).len(), 31);
        assert!(public_key_of(short).is_err());
        let (a, _) = fresh_ephemeral();
        assert!(shared_secret(&a, short).is_err());
        let ok = "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE=";
        assert_eq!(public_key_of(ok).unwrap(), [1u8; 32]);
    }

    #[test]
    fn a_low_order_peer_whose_agreement_is_all_zero_is_an_error() {
        let (a, _) = fresh_ephemeral();
        assert!(shared_secret(&a, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=").is_err());
    }

    #[test]
    fn seal_keys_printed_for_debugging_show_neither_key() {
        let keys = seal_keys(&[1u8; 32], "p_x");
        let printed = format!("{keys:?}");
        assert!(!printed.contains(&format!("{:?}", keys.host_to_place)), "{printed}");
        assert!(!printed.contains(&format!("{:?}", keys.place_to_host)), "{printed}");
    }

    #[test]
    fn derives_both_direction_keys_from_the_place_id_salt() {
        let keys = seal_keys(&[1u8; 32], "p_x");
        assert_eq!(hex(&keys.host_to_place), "91e1d33fb55aa389bbbfcbe6bf9d217fcea28dfeef292b03ac861eb185708039");
        assert_eq!(hex(&keys.place_to_host), "810693d62628fd12e589906175838c3354f95b61ddc3cb6f7567cc90f13427bb");
        assert_eq!(hex(&seal_keys(&[], "").host_to_place), "8e374b9c555063f565d6a0d5cc2aa9e57743c450dc86ba1156dcf21dcc9408d3");
    }
}

mod seal {
    use super::*;

    const KEY: [u8; 32] = [7u8; 32];

    #[test]
    fn the_nonce_is_four_zero_bytes_then_the_counter_big_endian() {
        assert_eq!(hex(&seal_nonce(258)), "000000000000000000000102");
        assert_eq!(hex(&seal_nonce(0)), "000000000000000000000000");
        assert_eq!(hex(&seal_nonce(u64::MAX)), "00000000ffffffffffffffff");
    }

    #[test]
    fn seals_to_the_bytes_node_seals_ciphertext_then_tag() {
        assert_eq!(hex(&seal_with(&KEY, 0, "héllo")), "091b0dde97fd9822b3676729adccf5ddb0f6ed06c653");
        assert_eq!(hex(&seal_with(&KEY, 5, "")), "622509477b5a98676d8f972a36197704");
    }

    #[test]
    fn opens_what_it_sealed_the_empty_text_included() {
        assert_eq!(unseal_with(&KEY, 0, &unhex("091b0dde97fd9822b3676729adccf5ddb0f6ed06c653")).unwrap(), "héllo");
        assert_eq!(unseal_with(&KEY, 5, &unhex("622509477b5a98676d8f972a36197704")).unwrap(), "");
    }

    #[test]
    fn a_short_frame_a_bad_tag_and_a_wrong_counter_refuse_alike() {
        let sealed = unhex("091b0dde97fd9822b3676729adccf5ddb0f6ed06c653");
        assert_eq!(unseal_with(&KEY, 0, &[0u8; 15]), Err(SealRefusal));
        assert_eq!(unseal_with(&KEY, 0, &[]), Err(SealRefusal));
        let mut tampered = sealed.clone();
        tampered[0] ^= 1;
        assert_eq!(unseal_with(&KEY, 0, &tampered), Err(SealRefusal));
        assert_eq!(unseal_with(&KEY, 1, &sealed), Err(SealRefusal));
        assert_eq!(SealRefusal.to_string(), SEAL_REFUSAL);
    }

    #[test]
    fn a_plaintext_that_is_not_utf8_opens_with_a_replacement_character() {
        assert_eq!(unseal_with(&KEY, 0, &unhex("9e99a1cd397c835b49c52dee5cc9d3866b6c")).unwrap(), "\u{fffd}A");
    }

    #[test]
    fn each_end_counts_its_own_direction_and_a_refused_frame_moves_no_counter() {
        let keys = seal_keys(&[1u8; 32], "p_x");
        let mut host = make_seal(&keys, Side::Host);
        let mut place = make_seal(&keys, Side::Place);
        let one = host.seal("one");
        let two = host.seal("two");
        assert_ne!(one, two);
        assert_eq!(one, seal_with(&keys.host_to_place, 0, "one"));
        assert_eq!(two, seal_with(&keys.host_to_place, 1, "two"));
        assert_eq!(place.unseal(&two), Err(SealRefusal));
        assert_eq!(place.unseal(&one).unwrap(), "one");
        assert_eq!(place.unseal(&two).unwrap(), "two");
        assert_eq!(host.unseal(&one), Err(SealRefusal));
        let back = place.seal("back");
        assert_eq!(back, seal_with(&keys.place_to_host, 0, "back"));
        assert_eq!(host.unseal(&back).unwrap(), "back");
    }
}

mod frames {
    use super::*;

    #[test]
    fn before_a_seal_a_frame_is_its_own_text() {
        assert_eq!(open_frame(None, Frame::Text("text")).unwrap(), "text");
        assert_eq!(open_frame(None, Frame::Binary(b"hi")).unwrap(), "hi");
        assert_eq!(open_frame(None, Frame::Binary(&[0xff, b'A'])).unwrap(), "\u{fffd}A");
    }

    #[test]
    fn after_a_seal_a_text_frame_is_refused_and_a_binary_one_opens_in_order() {
        let keys = seal_keys(&[1u8; 32], "p_x");
        let mut host = make_seal(&keys, Side::Host);
        let mut place = make_seal(&keys, Side::Place);
        assert_eq!(open_frame(Some(&mut place), Frame::Text("plain")), Err(SealRefusal));
        let one = host.seal("one");
        assert_eq!(open_frame(Some(&mut place), Frame::Binary(&one)).unwrap(), "one");
        assert_eq!(open_frame(Some(&mut place), Frame::Binary(&one)), Err(SealRefusal));
        let two = host.seal("two");
        assert_eq!(open_frame(Some(&mut place), Frame::Binary(&two)).unwrap(), "two");
    }
}

mod citations {
    use super::*;

    /// Every `(index.ts:N)` the port cites names the TS line that defines the item it documents.
    #[test]
    fn each_cited_line_of_the_ts_defines_the_item_cited() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let ts = fs::read_to_string(root.join("../../../packages/keys/src/index.ts")).unwrap();
        let ts: Vec<&str> = ts.lines().collect();
        let rs = fs::read_to_string(root.join("src/keys.rs")).unwrap();
        let rs: Vec<&str> = rs.lines().collect();
        let mut wrong = Vec::new();
        for (i, line) in rs.iter().enumerate() {
            let Some(at) = line.find("index.ts:") else { continue };
            let n: usize = line[at + 9..].chars().take_while(char::is_ascii_digit).collect::<String>().parse().unwrap();
            let item = rs[i..].iter().find_map(|l| {
                let l = l.trim_start().strip_prefix("pub ")?;
                let l = ["fn ", "const ", "struct ", "enum "].iter().find_map(|k| l.strip_prefix(k))?;
                Some(l.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect::<String>())
            });
            let Some(item) = item else { continue };
            let camel = if let Some(ts_name) = [("EphemeralKey", "freshEphemeral"), ("SealRefusal", "SEAL_REFUSAL")].iter().find(|(r, _)| *r == item) {
                ts_name.1.to_string()
            } else if item.chars().all(|c| c.is_uppercase() || c == '_') {
                item.clone()
            } else {
                let mut parts = item.split('_');
                let first = parts.next().unwrap().to_string();
                parts.fold(first, |acc, p| acc + &p[..1].to_uppercase() + &p[1..])
            };
            if !ts.get(n - 1).is_some_and(|l| l.contains(&camel)) {
                wrong.push(format!("keys.rs:{} cites index.ts:{n} for {item}", i + 1));
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }
}
