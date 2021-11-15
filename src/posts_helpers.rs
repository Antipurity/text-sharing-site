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
    GetNotTopLevel, // post → bool
    GetPostReward, // post → num
    GetUserReward, // post, num → bool (checks equality, for coloring buttons)
    GetEditable, // post, user → bool
    GetPostable, // post, user → bool
    GetParentId, // post → post_id
    GetSummary, // post → string (the first line of content)
    GetContent, // post → string (the whole Markdown content, parsed into HTML)
    GetPostChildren, // post, user, page_index → array<post>
    GetPostChildrenLength, // post → length
    GetUserRewarded, // user, page_index → array<post>
    GetUserRewardedLength, // user → length
    GetUserPosts, // user, page_index → array<post>
    GetUserPostsLength, // user → length
    // (All this authentication is a LOT of hashing and DB lookups per page-view. So uncivilized.)
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
        let i64_arg = |i| arg(i).as_i64().unwrap();
        let post = |id: String| self.data.read(vec!(&id)).pop().unwrap();
        let auth = |user| match self.data.login(user).map(post) {
            Some(Some(first_post)) => Some(first_post),
            _ => None,
        };
        let page = |i| {
            let start = (i * PAGE_LEN) as usize;
            (start, start + PAGE_LEN as usize)
        };
        let post_ids_to_post_json = |ids: Vec<String>, first_post: Option<crate::posts_api::Post>| {
            let posts = self.data.read(ids.iter().map(|s| &s[..]).collect());
            json!(posts.iter().map(|maybe_post| match maybe_post {
                Some(post) => post.to_json(first_post.as_ref()),
                None => json!(null),
            }).collect::<handlebars::JsonValue>())
        };
        let f = |x| Ok(Some(ScopedJson::from(x)));
        f(match &self.which {
            Which::GetPostById => {
                let first_post = auth(str_arg(1));
                let first_post_ref = first_post.as_ref();
                match post(str_arg(0).to_string()) {
                    Some(ref post) => post.to_json(first_post_ref),
                    None => json!(null),
                }
            },
            Which::GetNotTopLevel => {
                match arg(0).get("id") {
                    Some(i) => match arg(0).get("parent_id") {
                        Some(p) => json!(i != p),
                        None => json!(false),
                    },
                    None => json!(false),
                }
            },
            Which::GetPostReward => match arg(0).get("post_reward") {
                Some(v) => json!(v.as_i64().unwrap()),
                None => json!(0i64),
            },
            Which::GetUserReward => {
                let expect = arg(1).as_i64().unwrap();
                match arg(0).get("user_reward") {
                    Some(v) => json!(v.as_i64().unwrap() == expect),
                    None => json!(0i64 == expect),
                }
            },
            Which::GetEditable => match arg(0).get("access_hash").map(|v| v.as_str()) {
                Some(Some(v)) => json!(v == access_token_hash(str_arg(1))),
                _ => json!(false),
            },
            Which::GetPostable => {
                let user = access_token_hash(str_arg(1));
                let children_rights = arg(0).get("children_rights").unwrap().as_str().unwrap();
                json!(if children_rights == "none" {
                    false
                } else if children_rights == "all" {
                    true
                } else {
                    let expect = arg(0).get("access_hash").unwrap().as_str().unwrap();
                    expect == user
                })
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
                    match v.split_once('\n') {
                        Some((_line, rest)) => {
                            let parser = Parser::new(rest);
                            let mut html_output = String::new();
                            html::push_html(&mut html_output, parser);
                            json!(ammonia::clean(&*html_output))
                        },
                        None => json!(v),
                    }
                },
                _ => json!(""),
            },
            Which::GetPostChildren => match arg(0).get("id").map(|v| v.as_str()) {
                Some(Some(v)) => match post(v.to_string()) {
                    Some(ref post) => {
                        let first_post = auth(str_arg(1));
                        let (start, end) = page(i64_arg(2));
                        let ch = post.get_children_newest_first(start, end).unwrap();
                        post_ids_to_post_json(ch, first_post)
                    },
                    None => json!(null),
                },
                _ => json!(null),
            },
            Which::GetPostChildrenLength => match arg(0).get("children").map(|v| v.as_i64()) {
                Some(Some(v)) => json!(1 + (v-1) / PAGE_LEN), // Always at least 1.
                _ => json!(0i64),
            },
            Which::GetUserRewarded => {
                match auth(str_arg(0)) {
                    Some(first_post) => {
                        let (start, end) = page(i64_arg(1));
                        let ch = first_post.get_rewarded_posts(start, end).unwrap();
                        post_ids_to_post_json(ch, Some(first_post))
                    },
                    None => json!(null),
                }
            },
            Which::GetUserRewardedLength => {
                match auth(str_arg(0)) {
                    Some(first_post) => json!(1 + (first_post.get_rewarded_posts_length() as i64 - 1)/PAGE_LEN),
                    None => json!(0),
                }
            },
            Which::GetUserPosts => {
                match auth(str_arg(0)) {
                    Some(first_post) => {
                        let (start, end) = page(i64_arg(1));
                        let ch = first_post.get_created_posts(start, end).unwrap();
                        post_ids_to_post_json(ch, Some(first_post))
                    },
                    None => json!(null),
                }
            },
            Which::GetUserPostsLength => {
                match auth(str_arg(0)) {
                    Some(first_post) => json!(1 + (first_post.get_created_posts_length() as i64 - 1)/PAGE_LEN),
                    None => json!(0),
                }
            },
        })
    }
}

impl PostHelper {
    pub fn register<'reg>(templates: &mut Handlebars<'reg>, d: &Arc<Database>) {
        let mut f = |s, t| templates.register_helper(s, Box::new(PostHelper{ which:t, data:d.clone() }));
        f("GetPostById", Which::GetPostById);
        f("GetNotTopLevel", Which::GetNotTopLevel);
        f("GetPostReward", Which::GetPostReward);
        f("GetUserReward", Which::GetUserReward);
        f("GetEditable", Which::GetEditable);
        f("GetPostable", Which::GetPostable);
        f("GetParentId", Which::GetParentId);
        f("GetSummary", Which::GetSummary);
        f("GetContent", Which::GetContent);
        f("GetPostChildren", Which::GetPostChildren);
        f("GetPostChildrenLength", Which::GetPostChildrenLength);
        f("GetUserRewarded", Which::GetUserRewarded);
        f("GetUserRewardedLength", Which::GetUserRewardedLength);
        f("GetUserPosts", Which::GetUserPosts);
        f("GetUserPostsLength", Which::GetUserPostsLength);
    }
}