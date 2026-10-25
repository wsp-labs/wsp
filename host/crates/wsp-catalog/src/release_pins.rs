// SPDX-License-Identifier: AGPL-3.0-only
//! The release every catalog row on the release road installs. Port of packages/catalog/src/release-pins.ts at
//! a3b870a6e; the spec is spec/roads.md.
use std::sync::LazyLock;

use indexmap::IndexMap;

use crate::roads::{InstallRoad, ReleaseAsset, ReleaseAssets};
use wsp_words::exit::HostError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleasePin {
    pub tag: String,
    pub assets: ReleaseAssets,
}

fn entry(repo: &str, tag: &str, x86: (&str, &str), arm: (&str, &str)) -> (String, ReleasePin) {
    let asset = |(name, sha256): (&str, &str)| ReleaseAsset { name: name.to_string(), sha256: sha256.to_string() };
    (repo.to_string(), ReleasePin { tag: tag.to_string(), assets: ReleaseAssets { x86_64: Some(asset(x86)), aarch64: Some(asset(arm)) } })
}

/// By repository, in the table's order (release-pins.ts:342).
pub static RELEASE_PINS: LazyLock<IndexMap<String, ReleasePin>> = LazyLock::new(|| {
    IndexMap::from([
        entry(
            "cli/cli",
            "v2.101.0",
            ("gh_2.101.0_linux_amd64.tar.gz", "9bca2d1c16825f109907a23307628a2f0698fbf99662b73a5cf0b020293072b8"),
            ("gh_2.101.0_linux_arm64.tar.gz", "b57e8063f18862647c9d22727c32e9da1b963f8bf9db648fe123a6975695640f"),
        ),
        entry(
            "cloudflare/cloudflared",
            "2026.9.1",
            ("cloudflared-linux-amd64", "03f1f25d1cc93b9ad6c60569d44060bc4f17ed97075760ed8cfca4b12dcd68cc"),
            ("cloudflared-linux-arm64", "3d97437c71848bd8df68041e12436b484a661d95073ea1937f01a845ce88faa3"),
        ),
        entry(
            "superfly/flyctl",
            "v0.4.105",
            ("flyctl_0.4.105_Linux_x86_64.tar.gz", "6e78298d82503f2aff2c6f41ede1178463ed286ce910c6be946a4b994ee68f29"),
            ("flyctl_0.4.105_Linux_arm64.tar.gz", "6ec0dc3656ecbe281231994080ec00238cecaa11aa65a121f53c51e3d2a69940"),
        ),
        entry(
            "supabase/cli",
            "v2.117.0",
            ("supabase_2.117.0_linux_amd64.tar.gz", "69c05f85b9e47ee706d30f1a6ca8a526b4e337bfd12c7ef1ef522d24e7280d24"),
            ("supabase_2.117.0_linux_arm64.tar.gz", "598c56a936fdf179ea486717901e6e49bc5d777b3ad317eab02f41faf21a95cb"),
        ),
        entry(
            "DopplerHQ/cli",
            "3.76.6",
            ("doppler_3.76.6_linux_amd64.tar.gz", "67e4e020761adf3ffe5a030712d61721b4e2752182670bf90de5a2a88e4961e3"),
            ("doppler_3.76.6_linux_arm64.tar.gz", "621456ac08436c4037a72c8641e4bdcccd0e404ab43b79a61d449445bb02e6e5"),
        ),
        entry(
            "mikefarah/yq",
            "v4.53.6",
            ("yq_linux_amd64", "c5f056448f973ae7d39b5401949648a78f2dc1947d6a8eb65be60d5c504b9385"),
            ("yq_linux_arm64", "88a1016bc1d657375a35864e4f44b6f333df8ff97b559f51bba0adcb2169df09"),
        ),
        entry(
            "golangci/golangci-lint",
            "v2.13.2",
            ("golangci-lint-2.13.2-linux-amd64.tar.gz", "2277d43b98ec0054280f2ac26b53268bae97682444678a59a657dd565da021d6"),
            ("golangci-lint-2.13.2-linux-arm64.tar.gz", "a2a4e0065aa41be71f7c5ac90f271b61751331e5d04314e62afe4027855f0893"),
        ),
        entry(
            "jdx/mise",
            "v2026.9.12",
            ("mise-v2026.9.12-linux-x64", "e79ae57945034903aee8aa2ea66b4c7ca9cd4f4edd5a8a78a589cbae6d0f428a"),
            ("mise-v2026.9.12-linux-arm64", "f344c6961190ed2f68e595ed7cb4f03c36c17812bd608886bec799a3082180ff"),
        ),
        entry(
            "dandavison/delta",
            "0.19.2",
            ("delta-0.19.2-x86_64-unknown-linux-gnu.tar.gz", "8e695c5f586a8c53d6c3b01be0b4a422ed218bfed2a56191caebe373a1c18ab2"),
            ("delta-0.19.2-aarch64-unknown-linux-gnu.tar.gz", "0bfce159a5cddd5feb3d6db4a616d883ff51253ce08ac7ec11cb1d208cfaab9e"),
        ),
        entry(
            "bazelbuild/bazelisk",
            "v1.29.0",
            ("bazelisk-linux-amd64", "5a408715e932c0250d28bd84555f12edbf70117de42f9181691c736eacc4a992"),
            ("bazelisk-linux-arm64", "e20e8b0f4f240091b7a55bf17b9398bd4f40ee70ae0208dff95dd4c445fb4010"),
        ),
        entry(
            "charmbracelet/crush",
            "v0.96.1",
            ("crush_0.96.1_Linux_x86_64.tar.gz", "5411b0906a82162dcab4a99071d70accf1caad0eee69789416dd607943c6680d"),
            ("crush_0.96.1_Linux_arm64.tar.gz", "4bfe4a37aedeb4219d51eb7a25b837304084070378e77fd578999764b487f01f"),
        ),
        entry(
            "aaif-goose/goose",
            "v1.52.0",
            ("goose-x86_64-unknown-linux-gnu.tar.gz", "4aee1f770b405c44194c0e9407df1fb06bda4c50eee935f0d8fd10731821cc5e"),
            ("goose-aarch64-unknown-linux-gnu.tar.gz", "ae602c4f6e9a785bf087da52c89908d4dc6aa605dcc17bf83293873f626d9c85"),
        ),
    ])
});

/// The release road at the pinned tag and assets, `go` set where given; errors "the release pins table names no
/// <repo>" (release-pins.ts:373).
pub fn pinned_release(repo: &str, go: Option<&str>) -> Result<InstallRoad, HostError> {
    let pin = RELEASE_PINS.get(repo).ok_or_else(|| HostError::new(format!("the release pins table names no {repo}")))?;
    Ok(InstallRoad::Release {
        repo: Some(repo.to_string()),
        version: Some(pin.tag.clone()),
        assets: Some(pin.assets.clone()),
        pin: None,
        go: go.map(str::to_string),
    })
}
