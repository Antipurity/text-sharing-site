//! Defines Handlebars helpers, that store the database within.



use std::sync::Arc;

use crate::posts_store::Database;
use crate::posts_api::access_token_hash;

use handlebars::{HelperDef, Helper, Handlebars, Context, RenderContext, ScopedJson, RenderError};
use serde_json::json;
use pulldown_cmark::{Parser, html};



const PAGE_LEN: i64 = 50;



pub enum Which {
    // Viewing.
    GetPostById, // post_id, user → post
    GetPostReward, // post → num
    GetUserReward, // post → num
    GetEditable, // post, user → bool
    GetPostable, // post, user → bool
    GetParentId, // post → post_id
    GetSummary, // post → string (the first line of content)
    GetContent, // post → string (the whole Markdown content, parsed into HTML)
    GetPostChildren, // post, page_index → array<post>
    GetPostChildrenLength, // post → length
    GetUserRewards, // user, page_index → array<post>
    GetUserRewardsLength, // user → length
    GetUserPosts, // user, page_index → array<post>
    GetUserPostsLength, // user → length
}



pub struct PostHelper {
    which: Which,
    data: Arc<Database>,
}

impl HelperDef for PostHelper {
    fn call_inner<'reg: 'rc, 'rc>(
        &self,
        h: &Helper<'reg, 'rc>,
        _: &'reg Handlebars<'reg>,
        _: &'rc Context,
        _: &mut RenderContext<'reg, 'rc>,
    ) -> Result<Option<ScopedJson<'reg, 'rc>>, RenderError> {
        let arg = |i| h.param(i).unwrap().value();
        let str_arg = |i| arg(i).as_str().unwrap();
        let f = |x| Ok(Some(ScopedJson::from(x)));
        f(match &self.which {
            Which::GetPostById => json!({"post": match self.data.read(vec![str_arg(0)])[0] {
                Some(ref p) => p.to_json(None), // TODO: Needs the user: Some(&Post). Read from the database. ...Or accept as a JSON value, and have `GetUserFirstPost(access_token)`?
                // TODO: `posts_store` should have support for getting and setting a user's first post! (Maybe even automatic, by `read` and `update`.)
                None => json!(null),
            }}),
            Which::GetPostReward => match arg(0).get("post_reward") {
                Some(v) => json!(v.as_i64().unwrap()),
                None => json!(0i64),
            },
            Which::GetUserReward => match arg(0).get("user_reward") {
                Some(v) => json!(v.as_i64().unwrap()),
                None => json!(0i64),
            },
            Which::GetEditable => match arg(0).get("access_hash").map(|v| v.as_str()) {
                Some(Some(v)) => json!(v == access_token_hash(str_arg(1))),
                _ => json!(false),
            },
            Which::GetPostable => {
                let user = access_token_hash(str_arg(1));
                let children_rights = arg(0).get("children_rights").unwrap().as_array().unwrap();
                json!(children_rights.iter().any(|v| {
                    let s = v.as_str().unwrap();
                    s == "" || s == user
                }))
            },
            Which::GetParentId => match arg(0).get("parent_id").map(|v| v.as_str()) {
                Some(Some(v)) => json!(v),
                _ => json!(""),
            },
            Which::GetSummary => match arg(0).get("content").map(|v| v.as_str()) {
                Some(Some(v)) => {
                    match v.split_once('\n') {
                        Some((line, _rest)) => json!(line.trim_start_matches('#').trim()),
                        None => json!(v),
                    }
                },
                _ => json!(""),
            },
            Which::GetContent => match arg(0).get("content").map(|v| v.as_str()) {
                Some(Some(v)) => {
                    let parser = Parser::new(v);
                    let mut html_output = String::new();
                    html::push_html(&mut html_output, parser);
                    json!(ammonia::clean(&*html_output))
                },
                _ => json!(""),
            },
            // TODO: Which::GetPostChildren
            Which::GetPostChildrenLength => match arg(0).get("children").map(|v| v.as_i64()) {
                Some(Some(v)) => json!(1 + (v-1) / PAGE_LEN), // Always at least 1.
                _ => json!(0i64),
            }
            // TODO: Which::GetUserRewards
            // TODO: Which::GetUserRewardsLength
            // TODO: Which::GetUserPosts
            // TODO: Which::GetUserPostsLength
            //   All these user things need access to the user's first post, right?
            _ => json!("what are you tellin me to do??"),
        })
    }
}

impl PostHelper {
    pub fn register<'reg>(templates: &mut Handlebars<'reg>, d: &Arc<Database>) {
        let mut f = |s, t| templates.register_helper(s, Box::new(PostHelper{ which:t, data:d.clone() }));
        f("GetPostById", Which::GetPostById);
        f("GetPostReward", Which::GetPostReward);
        f("GetUserReward", Which::GetUserReward);
        f("GetEditable", Which::GetEditable);
        f("GetPostable", Which::GetPostable);
        f("GetParentId", Which::GetParentId);
        f("GetSummary", Which::GetSummary);
        f("GetContent", Which::GetContent);
        f("GetPostChildren", Which::GetPostChildren);
        f("GetPostChildrenLength", Which::GetPostChildrenLength);
        f("GetUserRewards", Which::GetUserRewards);
        f("GetUserRewardsLength", Which::GetUserRewardsLength);
        f("GetUserPosts", Which::GetUserPosts);
        f("GetUserPostsLength", Which::GetUserPostsLength);
    }
}