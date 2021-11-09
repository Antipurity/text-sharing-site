//! This code implements a web site for publicly sharing text.

extern crate iron;
extern crate staticfile;

use std::fs;
use std::path::Path;
use std::collections::HashMap;

use iron::prelude::*;
use iron::mime::*;
use iron::Handler;
use staticfile::Static;
use uuid::Uuid;
use serde_json::json;

mod hashing;
use hashing::access_token_hash;



/// Creates a string that has practically no chance of being the same as another string.
fn new_uuid() -> String {
    Uuid::new_v4().to_string()
}


// TODO: Stores for sessions (temporary, sessionId→userId; session id is created on successful login, and stored as a cookie) and posts and access_hash→post_ids and URL name→id (name is like 2020/month/day/first_line if no overlaps).
//   TODO: Delete posts with less than -10 reward.
//   TODO: Use Firebase as the database.
//   TODO: In `store.rs`: `fn get(id)` and `fn set(id, post→post)`.



/// A post: a published piece of information, communicated through Markdown.
/// 
/// For example, a user is just another kind of post (with a new access_hash).
/// 
/// ```
/// assert_eq!(5*5, 25)
/// ```
struct Post {
    id: String,
    access_hash: String, // Username&password are concatenated & hashed to produce this.
    //   (No collisions this way. Especially if an access-file is used instead.)
    //   (Gates write access: creating posts, editing and 'deleting' them, and rewarding any posts.)
    //     (Password is copied into posts, so can't change it.)
    content: String, // Intended to be Markdown, with the first line displayed as the title.
    reward: i64, // Less than -10 should get deleted.
    parent_id: String,
    children_rights: Vec<String>, // An empty string, usually (must be empty to disallow comments).
    children_ids: Vec<String>,
    rewarded_posts: HashMap<String, i8>, // -100 (only own posts), -1, 1.
    //   (Only non-empty for initial access_hash posts, meaning, user accounts.)
    //   (Sum of non -100 rewards should be -10..=10, for balance.)
}

impl Post {
    /// Creates a new, top-level, open-to-comments, post.
    /// Exists because we have to root the post tree in *something*.
    pub fn new_public(content: String) -> Post {
        Post {
            id: new_uuid(),
            access_hash: "".to_string(), // No user can edit it (except for our own functions).
            content,
            reward: 0i64,
            parent_id: "".to_string(), // No parent.
            children_rights: vec!["".to_string()], // Open to all comments.
            children_ids: Vec::new(),
            rewarded_posts: HashMap::new(),
        }
    }
    /// Adds a new child-post to a parent-post.
    pub fn new(mut parent: Post, user: String, content: String, children_rights: Vec<String>) -> (Post, Option<Post>) {
        let hash = access_token_hash(&user);
        if parent.children_rights.iter().any(|s| s == "" || s == &hash) {
            let id = new_uuid();
            parent.children_ids.push(id.clone());
            let parent_id = parent.id.clone();
            (
                parent,
                Some(Post {
                    id,
                    access_hash: hash,
                    content,
                    reward: 0i64,
                    parent_id,
                    children_rights,
                    children_ids: Vec::new(),
                    rewarded_posts: HashMap::new(),
                })
            )
        } else {
            (parent, None)
        }
    }
    /// Changes a post's content and its openness-to-comments status.
    pub fn edit(self: Post, user: String, content: String, children_rights: Vec<String>) -> Option<Post> {
        if access_token_hash(&user) == self.access_hash {
            Some(Post {
                content,
                children_rights,
                ..self
            })
        } else {
            None
        }
    }
    /// Gives reward to a post, from a user: -100|-1|1.
    pub fn reward(self: Post, user: String, amount: i8) -> Option<Post> {
        let hash = access_token_hash(&user);
        if amount != -100 && amount != -1 && amount != 1 {
            return None
        };
        if amount == -100 && self.access_hash != user {
            return None
        };
        // TODO: ???.rewarded_posts[self.id.clone()] = amount
        return Some(Post{
            reward: self.reward + (amount as i64),
            ..self
        })
    }
    /// Returns `{ content, reward, parent_id, children_rights }` as a JSON string.
    /// `content` and `parent_id` are strings,  `reward` is an integer, `children_rights` is an array of strings.
    pub fn to_json(self: &Post) -> String {
        // TODO: Also, accept the user, and return how much said user has rewarded this post.
        json!({
            "content": self.content,
            "reward": self.reward,
            "parent_id": self.parent_id,
            "children_rights": self.children_rights,
        }).to_string()
    }

    // TODO: fn login(user): None if access_token_hash(user) is not in the database, Some(first_post_id) otherwise.
    //   Need a database for this, though. And be in another file.

    // TODO: Pagified:
    //   TODO: All children of a post.
    //     TODO: Sorted by date.
    //     TODO: Sorted by reward.
    //   TODO: All rewarded-post-IDs of a user (access_token).
    //   TODO: All owned post IDs of a user (access_token).
    //   …Doesn't this mean that children_ids and rewarded_posts should not be stored so directly on Post…

    // TODO: ...Also, string->Post conversion, for easy DB look up...
}
// TODO: What other methods do we want to implement on Posts?
// TODO: A POST API (get it?) at `/api/*` that:
//   TODO: allows viewing a post, editing post contents (if in-cookie session ID is OK), un/rewarding a post, creating a new post, login (creating a session).
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
