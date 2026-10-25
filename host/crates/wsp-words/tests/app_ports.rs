// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/protocol/test/app-ports.test.ts:24-136 (addresses, the port asked, the bind address, the join
//! address, the holder lines). Skipped, law: :138-151 (no other source file spells 4400 or the loopback address).

use wsp_wire::types::app_ports::{LOOPBACK, PORT_TAKEN_REFUSAL, WS_PATH};
use wsp_words::app_ports::*;

fn asked(port: f64, named: bool, address: &str) -> ListenAsked {
    ListenAsked { port, named, address: address.to_string() }
}

fn process(command: &str, pid: u32) -> PortHolder {
    PortHolder::Process(PortProcess { command: command.to_string(), pid })
}

fn host(state_path: &str) -> PortHolder {
    PortHolder::Host { state_path: state_path.to_string() }
}

mod an_address_where_an_alias_could_go {
    use super::*;

    #[test]
    fn is_a_word_with_an_http_https_ws_or_wss_scheme_and_nothing_else() {
        for word in ["http://box:4400", "https://box.example/wsp", "ws://box:4410", "WSS://box"] {
            assert!(is_url(word), "{word}");
        }
        for word in ["box", "box:4400", "127.0.0.1:14400", "ftp://box", "http:/box", ""] {
            assert!(!is_url(word), "{word}");
        }
    }

    #[test]
    fn the_name_of_the_computer_comes_back_only_for_an_address_a_host_is_served_at() {
        assert_eq!(served_hostname("http://box:4400"), Some("box".to_string()));
        assert_eq!(served_hostname("https://box.example/wsp"), Some("box.example".to_string()));
        assert_eq!(served_hostname("HTTP://Box.Example"), Some("box.example".to_string()));
        assert_eq!(served_hostname("http://[2001:db8::5]:4400"), Some("[2001:db8::5]".to_string()));
        // A ws or wss address is where a socket is dialled, not where a host is served, and every spelling that would
        // throw out of the URL parser reads as no address rather than as a stack.
        for word in ["ws://box:4410", "WSS://box", "http://", "https://", "box", "ftp://box", ""] {
            assert_eq!(served_hostname(word), None, "{word}");
        }
    }
}

mod the_port_the_app_is_served_on {
    use super::*;

    #[test]
    fn names_4400_as_the_port_nobody_named() {
        assert_eq!(ports_asked(None, None), asked(4400.0, false, LOOPBACK));
    }

    #[test]
    fn port_names_the_port_and_says_a_person_named_it() {
        assert_eq!(ports_asked(Some("4401"), None), asked(4401.0, true, LOOPBACK));
        assert_eq!(ports_asked(Some("5000"), None), asked(5000.0, true, LOOPBACK));
    }

    #[test]
    fn keeps_port_0_as_it_is_since_0_asks_for_any_free_port() {
        assert_eq!(ports_asked(Some("0"), None), asked(0.0, true, LOOPBACK));
    }
}

mod the_address_the_port_is_bound_on {
    use super::*;

    #[test]
    fn binds_this_computer_alone_unless_listen_names_another_and_an_empty_word_is_no_wo() {
        assert_eq!(ports_asked(None, None).address, "127.0.0.1");
        assert_eq!(ports_asked(None, Some("")).address, LOOPBACK);
        assert_eq!(ports_asked(None, Some("0.0.0.0")).address, "0.0.0.0");
        assert_eq!(ports_asked(None, Some("100.64.0.3")), asked(4400.0, false, "100.64.0.3"));
    }

    #[test]
    fn reads_127_x_1_and_localhost_as_this_computer_and_the_wildcard_a_private_address() {
        for at in [LOOPBACK, "127.0.0.2", "127.1.2.3", "::1", "[::1]", "localhost"] {
            assert!(is_loopback(at), "{at}");
        }
        for at in
            ["0.0.0.0", "::", "192.168.1.20", "100.64.0.3", "box.example.com", "1270.0.0.1", "127.evil.example", "127.0.0.1.evil.example"]
        {
            assert!(!is_loopback(at), "{at}");
        }
    }

    #[test]
    fn says_once_in_one_line_with_no_em_dash_that_the_page_is_reachable_and_pairing_is() {
        let line = listen_beyond_loopback_line("0.0.0.0");
        assert!(line.contains("0.0.0.0"), "{line}");
        assert!(line.contains("wsp host pair"), "{line}");
        assert!(!line.contains('\u{2014}'), "{line}");
    }

    #[test]
    fn serves_the_runtime_on_one_path_so_a_forwarded_port_carries_the_page_and_the_prot() {
        assert_eq!(WS_PATH, "/ws");
    }

    #[test]
    fn names_the_wildcard_which_covers_loopback_apart_from_an_address_that_answers_only() {
        for at in ["0.0.0.0", "::"] {
            assert!(is_wildcard(at), "{at}");
        }
        for at in [LOOPBACK, "::1", "100.64.0.3", "0.0.0.1"] {
            assert!(!is_wildcard(at), "{at}");
        }
    }

