//! This code implements a web site for publicly sharing text.

use std::path::Path;
use std::sync::Arc;

mod posts_api;
mod posts_store;
mod posts_helpers;

extern crate iron;
extern crate staticfile;
extern crate params;
extern crate handlebars;
extern crate cookie;

use iron::prelude::*;
use iron::mime::*;
use iron::Handler;
use iron::headers;
use iron::modifiers::Header;
use iron::error::IronError;
use iron::status;
use iron::method::Method;
use params::{Params, Value};
use staticfile::Static;
use handlebars::Handlebars;
use serde_json::json;
use cookie::Cookie;



//   TODO: UI: allow viewing (and editing if allowed) (and rewarding if logged in) a post & its children (with reward shown, along with the first line of Markdown contents, and first-lines of author contents), and a textfield & preview of a new post if you're allowed.
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

    let data = Arc::new(posts_store::Database::new());
    data.update(vec![""], |_: Vec<Option<posts_api::Post>>| {
        println!("Creating the initial post..."); // TODO
        vec![Some(posts_api::Post::new_public(Some("".to_string()), "# The initial post\n\nWhy hello there. This is the public post.\n\n<script>console.log('JS injection')</script>".to_string()))]
    });
    posts_helpers::PostHelper::register(&mut templates, &data);



    let templates = templates;
    let render = |templates: &Handlebars, name: &str, user: &str, post: &str| {
        let body = templates.render(name, &json!({
            "user": user,
            "post": post,
        })).unwrap();
        Ok(Response::with((mime!(Text/Html), status::Ok, body)))
    };
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
        // Actually handle the request, exposing the POST API.
        match req.url.path()[..] {
            [""] => {
                render(&templates, "post", &user, "")
            },
            ["login"] if req.method == Method::Post => {
                let map = req.get_ref::<Params>();
                // It's unclear how the `params` crate deals with too-large requests.
                //   But what's clear is that it's not my problem.
                let fail = || { // Logout on failure.
                    let cookie = "user=; Secure; HttpOnly".to_owned();
                    let h = Header(headers::SetCookie(vec![cookie]));
                    Err(IronError{
                        error: Box::new(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "could not login")),
                        response: Response::with((status::Forbidden, h, "Could not login")),
                    })
                };
                match map.map(|m| m.find(&["user"])) {
                    Ok(Some(&Value::String(ref access_token))) => {
                        match data.login(access_token) {
                            Some(_first_post_id) => {
                                let cookie = "user=".to_owned() + access_token + "; Secure; HttpOnly";
                                let h = Header(headers::SetCookie(vec![cookie]));
                                Ok(Response::with((status::Ok, h, "")))
                            },
                            None => fail(),
                        }
                    },
                    _ => fail(),
                }
            },
            // ["new"] if req.method == Method::Post => {
            //     let map = req.get_ref::<Params>();
            //     // TODO: Get parent_id, content, children_rights ("none"|"self"|"all") from `map`.
            //     //   ...How to do all that at the same time, though... Do we really have to go in sequence?
            //     // TODO: crate::posts_api::Post::new(parent, user_first_post, content, children_rights) -> (user_first_post, parent, Option<child>)
            //     //   And data.update(…)
            //     // TODO: How to return the post?
            // },
            /*
            ["edit", post_id, content, children_rights] => {
                // TODO: post.edit(&user, &content, children_rights) -> Option<post>
            },
            ["reward", post_id, amount] => {
                // TODO: Parse amount into i8, and fail if it is not -100, -1, 0, 1.
                // TODO: post.reward(user_first_post, amount)
            },
            */
            [template, post_id] if templates.has_template(template) => {
                let post_id = data.lookup_url(post_id).unwrap_or_else(|| post_id.to_string());
                render(&templates, &template, &user, &post_id)
            },
            _ => match files.handle(req) {
                Ok(x) => Ok(x),
                Err(_) => {
                    render(&templates, "404", &user, "")
                }
            },
        }
    });
    Iron::new(chain).http("localhost:1234").unwrap();
}
