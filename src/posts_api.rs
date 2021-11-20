use std::thread::JoinHandle;
use std::sync::Mutex;
use std::sync::Arc;

mod hashing;
pub use hashing::access_token_hash;

use crate::posts_store::fb_path;

use uuid::Uuid;
use serde_json::json;
use handlebars::JsonValue;
use firebase_rs::Firebase;
use serde::{Deserialize, Serialize};
use serde_json::{from_str, to_string};



/// Creates a string that has practically no chance of being the same as another string.
pub fn new_uuid() -> String {
    Uuid::new_v4().to_string()
}



/// Who can create sub-posts in the parent post.
#[derive(Clone)]
pub enum CanPost {
    None,
    Itself,
    All,
}



/// Returns how many seconds have passed since the Unix Epoch (1970-01-01 00:00:00 UTC).
fn timestamp() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}



/// Atomic counters? In Firebase? Too modern for firebase_rs (and probably the REST API).
/// Manually-implemented transactions? *Perfection*.
/// Each counter has to have a rule in the DB which checks that the update is possible.
fn atomic_update<F>(fb: &Firebase, at: &str, default: i64, mut update: F)
where F: FnMut(i64) -> i64 {
    // Until the write succeeds: read, then write updated value.
    let maybe_node = fb.at(at).ok();
    maybe_node.map(|node| {
        loop {
            let old = node.get().ok().map(|r| from_str::<i64>(&r.body).ok()).flatten().unwrap_or(default);
            let new = update(old);
            if new == old { break };
            let body = to_string(&new).ok();
            match body {
                Some(string) => {
                    let r = node.set(&string);
                    if r.is_ok() && r.unwrap().code / 100 == 2 { break };
                },
                None => break,
            }
        }
    });
}



/// A post: a published piece of information, communicated through Markdown.
/// 
/// For example, a user is just another kind of post (with a new access_hash).
/// 
/// ```
/// assert_eq!(5*5, 25)
/// ```
#[derive(Clone, Serialize, Deserialize)]
pub struct Post {
    pub id: String,
    pub access_hash: String, // Username&password are concatenated & hashed to produce this.
    //   (No collisions this way. Especially if an access-file is used instead.)
    //   (Gates write access: creating posts, editing and 'deleting' them, and rewarding any posts.)
    //     (Password is copied into posts, so can't change it.)
    pub human_readable_url: String, // A human-readable name, such as "2020_first_line".
    pub content: String, // Intended to be Markdown, with the first line displayed as the title.
    reward: i64,
    parent_id: String,
    children_rights: CanPost,
    gave_reward: i8,
    reverse_reward: i64,
    reverse_date_created: i64,
}