    #[test]
    fn brackets_an_ipv6_literal_in_a_url_authority_and_leaves_everything_else_alone() {
        assert_eq!(authority("127.0.0.1", 4400.0), "127.0.0.1:4400");
        assert_eq!(authority("box.example.com", 4400.0), "box.example.com:4400");
        assert_eq!(authority("2001:db8::5", 4410.0), "[2001:db8::5]:4410");
        assert_eq!(authority("[2001:db8::5]", 4410.0), "[2001:db8::5]:4410");
    }
}

mod the_address_a_person_types_to_join_a_wsp {
    use super::*;

    #[test]
    fn takes_the_authority_the_other_computer_shows_as_an_http_address_and_an_address_t() {
        assert_eq!(join_address_of("192.168.1.20:7788"), Some("http://192.168.1.20:7788".to_string()));
        assert_eq!(join_address_of(" 192.168.1.20:7788 "), Some("http://192.168.1.20:7788".to_string()));
        assert_eq!(join_address_of("old-macbook.local:4400"), Some("http://old-macbook.local:4400".to_string()));
        assert_eq!(join_address_of("[::1]:4400"), Some("http://[::1]:4400".to_string()));
        assert_eq!(join_address_of("http://192.168.1.20:7788"), Some("http://192.168.1.20:7788".to_string()));
        assert_eq!(join_address_of("https://p-x.usewsp.com"), Some("https://p-x.usewsp.com".to_string()));
    }

    #[test]
    fn is_nothing_for_a_word_that_names_no_wsp_a_name_with_no_port_a_socket_address_an() {
        for word in ["", "   ", "box", "192.168.1.20", "ws://192.168.1.20:7788", "wss://box", "http://", "192.168.1.20:", "not an address"]
        {
            assert_eq!(join_address_of(word), None, "{word}");
        }
    }
}

mod who_holds_a_port_and_the_lines_about_it {
    use super::*;

    #[test]
    fn names_a_wsp_host_by_the_state_file_it_serves_a_process_by_its_command_and_pid_an() {
        assert_eq!(port_holder_words(Some(&host("/Users/z/.wsp/state.json"))), "the host serving /Users/z/.wsp/state.json");
        assert_eq!(port_holder_words(Some(&process("node", 62569))), "node (pid 62569)");
        assert_eq!(port_holder_words(None), "another process");
    }

    #[test]
    fn the_taken_line_names_the_port_and_its_holder() {
        assert_eq!(port_taken_line(4410.0, Some(&process("node", 62569))), "Port 4410 is in use on this computer by node (pid 62569).");
        assert_eq!(port_taken_line(4400.0, None), "Port 4400 is in use on this computer by another process.");
    }

    #[test]
    fn the_picked_line_names_the_port_it_serves_on_and_the_port_it_stepped_over_with_it() {
        assert_eq!(
            ports_picked_line(&AppPorts { port: 4401 }, 4400.0, Some(&host("/Users/z/.wsp/state.json"))),
            "Serving on 4401; 4400 is held by the host serving /Users/z/.wsp/state.json."
        );
    }

    #[test]
    fn the_refusal_offers_stopping_the_holder_or_naming_a_free_port_with_port() {
        assert_eq!(PORT_TAKEN_REFUSAL, "Nothing was booted. Stop that process, or name a free port with --port.");
        assert!(!PORT_TAKEN_REFUSAL.contains('\u{2014}'), "{PORT_TAKEN_REFUSAL}");
    }

    #[test]
    fn the_state_file_line_names_the_file_and_the_flag_that_starts_a_fresh_setup() {
        assert_eq!(
            state_file_line("/Users/z/.wsp/state.json"),
            "Setting up /Users/z/.wsp/state.json; --state <path> starts a fresh setup instead."
        );
    }
}

mod ws_url_of {
    use wsp_words::app_ports::ws_url_of;

    #[test]
    fn turns_an_address_into_the_one_the_runtime_answers_on() {
        // node 22: wsUrlOf(url) for each address.
        let wsp = |url| ws_url_of(url).unwrap();
        assert_eq!(wsp("https://h.example/p/"), "wss://h.example/p/ws");
        assert_eq!(wsp("http://127.0.0.1:2222"), "ws://127.0.0.1:2222/ws");
        assert_eq!(wsp("wss://x/a//"), "wss://x/a/ws");
        assert_eq!(wsp("ws://[::1]:80/x"), "ws://[::1]/x/ws");
        assert_eq!(wsp("HTTPS://Host.Example:443/a b/ü?q#f"), "wss://host.example/a%20b/%C3%BC/ws");
        assert_eq!(wsp("http://h:80"), "ws://h/ws");
        assert_eq!(wsp("http://h:8080/%7Ea/./b/../c"), "ws://h:8080/%7Ea/c/ws");
        assert_eq!(wsp("https://bücher.example/x"), "wss://xn--bcher-kva.example/x/ws");
        assert_eq!(wsp("http://user:pw@h/x"), "ws://h/x/ws");
        assert_eq!(wsp("ssh://h/x"), "ws://h/x/ws");
        assert_eq!(wsp("http://h//"), "ws://h/ws");
        assert_eq!(wsp("https://h/a%2Fb/"), "wss://h/a%2Fb/ws");
    }

    #[test]
    fn throws_invalid_url_as_an_error_where_the_address_does_not_parse() {
        for bad in ["nope", ""] {
            assert_eq!(ws_url_of(bad).unwrap_err().message, "Invalid URL", "{bad:?}");
        }
    }
}
