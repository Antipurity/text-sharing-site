//! This code implements a web site for publicly sharing text.

use std::fs;
use std::path::Path;

mod posts_api;
mod posts_store;

extern crate iron;
extern crate staticfile;
extern crate handlebars;

use iron::prelude::*;
use iron::mime::*;
use iron::Handler;
use staticfile::Static;
use handlebars::Handlebars;
use serde_json::json;


// TODO: Stores for sessions (temporary, sessionId→userId; session id is created on successful login, and stored as a cookie) and posts and access_hash→post_id and URL name→id (name is like 2020/month/day/first_line if no overlaps).
//   TODO: Use Firebase as the database.
// TODO: fn login(user): None if access_token_hash(user) is not in the database, Some(first_post_id) otherwise.
//   Need a database for this, though. And be in another file.



// TODO: Specify each entry *exactly*. As string-taking+returning funcs.
// TODO: A POST API (get it?) at `/api/*` that:
//   TODO: allows viewing a post, editing post contents (if in-cookie session ID is OK), un/rewarding a post, creating a new post (also filling in the access_hash→post_id map), login (string to first post id).
// TODO: Templating with Handlebars. (Because all-JS sites are getting boring.)
//   TODO: Allow viewing (and editing if allowed) (and rewarding if logged in) a post & its children (with reward shown, along with the first line of Markdown contents, and first-lines of author contents), and a textfield & preview of a new post if you're allowed.
//     (And a way to expand-all.)
//     (And the post's username/password, switched by a checkbox to a file input (innovative), if anyone can post in the post, or if the checkbox "Create account" is checked. On submit, hash it client-side.)



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
    let templates = templates;
    let render = |templates: &Handlebars, name: &str, user: &str, post: &str| {
        let body = templates.render(name, &json!({
            "user": user,
            "post": post,
        })).unwrap();
        Ok(Response::with((iron::mime::mime!(Text/Html), iron::status::Ok, body)))
    };
    let chain = Chain::new(move |req: &mut Request| -> IronResult<Response> {
        let user = ""; // TODO: Look up the "user" cookie.
        println!("Request to {:?}", req.url.path());
        match req.url.path()[..] {
            [template] if templates.has_template(template) => {
                render(&templates, template, user, "")
            },
            _ => match files.handle(req) {
                Ok(x) => Ok(x),
                Err(_) => {
                    // TODO: Try serving post ID, else human-readable URL, else 404.
                    render(&templates, "404", user, "")
                }
            },
        }
    });
    Iron::new(chain).http("localhost:1234").unwrap();
}
