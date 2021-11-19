//! This code implements a web site for publicly sharing text.

use std::path::Path;
use std::sync::Arc;

mod posts_api;
mod posts_store;
mod posts_helpers;
use posts_api::{Post, CanPost};

extern crate iron;
extern crate staticfile;
extern crate params;
extern crate handlebars;
extern crate cookie;
extern crate firebase_rs;

use iron::prelude::*;
use iron::mime::*;
use iron::Handler;
use iron::headers;
use iron::modifiers::Header;
use iron::error::IronError;
use iron::status;
use iron::modifiers::RedirectRaw;
use params::{Params, Value};
use staticfile::Static;
use handlebars::Handlebars;
use serde_json::json;
use cookie::Cookie;
use firebase_rs::Firebase;



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

    let firebase = Firebase::new("https://text-sharing-site-default-rtdb.europe-west1.firebasedatabase.app/").unwrap(); // TODO: Auth this server, *in secret* (can't just commit this secret to Git).
    let data = Arc::new(posts_store::Database::new(firebase));
    if data.read(vec![""]).pop().unwrap().is_none() {
        data.update(vec![""], |_: Vec<Option<Post>>| {
            vec![Some(Post::new_public(Some("".to_string()), "# Text-sharing

Welcome to a website for publicly sharing mostly-text pieces of info: *posts*.

Say anything you want.

<details>
    <summary>How</summary>
    <div>

First, you'll need an account.

An account is just another post. So, post it somewhere, entering your username+password (or your authentication file) and describing yourself.

With an account, you can:
- Edit your posts.
- Reward other posts, to help others discern what you consider to be better. It's a cat-eats-cat world: every <code>+1</code> must be balanced by a <code>-1</code>, except for the initial <code>9</code>.

That's all you need to know. Good luck.
    </div>
</details>

---".to_string()))]
        });
        println!("Created the initial post.");
    }
    posts_helpers::PostHelper::register(&mut templates, &data);



    let templates = templates;
    let render = |data: &posts_store::Database, templates: &Handlebars, name: &str, user: &str, post_id: &str, page:u64| {
        let post = |id: &str| data.read(vec!(id)).pop().unwrap();
        let post = post(post_id);
        let body = templates.render(name, &json!({
            "user": user,
            "post": post_id,
            "page": page,
            "url": "/post/".to_owned() + &post.map_or_else(|| "".to_owned(), |post| post.human_readable_url),
            "max_depth": 1,
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
        let get = |map: &params::Map, key: &str| -> Option<String> {
            let v = map.get(key);
            if v.is_none() { return None };
            let v = v.unwrap();
            if let params::Value::String(s) = v {
                Some(s.to_string())
            } else {
                None
            }
        };
        let fail = || {
            Err(IronError{
                error: Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, "what are you even saying")),
                response: Response::with((status::BadRequest, "Bad data")),
            })
        };
        let not_logged_in = || {
            Err(IronError{
                error: Box::new(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "Not logged in")),
                response: Response::with((status::Forbidden, "Not logged in")),
            })
        };
        let login_cookie = |user| {
            let mut cookie = "user=".to_owned() + user + "; Secure; HttpOnly";
            if user == "" {
                cookie = cookie + "; expires=Thu, 01 Jan 1970 00:00:01 GMT"
            };
            Header(headers::SetCookie(vec![cookie]))
        };
        let elsewhere = status::SeeOther;
        match req.url.path()[..] {
            [""] => {
                render(&data, &templates, "post", &user, "", 0)
            },
            ["login"] => { // url, user
                // It's unclear how the `params` crate deals with too-large requests.
                //   But what's clear is that it's not my problem.
                let map = req.get_ref::<Params>();
                if map.is_err() { return fail() };
                let map = map.unwrap();
                let fail = |url| { // Logout on failure.
                    Ok(Response::with((elsewhere, login_cookie(""), RedirectRaw(url))))
                };
                let url = get(map, "url");
                match map.find(&["user"]) {
                    Some(&Value::String(ref access_token)) => {
                        match data.login(access_token) {
                            Some(_first_post_id) => {
                                let url = url.unwrap_or_else(|| "/".to_string());
                                Ok(Response::with((elsewhere, login_cookie(access_token), RedirectRaw(url))))
                            },
                            None => fail(url.unwrap_or_else(|| "".to_owned())),
                        }
                    },
                    _ => fail(url.unwrap_or_else(|| "".to_owned())),
                }
            },
            ["new"] => { // url, parent_id, content, rights, user
                // This might be the longest implementation of a simple behavior I've ever seen.
                //   And it's not even very efficient.
                //   Rust (and static typing in particular) forces a lot of boilerplate.
                let map = req.get_ref::<Params>();
                if map.is_err() { return fail() };
                let map = map.unwrap();
                let (url, parent_id, content, rights, user) = (get(map, "url"), get(map, "parent_id"), get(map, "content"), get(map, "rights"), get(map, "user"));
                if parent_id.is_none() || content.is_none() || rights.is_none() || user.is_none() { return fail() };
                let (parent_id, content, rights, user) = (parent_id.unwrap(), content.unwrap(), rights.unwrap(), user.unwrap());
                if let Ok(rights) = rights.parse::<CanPost>() {
                    let maybe_first_post_id = data.login(&user);
                    let was_logged_in = maybe_first_post_id.is_some();
                    let ids: Vec<&str> = vec![&parent_id, match maybe_first_post_id {
                        Some(ref first_post_id) => first_post_id,
                        None => "rfnerfbue4ntbweubiteruiertbnerngdoisfnoidn", // Should be non-existent.
                    }];
                    data.update(ids, |mut posts| {
                        if posts[0].is_none() { return vec![] };
                        let (parent, maybe_first_post) = (posts.remove(0).unwrap(), posts.remove(0));
                        let token = crate::posts_api::access_token_hash(&user);
                        let r = Post::new(parent, &token, maybe_first_post, content, rights);
                        let (parent, maybe_first_post, maybe_child) = r;
                        vec![Some(parent), maybe_first_post, maybe_child]
                    });
                    let url = url.unwrap_or_else(|| "/".to_string());
                    if was_logged_in {
                        Ok(Response::with((elsewhere, RedirectRaw(url))))
                    } else {
                        Ok(Response::with((elsewhere, login_cookie(&user), RedirectRaw(url))))
                    }
                } else {
                    fail()
                }
            },
            ["edit"] => { // url, post_id, content, rights
                let map = req.get_ref::<Params>();
                if map.is_err() { return fail() };
                let map = map.unwrap();
                let (url, post_id, content, rights) = (get(map, "url"), get(map, "post_id"), get(map, "content"), get(map, "rights"));
                if post_id.is_none() || content.is_none() || rights.is_none() { return fail() };
                let (post_id, content, rights) = (post_id.unwrap(), content.unwrap(), rights.unwrap());
                if let Ok(rights) = rights.parse::<CanPost>() {
                    data.update(vec![&post_id], |mut posts| {
                        match posts.remove(0) {
                            Some(post) => vec![post.edit(&user, content, rights)],
                            None => vec![],
                        }
                    });
                    let url = url.unwrap_or_else(|| "/".to_string());
                    Ok(Response::with((elsewhere, RedirectRaw(url))))
                } else {
                    fail()
                }
            },
            ["reward"] => { // url, post_id, amount
                let map = req.get_ref::<Params>();
                if map.is_err() { return fail() };
                let map = map.unwrap();
                let (url, post_id, amount) = (get(map, "url"), get(map, "post_id"), get(map, "amount"));
                if post_id.is_none() || amount.is_none() { return fail() };
                let (post_id, amount) = (post_id.unwrap(), amount.unwrap());
                if let Ok(amount) = amount.parse::<i8>() {
                    match data.login(&user) {
                        Some(first_post_id) => {
                            data.update(vec![&post_id, &first_post_id], |mut posts| {
                                if posts.iter().any(|p| p.is_none()) { return vec![] };
                                let (post, first_post) = (posts.remove(0).unwrap(), posts.remove(0).unwrap());
                                let (first_post, maybe_post) = post.reward(first_post, amount);
                                vec![Some(first_post), maybe_post]
                            });
                            let url = url.unwrap_or_else(|| "/".to_string());
                            Ok(Response::with((elsewhere, RedirectRaw(url))))
                        },
                        None => not_logged_in(),
                    }
                } else {
                    fail()
                }
            },
            [template, post_id] if templates.has_template(template) => {
                let post_id = data.lookup_url(post_id).unwrap_or_else(|| post_id.to_string());
                render(&data, &templates, &template, &user, &post_id, 0)
            },
            [template, post_id, page] if templates.has_template(template) => {
                let post_id = data.lookup_url(post_id).unwrap_or_else(|| post_id.to_string());
                let page = page.parse::<u64>();
                render(&data, &templates, &template, &user, &post_id, page.unwrap_or(0))
            },
            _ => match files.handle(req) {
                Ok(x) => Ok(x),
                Err(_) => {
                    render(&data, &templates, "404", &user, "", 0)
                }
            },
        }
    });
    Iron::new(chain).http("localhost:1234").unwrap();
}
