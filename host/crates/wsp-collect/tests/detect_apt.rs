// SPDX-License-Identifier: AGPL-3.0-only
//! The apt reader: which of a Linux computer's packages are a person's own choice, measured against the rule the base
//! image's own 300-odd packages need to be told apart from the handful somebody typed. Port of
//! packages/collect/test/apt.test.ts at a3b870a6e.
mod common;

use common::fake_host::*;
use indexmap::IndexMap;
use wsp_collect::detect::apt::{apt_packages, apt_sizes, parse_apt_manual, parse_dpkg_fields};
use wsp_collect::host::Platform;

const MANUAL: &str = "direnv\nfd-find\nfish\nfonts-dejavu-core\nlibpq5\nlsb-release\nsudo\ntmux\nubuntu-minimal";

const PRIORITY: &str = "direnv optional\nfd-find optional\nfish optional\nfonts-dejavu-core extra\nlibpq5 optional\nlsb-release optional\nsudo optional\ntmux optional\nubuntu-minimal important\n";

const SIZES: &str = "direnv 9000\nfd-find 3000\nfish 20000\ntmux 1100\n";

/// dpkg's own file lists: a package installed for one architecture is named with it.
const LISTS: [(&str, &str); 9] = [
    ("/var/lib/dpkg/info/direnv:amd64.list", "/usr/bin/direnv\n/usr/share/doc/direnv/copyright\n"),
    ("/var/lib/dpkg/info/fd-find.list", "/usr/bin/fdfind\n"),
    ("/var/lib/dpkg/info/fish.list", "/usr/share/fish/config.fish\n/usr/bin/fish\n"),
    ("/var/lib/dpkg/info/fonts-dejavu-core.list", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf\n"),
    ("/var/lib/dpkg/info/libpq5:amd64.list", "/usr/lib/x86_64-linux-gnu/libpq.so.5.16\n"),
    ("/var/lib/dpkg/info/lsb-release.list", "/usr/bin/lsb_release\n"),
    ("/var/lib/dpkg/info/sudo.list", "/usr/bin/sudo\n"),
    ("/var/lib/dpkg/info/tmux.list", "/usr/bin/tmux\n"),
    ("/var/lib/dpkg/info/ubuntu-minimal.list", "/usr/share/doc/ubuntu-minimal/copyright\n"),
];

fn ubuntu(over: &[(&str, &str)]) -> FakeHost {
    let mut laptop = FakeLaptop::new()
        .platform(Platform::Linux)
        .which(&["apt-mark", "dpkg-query"])
        .exec("apt-mark showmanual", MANUAL)
        .exec("dpkg-query -Wf ${Package} ${Priority}\n", PRIORITY)
        .exec("dpkg-query -Wf ${Package} ${Installed-Size}\n", SIZES);
    for (path, text) in LISTS {
        laptop = laptop.file(path, text);
    }
    for (key, out) in over {
        laptop = laptop.exec(key, out);
    }
    fake_host(laptop)
}

mod the_apt_reader {
    use super::*;

    #[test]
    fn reads_a_package_and_its_field_from_dpkg_query_dropping_a_line_that_names_no_value() {
        let got = parse_dpkg_fields("direnv optional\nfish extra\nbroken\n\n");
        let want: IndexMap<String, String> =
            [("direnv".to_string(), "optional".to_string()), ("fish".to_string(), "extra".to_string())].into_iter().collect();
        assert_eq!(got, want);
    }

    #[test]
    fn reads_the_manual_list_as_names_blank_lines_dropped() {
        assert_eq!(parse_apt_manual("direnv\n\nfish\n"), vec!["direnv".to_string(), "fish".to_string()]);
    }

    #[test]
    fn keeps_the_packages_a_person_chose_and_drops_the_distros_own() {
        // What the catalog or another rung already carries is dropped by the caller, so tmux and fd-find are in this list.
        let names: Vec<String> = block_on(apt_packages(&ubuntu(&[]))).into_iter().map(|p| p.name).collect();
        assert_eq!(names, vec!["direnv", "fd-find", "fish", "tmux"]);
    }

    #[test]
    fn reads_one_directory_and_two_dpkg_query_runs_never_a_dpkg_l_per_package() {
        let host = ubuntu(&[]);
        block_on(apt_packages(&host));
        let calls = host.calls();
        assert_eq!(calls.iter().filter(|c| c.starts_with("run dpkg-query")).count(), 1);
        assert_eq!(calls.iter().filter(|c| c.contains("dpkg -L")).cloned().collect::<Vec<_>>(), Vec::<String>::new());
        // Only the packages that got as far as the file test are read.
        assert_eq!(
            calls.iter().filter(|c| c.starts_with("lines ")).cloned().collect::<Vec<_>>(),
            vec![
                "lines /var/lib/dpkg/info/direnv:amd64.list",
                "lines /var/lib/dpkg/info/fd-find.list",
                "lines /var/lib/dpkg/info/fish.list",
                "lines /var/lib/dpkg/info/fonts-dejavu-core.list",
                "lines /var/lib/dpkg/info/libpq5:amd64.list",
                "lines /var/lib/dpkg/info/tmux.list",
            ]
        );
    }

    #[test]
    fn says_nothing_when_apt_mark_answers_nothing() {
        let got = block_on(apt_packages(&ubuntu(&[("apt-mark showmanual", "")])));
        assert!(got.is_empty());
    }

    #[test]
    fn reads_each_packages_size_from_dpkgs_own_record_in_bytes() {
        let got = block_on(apt_sizes(&ubuntu(&[]), &["direnv".to_string(), "fish".to_string()]));
        let want: IndexMap<String, f64> =
            [("direnv".to_string(), 9000.0 * 1024.0), ("fish".to_string(), 20000.0 * 1024.0)].into_iter().collect();
        assert_eq!(got, want);
    }

    #[test]
    fn asks_dpkg_nothing_when_no_package_needs_a_size() {
        let host = ubuntu(&[]);
        let got = block_on(apt_sizes(&host, &[]));
        assert!(got.is_empty());
        assert_eq!(host.calls(), Vec::<String>::new());
    }
}

/// Rules of spec/apt_tools.md the TS cases leave unpinned, values read off the TS at a3b870a6e.
mod pins {
    use super::*;

    #[test]
    fn a_repeated_dpkg_line_replaces_the_value_in_place() {
        let got = parse_dpkg_fields("a 1\nb 2\na 3\nc 4 5\nd\t6\r\n");
        let want: IndexMap<String, String> =
            [("a", "3"), ("b", "2"), ("d", "6")].into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        assert_eq!(got.into_iter().collect::<Vec<_>>(), want.into_iter().collect::<Vec<_>>());
    }

    #[test]
    fn apt_mark_naming_only_unchosen_packages_runs_no_dpkg_query() {
        let host = fake_host(
            FakeLaptop::new()
                .platform(Platform::Linux)
                .which(&["apt-mark", "dpkg-query"])
                .exec("apt-mark showmanual", "sudo\nlsb-release\n")
                .exec("dpkg-query -Wf ${Package} ${Priority}\n", "sudo optional\n"),
        );
        assert!(block_on(apt_packages(&host)).is_empty());
        assert_eq!(host.calls(), vec!["run apt-mark showmanual".to_string()]);
    }

    #[test]
    fn a_later_list_file_for_a_package_replaces_the_earlier() {
        // The fake lists names sorted, so `tool:amd64.list` comes before `tool:i386.list`, and the second is the one read.
        let host = fake_host(
            FakeLaptop::new()
                .platform(Platform::Linux)
                .exec("apt-mark showmanual", "tool\ntool\n")
                .exec("dpkg-query -Wf ${Package} ${Priority}\n", "tool optional\n")
                .file("/var/lib/dpkg/info/tool:amd64.list", "/usr/share/doc/tool\n")
                .file("/var/lib/dpkg/info/tool:i386.list", "/bin/tool\n")
                .file("/var/lib/dpkg/info/.list", "/bin/nameless\n"),
        );
        let got = block_on(apt_packages(&host));
        // A name the manual list repeats is kept twice.
        assert_eq!(
            got.iter().map(|p| (p.name.as_str(), p.version.clone(), p.path.clone())).collect::<Vec<_>>(),
            vec![("tool", None, None), ("tool", None, None)]
        );
        assert_eq!(
            host.calls().iter().filter(|c| c.starts_with("lines ")).cloned().collect::<Vec<_>>(),
            vec!["lines /var/lib/dpkg/info/tool:i386.list".to_string(); 2]
        );
    }

    #[test]
    fn a_size_is_read_as_javascripts_number_reads_it() {
        let host = fake_host(
            FakeLaptop::new()
                .exec("dpkg-query -Wf ${Package} ${Installed-Size}\n", "a 0\nb -5\nc abc\nd 1e1\ne 0x10\nf 12.5\ng Infinity\n"),
        );
        let names: Vec<String> = ["a", "b", "c", "d", "e", "f", "g", "missing", "d"].iter().map(|s| s.to_string()).collect();
        let got = block_on(apt_sizes(&host, &names));
        assert_eq!(
            got.into_iter().collect::<Vec<_>>(),
            vec![("d".to_string(), 10240.0), ("e".to_string(), 16384.0), ("f".to_string(), 12800.0)]
        );
    }
}