impl Post {
    /// Creates a new, top-level, open-to-comments, post.
    /// Exists because we have to root the post tree in *something*.
    pub fn new_public(id: Option<String>, content: String) -> Post {
        let id = id.unwrap_or_else(|| new_uuid());
        Post {
            id: id.clone(),
            access_hash: "".to_string(), // No user can edit it (except for our own functions).
            human_readable_url: "".to_string(),
            content,
            reward: 0i64,
            parent_id: id,
            children_rights: CanPost::All,
            gave_reward: 0i8,
            reverse_reward: 0i64,
            reverse_date_created: -timestamp(),
        }
    }
    /// Adds a new child-post to a parent-post.
    /// Also pushes to a user's created posts (no way to atomize this with firebase_rs).
    /// `access_hash` must be `crate::posts_api::access_token_hash(user)`.
    /// Returns (parent, Option<child>).
    pub fn new(fb: &Firebase, parent: Post, access_hash: &str, content: String, children_rights: CanPost) -> (Post, Option<Post>) {
        if content.len() > 50000 {
            return (parent, None)
        }
        let rights = &parent.children_rights;
        if matches!(rights, CanPost::All) || matches!(rights, CanPost::Itself) && &parent.access_hash == access_hash {
            let id = new_uuid();
            let mut handles: Vec<JoinHandle<()>> = vec![];
            fb.at(&fb_path(&["created_post_ids", access_hash])).ok().map(|node| {
                handles.push(node.push_async(&format!("{{\"post_id\":\"{}\"}}", id), |_| ()))
            });
            fb.at(&fb_path(&["children_ids", &parent.id])).ok().map(|node| {
                // TODO: We need to push a real object, not a mere string. (We're being ignored now.)
                //   Also, that object should include reward, which `post.reward` should update, so that we can index by that. ...So, basically, we need a hierarchy.
                handles.push(node.push_async(&id, |_| ()))
            });
            // TODO: ...Also, we can't filter on strings. If we want filtering (as opposed to getting all children on every request), then we NEED to store posts hierarchically inside parents.
            //   ...Or, store the reward in children_ids too, and store the child index in the post... But that's so desync-able and brittle and hard to implement...
            // ...So.
            // Do we want a DB for post_id→post_path?
            //   ...Wait a second: DB rules aren't recursive, so we won't be able to index recursive-post children by reward anyway.
            //   ...So we do actually need separate reward counters?...
            //     Well, at least we won't need *arrays* of children, since we can key them by ID. So we can at least know the path.
            atomic_update(fb, &fb_path(&["children_ids_length", &parent.id]), 0i64, |v| v+1);
            for handle in handles { handle.join().unwrap(); }
            let parent_id = parent.id.clone();
            (
                parent,
                Some(Post {
                    id,
                    access_hash: access_hash.to_string(),
                    human_readable_url: "".to_string(),
                    content,
                    reward: 0i64,
                    parent_id,
                    children_rights,
                    gave_reward: 0i8,
                    reverse_reward: 0i64,
                    reverse_date_created: -timestamp(),
                })
            )
        } else {
            (parent, None)
        }
    }
    /// Changes a post's content and its openness-to-comments status.
    pub fn edit(self: Post, user: &str, content: String, children_rights: CanPost) -> Option<Post> {
        if access_token_hash(user) == self.access_hash && self.access_hash != "" {
            Some(Post {
                content,
                children_rights,
                ..self
            })
        } else {
            None
        }
    }
    /// Gives reward to a post, from a user: -100|-1|0|1.
    /// Only succeeds if the user has given up to ±10 of ±1 rewards, to force normalization.
    /// Returns (user_first_post, Option<rewarded_post>), and has some side-effects.
    /// Disgustingly non-atomic.
    pub fn reward(self: Post, fb: &Firebase, mut user_first_post: Post, amount: i8) -> (Post, Option<Post>) {
        if amount != -100 && amount != -1 && amount != 0 && amount != 1 {
            return (user_first_post, None)
        };
        if amount == -100 && (self.access_hash != user_first_post.access_hash || self.access_hash == "") {
            return (user_first_post, None)
        };
        if amount != -100 {
            let will_be = user_first_post.gave_reward + amount;
            if will_be < -10 || will_be > 10 {
                return (user_first_post, None)
            }
            user_first_post.gave_reward += amount;
        };
        let at = fb_path(&["user_reward", &user_first_post.id, &self.id]);
        let old = fb.at(&at).ok().map(|n| n.get().ok()).flatten().map(|r| from_str::<i8>(&r.body).ok()).flatten().unwrap_or(0i8);
        to_string::<i8>(&amount).ok().map(|string| fb.at(&at).ok().map(|n| n.update(&string).ok()));
        let delta = amount - old;
        if self.id != user_first_post.id {
            (user_first_post, Some(Post{
                reward: self.reward + (delta as i64),
                reverse_reward: self.reverse_reward - (delta as i64),
                ..self
            }))
        } else {
            (Post{
                reward: self.reward + (delta as i64),
                reverse_reward: self.reverse_reward - (delta as i64),
                ..user_first_post
            }, None)
        }
    }

