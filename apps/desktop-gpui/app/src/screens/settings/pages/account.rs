// SPDX-License-Identifier: AGPL-3.0-only
//! Account (`account.tsx`): the one row that says who this wsp is signed in to. The button is held: no op on the wire
//! signs the app in yet, and the row's sentence says so.

use gpui_kit::{App, Window};

use crate::screens::settings::page::{CardData, Ctx, Item};
use crate::ui::Button;

/// `ACCOUNT_WORDS`.
const GITHUB: &str = "GitHub";
const SIGN_IN: &str = "Sign in with GitHub";
const SIGN_OUT: &str = "Sign out";
const REACH: &str = "Sign in to use this wsp from outside your network. Not from the app yet.";
const REACHABLE: &str = "Reachable from another device outside your network. Not from the app yet.";

pub fn cards(ctx: &Ctx, _window: &mut Window, _cx: &mut App) -> Vec<CardData> {
    let account = ctx.data.account.as_ref();
    let signed_in = account.is_some_and(|a| a.signed_in);
    let login = account.filter(|_| signed_in).and_then(|a| a.login.clone());
    let row = Item::row("github", GITHUB, Some(if signed_in { REACHABLE } else { REACH }), |row| {
        let row = match login {
            Some(login) => row.value(login),
            None => row,
        };
        row.control(Button::primary("account-action", if signed_in { SIGN_OUT } else { SIGN_IN }).xs().held(true))
    });
    vec![CardData::new("account").item(row)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::settings::model::Data;
    use crate::screens::settings::pages::devices::shape;
    use gpui_kit::SharedString;

    #[gpui_kit::test]
    fn the_row_says_what_a_sign_in_buys(cx: &mut gpui_kit::TestAppContext) {
        let out = shape(Data::default(), cards, cx);
        assert_eq!(out, [(vec![vec![SharedString::from(GITHUB), SharedString::from(REACH)]], 0, false)], "no answer reads as signed out");
        let signed_in =
            Data { account: serde_json::from_value(serde_json::json!({ "signedIn": true, "login": "zingzy" })).ok(), ..Data::default() };
        assert_eq!(shape(signed_in, cards, cx), [(vec![vec![SharedString::from(GITHUB), SharedString::from(REACHABLE)]], 0, false)]);
    }
}
