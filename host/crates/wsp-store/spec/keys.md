# keys

Source: packages/keys/src/index.ts (169 lines) at a3b870a6e. Tests: none in the package; the vectors live in
packages/daemon/test/place-link-seal.test.ts and place-link-signing.test.ts (1 case each, both regenerating
daemon/fixtures) and packages/engine/test/key-fingerprint.test.ts (2 cases). Rust tests: tests/keys.rs (25 cases).

## What it does

The pieces of a wsp link: an ed25519 pair per end (public SPKI DER base64, private PKCS#8 PEM, node's spellings),
signatures over transcripts, a key's ssh-style fingerprint, an X25519 agreement per socket, HKDF-SHA256 keys per
direction, and an AES-256-GCM seal per frame with a counter per direction. The daemon's `wsp-seal` crate is its twin
and both are held to the fixtures in daemon/fixtures.

## Behaviour

1. `key_fingerprint(b64)` = `"SHA256:"` + base64 (standard, padding stripped) of sha256 of the key's bytes read by
   node's base64 decoder (index.ts:64).
2. Node's base64 decoder (`Buffer.from(s, "base64")`) is the reader for every base64 input here: both alphabets,
   characters outside them skipped, padding optional, nothing read past the first `=`. `node_base64` is that reader.
3. `new_place_key_pair` gives a 44-byte SPKI DER (`302a300506032b6570032100` + 32 bytes) base64 and a PKCS#8 v1 PEM
   (`-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEI...\n-----END PRIVATE KEY-----\n`), index.ts:77.
4. `sign_place_bytes` signs with the PEM, base64 out; a PEM that does not parse is an error (node throws), index.ts:92.
   Ed25519 is deterministic, so the fixture's signatures are reproduced byte for byte.
5. `verify_place_bytes` is false for a key, a signature or a length that does not parse, never an error (index.ts:98).
6. `seal_keys`: HKDF-SHA256(ikm = secret, salt = place id UTF-8, info = the direction's word, 32 bytes), index.ts:126.
   Empty secret and empty id are accepted.
7. `raw_public_key` is the 32-byte X25519 public value, standard base64 with padding (index.ts:132); `public_key_of`
   refuses anything that is not 32 bytes (index.ts:135).
8. `shared_secret` refuses a peer that is not 32 bytes and a low-order peer whose agreement is all zero, as node's
   derivation does (probed: an all-zero peer throws "failed during derivation").
9. `seal_nonce(counter)`: 4 zero bytes then the u64 big endian (index.ts:150). The TS takes a bigint and throws at
   2^64; a u64 cannot get there.
10. `seal_with`: AES-256-GCM, ciphertext then the 16-byte tag (index.ts:157). The empty text seals to the tag alone.
11. `unseal_with`: under 16 bytes, a bad tag, or the wrong counter all give the one `SealRefusal` (index.ts:164-173).
    The plaintext is decoded as UTF-8 with U+FFFD for a bad sequence (`toString("utf8")`).
12. `make_seal`: host seals with host-to-place and opens place-to-host, the place the other way; each direction counts
    from 0; a seal counts after sealing, an unseal counts only after the frame opened (index.ts:193-210).
13. `open_frame`: no seal gives the frame's text (a binary frame decoded as UTF-8, lossily, as `String(buffer)`); with
    a seal a text frame is the refusal and a binary frame goes through `unseal` (index.ts:187).

## Edge cases

- `node_base64("QUJD=RA==")` = `ABC`; `"QUI"` = `AB` (probed on node 22).
- `key_fingerprint("")` = `SHA256:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU`.
- A refused unseal leaves the counter where it was, so the next frame in order still opens.

## Behaviour nobody wrote down

- Rule 13.8 order and restart: nothing here survives a restart; a seal lives for one socket.
- The TS `rawPublicKey` takes a public or a private KeyObject; the port takes the private `EphemeralKey`, which is
  the only caller's (`freshEphemeral`).
- `EphemeralKey::from_jwk_d` exists for the fixture test only, standing in for `createPrivateKey({ format: "jwk" })`.
  ring cannot rebuild an ephemeral from bytes, which is why the daemon's own test starts at the secret; the host side
  takes x25519-dalek's static secret to reach the fixture's privates.

## Tapes

The place link frames in the places and relay tapes of #2443; nothing here owns an op.

## Decisions

1. `SealRefusal` is a unit error whose text is `SEAL_REFUSAL`; `KeyError` carries the underlying library's text,
   which no caller reads (the TS callers catch and refuse with their own sentence).
2. `Frame` replaces the TS `raw: unknown`: the websocket hands a string or bytes.

## After the port

- None found.

## Rules to tests

| Rule | Test |
|---|---|
| 1 | fingerprint::is_the_word_every_ssh_tool_prints_for_the_same_key, fingerprint::of_nothing_is_the_sha256_of_no_bytes, fingerprint::wears_that_one_form_for_a_place_link_key_too_which_travels_as_spki_der |
| 2 | base64_as_node_reads_it::*, fingerprint::reads_the_key_as_node_reads_base64_url_safe_spaced_or_with_junk |
| 3 | signing::a_fresh_pair_is_node_spki_der_and_pkcs8_pem_and_signs_for_itself |
| 4 | signing::signs_both_transcripts_and_the_refusal_to_the_fixture_bytes, signing::signs_with_a_pem_node_wrote, signing::a_pem_that_does_not_parse_is_an_error |
| 5 | signing::verifies_a_signature_only_over_the_bytes_it_was_made_on, signing::a_key_or_signature_that_will_not_parse_is_false_never_an_error |
| 6 | agreement::derives_both_direction_keys_from_the_place_id_salt, agreement::regenerates_every_derived_field_of_the_seal_fixture |
| 7, 8 | agreement::a_peer_value_that_is_not_32_bytes_is_an_error, agreement::a_low_order_peer_whose_agreement_is_all_zero_is_an_error, agreement::two_fresh_ephemerals_agree_on_one_secret |
| 9 | seal::the_nonce_is_four_zero_bytes_then_the_counter_big_endian |
| 10, 11 | seal::seals_to_the_bytes_node_seals_ciphertext_then_tag, seal::opens_what_it_sealed_the_empty_text_included, seal::a_short_frame_a_bad_tag_and_a_wrong_counter_refuse_alike, seal::a_plaintext_that_is_not_utf8_opens_with_a_replacement_character |
| 12 | seal::each_end_counts_its_own_direction_and_a_refused_frame_moves_no_counter |
| 13 | frames::before_a_seal_a_frame_is_its_own_text, frames::after_a_seal_a_text_frame_is_refused_and_a_binary_one_opens_in_order |
