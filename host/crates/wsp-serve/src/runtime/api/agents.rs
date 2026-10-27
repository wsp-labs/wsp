// SPDX-License-Identifier: AGPL-3.0-only
use std::rc::Rc;

use indexmap::IndexMap;
use wsp_core::types::api::{
    AgentsApi, ConversationAsk, ConversationsApi, ImageApi, ImageBuildAsk, KeyWay, OutsideOpening, Placed, PlacedStart, PreferencesApi,
    PreferencesSet, SignInFollowing, SkillsAddAsk, ToolsAsk,
};
use wsp_core::types::pending::{CallbackForwards, SignInAsk, SkillAsk};
use wsp_core::types::wiring::Detach;
use wsp_machines::agents_read as reads;
use wsp_machines::images::ImageError;
use wsp_wire::types::agents_report::{
    AgentRow, AgentsReport, AgentsSignInEvent, AgentsSignInRun, AgentsTarget, PluginAsk, PluginRow, ServerAdd, ServerAsk,
    ServerToolsAnswer, SignInLine, SkillAdded, SkillHit, SkillPreview,
};
use wsp_wire::types::conversations::{ConversationRead, ConversationsAnswer, ResumeAsk};
use wsp_wire::types::thread_defaults::AgentSetupSet;
use wsp_wire::views::golden_image::{SealedImage, SealedImageBuilt};
use wsp_wire::views::preferences::{Preferences, PreferencesPatch};
use wsp_wire::views::snapshot_lineage::SealedImageView;
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

use super::{AgentsApiOf, ConversationsApiOf, ImageApiOf, PreferencesApiOf};

fn following_of(following: reads::Following) -> SignInFollowing {
    SignInFollowing { sign_in_id: following.sign_in_id, leave: following.leave }
}

// The wire never carries toolThere or login, so both stay unset.
fn read_sign_in(ask: SignInAsk) -> reads::SignInAsk {
    reads::SignInAsk {
        agent: ask.agent,
        server: ask.server,
        terminal: ask.terminal,
        tool_there: None,
        login: None,
        scope: ask.scope,
        project: ask.project,
    }
}

fn read_skill(ask: SkillAsk) -> reads::SkillAsk {
    reads::SkillAsk { name: ask.name, project: ask.project }
}

#[async_trait::async_trait(?Send)]
impl AgentsApi for AgentsApiOf {
    async fn read(&self, target: AgentsTarget, origin: Option<Caller>) -> Result<AgentsReport, HostError> {
        self.0.agents_read().read(&target, origin.as_ref()).await
    }

    async fn tools(&self, target: AgentsTarget, ask: ToolsAsk, origin: Option<Caller>) -> Result<ServerToolsAnswer, HostError> {
        self.0.agents_read().tools(&target, &ask.agent, &ask.name, ask.refresh, origin.as_ref()).await
    }

    async fn sign_in(
        &self,
        target: AgentsTarget,
        ask: SignInAsk,
        emit: Rc<dyn Fn(AgentsSignInEvent)>,
        origin: Option<Caller>,
    ) -> Result<SignInFollowing, HostError> {
        let following = self.0.agents_read().sign_in(target, read_sign_in(ask), emit, origin).await?;
        Ok(following_of(following))
    }

    fn sign_ins(&self) -> Vec<AgentsSignInRun> {
        self.0.agents_read().sign_ins()
    }

    fn sign_in_follow(&self, sign_in_id: &str, emit: Rc<dyn Fn(AgentsSignInEvent)>) -> Result<SignInFollowing, HostError> {
        Ok(following_of(self.0.agents_read().sign_in_follow(sign_in_id, emit)?))
    }

    async fn sign_in_code(&self, sign_in_id: &str, code: &str) -> Result<(), HostError> {
        self.0.agents_read().sign_in_code(sign_in_id, code).await
    }

    fn sign_in_stop(&self, sign_in_id: &str) {
        let _ = self.0.agents_read().sign_in_stop(sign_in_id);
    }

    async fn sign_in_line(&self, target: AgentsTarget, ask: SignInAsk, origin: Option<Caller>) -> Result<SignInLine, HostError> {
        self.0.agents_read().sign_in_line(&target, &read_sign_in(ask), origin.as_ref()).await
    }

    async fn key(&self, agent: &str, key: &str, way: Option<KeyWay>) -> Result<(), HostError> {
        let way = way.map(|way| match way {
            KeyWay::Token => "token",
            KeyWay::Key => "key",
        });
        self.0.agents_read().key(agent, key, way).await
    }

    async fn add_tools(&self, target: AgentsTarget, agent: &str, origin: Option<Caller>) -> Result<String, HostError> {
        self.0.agents_read().add_tools(&target, agent, origin.as_ref()).await
    }

    fn forwards(&self, relay: Rc<dyn CallbackForwards>) -> Detach {
        // A relay that is not the machines' holder is one no road reaches: held as no relay, as the TS holds none.
        let any: Rc<dyn std::any::Any> = relay;
        match any.downcast::<reads::ForwardsHolder>() {
            Ok(held) => self.0.agents_read().forwards(held.0.clone()),
            Err(_) => Box::new(|| {}),
        }
    }

