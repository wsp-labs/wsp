// SPDX-License-Identifier: AGPL-3.0-only
//! Why a dev server's page shows white (`browser/refusal.ts`): what one fetch of the route answered, read for the host
//! check that refused it. Vite reads the allowance the thread's environment sets on start; the others only their config.

/// `Refusal`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub title: String,
    pub detail: String,
    /// The answer may be the app's own page (a signed-out route, its own sign-in), so the sentence sits above the page.
    pub keeps_frame: bool,
}

/// `/Blocked request\. This host \(.+\) is not allowed/`, whose `.+` stays on one line.
fn vite_blocked(body: &str) -> bool {
    const HEAD: &str = "Blocked request. This host (";
    const TAIL: &str = ") is not allowed";
    body.match_indices(HEAD).any(|(at, _)| {
        let rest = &body[at + HEAD.len()..];
        let line = rest.split(['\n', '\r', '\u{2028}', '\u{2029}']).next().unwrap_or("");
        line.match_indices(TAIL).any(|(i, _)| i >= 1)
    })
}

/// `/Blocked hosts?:/`.
fn rails_blocked(body: &str) -> bool {
    body.contains("Blocked host:") || body.contains("Blocked hosts:")
}

/// `explainRefusal`: the sentence for a 401 or 403 the route answered with, none for any other answer.
pub fn explain_refusal(status: u16, body: &str, port: u16, host: &str) -> Option<Refusal> {
    let refusal = |title: String, detail: String, keeps_frame: bool| Some(Refusal { title, detail, keeps_frame });
    if status == 401 {
        return refusal(
            format!(":{port} answered 401 through the preview route"),
            format!(
                "The route refused a fresh token too: the host reminted it after the first 401 and reloaded once. If the server on :{port} asks for its own sign-in, this is its page; otherwise the preview edge is refusing this thread."
            ),
            true,
        );
    }
    if status != 403 {
        return None;
    }
    let refused = format!(":{port} refused the preview host");
    let body = body.trim();
    if vite_blocked(body) {
        return refusal(
            refused,
            format!(
                "Vite blocked the request because {host} is not in server.allowedHosts. Restart the dev server so it picks up the allowance the thread's environment already sets, or add the host to server.allowedHosts in vite.config."
            ),
            false,
        );
    }
    if body == "Invalid Host header" {
        return refusal(
            refused,
            format!(
                "webpack-dev-server blocked the request because {host} is not in devServer.allowedHosts. Add the host to devServer.allowedHosts in the webpack config, or start it with --allowed-hosts {host}, and restart it."
            ),
            false,
        );
    }
    if rails_blocked(body) {
        return refusal(
            refused,
            format!(
                "Rails blocked the request because {host} is not in config.hosts. Add config.hosts << \"{host}\" to config/environments/development.rb and restart the server."
            ),
            false,
        );
    }
    if body == "Unauthorized" {
        return refusal(
            refused,
            format!(
                "If this is Next.js, its dev server blocked a request from {host}: add the host to allowedDevOrigins in next.config and restart it. Any server can answer 403 Unauthorized, so check the server's own log if that is not it."
            ),
            false,
        );
    }
    refusal(
        format!(":{port} answered 403 through the preview route"),
        format!("The server on :{port} refused {host}. If it checks the Host header, allow that host in its config and restart it."),
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOST: &str = "d1f292946d3bf9229dff-5173.preview.getsolari.com";

    fn explain(status: u16, body: &str, port: u16) -> Refusal {
        explain_refusal(status, body, port, HOST).unwrap()
    }

    #[test]
    fn vites_host_check_is_named_from_the_measured_body_with_both_fixes_and_no_page() {
        let body = format!("Blocked request. This host ({HOST}) is not allowed. To allow this host, add it to server.allowedHosts");
        let refusal = explain(403, &body, 5173);
        assert_eq!(refusal.title, ":5173 refused the preview host");
        for word in ["Vite", HOST, "Restart the dev server", "server.allowedHosts"] {
            assert!(refusal.detail.contains(word), "{word}");
        }
        assert!(!refusal.keeps_frame);
    }

    #[test]
    fn vites_newer_body_reads_too_with_the_host_quoted_and_the_fix_on_its_own_line() {
        let body = format!("Blocked request. This host (\"{HOST}\") is not allowed.\nTo allow this host, add \"{HOST}\" to `server.allowedHosts` in vite.config.js.");
        assert!(explain(403, &body, 5173).detail.contains("Vite"));
    }

    #[test]
    fn vites_check_needs_the_host_and_the_refusal_on_one_line() {
        let refusal = explain(403, "Blocked request. This host (\n) is not allowed", 5173);
        assert_eq!(refusal.title, ":5173 answered 403 through the preview route");
    }

    #[test]
    fn webpack_dev_servers_allowed_hosts_is_named_from_its_exact_body() {
        let refusal = explain(403, "Invalid Host header", 8080);
        assert_eq!(refusal.title, ":8080 refused the preview host");
        for word in ["webpack-dev-server", HOST, "allowedHosts"] {
            assert!(refusal.detail.contains(word), "{word}");
        }
        assert!(!refusal.keeps_frame);
    }

    #[test]
    fn rails_config_hosts_is_named_from_its_host_authorization_page() {
        let body = format!(
            "<!DOCTYPE html><html><head><title>Action Controller: Exception caught</title></head><body><header><h1>Blocked hosts: {HOST}</h1></header><div id=\"container\"><h2>To allow requests to {HOST}, add the following to your environment configuration:</h2><pre>config.hosts &lt;&lt; \"{HOST}\"</pre></div></body></html>"
        );
        let refusal = explain(403, &body, 3000);
        assert_eq!(refusal.title, ":3000 refused the preview host");
        for word in ["Rails", HOST, "config.hosts"] {
            assert!(refusal.detail.contains(word), "{word}");
        }
        assert!(!refusal.keeps_frame);
    }

    #[test]
    fn a_bare_403_unauthorized_offers_nexts_allowed_dev_origins_hedged() {
        let refusal = explain(403, "Unauthorized", 3000);
        assert_eq!(refusal.title, ":3000 refused the preview host");
        assert!(refusal.detail.starts_with("If this is Next.js"));
        assert!(refusal.detail.contains("allowedDevOrigins"));
        assert!(refusal.detail.contains("next.config"));
        assert!(!refusal.keeps_frame);
    }

    #[test]
    fn any_other_403_keeps_the_page_and_explains_it_without_guessing_the_server() {
        let refusal = explain(403, "<html>Forbidden</html>", 5173);
        assert_eq!(refusal.title, ":5173 answered 403 through the preview route");
        assert!(refusal.detail.contains(HOST));
        assert!(!refusal.detail.contains("Vite"));
        assert!(!refusal.detail.contains("Next"));
        assert!(refusal.keeps_frame);
    }

    #[test]
    fn a_401_keeps_the_page_and_says_the_token_was_already_reminted_once() {
        let refusal = explain(401, "", 5173);
        assert_eq!(refusal.title, ":5173 answered 401 through the preview route");
        assert!(refusal.detail.contains("fresh token"));
        assert!(refusal.keeps_frame);
    }

    #[test]
    fn every_other_answer_is_left_to_the_page() {
        for (status, body) in [(200, "<!doctype html>"), (302, ""), (404, "Not found"), (500, "boom"), (502, "")] {
            assert_eq!(explain_refusal(status, body, 5173, HOST), None, "{status}");
        }
    }
}
