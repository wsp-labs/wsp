// SPDX-License-Identifier: AGPL-3.0-only
//! The agents and tools wsp knows: the catalog's rows, their install roads, sign-ins, MCP config formats, skills,
//! plugins and marks. Port of packages/catalog at a3b870a6e; each module names its TS file and its spec under spec/.
pub mod pending;

pub mod js_regex;
pub mod json_parse;
pub mod jsonc;
pub mod jsonc_tree;
pub mod mcp_bearer;
pub mod mcp_travelled;

pub mod json_edit;

pub mod mcp;
pub mod mcp_refs;

pub mod mcp_json;
pub mod mcp_json_refer;

pub mod mcp_codex;
pub mod mcp_toml;
pub mod mcp_toml_refer;

pub mod mcp_check;
pub mod mcp_config;
pub mod mcp_login;
pub mod mcp_oauth;

pub mod config_write;
pub mod mcp_launch;

pub mod base_image;
pub mod linux_casks;
pub mod mac_paths;
pub mod release_pins;
pub mod roads;

pub mod road_modules;
pub mod road_release;

pub mod signin;
pub mod signin_rows;

pub mod bwrap_apparmor;
pub mod catalog;
pub mod catalog_data;

pub mod context;
pub mod skills;

pub mod hooks;

pub mod plugins;
pub mod plugins_codex;

pub mod first_run;
pub mod git_hosts;
pub mod seed;

pub mod marks;
