// SPDX-License-Identifier: AGPL-3.0-only
//! The image tools: the record this host owns with its copies, and a copy built at a place. What the TypeScript tool parses through the protocol's schema is typed here in that
//! schema's order, a field it does not know dropped as the parse drops it; the recipe a record carries passes through
//! as the host wrote it.

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use serde_json::{Map, Number, Value};

use super::workspace::{self, fmt_bytes, params, read};
use super::{input, Answer, Refused, Tool};
use crate::failure::Failure;
use crate::host::Host;

type Arc<T> = std::sync::Arc<T>;

/// packages/protocol's SealedImage.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedImage {
    name: String,
    version: Number,
    hash: String,
    recipe_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    recipe: Option<Box<RawValue>>,
    logins: Vec<Login>,
    sealed_at: String,
    sealed_from: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vault: Option<SealedVault>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    used_bytes: Option<Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    place: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Login {
    name: String,
    state: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedVault {
    sha256: String,
    bytes: Number,
    paths: Number,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    held: Option<Vec<String>>,
    taken_at: String,
}

/// packages/protocol's SealedImageCopy.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedImageCopy {
    place: String,
    version: Number,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hash: Option<String>,
    snapshot_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    template_id: Option<String>,
    built_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    size_bytes: Option<Number>,
}

/// packages/protocol's SealedImageView.
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct ImageOut {
    #[cfg_attr(test, schemars(with = "serde_json::Value"))]
    image: Option<SealedImage>,
    #[cfg_attr(test, schemars(with = "Vec<serde_json::Value>"))]
    copies: Vec<SealedImageCopy>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct ImageIn {}

fn f64_of(n: &Number) -> f64 {
    n.as_f64().unwrap_or(0.0)
}

/// Whether the copy was built from the record as it is now; nothing where the record holds no vault, since such a
/// record was read back off its copies and judges none of them.
fn standing(image: &SealedImage, copy: &SealedImageCopy) -> Option<String> {
    let words = workspace::words();
    image.vault.as_ref()?;
    Some(if copy.hash.as_deref() == Some(image.hash.as_str()) { words.copy_current } else { words.copy_stale })
}

fn copy_line(image: &SealedImage, copy: &SealedImageCopy) -> String {
    let mut parts = vec![copy.place.clone(), format!("v{}", copy.version)];
    parts.extend(copy.size_bytes.as_ref().map(|n| fmt_bytes(f64_of(n))));
    parts.extend(standing(image, copy));
    parts.join("  ")
}

fn image_line(image: &SealedImage) -> String {
    let words = workspace::words();
    let held = match &image.vault {
        None => words.image_no_vault,
        Some(vault) => {
            let signed = image.logins.iter().filter(|l| words.logins_held.contains(&l.state)).count();
            format!("{} held, {}", workspace::plural(&Number::from(signed), "sign-in"), workspace::plural(&vault.paths, "path"))
        }
    };
    let mut parts = vec![format!("{} v{}", image.name, image.version), image.hash.clone(), held];
    parts.extend(image.used_bytes.as_ref().map(|n| fmt_bytes(f64_of(n))));
    parts.push(format!("sealed on {}", image.sealed_from));
    parts.join("  ")
}

fn image_lines(view: &ImageOut) -> String {
    let Some(image) = &view.image else { return workspace::words().no_sealed_image };
    let mut lines = vec![image_line(image)];
    lines.extend(view.copies.iter().map(|c| copy_line(image, c)));
    lines.join("\n")
}

async fn image_view(client: &crate::client::Client) -> Result<ImageOut, Failure> {
    #[derive(Deserialize)]
    struct Got {
        view: Box<RawValue>,
    }
    let got: Got = client.request("image.get", Map::new()).await?;
    read(&got.view, "image.get")
}

const IMAGE_NAME: &str = "image";

pub const IMAGE: Tool = Tool {
    name: IMAGE_NAME,
    listed: include_str!(concat!(env!("OUT_DIR"), "/record/tools/image.json")),
    call: |host, args| Box::pin(image(host, args)),
};

async fn image(host: Arc<Host>, arguments: Value) -> Result<Answer, Refused> {
    let ImageIn {} = input(IMAGE_NAME, arguments)?;
    let client = host.client().await?;
    let view = image_view(&client).await?;
    Ok(Answer::text(image_lines(&view), &view))
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct BuildIn {
    pub place: String,
    #[serde(default)]
    pub force: Option<bool>,
}

/// packages/protocol's SealedImageBuilt.
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct BuildOut {
    #[cfg_attr(test, schemars(with = "serde_json::Value"))]
    copy: SealedImageCopy,
    built: bool,
}

const BUILD_NAME: &str = "image_build";

pub const BUILD: Tool = Tool {
    name: BUILD_NAME,
    listed: include_str!(concat!(env!("OUT_DIR"), "/record/tools/image_build.json")),
    call: |host, args| Box::pin(build(host, args)),
};

/// The word goes to the host as typed, since the host is the one judge of what it names. The list is read as the
/// command line reads it to follow the build's frames, so both servers ask the host the same things.
async fn build(host: Arc<Host>, arguments: Value) -> Result<Answer, Refused> {
    #[derive(Deserialize)]
    struct Built {
        build: Box<RawValue>,
    }
    let BuildIn { place, force } = input(BUILD_NAME, arguments)?;
    let client = host.client().await?;
    let _: Value = client.request("places.list", Map::new()).await?;
    let mut asked = params([("place", Value::from(place))]);
    if force == Some(true) {
        asked.insert("force".to_owned(), Value::Bool(true));
    }
    let built: Built = client.request("image.build", asked).await?;
    let view = image_view(&client).await?;
    let Some(image) = &view.image else { return Err(Failure::new(workspace::words().no_sealed_image).into()) };
    let built: BuildOut = read(&built.build, "image.build")?;
    let line = copy_line(image, &built.copy);
    let said = if built.built { line } else { format!("{line}  already built from this image; nothing was built") };
    Ok(Answer::text(said, &built))
}

#[cfg(test)]
mod tests {
    use super::super::held::to_the_record;
    use super::*;

    #[test]
    fn its_structs_are_the_recorded_schemas() {
        to_the_record::<ImageIn, ImageOut>(IMAGE.listed);
        to_the_record::<BuildIn, BuildOut>(BUILD.listed);
    }
}
