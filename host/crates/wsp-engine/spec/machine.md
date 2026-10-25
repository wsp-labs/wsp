# machine (machine.ts, 313 lines; no-provider-backend.ts, 78 lines)

The seam every backend sits behind, and the backend of a host with no provider key. Rust: `src/machine.rs`,
`src/no_provider_backend.rs`, `src/fetch.rs`.

## The two traits (machine.ts:101, :252)

- `Machine` and `MachineBackend` are `#[async_trait]` traits, dyn-compatible: a backend travels as
  `Arc<dyn MachineBackend>`, a machine as `Arc<dyn Machine>`. Every call answers `errors::Result`.
- **Optional methods.** A TS optional method (`previewUrl?`, `listSnapshots?`) is a trait method with a default
  body that answers `EngineError::Other { message: NOT_ON_THIS_ROAD }`, plus a flag in `offers()`
  (`MachineOffers`, `BackendOffers`). A caller branches on the flag exactly where the TS reads the property; a backend
  that has the method overrides both. Groups the TS declares together share one flag: `keyed_create` is
  createRequest, replayCreate and namedAfter (machine.ts:268-280); `templates` is promote, get, list and delete
  (:303-311). `Lifecycle::backstop` and `quiet_for_ms` have `has_backstop()` / `has_quiet_for_ms()`.
- **Owned getters.** `capabilities()` answers an owned `Capabilities` (Box computes its sizes per read);
  `pricing()` an `Arc<dyn BackendPricing>`; `lifecycle()` an `Option<Arc<dyn Lifecycle>>`; `base_templates()`
  wsp-wire's `{ sandbox, desktop }`. A backend whose capabilities carry a `pauseMode` answers a lifecycle (:254).
- `root_disk_of` is synchronous (:298); `None` both for a size nobody measured and for a backend without it.
- Options: `RunOptions` (:44: `deadline_ms`, `on_line`, `poll_ms`, `unlogged`, `signal`, `launch_on`), `ExecOptions`
  (:104: `timeout_ms`, `idempotency_key`, `stdin`), `SnapshotOptions { on_progress }`. A cancel is a
  `Cancel` (src/cancel.rs), taken as `Option<&Cancel>` by a method and held as `Option<Cancel>` in `RunOptions`.
- `SnapshotStoragePricing` (:27) is the wire shape plus `free_named` and `usd_per_named_month`, absent on every
  backend but Box.
- `fetch::Fetch` stands for `globalThis.fetch`: a request is method, URL, headers in the order written, body bytes and
  an optional `Abort`; a failure is a `RoadError`. `fetch_stream` reads the body chunk by chunk.

## NoProviderBackend (no-provider-backend.ts)

- `new(None)` refuses with wsp-words' `NO_PROVIDER_LINE`; `new(Some(line))` with the host's own line (:37).
- `capabilities()`: every flag false, `sizes` empty, no `pauseMode` (:39-55). Pricing: rate 0 for any size, default
  size 0 cpu and 0 MB, storage free 0 GB at $0 billed from `""`, no named fields (:17, :57).
- `list(_)` answers `Ok(vec![])` whatever the labels: a host reading its fleet finds none, never a refusal (:73).
- `create`, `get`, `delete_snapshot` answer `EngineError::NoProvider { line }`; its message is the line (:65-77).
  `is_no_provider(e)` is true of that variant only (:29), so a record waits for the key rather than reading gone.
- `offers()` all false, no lifecycle, no base templates, `names_workspace` false. Nothing is kept; nothing survives.

## Open points

1. Key order. TS Box capabilities write `sizes` last (`{ ...BOX_CAPABILITIES, sizes }`, box-backend.ts:546) and each
   size `{ name, cpu, memMb, rateUsdPerHour }`; wsp-wire's `Capabilities` and `MachineSizeOffer` serialize in schema
   order (`sizes` after `templates`, `name` last). NoProvider's TS literal already matches the schema order. A tape
   comparing a Box capabilities reply byte for byte differs until wsp-wire rules.

## Rules against tests

| Rule | Tests |
|---|---|
| every flag false, no sizes | `machine::every_capability_is_false_and_it_offers_no_sizes`, `machine_hard::no_provider_capabilities_are_the_ts_object_in_its_key_order` |
| the listing is empty, never a refusal | `machine::it_holds_nothing_so_the_listing_is_empty_rather_than_a_refusal` |
| every other road refuses with the line, typed | `machine::every_road_that_would_touch_a_machine_says_how_to_get_one`, `machine_hard::the_refusal_is_typed_so_a_record_waits_and_a_host_line_replaces_the_default` |
| nothing bills | `machine::nothing_bills_on_it_and_it_names_no_size_to_bill_for`, `machine_hard::no_provider_offers_nothing_optional` |
| copies flags | `machine::copies_flags_box_copies_and_its_copy_has_its_own_network`, `machine::copies_flags_no_provider_copies_nothing_and_has_no_network_of_its_own` |
| a default optional method answers not on this road | `machine_hard::a_machine_without_an_optional_method_answers_not_on_this_road` |

## Pins (wave 2)

Values from `oracle/unit_pins.oracle.ts` (`tests/fixtures/scenes/units.json`, section `machine`), held by
`tests/machine_pins.rs`.

- N-1 `NoProviderBackend::new(line)` takes the default line only for `None`: `Some("")` is kept, and every road
  refuses with no words (`an_empty_line_is_kept_and_refuses_with_no_words`).
