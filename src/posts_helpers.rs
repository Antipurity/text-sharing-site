//! Defines Handlebars helpers, that store the database within.



use std::sync::Arc;

use crate::posts_store::Database;
use crate::posts_api::access_token_hash;
use crate::posts_api::Post;

use handlebars::{HelperDef, Helper, Handlebars, Context, RenderContext, ScopedJson, RenderError, JsonValue};
use serde_json::json;
use pulldown_cmark::{Parser, html};



const PAGE_LEN: i64 = 32;



pub enum Which {
    // Viewing.
    // (All this authentication is a LOT of hashing and DB lookups per page-view. So uncivilized.)
    GetPostById, // post_id, user → post
    GetNotTopLevel, // post → bool
    GetPostReward, // post → num
    GetUserReward, // post, num → bool (checks equality of reward and num, for coloring buttons)
    GetEditable, // post, user → bool
    GetPostable, // post, user → bool
    GetParentId, // post → post_id
    GetSummary, // post → string (the first line of content)
    GetContent, // post → string (the whole Markdown content, parsed into HTML)
    GetPostChildren, // post_id, user, page_index, length → array<post> (sorted by descending reward) (`length` should be `post.children_length`)
    GetUserFirstPostId, // user → post_id
    IsLoggedIn, // user → bool
    Plus1, // num → num (for recursion, to increment `depth`)
    Less, // num, num → bool
    Equal, // num, num → bool OR str, str → bool
    Pages, // current_page, length → array<pagination_pages> (length is actual-item-count)
    NewUUID, // → str
    Hash, // str → str (pass in the `user` to get something that you can expose other users)
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
        let auth = |user| self.data.login(user);
        let page = |i| {
            let start = (i * PAGE_LEN) as usize;
            (start, start + PAGE_LEN as usize)
        };
        let post_ids_to_post_json = |ids: Vec<String>, first_post_id: Option<&str>| {
            // Collect user-data rewards in parallel.
            let mut posts = self.data.read(ids.iter().map(|s| &s[..]).collect());
            let mut perhaps_promises = posts.drain(..).map(|maybe_post| match maybe_post {
                Some(post) => post.to_json(&self.data, first_post_id),
                None => Ok(json!(null)),
            }).collect::<Vec<Result<JsonValue, Box<dyn FnOnce()->JsonValue>>>>();
            json!(perhaps_promises.drain(..).map(|p| match p {
                Ok(v) => v,
                Err(closure) => closure(),
            }).collect::<JsonValue>())
        };
        let f = |x| Ok(Some(ScopedJson::from(x)));
        f(match &self.which {
            Which::GetPostById => {
                if arg(0).is_string() {
                    let post = |id: String| self.data.read(vec!(&id)).pop().unwrap();
                    let first_post_id = auth(str_arg(1));
                    match post(str_arg(0).to_string()) {
                        Some(ref post) => post.to_json_sync(&self.data, first_post_id().as_deref()),
                        None => json!(null),
                    }
                } else {
                    json!(null)
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
                match arg(0).get("children_rights") {
                    Some(rights) => {
                        let rights = rights.as_str().unwrap();
                        json!(if rights == "none" {
                            false
                        } else if rights == "all" {
                            true
                        } else {
                            let expect = arg(0).get("access_hash").unwrap().as_str().unwrap();
                            expect == user
                        })
                    },
                    None => json!(false),
                }
            },
            Which::GetParentId => match arg(0).get("parent_id").map(|v| v.as_str()) {
                Some(Some(v)) => json!(v),
                _ => json!(""),
            },
            Which::GetSummary => match arg(0).get("content").map(|v| v.as_str()) {
                Some(Some(v)) => {
                    let s = match v.split_once('\n') {
                        Some((line, _rest)) => line,
                        None => v,
                    };
                    json!(s.trim_start_matches('#').trim())
                },
                _ => json!(""),
            },
            Which::GetContent => match arg(0).get("content").map(|v| v.as_str()) {
                Some(Some(v)) => {
                    let s = match v.split_once('\n') {
                        Some((_line, rest)) => rest,
                        None => "",
                    };
                    let parser = Parser::new(s);
                    let mut html_output = String::new();
                    html::push_html(&mut html_output, parser);
                    json!(ammonia::clean(&*html_output))
                },
                _ => json!(""),
            },
            Which::GetPostChildren => {
                let id = str_arg(0);
                let first_post_id = auth(str_arg(1));
                let (start, end) = page(i64_arg(2));
                let len = i64_arg(3);
                let ch = Post::get_children_by_reward(id, &self.data, start, end, len as usize).unwrap();
                post_ids_to_post_json(ch, first_post_id().as_deref())
            },
            Which::GetUserFirstPostId => {
                json!(self.data.get_first_post(&str_arg(0))().unwrap_or_else(|| "".to_owned()))
            },
            Which::IsLoggedIn => json!(str_arg(0) != ""),
            Which::Plus1 => json!(i64_arg(0) + 1),
            Which::Less => json!(i64_arg(0) < i64_arg(1)),
            Which::Equal => {
                if arg(0).is_string() {
                    json!(str_arg(0) == str_arg(1))
                } else {
                    json!(i64_arg(0) == i64_arg(1))
                }
            },
            Which::Pages => {
                let (cur, len) = (i64_arg(0), i64_arg(1));
                let len = 1 + (len-1) / PAGE_LEN; // Always at least 1.
                let mut pages: Vec<i64> = Vec::new();
                let mut push = |p| {
                    if p >= 0 && p < len {
                        match pages.last() {
                            Some(last) => if last != &p { pages.push(p) },
                            None => pages.push(p),
                        }
                    }
                };
                push(0);
                push(cur-2);
                push(cur-1);
                push(cur  );
                push(cur+1);
                push(cur+2);
                push(len-1);
                json!(pages)
            },
            Which::NewUUID => json!(crate::posts_api::new_uuid()),
            Which::Hash => json!(access_token_hash(str_arg(0))),
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
        f("GetUserFirstPostId", Which::GetUserFirstPostId);
        f("IsLoggedIn", Which::IsLoggedIn);
        f("Plus1", Which::Plus1);
        f("Less", Which::Less);
        f("Equal", Which::Equal);
        f("Pages", Which::Pages);
        f("NewUUID", Which::NewUUID);
        f("Hash", Which::Hash);
    }
}