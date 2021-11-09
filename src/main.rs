//! This code implements a web site for publicly sharing text.

extern crate iron;
extern crate staticfile;

use std::fs;
use std::path::Path;

use iron::prelude::*;
use iron::mime::*;
use iron::Handler;
use staticfile::Static;

mod posts_api;


// TODO: Stores for sessions (temporary, sessionId→userId; session id is created on successful login, and stored as a cookie) and posts and access_hash→post_id and URL name→id (name is like 2020/month/day/first_line if no overlaps).
//   TODO: Delete posts with less than -10 reward.
//   TODO: Use Firebase as the database.
//   TODO: In `store.rs`: `fn get(id)` and `fn set(id, post→post)`.
// TODO: fn login(user): None if access_token_hash(user) is not in the database, Some(first_post_id) otherwise.
//   Need a database for this, though. And be in another file.



// TODO: A POST API (get it?) at `/api/*` that:
//   TODO: allows viewing a post, editing post contents (if in-cookie session ID is OK), un/rewarding a post, creating a new post (also filling in the access_hash→post_id map), login (string to first post id).
// TODO: Templating with Handlebars. (Because all-JS sites are getting boring.)
//   TODO: Allow viewing (and editing if allowed) (and rewarding if logged in) a post & its children (with reward shown, along with the first line of Markdown contents, and first-lines of author contents), and a textfield & preview of a new post if you're allowed.
//     (And a way to expand-all.)
//     (And the post's username/password, if anyone can post in the post, or if the checkbox "Create account" is checked. Up to 3 passwords, I guess.)
//   TODO: Allow login/accounts (OAuth, preferably) (or just ferry cookies ourselves).
//     (Would be great if the password can be both text and a file, like an image that you always have access to. Innovative.)



fn main() {
    let files = Static::new(Path::new("static"));
    let chain = Chain::new(move |req: &mut Request| -> IronResult<Response> {
        println!("Request to {:?}", req.url.path());
        match files.handle(req) {
            Ok(x) => Ok(x),
            Err(_) => {
                let mime_type = iron::mime::mime!(Text/Html);
                let mut status = iron::status::Ok;
                let content = match req.url.path()[..] {
                    [""] => fs::read_to_string("static/index.html").unwrap(),
                    _ => {
                        status = iron::status::NotFound;
                        fs::read_to_string("static/404.html").unwrap()
                    },
                };
                Ok(Response::with((mime_type, status, content)))
            }
        }
    });
    Iron::new(chain).http("localhost:1234").unwrap();
}
