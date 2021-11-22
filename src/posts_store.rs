//! Stores posts in Firebase.



use crate::posts_api::Post;

use std::sync::Arc;
use std::sync::Mutex;

use chrono::Datelike;
use firebase_rs::Firebase;
use serde::{Deserialize, Serialize};
use serde_json::{from_str, to_string};



/// Persistence.
/// 
/// Determination.
/// 
/// Resolve.
/// 
/// Endurance.
/// 
/// Of data.
pub struct Database {
    firebase: Firebase, // This doesn't need our synchronization.
}



impl Database {
    /// Accesses Firebase.
    /// Pass in an array of strings (a path).
    pub fn at(&self, path: &[&str]) -> Result<Firebase, firebase_rs::UrlParseError> {
        let p = fb_path(path);
        let secret = std::env::var("SHARED_SECRET").unwrap();
        self.firebase.at(&(secret + "/" + &p))
    }
}



/// Concatenates parts of a Firebase API URL properly.
/// 
/// (Why: for example, "" parts don't create entries, so the tree would become messed up without special handling as in here.)
/// 
/// ```
/// assert_eq!(&fb_path(&["a", "", "c"]), "a/_/c");
/// ```
fn fb_path(path: &[&str]) -> String {
    let mut first = true;
    path.iter().fold(String::new(), |mut a, b| {
        a.reserve((if b.is_empty() {1} else {b.len()}) + (if first {0} else {1}));
        if !first { a.push_str("/") };
        first = false;
        a.push_str(if b.is_empty() {"_"} else {b});
        a
    })
}



/// This exists because Firebase only wants to store objects at its nodes, for some reason.
#[derive(Clone, Serialize, Deserialize)]
struct UserFirstPost {
    first_post_id: String,
}
#[derive(Clone, Serialize, Deserialize)]
struct Shortened {
    post_id: String,
}



impl Database {
    /// Initializes the database connection.
    pub fn new(fb: Firebase) -> Database {
        Database{
            firebase: fb,
        }
    }
    /// Reads many posts from the database at once.
    pub fn read(&self, ids: Vec<&str>) -> Vec<Option<Post>> {
        // `firebase_rs`'s `.get_async` API is really dumb. It's forcing Arc and Mutex on us.
        let mut values: Vec<Arc<Mutex<Option<Post>>>> = vec![];
        let mut handles: Vec<Option<std::thread::JoinHandle<()>>> = vec![];
        for (i, id) in ids.iter().enumerate() {
            values.push(Arc::new(Mutex::new(None)));
            let item = values[i].clone();
            let maybe_node = self.at(&["posts", id]).ok();
            handles.push(maybe_node.map(|node| node.get_async(move |res| {
                let maybe_r = res.ok();
                let maybe_r = maybe_r.map(|r| from_str(&r.body).ok()).flatten();
                *item.lock().unwrap() = maybe_r;
            })));
        }
        for maybe_handle in handles { maybe_handle.map(|h| h.join().unwrap()); }
        values.drain(..).map(|mutex_ptr| {
            match Arc::try_unwrap(mutex_ptr).ok() {
                Some(mutex) => mutex.into_inner().unwrap(),
                None => None,
            }
        }).collect()
    }
    /// Updates many posts in the database at once: read, process, write, as one "atomic" operation.
    /// 
    /// (Well, "atomic" is a word too strong for this: it was a nice thought, but firebase-rs has never heard of atomicity, and we don't care enough to implement transactions ourselves (https://stackoverflow.com/questions/23041800/firebase-transactions-via-rest-api).)
    /// (It first reads all, then writes all, each of these being atomic.)
    /// (And, `posts_api` reads/updates `children`, `rewarded_posts`, `created_post_ids` directly, with no regard for atomicity.)
    pub fn update<F>(&self, ids: Vec<&str>, action: F)
    where F: FnOnce(Vec<Option<Post>>) -> Vec<Option<Post>> {
        let posts = self.read(ids);
        let posts = action(posts);
        let mut handles: Vec<std::thread::JoinHandle<()>> = vec![];
        for maybe_post in posts {
            if let Some(mut post) = maybe_post {
                if post.human_readable_url == "" {
                    post.human_readable_url = to_url_part(&post.content);
                }
                if post.access_hash != "" {
                    self.at(&["access_hash", &post.access_hash]).ok().map(|node| {
                        // To not overwrite, this needs a `".validate": "!data.exists()"` rule on `"access_hash"/"$hash"`.
                        let b = to_string(&UserFirstPost{
                            first_post_id: post.id.clone(),
                        }).ok();
                        b.map(|body| handles.push(node.update_async(body, |_| ())));
                    });
                }
                self.at(&["human_readable_url", &post.human_readable_url]).ok().map(|node| {
                    let b = to_string(&Shortened {
                        post_id: post.id.clone(),
                    }).ok();
                    b.map(|body| handles.push(node.update_async(body, |_| ())));
                });
                self.at(&["posts", &post.id]).ok().map(|node| {
                    let b = to_string(&post).ok();
                    b.map(|body| handles.push(node.set_async(body, |_| ())));
                });
            }
        }
        for handle in handles { handle.join().unwrap(); }
    }

    /// Looks up the access hash in the database, to get the first post ID that was made by it.
    /// Useful for retrieving a post's author (another post).
    /// Call the returned closure to get the result (the request is done async, so this is likely faster).
    pub fn get_first_post(&self, access_hash: &str) -> Box<dyn FnOnce()->Option<String>> {
        if access_hash == "" { return Box::new(|| None) }
        let value: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
        let value2 = value.clone();
        let handle = self.at(&["access_hash", access_hash]).ok().map(|n| n.get_async(move |r| {
            let id = r.ok().map(|r| from_str::<UserFirstPost>(&r.body).ok().map(|u| u.first_post_id)).flatten().unwrap_or_else(|| "".to_owned());
            let mut l = value2.lock().unwrap();
            (*l).replace_range(.., &id);
        }));
        Box::new(move || {
            handle.map(|h| h.join());
            let s = Arc::try_unwrap(value).unwrap().into_inner().unwrap();
            if s != "" {Some(s)} else {None}
        })
    }
    /// Authenticates a user's access token (username+password hashed), returning the first-post ID if there is such a user registered, else `None`.
    /// Call the returned closure to get the result (the request is done async, so this is likely faster).
    pub fn login(&self, user: &str) -> Box<dyn FnOnce()->Option<String>> {
        return self.get_first_post(&crate::posts_api::access_token_hash(user))
    }
    /// Converts a human-readable URL to the post ID, if present in the database.
    /// To get a post's URL, read `post.human_readable_url`: an empty string if not assigned.
    /// These URLs are auto-assigned, and will never collide with raw post IDs, nor with statically-served files (since these URLs are like `"2020_first_line_of_content"`).
    pub fn lookup_url(&self, url: &str) -> Option<String> {
        self.at(&["human_readable_url", url]).ok().map(|node| {
            node.get().ok().map(|r| from_str::<Shortened>(&r.body).ok().map(|s| s.post_id)).flatten()
        }).flatten()
    }
}



fn to_url_part(content: &str) -> String {
    let year = chrono::Utc::now().year().to_string();
    let simpler = match content.split_once('\n') {
        Some((line, _rest)) => line,
        None => content,
    };
    let simpler = simpler.replace(|c:char| !c.is_ascii_alphanumeric(), "_");
    let simpler = simpler.split('_').filter(|s| s.len() > 0).collect::<Vec<&str>>().join("_");
    let simpler = simpler.to_ascii_lowercase();
    year + "_" + if simpler.len() < 80 { &simpler } else { &simpler[..80] }
}