    /// Returns `{ content, post_reward, user_reward, parent_id, children_rights, access_hash, human_readable_url, logged_in }` as a JSON object, eventually. (`.to_string()` will convert it to a JSON string.)
    /// 
    /// Despite the signature, the result contains no error, only different paths depending on whether parallelization is possible; consider using `to_json_sync` if no parallelization is OK.
    /// 
    /// `content` and `parent_id` and `human_readable_url` are strings, rewards are integers, `children_rights` is 'none'|'itself'|'all', `access_hash` is what the owner's access token must hash to, `logged_in` is a boolean.
    pub fn to_json(self: &Post, fb: &Firebase, user: Option<&Post>) -> Result<JsonValue, Box<dyn FnOnce()->JsonValue>> {
        let logged_in = user.is_some();
        let json_value = json!({
            "id": self.id,
            "content": self.content,
            "post_reward": self.reward,
            "user_reward": 0i8,
            "parent_id": self.parent_id,
            "children_rights": self.children_rights.to_string(),
            "access_hash": self.access_hash,
            "human_readable_url": "/post/".to_owned() + if &self.human_readable_url == "" {
                &self.id
            }  else {
                &self.human_readable_url
            },
            "logged_in": logged_in,
        });
        if logged_in {
            let at = fb_path(&["user_reward", &user.unwrap().id, &self.id]);
            let value: Arc<Mutex<JsonValue>> = Arc::new(Mutex::new(json_value));
            let value2 = value.clone();
            let handle = fb.at(&at).ok().map(|n| n.get_async(move |r| {
                let user_reward = r.ok().map(|r| from_str::<i8>(&r.body).ok()).flatten().unwrap_or(0i8);
                let mut l = value2.lock().unwrap();
                (*l).as_object_mut().unwrap().insert("user_reward".to_owned(), json!(user_reward));
            }));
            Err(Box::new(move || {
                handle.map(|h| h.join());
                Arc::try_unwrap(value).unwrap().into_inner().unwrap()
            }))
        } else {
            Ok(json_value)
        }
    }
    /// Like `.to_json` but foregoes parallelization.
    pub fn to_json_sync(self: &Post, fb: &Firebase, user: Option<&Post>) -> JsonValue {
        match self.to_json(fb, user) {
            Ok(v) => v,
            Err(closure) => closure(),
        }
    }

    /// Gets the specified child-post IDs of a post, most-reward first.
    pub fn get_children_by_reward(&self, fb: &Firebase, start:usize, end:usize, len:usize) -> Result<Vec<String>, ()> {
        let start = std::cmp::min(start, len);
        let end = std::cmp::min(end, len);
        if start <= end {
            let node = fb.at(&fb_path(&["children_ids", &self.id])).ok();
            let response = node.map(|n| {
                // `firebase_rs` is so broken, it can't even set more than one param in one query with any of its methods.
                //   But life persists despite that.
                let mut n = n.with_params();
                let url = Arc::get_mut(&mut n.url).unwrap();
                url.set_query(Some(&format!("startAt={}&limitToFirst={}&orderBy={}", start, end-start, "\"reverse_reward\"")));
                n.get().ok()
            }).flatten();
            println!("  get_children_by_reward {:?}", response.as_ref()); // TODO: `orderBy must be defined when other query parameters are defined`... So, what do we order by? ...Wait, we *do* order by reverse reward, what the fuck.
            let ids = response.map(|r| from_str::<Vec<String>>(&r.body).ok()).flatten();
            match ids {
                Some(ids) => Ok(ids),
                None => Ok(vec![]),
            }
        } else {
            Err(())
        }
    }
    pub fn get_children_length(fb: &Firebase, post_id: &str) -> i64 {
        let node = fb.at(&fb_path(&["children_ids_length", post_id])).ok();
        let response = node.map(|n| n.get().ok()).flatten();
        let len = response.map(|r| from_str::<i64>(&r.body).ok()).flatten();
        len.unwrap_or(0i64)
    }
}



// Warning: 9000 lines of boilerplate ahead.
impl ToString for CanPost {
    fn to_string(&self) -> String {
        match self {
            Self::None => "none",
            Self::Itself => "itself",
            Self::All => "all",
        }.to_string()
    }
}
impl core::str::FromStr for CanPost {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(Self::None),
            "itself" => Ok(Self::Itself),
            "all" => Ok(Self::All),
            _ => Err(()),
        }
    }
}
impl Serialize for CanPost {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        serializer.serialize_str(match self {
            Self::None => "none",
            Self::Itself => "itself",
            Self::All => "all",
        })
    }
}
struct CanPostVisitor;
impl<'de> serde::de::Visitor<'de> for CanPostVisitor {
    type Value = CanPost;
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "none|itself|all")
    }
    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where E: serde::de::Error,
    {
        match s {
            "none" => Ok(CanPost::None),
            "itself" => Ok(CanPost::Itself),
            "all" => Ok(CanPost::All),
            _ => Err(serde::de::Error::invalid_value(serde::de::Unexpected::Str(s), &self)),
        }
    }
}
impl<'de> Deserialize<'de> for CanPost {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        deserializer.deserialize_str(CanPostVisitor)
    }
}