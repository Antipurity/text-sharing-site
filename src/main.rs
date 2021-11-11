//! This code implements a web site for publicly sharing text.

use std::path::Path;

mod posts_api;
mod posts_store;

extern crate iron;
extern crate staticfile;
extern crate handlebars;
extern crate cookie;

use iron::prelude::*;
use iron::mime::*;
use iron::Handler;
use staticfile::Static;
use handlebars::Handlebars;
use serde_json::json;
use cookie::Cookie;


// TODO: Stores for sessions (temporary, sessionId→userId; session id is created on successful login, and stored as a cookie) and posts and access_hash→post_id and URL name→id (name is like 2020/month/day/first_line if no overlaps).
//   TODO: Use Firebase as the database.
// TODO: fn login(user): None if access_token_hash(user) is not in the database, Some(first_post_id) otherwise.
//   Need a database for this, though. And be in another file.
//     ...Should we maybe use the same object as in `posts_store`, but store JSON strings and reconstruct posts from that JSON, and use other keys to access other data...
//       (Would allow us to re-use the functions, except for parse-post. …Or maybe we should extract those, and have Post-processing be separate…)
//     ...Also, `posts_store`'s `update` isn't actually atomic; the whole function should be protected by one write-lock to make that true (otherwise updates can get swallowed).



// TODO: Specify each entry *exactly*. As string-taking+returning funcs.
// TODO: A POST API (get it?) at `/api/*` that:
//   TODO: allows viewing a post, editing post contents (if in-cookie session ID is OK), un/rewarding a post, creating a new post (also filling in the access_hash→post_id map), login (string to first post id).
// TODO: Templating with Handlebars. (Because all-JS sites are getting boring.)
//   TODO: Allow viewing (and editing if allowed) (and rewarding if logged in) a post & its children (with reward shown, along with the first line of Markdown contents, and first-lines of author contents), and a textfield & preview of a new post if you're allowed.
//     (And a way to expand-all.)
//     (And the post's username/password, switched by a checkbox to a file input (innovative), if anyone can post and not logged in (else it would be too irksome to see it everywhere). On submit, hash it client-side.)
//       (The login page should transmit the header `Set-Cookie: user=…`.)
//     (And if the "" post does not exist, allow creating it.)



fn main() {
    let files = Static::new(Path::new("static"));
    // Register Handlebars templates, from the `static` directory.
    let mut templates = Handlebars::new();
    for path in std::fs::read_dir("static").unwrap() {
        let path = path.unwrap().path();
        let full_path = path.to_str().unwrap();
        let filename = path.file_name().unwrap().to_str().unwrap();
        if filename.ends_with(".html") {
            let name = &filename[..filename.len()-5];
            templates.register_template_file(name, full_path).unwrap();
        }
    }
    // TODO: Expose the whole string-based API as helpers.
    //   `handlebars_helper!(hex: |v: i64| format!("0x{:x}", v))`
    //   `templates.register_helper("hex", Box::new(hex))`
    //   `{{hex 16}}`
    //   TODO: Helpers:
    //     TODO: Get post by ID.
    //       TODO: Get post's reward.
    //       TODO: Get user's reward to post, if logged in.
    //       TODO: Get whether we can edit the post (logged in, and owning it).
    //       TODO: Get post's parent ID.
    //       TODO: Get Markdown post content's first line, as a 'safe' string (put as-is, as HTML).
    //       TODO: Get Markdown post content, as a 'safe' string (put as-is, as HTML).
    //     TODO: Get post's children, pagified to 50 posts per page.
    //     TODO: Get user's rewarded posts, pagified to 50 posts per page.
    //     TODO: Get user's created posts, pagified to 50 posts per page.
    //   TODO: Helpers that edit posts, and report whether editing was successful:
    //     TODO: New post, by user, in post, with content, with sub-posting by none/self/all.
    //       (Probably need to handle POST requests and parse form data to get the content.)
    //     TODO: Edit post, by user, with content, with sub-posting by none/self/all.
    //     TODO: Reward post, by user, by amount (-100|-1|1).
    let templates = templates;
    let render = |templates: &Handlebars, name: &str, user: &str, post: &str| {
        let body = templates.render(name, &json!({
            "user": user,
            "post": post,
        })).unwrap();
        Ok(Response::with((iron::mime::mime!(Text/Html), iron::status::Ok, body)))
    };
    let data = posts_store::Database::new();
    let chain = Chain::new(move |req: &mut Request| -> IronResult<Response> {
        // Get the `user=…` cookie. (It's a whole big process. The `cookie` library is questionably designed.)
        let cookie = req.headers.get::<iron::headers::Cookie>();
        let user = match cookie {
            Some(cs) => match (*cs).iter().map(|string| {
                Cookie::parse(string).unwrap_or_else(|_| Cookie::new("z", "z"))
            }).find(|c| c.name() == "user") {
                Some(c) => c.value().to_string(),
                None => "".to_string(),
            },
            None => "".to_string(),
        };
        // Actually handle the request.
        match req.url.path()[..] {
            [template] if templates.has_template(template) => {
                render(&templates, template, &user, "")
            },
            _ => match files.handle(req) {
                Ok(x) => Ok(x),
                Err(_) => {
                    // TODO: Try serving post ID, else human-readable URL (that maps to post's ID), else 404.
                    render(&templates, "404", &user, "")
                }
            },
        }
    });
    Iron::new(chain).http("localhost:1234").unwrap();
}
