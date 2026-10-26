// SPDX-License-Identifier: AGPL-3.0-only
//! Every read Settings makes as it opens, and the ones it holds past its closing.

use std::collections::HashMap;

use futures::channel::mpsc;
use gpui_kit::Global;
use serde_json::json;
use wsp_host_client::ops::settings::{
    Account, CapabilitiesReply, Devices, Editors, Image, Places, PreferencesReply, Recipes, Release, SshInclude,
};
use wsp_host_client::ops::sidebar::Projects;
use wsp_host_client::ops::thread::Harnesses;
use wsp_host_client::protocol::{
    AccountGetRequest, CapabilitiesGetRequest, DevicesListRequest, EditorListRequest, HarnessesListRequest, ImageGetRequest,
    InitGetRequest, PlacesListRequest, PreferencesGetRequest, ProjectsListRequest, RecipesListRequest, ReleaseGetRequest,
    SshIncludeRequest,
};
use wsp_host_client::HostClient;

use super::model::{req, Data, Read};
use super::{said, said_and_fix, Arrival};

/// `SETTINGS_HOLD_MS`: Settings opened again inside it of its last reads asks the host for none of the held ones.
pub(super) const HOLD: std::time::Duration = std::time::Duration::from_secs(60);

/// The reads `settingsReads.ts` holds past the page (the setup, the editors, the ssh config's word, the account, the
/// devices and the image), as the last Settings on each host left them, and when they were asked. Settings goes with
/// its closing, so they are kept here, by the host's client, as the Agents pages' store is.
#[derive(Default)]
pub(super) struct Held(pub(super) HashMap<usize, (std::time::Instant, Data)>);

impl Global for Held {}

/// The held reads taken out of `data`, the rest left.
pub(super) fn held_of(data: &mut Data) -> Data {
    Data {
        setup: data.setup.take(),
        editors: data.editors.take(),
        ssh_include: data.ssh_include.take(),
        account: data.account.take(),
        devices: data.devices.take(),
        devices_refused: data.devices_refused,
        image: data.image.take(),
        product_usage_off: data.product_usage_off.take(),
        ..Data::default()
    }
}

/// The held reads laid back onto `data`.
pub(super) fn hold_into(data: &mut Data, held: Data) {
    data.setup = held.setup;
    data.editors = held.editors;
    data.ssh_include = held.ssh_include;
    data.account = held.account;
    data.devices = held.devices;
    data.devices_refused = held.devices_refused;
    data.image = held.image;
    data.product_usage_off = held.product_usage_off;
}

/// What a refused read says, as `notRead` says it. The lists, whose pages say a refusal in place, and the release as
/// Settings opens, which the web app reads quietly, say nothing here; a refused release.check says so where it is asked.
pub(super) fn refusal(read: &Read) -> Option<String> {
    let (what, said) = match read {
        Read::Preferences(Err(said)) => ("Preferences", said),
        Read::Harnesses(Err(said)) => ("Agents", said),
        Read::Setup(Err(said)) => ("Setup", said),
        Read::Account(Err(said)) => ("Account", said),
        Read::Editors(Err(said)) => ("Editors", said),
        Read::SshInclude(Err(said)) => ("ssh config", said),
        Read::Image(Err(said)) => ("Image", said),
        Read::Workspaces(Err(said)) | Read::Sessions(Err(said)) => ("Threads", said),
        Read::Devices(Err((said, false))) => ("Devices", said),
        Read::Capabilities(Err(said)) => return Some(format!("What the host can do was not read: {said}")),
        _ => return None,
    };
    Some(format!("{what} not read: {said}"))
}

pub(super) async fn places(host: &HostClient) -> Read {
    Read::Places(host.call(req::<PlacesListRequest>(json!({}))).await.map(|r: Places| (r.places, r.pending)).map_err(said))
}

pub(super) async fn devices(host: &HostClient) -> Read {
    Read::Devices(host.call(req::<DevicesListRequest>(json!({}))).await.map(|r: Devices| r.devices).map_err(|f| {
        let ticket = f.kind.as_deref() == Some("ticket");
        (f.message, ticket)
    }))
}

