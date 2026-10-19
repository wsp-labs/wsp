// SPDX-License-Identifier: AGPL-3.0-only
//! The protocol's types, generated from record/protocol.json by typify as `import_types!` would, then opened to a
//! newer host: no type refuses a field the record does not list, every word list reads a word it does not know as
//! `Unknown`, every union tagged by a field reads a tag it does not know as `Unknown`, and every untagged union keeps
//! a shape it does not know in `Unknown` as it was sent.

use std::path::Path;

use quote::ToTokens;
use serde_json::Value;
use syn::{parse_quote, Attribute, Expr, Fields, ImplItem, Item, ItemEnum, Stmt, Type};
use typify::{TypeSpace, TypeSpaceSettings};

/// The union whose frames `Event::read` hands on as they were sent when they do not read, so their channel is kept.
const AS_SENT: &[&str] = &["DaemonChannelEvent"];

fn main() {
    let record = Path::new("record/protocol.json");
    println!("cargo:rerun-if-changed={}", record.display());
    println!("cargo:rerun-if-changed=build.rs");
    let text = std::fs::read_to_string(record).expect("record/protocol.json reads");
    let mut schema: Value = serde_json::from_str(&text).expect("record/protocol.json is JSON");
    drop_closed(&mut schema);
    let root: schemars::schema::RootSchema = serde_json::from_value(schema).expect("record/protocol.json is a JSON schema");
    let mut space = TypeSpace::new(&TypeSpaceSettings::default());
    space.add_root_schema(root).expect("typify reads record/protocol.json");
    let mut file: syn::File = syn::parse2(space.to_stream()).expect("typify writes Rust");
    open_unions(&mut file);
    let out = Path::new(&std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("protocol.rs");
    std::fs::write(out, file.into_token_stream().to_string()).expect("OUT_DIR takes protocol.rs");
}

/// Every `additionalProperties: false` taken out, so a field the record does not list reads.
fn drop_closed(schema: &mut Value) {
    match schema {
        Value::Object(map) => {
            if map.get("additionalProperties") == Some(&Value::Bool(false)) {
                map.remove("additionalProperties");
            }
            map.values_mut().for_each(drop_closed);
        }
        Value::Array(items) => items.iter_mut().for_each(drop_closed),
        _ => {}
    }
}

/// The `Unknown` arm on each word list, each tagged union and each untagged union, and on the `Display` of each word
/// list and each untagged union typify gives one.
fn open_unions(file: &mut syn::File) {
    let (mut words, mut shapes) = (Vec::new(), Vec::new());
    for item in &mut file.items {
        let Item::Enum(e) = item else { continue };
        let container = serde_container(&e.attrs);
        let unit = e.variants.iter().all(|v| matches!(v.fields, Fields::Unit));
        if unit && container.is_empty() {
            words.push(e.ident.to_string());
            add_unknown(e);
        } else if container.contains("tag =") && !AS_SENT.contains(&e.ident.to_string().as_str()) {
            assert!(!e.variants.iter().any(|v| v.ident == "Unknown"), "{} has a variant named Unknown", e.ident);
            add_unknown(e);
        } else if container.contains("untagged") {
            // serde takes no `other` on an untagged enum; a catch-all last is tried only once every shape it knows failed.
            assert!(!e.variants.iter().any(|v| v.ident == "Unknown"), "{} has a variant named Unknown", e.ident);
            shapes.push(e.ident.to_string());
            e.variants.push(parse_quote! {
                /// A shape this release does not know, from a newer host, as it was sent; it is never one to send.
                Unknown(::serde_json::Value)
            });
        }
    }
    for item in &mut file.items {
        let Item::Impl(i) = item else { continue };
        let Some((path, _)) = &i.trait_ else { continue };
        let Type::Path(of) = &*i.self_ty else { continue };
        let named = |p: &syn::Path, name: &str| p.segments.last().is_some_and(|s| s.ident == name);
        let shape = shapes.iter().any(|w| named(&of.path, w));
        if !named(path, "Display") || !(shape || words.iter().any(|w| named(&of.path, w))) {
            continue;
        }
        let Some(ImplItem::Fn(fmt)) = i.items.first_mut() else { panic!("Display for {} has no fmt", of.to_token_stream()) };
        let Some(Stmt::Expr(Expr::Match(arms), _)) = fmt.block.stmts.first_mut() else {
            panic!("Display for {} is not one match", of.to_token_stream())
        };
        if shape {
            arms.arms.push(parse_quote!(Self::Unknown(x) => x.fmt(f),));
        } else if !arms.arms.iter().any(|a| a.pat.to_token_stream().to_string().ends_with("Unknown")) {
            arms.arms.push(parse_quote!(Self::Unknown => f.write_str("unknown"),));
        }
    }
}

/// `#[serde(other)]` on the enum's `Unknown` arm, added where the record does not already list "unknown".
fn add_unknown(e: &mut ItemEnum) {
    match e.variants.iter_mut().find(|v| v.ident == "Unknown") {
        Some(known) => known.attrs.push(parse_quote!(#[serde(other)])),
        None => e.variants.push(parse_quote! {
            /// A value this release does not know, from a newer host; it is never one to send.
            #[serde(rename = "unknown", other)]
            Unknown
        }),
    }
}

/// What the enum's own `#[serde(...)]` says, empty where it has none.
fn serde_container(attrs: &[Attribute]) -> String {
    attrs.iter().filter(|a| a.path().is_ident("serde")).map(|a| a.meta.to_token_stream().to_string()).collect()
}