    async fn skills_search(&self, q: &str, limit: Option<u32>) -> Result<Vec<SkillHit>, HostError> {
        self.0.agents_read().skills_search(q, limit).await
    }

    async fn skills_get(&self, skill: &str) -> Result<SkillPreview, HostError> {
        self.0.agents_read().skills_get(skill).await
    }

    async fn skills_preview(&self, target: AgentsTarget, ask: SkillAsk, origin: Option<Caller>) -> Result<SkillPreview, HostError> {
        self.0.agents_read().skills_preview(&target, &read_skill(ask), origin.as_ref()).await
    }

    async fn skills_add(&self, target: AgentsTarget, ask: SkillsAddAsk, origin: Option<Caller>) -> Result<SkillAdded, HostError> {
        let ask = reads::SkillAddAsk { skill: ask.skill, agents: ask.agents, project: ask.project };
        self.0.agents_read().skills_add(&target, &ask, origin.as_ref()).await
    }

    async fn skills_remove(&self, target: AgentsTarget, ask: SkillAsk, origin: Option<Caller>) -> Result<Vec<String>, HostError> {
        self.0.agents_read().skills_remove(&target, &read_skill(ask), origin.as_ref()).await
    }

    async fn skills_toggle(&self, target: AgentsTarget, ask: SkillAsk, on: bool, origin: Option<Caller>) -> Result<Vec<String>, HostError> {
        self.0.agents_read().skills_toggle(&target, &read_skill(ask), on, origin.as_ref()).await
    }

    async fn servers_add(&self, target: AgentsTarget, ask: ServerAdd, origin: Option<Caller>) -> Result<String, HostError> {
        self.0.agents_read().servers_add(&target, &ask, origin.as_ref()).await
    }

    async fn servers_remove(&self, target: AgentsTarget, ask: ServerAsk, origin: Option<Caller>) -> Result<String, HostError> {
        self.0.agents_read().servers_remove(&target, &ask, origin.as_ref()).await
    }

    async fn servers_toggle(&self, target: AgentsTarget, ask: ServerAsk, on: bool, origin: Option<Caller>) -> Result<String, HostError> {
        self.0.agents_read().servers_toggle(&target, &ask, on, origin.as_ref()).await
    }

    async fn servers_icon(&self, host: &str, refresh: Option<bool>) -> Result<Option<String>, HostError> {
        self.0.agents_read().servers_icon(host, refresh).await
    }

    async fn plugins_toggle(&self, target: AgentsTarget, ask: PluginAsk, on: bool, origin: Option<Caller>) -> Result<PluginRow, HostError> {
        self.0.agents_read().plugins_toggle(&target, &ask, on, origin.as_ref()).await
    }

    async fn setup(&self, place_id: &str, agent: &str, change: AgentSetupSet, origin: Option<Caller>) -> Result<AgentRow, HostError> {
        self.0.agents_read().setup(place_id, agent, change, origin.as_ref()).await
    }

    async fn homes_here(&self) -> Result<IndexMap<String, String>, HostError> {
        self.0.agents.get().homes_here().await
    }
}

#[async_trait::async_trait(?Send)]
impl PreferencesApi for PreferencesApiOf {
    async fn get(&self) -> Result<Preferences, HostError> {
        self.0.preferences.get().get().await.map_err(HostError::from)
    }

    async fn set(&self, patch: PreferencesPatch) -> Result<PreferencesSet, HostError> {
        let answer = self.0.preferences.get().set(patch).await?;
        Ok(PreferencesSet { preferences: answer.preferences, notice: answer.notice })
    }
}

#[async_trait::async_trait(?Send)]
impl ConversationsApi for ConversationsApiOf {
    async fn list(&self, project: Option<String>, agent: Option<String>, origin: Option<Caller>) -> Result<ConversationsAnswer, HostError> {
        self.0.conversations.get().list(project, agent, origin).await
    }

    async fn read(&self, ask: ConversationAsk, origin: Option<Caller>) -> Result<ConversationRead, HostError> {
        self.0.conversations.get().read(ask, origin).await
    }

    async fn opening(
        &self,
        workspace_id: &str,
        harness: Option<String>,
        resume: ResumeAsk,
        origin: Option<Caller>,
    ) -> Result<OutsideOpening, HostError> {
        self.0.conversations.get().opening(workspace_id, harness, resume, origin).await
    }

    async fn placed(&self, start: PlacedStart, resume: ResumeAsk, origin: Option<Caller>) -> Result<Placed, HostError> {
        self.0.conversations.get().placed(start, resume, origin).await
    }
}

#[async_trait::async_trait(?Send)]
impl ImageApi for ImageApiOf {
    async fn get(&self, name: Option<String>) -> Result<SealedImageView, HostError> {
        self.0.image.get().get(name.as_deref()).await
    }

    async fn vault(&self, name: Option<String>) -> Result<(SealedImage, Vec<u8>), HostError> {
        self.0.image.get().vault(name.as_deref()).await
    }

    async fn build(&self, ask: ImageBuildAsk) -> Result<SealedImageBuilt, HostError> {
        self.0.image.get().build(ask).await.map_err(ImageError::into_host)
    }

    async fn keep_current(&self, place: &str, name: Option<String>) {
        self.0.image.get().keep_current(place, name.as_deref()).await
    }
}