/// Every read Settings makes as it opens, each sent as it lands (`settingsReads.ts` and the store's `pull`); `held`
/// leaves out the ones an opening inside `HOLD` draws from what the last Settings read.
pub(super) async fn read_all(host: &HostClient, tx: &mpsc::UnboundedSender<Arrival>, held: bool) {
    let send = |read: Read| {
        _ = tx.unbounded_send(Arrival::Read(Box::new(read)));
    };
    let preferences = async {
        send(Read::Preferences(
            host.call(req::<PreferencesGetRequest>(json!({}))).await.map(|r: PreferencesReply| r.preferences).map_err(said),
        ))
    };
    let places = async { send(places(host).await) };
    let projects = async {
        send(Read::Projects(host.call(req::<ProjectsListRequest>(json!({}))).await.map(|r: Projects| r.projects).map_err(said_and_fix)))
    };
    let harnesses = async {
        send(Read::Harnesses(host.call(req::<HarnessesListRequest>(json!({}))).await.map(|r: Harnesses| r.harnesses).map_err(said)))
    };
    let recipes =
        async { send(Read::Recipes(host.call(req::<RecipesListRequest>(json!({}))).await.map(|r: Recipes| r.recipes).map_err(said))) };
    let release =
        async { send(Read::Release(host.call(req::<ReleaseGetRequest>(json!({}))).await.map(|r: Release| r.release).map_err(said))) };
    let capabilities = async {
        send(Read::Capabilities(
            host.call(req::<CapabilitiesGetRequest>(json!({}))).await.map(|r: CapabilitiesReply| r.capabilities).map_err(said),
        ))
    };
    let setup = async {
        if !held {
            send(Read::Setup(host.call(req::<InitGetRequest>(json!({}))).await.map(|r| r.setup).map_err(said)))
        }
    };
    let account = async {
        if !held {
            send(Read::Account(host.call(req::<AccountGetRequest>(json!({}))).await.map(|r: Account| r.account).map_err(said)))
        }
    };
    let devices = async {
        if !held {
            send(devices(host).await)
        }
    };
    let image = async {
        if !held {
            send(Read::Image(host.call(req::<ImageGetRequest>(json!({}))).await.map(|r: Image| r.view).map_err(said)))
        }
    };
    let editors = async {
        if !held {
            send(Read::Editors(host.call(req::<EditorListRequest>(json!({}))).await.map(|r: Editors| r.editors).map_err(said)))
        }
    };
    let ssh = async {
        if !held {
            send(Read::SshInclude(host.call(req::<SshIncludeRequest>(json!({}))).await.map(|r: SshInclude| r.ssh_include).map_err(said)))
        }
    };
    let usage_off = async {
        if !held {
            send(Read::UsageOff(usage_off(host).await))
        }
    };
    let statuses = async {
        if let Ok(statuses) = host.statuses().await {
            send(Read::Statuses(statuses));
        }
        // The release is the one named as the socket that read the statuses was let in.
        send(Read::Version(host.host_release()));
        send(Read::Elsewhere(!host.here()));
    };
    futures::join!(
        preferences,
        places,
        projects,
        harnesses,
        recipes,
        release,
        capabilities,
        setup,
        account,
        devices,
        image,
        editors,
        ssh,
        usage_off,
        statuses
    );
}

/// `bootPayload()?.productUsageOff`: why the host sends no usage counts, off the boot line of the page the host here
/// serves; none from a host elsewhere, whose page this app does not load, and none where the page did not answer.
async fn usage_off(host: &HostClient) -> Option<String> {
    let page = crate::screens::sidebar::host_page(host.state()).filter(|_| host.here())?;
    let read = tokio::task::spawn_blocking(move || crate::screens::terminal::boot::read_boot_key(&page, "productUsageOff"));
    read.await.ok()?.ok().flatten().and_then(|off| off.as_str().map(str::to_owned))
}
