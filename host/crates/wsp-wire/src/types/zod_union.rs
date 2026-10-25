// SPDX-License-Identifier: AGPL-3.0-only
//! Reads a `z.union` of types as zod does: the value is the first member that reads with no issue, which serde's own
//! untagged read does not do (it takes the first member that merely deserializes, rules or not). See spec/check.md rule 7.

/// `zod_union!(Name { Variant(Type), ... })` writes `Deserialize` for an enum of newtype variants kept in zod's order.
/// The enum derives `Serialize` with `#[serde(untagged)]` and leaves `Deserialize` to this.
#[macro_export]
macro_rules! zod_union {
    ($name:ident { $($variant:ident($ty:ty)),+ $(,)? }) => {
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let value = <serde_json::Value as serde::Deserialize>::deserialize(d)?;
                $(
                    if $crate::types::check::issues_of::<$ty>(&value).is_empty() {
                        return serde_json::from_value::<$ty>(value).map($name::$variant).map_err(serde::de::Error::custom);
                    }
                )+
                Err(serde::de::Error::custom("data did not match any variant of untagged enum"))
            }
        }
    };
}
