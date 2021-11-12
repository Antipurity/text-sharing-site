//! Defines Handlebars helpers, that store the database within.



use std::sync::Arc;

use crate::posts_store::Database;

use handlebars::{HelperDef, Helper, Handlebars, Context, RenderContext, ScopedJson, RenderError};
use serde_json::json;



pub enum Which {
    // Viewing.
    GetPostById, // post_id, user → post
    GetPostReward, // post → num
    GetUserReward, // user, post → num
    IsEditable, // user, post → bool
    GetParent, // post → num
    GetSummary, // post → string (the first line of content, parsed into HTML)
    GetContent, // post → string (the whole Markdown content, parsed into HTML)
    GetPostChildren, // post, page_index → array<post>
    GetPostChildrenLength, // post → length
    GetUserRewards, // user, page_index → array<post>
    GetUserRewardsLength, // user → length
    GetUserPosts, // user, page_index → array<post>
    GetUserPostsLength, // user → length
    // TODO: Editing.
    // TODO: Login & logout.
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
            Which::GetPostById => match self.data.read(vec![str_arg(0)])[0] {
                Some(ref p) => p.to_json(None), // TODO: Needs the user: Some(&Post). Read from the database.
                None => json!("NOT FOUND"), // TODO: ...Why not found?
            },
            Which::GetPostReward => match arg(0).get("reward") {
                Some(v) => json!(v.as_i64().unwrap()),
                None => json!(0i64),
            },
            _ => json!("what are you tellin me to do??"),
            // TODO: Do all the ops. Get the args with h.param(0/1/2)?.value(), which returns https://docs.rs/serde_json/1.0.68/serde_json/value/enum.Value.html
        })
        // Ok(Some(ScopedJson::from(json!("hello there")))) // TODO
    }
}

impl PostHelper {
    pub fn register<'reg>(templates: &mut Handlebars<'reg>, d: &Arc<Database>) {
        let mut f = |s, t| templates.register_helper(s, Box::new(PostHelper{ which:t, data:d.clone() }));
        f("GetPostById", Which::GetPostById);
        f("GetPostReward", Which::GetPostReward);
        f("GetContent", Which::GetContent);
        // templates.register_helper("GetPostById", Box::new(PostHelper{ which:Which::GetPostById, d: data.clone() }));
        // templates.register_helper("GetPostReward", Box::new(PostHelper{ which:Which::GetPostReward, d: data.clone() }));
        // templates.register_helper("GetContent", Box::new(PostHelper{ which:Which::GetContent, d: data.clone() }));
    }
}



// TODO: A function to register all helpers on a `handlebars::Handlebars` instance.